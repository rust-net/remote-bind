use tokio::net::{TcpStream, TcpListener};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, p2p::{get_client_endpoint, tcp2udp, question_stun},
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
                    let Ok((mut conn, _addr)) = listener.accept().await
                        else { continue; };
                    let server_addr = self.server.clone();
                    tokio::spawn(async move {
                        i!("正在连接");
                        let mut server: TcpStream = TcpStream::connect(&it.server).await.unwrap();
                        i!("连接成功");

                        let udp = get_client_endpoint(None).unwrap();
                        let hole_addr = udp.local_addr().unwrap();
                        let my_udp_addr = question_stun(&udp, &server_addr).await;
                        // udp.rebind(std::net::UdpSocket::bind("0.0.0.0:0").unwrap()).unwrap(); // drop old client port
                        // drop(udp);

                        // let udp = get_client_endpoint(None).unwrap();
                        // let udp = get_client_endpoint(Some(&hole_addr.to_string())).unwrap();
                        // udp.rebind(std::net::UdpSocket::bind(hole_addr).unwrap()).unwrap(); // drop old client port
                        let Ok(_) = write_cmd(&mut server, Command::P2pRequest { port: it.port, udp_addr: my_udp_addr.clone() }, "".into()).await
                            else {
                                return e!("请求失败！");
                            };
                        match read_cmd(&mut server, "".into()).await {
                            Command::AcceptP2P { addr: _, udp_addr } => {
                                i!("AcceptP2P -> {my_udp_addr} <--> {udp_addr}");
                                i!("AcceptP2P -> Finished {my_udp_addr} <--> {udp_addr}");
                            },
                            Command::Failure { reason } => {
                                i!("连接失败：{reason}");
                            }
                            it => {
                                wtf!(it)
                            }
                        }
                    });
                }
            },
            Err(e) => Err(e),
        }
    }
}