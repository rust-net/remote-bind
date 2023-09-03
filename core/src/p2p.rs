use std::{error::Error, net::SocketAddr, fmt::Display, str::FromStr, time::Duration};

use quinn::{Endpoint, SendStream, RecvStream};
use tokio::{net::{tcp::{ReadHalf, WriteHalf}, TcpStream}, time::timeout};

use crate::{p2p_utils::{make_client_endpoint, make_server_endpoint}, unsafe_quic_client, *};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NatType {
    /// 可以作为quic服务器
    Server,
    /// 通过端口增量猜测公网地址进行侦听
    Nat4Increment(i32),
    /// 无法主动侦听
    Nat4Random,
}

impl NatType {
    fn to_string(&self) -> String {
        match self {
            NatType::Server => format!("Server"),
            NatType::Nat4Increment(increment) => format!("Nat4Increment:{increment}"),
            NatType::Nat4Random => format!("Nat4Random"),
        }
    }
}

impl Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl From<&str> for NatType {
    fn from(value: &str) -> Self {
        match value {
            "Server" => NatType::Server,
            "Nat4Random" => NatType::Nat4Random,
            _ => {
                let (_, increment) = value.split_once(":").unwrap_or_default();
                NatType::Nat4Increment(increment.parse().unwrap_or_default())
            }
        }
    }
}

/// 向 server_addr 指向的两个 STUN 服务器发起 UDP 连接，查询 NAT 类型和可侦听的地址
pub async fn question_stun(udp: &Endpoint, server_addr: &str) -> (NatType, String) {
    let my_udp_addr1 = get_stun_addr(udp, server_addr).await;

    let (host, port) = server_addr.split_once(":").unwrap();
    let port = port.parse::<u16>().unwrap() + 1;
    let my_udp_addr2 = get_stun_addr(udp, &format!("{host}:{port}")).await;

    let nat_type = match (my_udp_addr1.ip().eq(&my_udp_addr2.ip()), my_udp_addr2.port() as i32 - my_udp_addr1.port() as i32) {
        (true, 0) => NatType::Server,
        (true, increment) if increment < 10 => NatType::Nat4Increment(increment),
        _ => NatType::Nat4Random,
    };
    nat_type.to_string();
    (nat_type, my_udp_addr2.to_string())
}

pub async fn get_stun_addr(udp: &Endpoint, server_addr: &str) -> SocketAddr {
    let udp_conn = udp.connect(server_addr.parse().unwrap(), "localhost").unwrap()
        .await.expect("无法连接UDP服务器");
    let mut udp_read = udp_conn.accept_uni().await.expect("无法读取UDP数据");
    let mut buf = vec![0; 64];
    let le = udp_read.read(&mut buf).await.unwrap().unwrap();
    let my_udp_addr = String::from_utf8_lossy(&buf[..le]).to_string();
    my_udp_addr.parse().unwrap()
}

pub fn increment_port(peer_udp_addr: &SocketAddr, increment: i32) -> SocketAddr {
    let mut peer_udp_addr = peer_udp_addr.clone();
    peer_udp_addr.set_port((peer_udp_addr.port() as i32 + increment) as u16);
    peer_udp_addr
}

