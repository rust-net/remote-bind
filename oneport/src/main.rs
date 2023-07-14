mod config;

use core::*;
use config::*;
use std::{net::SocketAddr, time::Duration};

use tokio::{net::{TcpListener, TcpStream}, io::AsyncWriteExt, task::JoinHandle, time::sleep};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut config_file = None;
    while let Some(arg)= args.next() {
        match arg.as_str() {
            "--config" | "-config" | "--c" | "-c" => {
                config_file = args.next();
            },
            "--reload" | "-reload" | "--r" | "-r" => {
                return reload(config_file).await;
            }
            _ => {}
        }
    }
    loop {
        boot(config_file.clone()).await;
        sleep(Duration::from_millis(5000)).await;
    }
}

/// 热重启, 目前暂时不支持修改热重启接口, 修改将导致无法再次通过命令行进行热重启
async fn reload(config_file: Option<String>) {
    let config = match read_config(config_file).await {
        Some(v) => v,
        None => return,
    };
    match load_config(&config).await {
        Ok((_listen, api)) => {
            match TcpStream::connect(api).await {
                Ok(_) => {
                    i!("Restarting...");
                }
                _ => ()
            }
        },
        Err(e) => {
            return e!("Config load failed: {e}");
        }
    }
}

async fn boot(config_file: Option<String>) {
    let config = match read_config(config_file).await {
        Some(v) => v,
        None => return,
    };
    let (listen, api) = match load_config(&config).await {
        Ok((listen, api)) => {
            i!("Config loaded");
            (listen, api)
        },
        Err(e) => {
            return e!("Config load failed: {e}");
        }
    };
    let task = tokio::spawn(boot_oneport(listen));
    boot_api(api, task).await;
}

/// 启动热重启服务, 默认监听 127.0.0.111:11111, 当有客户端连接时, 在不断开已有会话的前提下重启服务
async fn boot_api(api: String, task: JoinHandle<()>) {
    i!("Starting api service on {api}");
    let listener = TcpListener::bind(api).await.unwrap();
    loop {
        let (_client, addr) = listener.accept().await.unwrap();
        i!("API Request from {addr}");
        task.abort();
        task.await.unwrap_err();
        i!("Restarting...");
        break;
    }
}

/// 启动oneport主服务, 默认监听 0.0.0.0:1111
async fn boot_oneport(listen: String) {
    i!("Starting oneport service on {listen}");
    let listener = TcpListener::bind(listen).await.unwrap();
    loop {
        let (visitor, addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => unreachable!("{:?}", e),
        };
        i!("Request {addr} incoming");
        // Feature: 已有的会话不会在热重启时断开
        tokio::spawn(async move {
            serv(visitor, addr).await;
        });
    }
}

async fn serv(mut visitor: TcpStream, addr: SocketAddr) {
    visitor.readable().await.unwrap();
    let mut msg = vec![0; 1024];
    match visitor.try_read(&mut msg) {
        Ok(n) => {
            if n < 1 {
                i!("Request {addr} read EOF");
                return;
            }
            msg.truncate(n);
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            i!("Request {addr} read would block");
        }
        Err(e) => {
            e!("Request {addr} read error: {e}");
            return;
        }
    }
    i!("Request {addr} msg = {:?}", &msg[..if msg.len() > 10 { 10 } else { msg.len() }]);
    let rules = RULES.lock().await;
    let mut address = None;
    for (rule, target) in rules.as_slice() {
        if rule.len() <= msg.len() && rule == &msg[..rule.len()] {
            i!("Request {addr} matched: {target}");
            address = Some(target.clone());
            break;
        }
    }
    drop(rules);
    match address {
        None => return i!("Request {addr} not match"),
        Some(address) => {
            let mut stream = match TcpStream::connect(address).await {
                Ok(v) => v,
                Err(e) => return e!("Request {addr} serv error: {e}"),
            };
            stream.write_all(&msg).await.unwrap();
            let a = visitor.split();
            let b = stream.split();
            a2b::a2b(a, b).await;
            i!("Request {addr} finished");
        }
    }
}