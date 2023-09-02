use core::client::Client;
use core::client_p2p::ClientP2P;
use core::log::*;
use core::panic;
use std::future::Future;

pub static mut SERVER: Option<String> = None;
pub static mut PORT: u16 = 0;
pub static mut PASSWORD: Option<String> = None;
pub static mut LOCAL_SERVICE: Option<String> = None;

fn print_help() {
    println!(
        r#"Help:
{exe} [server:port] [bind_port] [password] [local_service:port]
{exe} p2p [server:port] [bind_port] [local_listen:port]
"#,
        exe = std::env::args().nth(0).unwrap()
    );
}

fn main() {
    panic::custom_panic();
    let mut args = std::env::args();
    if args.len() < 5 {
        return print_help();
    }
    let mut is_p2p = false;
    unsafe {
        SERVER = match args.nth(1) {
            Some(v) if v == "p2p" => {
                is_p2p = true;
                args.next()
            }
            v => v,
        };
        let port = args.next().unwrap();
        PORT = match port.parse() {
            Ok(p) => p,
            Err(_) => {
                println!(
                    "端口号错误: {}, 请选择一个1~65535之间的端口号\n",
                    port
                );
                return print_help();
            }
        };
        if !is_p2p {
            PASSWORD = args.next().map(|s| s.to_string());
        }
        LOCAL_SERVICE = args.next().map(|s| s.to_string());
    }
    loop {
        if is_p2p {
            serv_p2p();
        } else {
            serv();
        }
        std::thread::sleep(std::time::Duration::from_millis(5000));
    }
}

fn task_guard(future: impl Future<Output = ()> + Send + 'static) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let _ = tokio::spawn(future).await;
        });
}

fn serv_p2p() {
    let server = unsafe { SERVER.as_ref().unwrap() };
    let port = unsafe { PORT };
    let local_service = unsafe { LOCAL_SERVICE.as_ref().unwrap() };
    task_guard(async move {
        let p2p = ClientP2P::new(server.into(), port, local_service.into());
        match p2p.serv().await {
            Err(e) => {
                e!("启动失败：{e}");
            },
            _ => ()
        };
    });
}

fn serv() {
    let server = unsafe { SERVER.as_ref().unwrap() };
    let port = unsafe { PORT };
    let password = unsafe { PASSWORD.as_ref().unwrap() };
    let local_service = unsafe { LOCAL_SERVICE.as_ref().unwrap() };
    task_guard(boot(server.into(), port, password.into(), local_service.into()));
}

async fn boot(server: String, port: u16, password: String, local_service: String) {
    i!("正在连接服务器：{server}");
    let mut c = match Client::new(server.clone(), password).await {
        Ok(v) => v,
        Err(e) => {
            return e!("连接失败！{}", e.to_string());
        }
    };
    i!("正在绑定端口：{port}");
    match c.bind(port).await {
        Ok(()) => {
            let host = server.split(":").next().unwrap();
            i!("服务已绑定: {} -> {}:{}", local_service, host, port);
            c.proxy(local_service, |_task| {
                async move {
                    // task.abort();
                }
            }).await;
        }
        Err(e) => e!("绑定失败！{}", e.to_string()),
    };
}