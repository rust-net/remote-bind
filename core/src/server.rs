use std::{borrow::BorrowMut, sync::Arc, collections::HashMap, io::ErrorKind, time::Duration};

use once_cell::sync::Lazy;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex, task::JoinHandle, time::sleep,
};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*,
    a2b::*,
};
use uuid::Uuid;

pub struct Server {
    address: String,
    password: String,
    listener: TcpListener,
}

static VISITORS: Lazy<Mutex<HashMap<String, TcpStream>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

async fn bind(port: u16, agent: Arc<Mutex<TcpStream>>) -> std::io::Result<JoinHandle<()>> {
    let listener = TcpListener::bind(&format!("0.0.0.0:{}", port)).await?;
    i!("PORT({port}) -> Bind succeed!");
    let task = tokio::spawn(async move {
        loop {
            let (visitor, addr)= match listener.accept().await {
                Ok(v) => v,
                Err(e) => unreachable!("{:?}", e),
            };
            let id = Uuid::new_v4().to_string();
            i!("PORT({port}) -> Tcp request from client: {addr}. (ID: {id})");
            let _ = write_cmd(
                agent.lock().await.borrow_mut(),
                Command::Accept { port, id: id.clone() },
                "",
            ).await;
            VISITORS.lock().await.insert(id.clone(), visitor);
            // 代理人必须在5秒内执行Accept命令，取出TcpStream，否则结束访问者请求
            tokio::spawn(async move {
                sleep(Duration::from_millis(5000)).await;
                match VISITORS.lock().await.remove(&id) {
                    Some(_) => {
                        i!("PORT({port}) -> Accept timeout, deny request: {addr}! (ID: {id})");
                    }
                    _ => ()
                }
            });
        }
    });
    Ok(task)
}

impl Server {
    pub async fn new(address: String, password: String) -> std::io::Result<Self> {
        let listener = TcpListener::bind(&address).await?;
        Ok(Self {
            address,
            password,
            listener,
        })
    }
    pub async fn serv(self: &Self) {
        loop {
            let (agent, addr) = match self.listener.accept().await {
                Ok((tcp, addr)) => (Arc::new(Mutex::new(tcp)), addr),
                Err(e) => break,
            };
            i!("PORT({}) -> Agent connected: {}", self.listener.local_addr().unwrap().port(), addr);
            let password = self.password.clone();
            tokio::spawn(async move {
                loop {
                    i!("AGENT({}) -> Reading command...", addr);
                    let cmd: Command = read_cmd(&mut *agent.lock().await, &password).await;
                    wtf!(&cmd);
                    match cmd {
                        Command::Bind { port } => {
                            match bind(port, agent.clone()).await {
                                Ok(task) => {
                                    let _ = write_cmd(
                                        &mut *agent.lock().await,
                                        Command::success(),
                                        "",
                                    ).await;
                                    // KEEPALIVE
                                    loop {
                                        sleep(Duration::from_millis(5000)).await;
                                        match write_cmd(agent.lock().await.borrow_mut(), Command::Nothing, "").await {
                                            Err(_) => {
                                                i!("PORT({port}) -> Agent offline, release the port!");
                                                task.abort();
                                                break;
                                            }
                                            _ => ()
                                        }
                                    }
                                }
                                Err(e) => {
                                    // &mut *conn.lock().await -> conn.lock().await.borrow_mut()
                                    let _ = write_cmd(
                                        agent.lock().await.borrow_mut(),
                                        Command::failure(e.to_string()),
                                        "",
                                    ).await;
                                    sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
                                    break;
                                }
                            };
                        }
                        Command::Accept { port, id } => {
                            let mut visitor = match VISITORS.lock().await.remove(&id) {
                                Some(v) => v,
                                _ => break,
                            };
                            let mut agent = agent.lock().await;
                            let agent = agent.split();
                            let visitor = visitor.split();
                            a2b(visitor, agent).await;
                            break;
                        }
                        Command::Error(ref e) if e.kind() == ErrorKind::PermissionDenied => {
                            let _ = write_cmd(
                                agent.lock().await.borrow_mut(),
                                Command::failure("密码错误".into()),
                                "",
                            ).await;
                            sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
                            break;
                        }
                        Command::Error(e) => {
                            let _ = write_cmd(
                                agent.lock().await.borrow_mut(),
                                Command::failure(e.to_string()),
                                "",
                            ).await;
                            sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
                            break;
                        }
                        _ => continue,
                    };
                }
            });
        }
    }
}
