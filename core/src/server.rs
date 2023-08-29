use std::{borrow::BorrowMut, sync::Arc, collections::HashMap, io::ErrorKind, time::Duration};

use once_cell::sync::Lazy;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex, task::JoinHandle, time::sleep,
};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*,
    a2b::*, p2p_utils::make_server_endpoint,
};
use uuid::Uuid;

pub struct Server {
    host: String,
    port: u16,
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
static P2P_VISITORS: LockMap<String, Arc<Mutex<TcpStream>>> = Lazy::new(|| {
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


async fn stun(address: &str) {
    let server_addr = address.parse().unwrap();
    let (endpoint, _server_cert) = make_server_endpoint(server_addr).expect("Can't start a STUN service");
    i!("STUN started on {address}");
    // wtf!(_server_cert); // 服务器自签证书，需要发送给客户端，除非客户端使用不安全配置，否则无法连接服务器
    loop {
        let address = address.to_string().clone();
        let Some(incoming_conn) = endpoint.accept().await else {
            continue;
        };
        let _task = tokio::spawn(async move {
            let conn = incoming_conn.await.unwrap();
            i!(
                "STUN({}) connection accepted: addr={}",
                address,
                conn.remote_address()
            );
            // let (mut s, _r) = conn.accept_bi().await.unwrap();
            let mut s = conn.open_uni().await.unwrap();
            s.write_all(conn.remote_address().to_string().as_bytes()).await.unwrap();
            // Dropping all handles associated with a connection implicitly closes it
            tokio::time::sleep(std::time::Duration::from_millis(5000)).await; // 需要一点延迟，否则客户端读取时 EOF
        });
    }
}

impl Server {
    pub async fn new(host: String, port: u16, password: String) -> std::io::Result<Self> {
        let address = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&address).await?;
        Ok(Self {
            host,
            port,
            password,
            listener,
        })
    }
    pub fn boot_stun(self: &Self) {
        let address = format!("{}:{}", self.host, self.port);
        tokio::spawn(async move { stun(&address).await });
        let address = format!("{}:{}", self.host, self.port + 1);
        tokio::spawn(async move { stun(&address).await });
    }
    pub async fn serv(self: &Self) {
        self.boot_stun();
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
                    // wtf!(&cmd);
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
                                                // i!("AGENT({agent_addr}) -> Living"); 
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
                        Command::P2pRequest { port, nat_type, udp_addr } => {
                            i!("AGENT({agent_addr}) -> P2P request port {port}");
                            let lock = SERVICES.lock().await;
                            match lock.get(&port) {
                                Some(bind_agent) => {
                                    let bind_agent = bind_agent.clone();
                                    drop(lock);
                                    P2P_VISITORS.lock().await.insert(agent_addr.to_string(), agent.clone());
                                    // TODO: delete the p2p visitor
                                    let mut bind_agent = bind_agent.lock().await;
                                    let bind_agent = &mut *bind_agent;
                                    match write_cmd(bind_agent, Command::AcceptP2P { addr: agent_addr.to_string(), nat_type, udp_addr }, "").await {
                                        Ok(_) => {
                                            //
                                        }
                                        Err(_) => {
                                            let _ = write_cmd(
                                                agent.lock().await.borrow_mut(),
                                                Command::failure("代理响应超时".to_string()),
                                                "",
                                            ).await;
                                            sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
                                        }
                                    }
                                },
                                None => {
                                    drop(lock);
                                    let _ = write_cmd(
                                        agent.lock().await.borrow_mut(),
                                        Command::failure("端口未绑定服务".to_string()),
                                        "",
                                    ).await;
                                    sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
                                },
                            }
                            break;
                        }
                        Command::AcceptP2P { addr, nat_type, udp_addr } => {
                            i!("AGENT({agent_addr}) -> P2P Response {addr} via {udp_addr}");
                            // 取出对应的p2p请求端
                            match P2P_VISITORS.lock().await.remove(&addr) {
                                Some(visitor) => {
                                    let mut visitor = visitor.lock().await;
                                    let visitor_addr = visitor.peer_addr().unwrap();
                                    assert_eq!(addr, visitor_addr.to_string());
                                    let _ = write_cmd(
                                        &mut visitor,
                                        Command::AcceptP2P { addr: agent_addr.to_string(), nat_type, udp_addr },
                                        "",
                                    ).await;
                                },
                                None => break,
                            }
                            // std::future::pending::<()>().await;
                            sleep(Duration::from_millis(2000)).await; // 为客户端执行read_cmd()留出时间
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
