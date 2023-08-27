use std::error::Error;

use quinn::{Endpoint, SendStream, RecvStream};
use tokio::net::tcp::{ReadHalf, WriteHalf};

use crate::{p2p_utils::{make_client_endpoint, make_server_endpoint}, unsafe_quic_client};

pub fn get_client_endpoint(bind: Option<&str>) -> Result<Endpoint, Box<dyn Error>> {
    let client_addr = bind.unwrap_or("0.0.0.0:0").parse().unwrap();
    let mut endpoint = make_client_endpoint(client_addr, &[])?;
    endpoint.set_default_client_config(unsafe_quic_client::get_config());
    Ok(endpoint)
}

pub fn get_server_endpoint(bind: Option<&str>) -> Result<Endpoint, Box<dyn Error>> {
    let server_addr = bind.unwrap_or("0.0.0.0:0").parse().unwrap();
    let (endpoint, _server_cert) = make_server_endpoint(server_addr)?;
    Ok(endpoint)
}

pub async fn tcp2udp(a: (ReadHalf<'_>, WriteHalf<'_>), b: (SendStream, RecvStream)) {
    let (mut ar, mut aw) = a;
    let (mut bw, mut br) = b;
    let a = tokio::io::copy(&mut ar, &mut bw);
    let b = tokio::io::copy(&mut br, &mut aw);
    tokio::select! {
        _ = a => {}
        _ = b => {}
    }
}

pub async fn question_stun(udp: &Endpoint, server_addr: &str) -> String {
    let udp_conn = udp.connect(server_addr.parse().unwrap(), "localhost").unwrap()
        .await.expect("无法连接UDP服务器");
    let mut udp_read = udp_conn.accept_uni().await.expect("无法读取UDP数据");
    let mut buf = vec![0; 64];
    let le = udp_read.read(&mut buf).await.unwrap().unwrap();
    let my_udp_addr = String::from_utf8_lossy(&buf[..le]).to_string();
    my_udp_addr
}

pub async fn bridge(udp: &Endpoint, server_addr: &str) -> String {
    let udp_conn = udp.connect(udp_addr.parse().unwrap(), "localhost").unwrap()
        .await.expect("无法连接UDP服务器");
    wtf!(udp.local_addr().unwrap(), udp_conn.remote_address());
    let (s, mut r) = udp_conn.accept_bi().await.expect("无法读取UDP数据");
    let mut buf = vec![0; 64];
    let le = r.read(&mut buf).await.unwrap().unwrap();
    let _hello = String::from_utf8_lossy(&buf[..le]).to_string();
    // assert_eq!(_hello, "Hello");
    let a = conn.split();
    let b = (s, r);
    tcp2udp(a, b).await;
    todo!("?")
}