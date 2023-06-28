mod a2b;
pub mod client;
mod cmd;
pub mod log;
pub mod server;

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server() {
        use server::Server;

        let s = match Server::new("0.0.0.0:1234".to_string(), "test".to_string()).await {
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
                c.proxy(format!("127.0.0.1:3389")).await;
            }
            Err(e) => println!("连接失败！{}", e.to_string()),
        };
    }
}
