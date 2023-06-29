#![allow(unused)]

use core::client::Client;
use core::log::*;
use std::collections::HashMap;

use once_cell::sync::Lazy;
use tokio::{
    sync::Mutex, task::JoinHandle, runtime::Runtime,
};

use uuid::Uuid;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});
static TASKS: Lazy<Mutex<HashMap<String, JoinHandle<()>>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

#[no_mangle]
pub fn start(server: String, port: u16, password: String, local_service: String) -> String {
    let id = Uuid::new_v4().to_string();
    let handler = id.clone();
    std::thread::spawn(move || {
        serv(id, server, port, password, local_service);
    });
    return handler;
}

#[no_mangle]
pub fn stop(handler: String) {
    RUNTIME.block_on(async {
        let task = TASKS.lock().await.remove(&handler).unwrap();
        task.abort();
        i!("任务 {handler} 已取消");
    });
}

fn serv(id: String, server: String, port: u16, password: String, local_service: String) {
    RUNTIME.block_on(async {
        let task = tokio::spawn(async move {
            loop {
                boot(server.clone(), port, password.clone(), local_service.clone()).await;
                tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
            }
        });
        TASKS.lock().await.insert(id, task);
    });
}

async fn boot(server: String, port: u16, password: String, local_service: String) {
    i!("正在连接服务器：{server}");
    let mut c = match Client::new(server.clone(), password).await {
        Ok(v) => v,
        Err(e) => {
            return i!("连接失败！{}", e.to_string());
        }
    };
    i!("正在绑定端口：{port}");
    match c.bind(port).await {
        Ok(()) => {
            let host = server.split(":").next().unwrap();
            i!("服务已绑定: {} -> {}:{}", local_service, host, port);
            c.proxy(local_service).await;
        }
        Err(e) => e!("绑定失败！{}", e.to_string()),
    };
}

/// 无期限堵塞线程
fn pending() {
    RUNTIME.block_on(std::future::pending::<()>());
}

mod test {
    use super::*;

    #[test]
    fn test() {
        let id = start("43.132.196.171:1234".into(), 9833, "test".into(), "127.0.0.1:3389".into());
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(10000));
            stop(id);
        });
        pending();
    }
}