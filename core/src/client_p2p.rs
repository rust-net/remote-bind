use std::{io::{Error, ErrorKind}, future::Future};

use tokio::{net::{TcpStream, TcpListener}, task::JoinHandle};

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, a2b::a2b,
};

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
                    if let Ok((conn, _addr)) = listener.accept().await {
                        i!("正在连接");
                        let mut server: TcpStream = TcpStream::connect(&self.server).await?;
                        if let Ok(_) = write_cmd(&mut server, Command::P2pRequest { port: self.port }, "".into()).await {
                            match read_cmd(&mut server, "".into()).await {
                                Command::AcceptP2P { addr } => {
                                    i!("p2p -> {addr}");
                                },
                                _ => ()
                            }
                        }
                    }
                }
                Ok(())
            },
            Err(e) => Err(e),
        }
    }
}