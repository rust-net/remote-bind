use tokio::net::{TcpStream, TcpListener};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, p2p::{get_client_endpoint, question_stun, bridge},
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
                    let Ok((conn, _addr)) = listener.accept().await
                        else { continue; };
                    let server_addr = self.server.clone();
                    tokio::spawn(async move {
                        i!("正在连接");
                        let mut server: TcpStream = TcpStream::connect(&it.server).await.unwrap();
                        i!("连接成功");
                        let udp = get_client_endpoint(None).unwrap();
                        let (my_nat_type, my_udp_addr) = question_stun(&udp, &server_addr).await;
                        let Ok(_) = write_cmd(&mut server, Command::P2pRequest { port: it.port, nat_type: my_nat_type, udp_addr: my_udp_addr.clone() }, "".into()).await
                            else {
                                return e!("请求失败！");
                            };
                        match read_cmd(&mut server, "".into()).await {
                            Command::AcceptP2P { addr: _, nat_type: peer_nat_type, udp_addr: peer_udp_addr } => {
                                i!("AcceptP2P -> {my_udp_addr} <--> {peer_udp_addr}");
                                bridge(udp, my_nat_type, &my_udp_addr, peer_nat_type, &peer_udp_addr, conn, true).await;
                                i!("AcceptP2P -> Finished {my_udp_addr} <--> {peer_udp_addr}");
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