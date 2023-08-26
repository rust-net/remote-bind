use std::{io::{Error, ErrorKind}, future::Future};

use tokio::{net::TcpStream, task::JoinHandle};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, a2b::a2b, p2p::{get_client_endpoint, get_server_endpoint, tcp2udp},
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
                    i!("AcceptP2P -> {udp_addr}");
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

                        let _ = write_cmd(&mut new_client.stream, Command::AcceptP2P { addr, udp_addr: my_udp_addr.clone() }, &new_client.password).await;
                        let addr = udp.local_addr().unwrap();
                        udp.rebind(std::net::UdpSocket::bind("0.0.0.0:0").unwrap()).unwrap(); // drop old client port
                        udp.close(0u32.into(), b"done");
                        udp.wait_idle().await;
                        drop(udp);

                        let udp = get_server_endpoint(Some(&addr.to_string())).unwrap();
                        i!("UDP({my_udp_addr}) -> await connect");
                        let incoming_conn = udp.accept().await.unwrap();
                        let visitor = incoming_conn.remote_address().to_string();
                        i!("UDP({my_udp_addr}) -> {visitor} incoming");
                        // assert_eq!(visitor, udp_addr);
                        let _task = tokio::spawn(async move {
                            let conn = incoming_conn.await.unwrap();
                            let (mut s, r) = conn.open_bi().await.unwrap();
                            s.write_all(b"Hello").await.unwrap();
                            let mut local = match TcpStream::connect(local_service).await {
                                Ok(v) => v,
                                Err(e) => {
                                    return e!("本地代理服务连接失败：{e}");
                                }
                            };
                            let a = local.split();
                            let b = (s, r);
                            tcp2udp(a, b).await;
                            i!("AcceptP2P -> Finished {visitor}");
                        });
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
