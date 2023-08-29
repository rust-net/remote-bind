pub mod a2b;
pub mod p2p;
pub mod client;
pub mod client_p2p;
mod cmd;
pub mod log;
pub mod server;
pub mod panic;
pub mod time;
pub mod p2p_utils;
pub mod unsafe_quic_client;

#[allow(unused)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server() {
        use server::Server;

        let s = match Server::new("0.0.0.0".into(), 1234, "test".to_string()).await {
            Ok(v) => v,
            Err(e) => {
                return log::wtf!(e);
            }
        };
        s.serv().await;
    }

    #[tokio::test]
    async fn test_client() {
        use client::Client;

        let mut c = match Client::new("127.0.0.1:1234".to_string(), "test".to_string()).await {
            Ok(v) => v,
            Err(e) => {
                return log::wtf!(e);
            }
        };
        match c.bind(9833).await {
            Ok(()) => {
                println!("服务已连接！");
                c.proxy(format!("127.0.0.1:3389"), |task| {
                    async move {
                        // task.abort();
                    }
                }).await;
            }
            Err(e) => println!("连接失败！{}", e.to_string()),
        };
    }
}
