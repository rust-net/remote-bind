use std::io::{Read, Write};

pub static mut SERVER: Option<String> = None;
pub static mut PORT: u16 = 0;
pub static mut PASSWORD: Option<String> = None;
pub static mut LOCAL_SERVICE: Option<String> = None;

fn print_help() {
    println!(
        r#"Help:
{} [server:port] [bind_port] [password] [local_service:port]
"#,
        std::env::args().nth(0).unwrap()
    );
}

fn main() {
    let mut args = std::env::args();
    if args.len() < 5 {
        return print_help();
    }
    unsafe {
        SERVER = Some(args.nth(1).unwrap());
        PORT = match args.next().unwrap().parse() {
            Ok(p) => p,
            Err(_) => {
                println!(
                    "端口号错误: {}, 请选择一个1~65535之间的端口号\n",
                    std::env::args().nth(2).unwrap()
                );
                return print_help();
            }
        };
        PASSWORD = args.next().map(|s| s.to_string());
        LOCAL_SERVICE = args.next().map(|s| s.to_string());
    }
    loop {
        serv(unsafe { SERVER.as_ref().unwrap() });
        std::thread::sleep(std::time::Duration::from_millis(5000));
    }
}

fn serv(server: &str) {
    println!("Connecting to {}", server);
    let mut conn = match std::net::TcpStream::connect(server) {
        Ok(conn) => conn,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    match bind(&mut conn, unsafe { PORT }) {
        Ok(true) => {
            println!(
                "连接服务器成功! 服务 {} 已暴露在公网 {}{}",
                unsafe { LOCAL_SERVICE.as_ref().unwrap() },
                unsafe { SERVER.as_ref().unwrap() }.trim_end_matches(char::is_numeric),
                unsafe { PORT }
            );
        }
        Ok(false) => {
            println!("绑定服务器端口失败, 请检查参数!");
            return;
        }
        Err(e) => {
            println!("与服务器通信失败：{}", e);
            return;
        }
    }
    wait_conn(conn);
}

fn bind(conn: &mut std::net::TcpStream, port: u16) -> std::io::Result<bool> {
    let mut buf = vec![0xba, 0xbe, ' ' as u8];
    let cmd = format!("{} bind {}", unsafe { PASSWORD.as_ref().unwrap() }, port);
    for ch in cmd.bytes() {
        buf.push(ch);
    }
    conn.write(&buf)?;
    let le = conn.read(&mut buf)?;
    match &buf[..le] {
        b"OK" => Ok(true),
        _ => Ok(false),
    }
}

fn wait_conn(mut conn: std::net::TcpStream) {
    let mut buf = vec![0; 1024];
    loop {
        let le = match conn.read(&mut buf) {
            Ok(0) | Err(_) => {
                println!("连接断开!");
                break;
            }
            Ok(le) => le,
        };
        let msg = String::from_utf8_lossy(&buf[..le]);
        let cmds: Vec<&str> = msg.split(" ").collect();
        if let Some(cmd) = cmds.get(0) {
            if *cmd == "connect" {
                if let Some(port) = cmds.get(1) {
                    if let Ok(port) = port.parse::<u16>() {
                        let server = format!("{}:{}", conn.peer_addr().unwrap().ip(), port);
                        println!("有客户端连接，中继到 {}", server);
                        let loc = match std::net::TcpStream::connect(unsafe {
                            LOCAL_SERVICE.as_ref().unwrap()
                        }) {
                            Ok(loc) => loc,
                            Err(err) => {
                                println!("本地服务连接失败，请检查网络，错误信息: {}", err);
                                continue;
                            }
                        };
                        let conn = match std::net::TcpStream::connect(server) {
                            Ok(conn) => conn,
                            Err(err) => {
                                println!("中继端口连接失败，请检查网络，错误信息: {}", err);
                                continue;
                            }
                        };
                        relay(conn, loc);
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn a_to_b(
    client: &mut std::net::TcpStream,
    server: &mut std::net::TcpStream,
    buf: &mut Vec<u8>,
) -> Option<bool> {
    match server.read(buf) {
        Ok(0) => {
            client.shutdown(std::net::Shutdown::Write).unwrap();
            Some(false)
        }
        Ok(le) => {
            client.set_nonblocking(false).unwrap();
            let result = match client.write(&buf[..le]) {
                Ok(0) => {
                    server.shutdown(std::net::Shutdown::Read).unwrap();
                    Some(false)
                }
                Ok(_) => Some(true),
                // 写入client之前设为阻塞，写入完成后设为非阻塞
                // Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Some(true),
                Err(_e) => None,
            };
            client.set_nonblocking(true).unwrap();
            result
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            // server would block
            Some(true)
        }
        Err(_e) => None,
    }
}

fn relay(mut client: std::net::TcpStream, mut server: std::net::TcpStream) {
    std::thread::spawn(move || {
        let server_addr = server.peer_addr().unwrap();
        let client_addr = client.peer_addr().unwrap();
        client.set_nonblocking(true).unwrap();
        server.set_nonblocking(true).unwrap();
        let mut buf = vec![0u8; 102400];
        let mut sr = true;
        let mut cr = true;
        loop {
            if sr {
                match a_to_b(&mut client, &mut server, &mut buf) {
                    Some(ok) => {
                        if !ok {
                            println!("服务器 {} 不向客户端 {} 发送数据", server_addr, client_addr);
                            sr = false;
                        }
                    }
                    None => break,
                }
            }

            if cr {
                match a_to_b(&mut server, &mut client, &mut buf) {
                    Some(ok) => {
                        if !ok {
                            println!("客户端 {} 不向服务器 {} 发送数据", client_addr, server_addr);
                            cr = false;
                        }
                    }
                    None => break,
                }
            }

            if !sr && !cr {
                println!(
                    "客户端 {} 与服务器 {} 双方都不再发送数据",
                    client_addr, server_addr
                );
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        println!("结束服务 {}", client_addr);
    });
}
