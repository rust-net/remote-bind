use std::io::{Error, ErrorKind};

use tokio::net::TcpStream;

use crate::{
    cmd::{read_cmd, write_cmd, Command},
    log::*, a2b::a2b,
};

pub struct Client {
    server: String,
    password: String,
    stream: TcpStream,
}

impl Client {
    pub async fn new(server: String, password: String) -> std::io::Result<Self> {
        let stream: TcpStream = TcpStream::connect(&server).await?;
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
    pub async fn proxy(self: &mut Self, local_service: String) {
        loop {
            let cmd: Command = read_cmd(&mut self.stream, "").await;
            let local_port = local_service.clone();
            wtf!(&cmd);
            match cmd {
                Command::Accept { port, id } => {
                    let mut new_client = Client::new(self.server.clone(), self.password.clone()).await.unwrap();
                    let _ = write_cmd(&mut new_client.stream, Command::Accept { port, id }, &new_client.password).await;
                    tokio::spawn(async move {
                        let mut local = match TcpStream::connect(local_port).await {
                            Ok(v) => v,
                            Err(e) => {
                                e!("本地代理服务连接失败! {e}");
                                return;
                            }
                        };
                        let a = local.split();
                        let b = new_client.stream.split();
                        a2b(a, b).await;
                    });
                }
                Command::Error(e) => {
                    eprintln!("{}", e);
                    break;
                }
                _ => continue,
            };
        }
    }
}
