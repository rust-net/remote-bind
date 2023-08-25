use std::{io::{Error, ErrorKind}, future::Future};

use tokio::{net::TcpStream, task::JoinHandle};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, a2b::a2b, p2p::{get_client_endpoint, get_server_endpoint},
};

pub struct Client {
    server: String,
    password: String,
    stream: TcpStream,
}

impl Client {
    pub async fn new(server: String, password: String) -> std::io::Result<Self> {
        i!("正在连接");
        let stream: TcpStream = TcpStream::connect(&server).await?;
        i!("连接完成");
        Ok(Self {
            server,
            password,
            stream,
        })
    }
    pub async fn bind(self: &mut Self, port: u16) -> std::io::Result<()> {
        write_cmd(&mut self.stream, Command::Bind { port }, &self.password).await?;
        match read_cmd(&mut self.stream, "").await {
            Command::Success => Ok(()),
            Command::Failure { reason } => Err(Error::new(ErrorKind::Other, reason)),
            Command::Error(e) => Err(Error::new(ErrorKind::Other, e)),
            _ => Err(ErrorKind::Other.into()),
        }
    }
    pub async fn proxy</*F,*/R>(self: &mut Self, local_service: String, handle: /*F*/impl Fn(JoinHandle<()>) -> R)
    where
        // F: Fn(JoinHandle<()>) -> R,
        R: Future<Output = ()>,
    {
        loop {
            let cmd: Command = read_cmd(&mut self.stream, "").await;
            let local_service = local_service.clone();
            wtf!(&cmd);
            match cmd {
                Command::Accept { port, id, addr } => {
                    let session_id = id.clone();
                    i!("Accept -> Response {addr}. (ID: {session_id})");
                    let mut new_client = match Self::new(self.server.clone(), self.password.clone()).await {
                        Ok(v) => v,
                        Err(e) => break e!("新建会话失败：{e}"),
                    };
                    let _ = write_cmd(&mut new_client.stream, Command::Accept { port, id, addr: "".into() }, &new_client.password).await;
                    // 请考虑：这些任务应该如何取消？
                    let task = tokio::spawn(async move {
                        let mut local = match TcpStream::connect(local_service).await {
                            Ok(v) => v,
                            Err(e) => {
                                return e!("本地代理服务连接失败：{e}");
                            }
                        };
                        let a = local.split();
                        let b = new_client.stream.split();
                        a2b(a, b).await;
                        i!("Accept -> Finished {addr}. (ID: {session_id})");
                    });
                    handle(task).await;
                }
                Command::AcceptP2P { addr, udp_addr } => {
                    i!("绑定者开始响应 {addr} via {udp_addr}");
                    let mut new_client = match Self::new(self.server.clone(), self.password.clone()).await {
                        Ok(v) => v,
                        Err(e) => break e!("新建会话失败：{e}"),
                    };
                    // 请考虑：这些任务应该如何取消？
                    let server = self.server.clone();
                    let task = tokio::spawn(async move {
                        let udp = get_client_endpoint(None).unwrap();
                        let udp_conn = udp.connect(server.parse().unwrap(), "localhost").unwrap()
                            .await.expect("无法连接UDP服务器");
                        let mut udp_read = udp_conn.accept_uni().await.expect("无法读取UDP数据");
                        let mut buf = vec![0; 64];
                        let le = udp_read.read(&mut buf).await.unwrap().unwrap();
                        let my_udp_addr = String::from_utf8_lossy(&buf[..le]).to_string();

                        let _ = write_cmd(&mut new_client.stream, Command::AcceptP2P { addr, udp_addr: my_udp_addr }, &new_client.password).await;
                        drop(udp_conn);
                        udp.wait_idle().await;
                        drop(udp_read);
                        udp.wait_idle().await;
                        let addr = udp.local_addr().unwrap();
                        i!("重用地址{addr}");
                        drop(udp);
                        let udp = get_server_endpoint(Some(&addr.to_string())).unwrap();
                        i!("等待打洞");
                        let incoming_conn = udp.accept().await.unwrap();
                        i!("连接成功");
                        let _task = tokio::spawn(async move {
                            let conn = incoming_conn.await.unwrap();
                            i!(
                                "[server] connection accepted: addr={}",
                                conn.remote_address()
                            );
                            // let (mut s, _r) = conn.accept_bi().await.unwrap();
                            let mut s = conn.open_uni().await.unwrap();
                            s.write_all(b"Hello").await.unwrap();
                            // Dropping all handles associated with a connection implicitly closes it
                            tokio::time::sleep(std::time::Duration::from_millis(5000)).await; // 需要一点延迟，否则客户端读取时 EOF
                        });
                

                        let mut local = match TcpStream::connect(local_service).await {
                            Ok(v) => v,
                            Err(e) => {
                                return e!("本地代理服务连接失败：{e}");
                            }
                        };
                        let a = local.split();
                        let b = new_client.stream.split();
                        a2b(a, b).await;
                        // i!("Accept -> Finished {addr}. (ID: {session_id})");
                    });
                    handle(task).await;
                }
                Command::Error(e) => {
                    e!("会话异常：{}", e);
                    break;
                }
                Command::Nothing => {
                    let _ = write_cmd(&mut self.stream, Command::Nothing, "").await;
                },
                _ => continue,
            };
        }
    }
}
