use core::i;
use std::{sync::Arc, time::Duration};

use axum::{extract::State, response::IntoResponse, routing::get, Router};
use tokio::{net::TcpListener, task::AbortHandle, time::sleep};

/// 启动api服务, 默认监听 127.0.0.111:11111
pub async fn boot_api(api: String, task: AbortHandle) {
    i!("Starting api service on {api}");
    let listener = TcpListener::bind(api).await.unwrap();
    let server = axum::Server::from_tcp(listener.into_std().unwrap()).unwrap();
    let api = Router::new()
        .route("/reload", get(reload).with_state(Arc::new(task)))
        .route("/status", get(status));
    let router = Router::new().nest("/oneport", api);
    server.serve(router.into_make_service()).await.unwrap();
}

/// 在不断开已有会话的前提下重启服务
async fn reload(task: State<Arc<AbortHandle>>) -> impl IntoResponse {
    i!("Reloading...");
    task.abort();
    sleep(Duration::from_millis(1000)).await;
    task.is_finished().to_string()
}

async fn status() -> impl IntoResponse {
    "Running"
}
