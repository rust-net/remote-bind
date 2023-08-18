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
    password: String,
    listener: TcpListener,
}

type LockMap<K, V> = Lazy<Mutex<HashMap<K, V>>>;

static VISITORS: LockMap<String, TcpStream> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});
static SERVICES: LockMap<u16, Arc<Mutex<TcpStream>>> = Lazy::new(|| {
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
                Command::Accept { port, id: id.clone(), addr: addr.to_string() },
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
            password,
            listener,
        })
    }
    pub async fn serv(self: &Self) {
        loop {
            let (agent, agent_addr) = match self.listener.accept().await {
                Ok((tcp, addr)) => (Arc::new(Mutex::new(tcp)), addr),
                Err(e) => break e!("Accept error: {e}"),
            };
            i!("PORT({}) -> Agent connected: {}", self.listener.local_addr().unwrap().port(), agent_addr);
            let password = self.password.clone();
            tokio::spawn(async move {
                loop {
                    i!("AGENT({agent_addr}) -> Reading command...");
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
                                    SERVICES.lock().await.insert(port, agent.clone());
                                    // KEEPALIVE
                                    loop {
                                        sleep(Duration::from_millis(5000)).await;
                                        // Bug: 代理人已经死掉还能写入命令
                                        let mut agent = agent.lock().await;
                                        let agent = agent.borrow_mut();
                                        match write_cmd(agent, Command::Nothing, "").await {
                                            Err(_) => {
                                                i!("PORT({port}) -> Agent {agent_addr} offline, release the port!");
                                                task.abort();
                                                break;
                                            }
                                            _ => ()
                                        }
                                        match read_cmd(agent, "").await {
                                            Command::Nothing => {
                                                i!("AGENT({agent_addr}) -> Living"); 
                                            }
                                            _ => {
                                                i!("PORT({port}) -> Agent {agent_addr} no response, release the port!");
                                                task.abort();
                                                break;
                                            }
                                        }
                                    }
                                    SERVICES.lock().await.remove(&port);
                                    break; // End the Binding
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
                        Command::Accept { port: _, id, addr: _ } => {
                            let mut visitor = match VISITORS.lock().await.remove(&id) {
                                Some(v) => v,
                                _ => break,
                            };
                            let visitor_addr = visitor.peer_addr().unwrap();
                            i!("AGENT({agent_addr}) -> Response {}. (ID: {id})", visitor_addr);
                            let mut agent = agent.lock().await;
                            let agent = agent.split();
                            let visitor = visitor.split();
                            a2b(visitor, agent).await;
                            i!("AGENT({agent_addr}) -> Finished {}. (ID: {id})", visitor_addr);
                            break;
                        }
                        Command::P2pRequest { port } => {
                            i!("p2p 请求端口 {port}, from {}", agent_addr.ip());
                            match SERVICES.lock().await.get(&port) {
                                Some(bind_agent) => {
                                    let mut bind_agent = bind_agent.lock().await;
                                    let bind_agent = &mut *bind_agent;
                                    match write_cmd(bind_agent, Command::AcceptP2P { addr: agent_addr.to_string() }, "").await {
                                        Ok(_) => {
                                            //
                                        }
                                        Err(_) => {
                                        }
                                    }
                                },
                                None => {
                                    let _ = write_cmd(
                                        agent.lock().await.borrow_mut(),
                                        Command::failure("端口未绑定服务".to_string()),
                                        "",
                                    ).await;
                                    sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
                                },
                            }
                        }
                        Command::AcceptP2P { addr } => {
                            i!("p2p 响应 {addr}");
                            // 取出对应的p2p请求端
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
