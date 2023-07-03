use std::{io::{Error, ErrorKind}, future::Future};

use tokio::{net::TcpStream, task::JoinHandle};

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
                    let mut new_client = match Client::new(self.server.clone(), self.password.clone()).await {
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
