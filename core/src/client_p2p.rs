use std::{io::{Error, ErrorKind}, future::Future};

use tokio::{net::{TcpStream, TcpListener}, task::JoinHandle};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, a2b::a2b, p2p::get_client_endpoint,
};

#[derive(Clone)]
pub struct ClientP2P {
    server: String,
    port: u16,
    local_service: String,
}

impl ClientP2P {
    pub fn new(server: String, port: u16, local_service: String) -> Self {
        Self {
            server,
            port,
            local_service,
        }
    }
    pub async fn serv(self: &Self) -> std::io::Result<()> {
        match TcpListener::bind(&self.local_service).await {
            Ok(listener) => {
                {
                    i!("正在测试");
                    let _: TcpStream = TcpStream::connect(&self.server).await?;
                    i!("测试成功");
                }
                i!("服务已启动: {}", self.local_service);
                loop {
                    let it = self.clone();
                    if let Ok((conn, _addr)) = listener.accept().await {
                        let server_addr = self.server.clone();
                        tokio::spawn(async move {
                            i!("正在连接");
                            let mut server: TcpStream = TcpStream::connect(&it.server).await.unwrap();
                            i!("连接成功");

                            let udp = get_client_endpoint(None).unwrap();
                            let udp_conn = udp.connect(server_addr.parse().unwrap(), "localhost").unwrap()
                                .await.expect("无法连接UDP服务器");
                            let mut udp_read = udp_conn.accept_uni().await.expect("无法读取UDP数据");
                            let mut buf = vec![0; 64];
                            let le = udp_read.read(&mut buf).await.unwrap().unwrap();
                            let my_udp_addr = String::from_utf8_lossy(&buf[..le]).to_string();

                            if let Ok(_) = write_cmd(&mut server, Command::P2pRequest { port: it.port, udp_addr: my_udp_addr }, "".into()).await {
                                i!("写入完成");
                                match read_cmd(&mut server, "".into()).await {
                                    Command::AcceptP2P { addr, udp_addr } => {
                                        i!("p2p -> {addr} {udp_addr}");
                                        let udp_conn = udp.connect(udp_addr.parse().unwrap(), "localhost").unwrap()
                                            .await.expect("无法连接UDP服务器");
                                        let mut udp_read = udp_conn.accept_uni().await.expect("无法读取UDP数据");
                                        let mut buf = vec![0; 64];
                                        let le = udp_read.read(&mut buf).await.unwrap().unwrap();
                                        let my_udp_addr = String::from_utf8_lossy(&buf[..le]).to_string();
                                        wtf!(my_udp_addr);
                                    },
                                    Command::Failure { reason } => {
                                        i!("连接失败：{reason}");
                                    }
                                    it => {
                                        wtf!(it)
                                    }
                                }
                            }
                        });
                    }
                }
            },
            Err(e) => Err(e),
        }
    }
}