pub async fn bridge(udp: Endpoint, my_nat_type: NatType, my_udp_addr: &str, peer_nat_type: NatType, peer_udp_addr: &str, mut tcp: TcpStream,
    be_server_if_both_can: bool) {
    let hole_addr = udp.local_addr().unwrap();
    if peer_nat_type == NatType::Nat4Random && my_nat_type == NatType::Nat4Random {
        i!("Sorry, both Nat4Random");
        return;
    }
    // 服务器选举
    let should_be_server = (my_nat_type == NatType::Server && peer_nat_type != NatType::Server) || // 我可作为服务器，对方可能要猜测端口
        (my_nat_type == NatType::Server && peer_nat_type == NatType::Server && be_server_if_both_can) || // 我可作为服务器，对方也能作为服务器，由函数参数决断
        (matches!(my_nat_type, NatType::Nat4Increment(_)) && peer_nat_type == NatType::Nat4Random) || // 对方绝无可能作为服务器，我可猜测端口
        (matches!(my_nat_type, NatType::Nat4Increment(_)) && matches!(peer_nat_type, NatType::Nat4Increment(_)) && be_server_if_both_can) // 双方都需要猜测端口，由函数参数决断
    ;
    let peer_udp_addr = SocketAddr::from_str(peer_udp_addr).unwrap();
    if should_be_server {
        udp.rebind(std::net::UdpSocket::bind("0.0.0.0:0").unwrap()).unwrap(); // drop old client port
        // Make sure the server has a chance to clean up
        udp.wait_idle().await;
        // 非开放型NAT需要打洞，这种情况下peer不能是随机型NAT
        let hole = std::net::UdpSocket::bind(hole_addr).unwrap();
        match peer_nat_type {
            NatType::Server => {
                let _hole = hole.send_to(b"Hello", peer_udp_addr);
                i!("send_to {peer_udp_addr}");
            },
            NatType::Nat4Increment(increment) => {
                for i in 0..5 {
                    let peer_udp_addr = increment_port(&peer_udp_addr, i * increment);
                    let _hole = hole.send_to(b"Hello", peer_udp_addr);
                    i!("send_to {peer_udp_addr}");
                }
            },
            // 非开放型NAT碰到随机型NAT，束手无策
            NatType::Nat4Random => {
                let _hole = hole.send_to(b"Hello", peer_udp_addr);
                i!("send_to {peer_udp_addr}");
            },
        }
        drop(hole);
        // quic server
        let udp = get_server_endpoint(Some(&hole_addr.to_string())).unwrap();
        i!("UDP({my_udp_addr}) -> await connect");
        let Ok(Some(incoming_conn)) = timeout(Duration::from_millis(10000), udp.accept()).await
            else {
                e!("UDP({my_udp_addr}) -> timeout");
                return;
            };
        let visitor = incoming_conn.remote_address().to_string();
        i!("UDP({my_udp_addr}) -> {visitor} incoming");
        // assert_eq!(visitor, udp_addr);
        let conn = incoming_conn.await.unwrap();
        let (mut s, r) = conn.open_bi().await.unwrap();
        s.write_all(b"Hello").await.unwrap();
        let a = tcp.split();
        let b = (s, r);
        tcp2udp(a, b).await;
    } else { // To be client
        tokio::time::sleep(std::time::Duration::from_millis(500)).await; // 等待打洞
        let udp_conn = match peer_nat_type {
            NatType::Nat4Increment(increment) => {
                let mut i = -1;
                let mut f = || {
                    let peer_udp_addr = increment_port(&peer_udp_addr, { i+=1; i } * increment);
                    i!("try connecting: {peer_udp_addr}");
                    udp.connect(peer_udp_addr, "localhost").unwrap()
                };
                tokio::select! {
                    Ok(conn) = f() => conn,
                    Ok(conn) = f() => conn,
                    Ok(conn) = f() => conn,
                    Ok(conn) = f() => conn,
                    Ok(conn) = f() => conn,
                    else => {
                        return e!("无法连接UDP服务器");
                    }
                }
            },
            _ => {
                i!("try connecting: {peer_udp_addr}");
                udp.connect(peer_udp_addr, "localhost").unwrap().await.unwrap()
            },
        };
        let (s, mut r) = udp_conn.accept_bi().await.expect("无法读取UDP数据");
        let mut buf = vec![0; 5];
        r.read_exact(&mut buf).await.unwrap();
        let _hello = String::from_utf8_lossy(&buf).to_string();
        // assert_eq!(_hello, "Hello");
        let a = tcp.split();
        let b = (s, r);
        tcp2udp(a, b).await;
    }
}