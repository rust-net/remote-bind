use std::io::{Read, Write};

mod public_ip;
use public_ip::*;

pub static PORT: u16 = 1234;
pub static mut PASSWORD: [u8; 1024] = [0; 1024];
pub static mut SLICE_PASSWORD: &[u8] = unsafe { &PASSWORD }; // Don't trim_matches(char::from(0))
pub static mut DEFAULT_PASSWORD: &str = "test";

fn print_help() {
    println!(
        r#"Help:
{} [port] [password]
"#,
        std::env::args().nth(0).unwrap()
    );
}

fn set_password(str: &str) {
    unsafe {
        let mut i = 0;
        for c in str.bytes() {
            PASSWORD[i] = c;
            i += 1;
        }
        PASSWORD[i] = 0;
        SLICE_PASSWORD = &PASSWORD[..i];
    }
}

fn main() {
    set_password(unsafe { DEFAULT_PASSWORD });
    let port = match std::env::args().nth(1) {
        Some(str) => match str.as_str() {
            "-h" | "--help" => return print_help(),
            _ => match str.parse::<u16>() {
                Ok(port) => port,
                Err(_) => {
                    set_password(&str);
                    PORT
                }
            },
        },
        _ => PORT,
    };
    match std::env::args().nth(2) {
        Some(str) => {
            set_password(&str);
        }
        _ => {}
    }
    serv("0.0.0.0", port);
}

fn serv(host: &str, port: u16) {
    let server = std::net::TcpListener::bind(format!("{}:{}", host, port));
    if server.is_err() {
        println!("{}", server.unwrap_err());
        return;
    }
    let server = server.unwrap();

    let local_addr = server.local_addr().unwrap();
    let port = local_addr.port();
    println!("server started on {}:{}", public_ip(), port);
    serv_logic(&server);
}

fn serv_logic(server: &std::net::TcpListener) {
    for stream in server.incoming() {
        match stream {
            Ok(mut stream) => {
                let addr = stream.peer_addr().unwrap();
                println!("客户端 {} 已连接, 请在2秒内发送正确的指令和口令", addr);
                stream
                    .set_read_timeout(Some(std::time::Duration::from_millis(2000)))
                    .unwrap();
                let mut buf = [0u8; 1024];
                let le = stream.read(&mut buf);
                if le.is_err() {
                    println!("客户端 {} 连接超时", addr);
                    continue;
                }
                let le = le.unwrap();
                if le == 0 {
                    println!("客户端 {} 连接主动关闭", addr);
                    continue;
                }
                if le < 2 || buf[0] != 0xba || buf[1] != 0xbe {
                    println!("客户端 {} 发送了错误的指令", addr);
                    continue;
                }
                println!("客户端 {} 发送了 {} 字节", addr, le);
                let user_cmd = String::from_utf8_lossy(&buf[3..le]).to_string(); // skip 0xba 0xbe 0x20
                println!("客户端 {} 发送了命令 {}", addr, user_cmd);
                let cmds: Vec<&str> = user_cmd.split(" ").collect();
                let user_passwd = cmds[0];
                println!("客户端 {} 提供的口令是 {}", addr, user_passwd);

                // if String::from_utf8(unsafe { PASSWORD.to_vec() }).unwrap().trim_matches(char::from(0)) != user_passwd {
                if String::from_utf8(unsafe { SLICE_PASSWORD.to_vec() }).unwrap() != user_passwd {
                    println!("客户端 {} 发送了错误的口令", addr);
                    continue;
                }
                match cmds.get(1) {
                    Some(cmd) => match *cmd {
                        "bind" => {
                            if cmds.len() > 2 {
                                if let Ok(port) = cmds[2].parse::<u16>() {
                                    println!("客户端 {} 发送了绑定指令，端口号 {}", addr, port);
                                    bind(stream, port);
                                }
                            }
                        }
                        _ => (),
                    },
                    None => (),
                }
            }
            Err(e) => {
                println!("{}", e);
            }
        }
    }
}

fn bind(mut stream: std::net::TcpStream, port: u16) {
    let server = match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(server) => {
            stream.write(b"OK").unwrap();
            server
        }
        Err(_e) => {
            stream.write(b"FAILED").unwrap();
            return;
        }
    };
    std::thread::spawn(move || {
        handle_request(server, stream);
    });
}

/// 非阻塞地响应accept，确保stream在线，否则释放端口，让客户端read stream出错后主动重连
fn handle_request(server: std::net::TcpListener, mut channel: std::net::TcpStream) {
    server.set_nonblocking(true).unwrap();
    let server_addr = server.local_addr().unwrap();
    let channel_addr = channel.peer_addr().unwrap();

    let mut last_check_time = std::time::Instant::now();
    loop {
        match server.accept() {
            Ok((conn, user)) => {
                println!("前端客户端 {} 连接端口 {}", user, server_addr.port());
                // 申请一个随机端口进行中继
                if let Ok(transfer) = std::net::TcpListener::bind("0.0.0.0:0") {
                    let local_port = transfer.local_addr().unwrap().port();
                    let cmd = format!("connect {}", local_port);
                    match channel.write(cmd.as_bytes()) {
                        // 通知客户端发起了连接
                        Ok(0) | Err(_) => {
                            println!("后端通信频道 {} 掉线", channel_addr);
                            return; // 结束线程，释放端口，后端主动检测，掉线后重新申请绑定
                        }
                        _ => (),
                    }

                    let transfer = match await_connect(transfer) {
                        Some(transfer) => transfer,
                        _ => {
                            println!("后端服务器连接超时，断开前端请求 {} ", user);
                            continue;
                        }
                    };

                    println!(
                        "后端服务器 {} 响应前端请求 {} ",
                        transfer.peer_addr().unwrap(),
                        user
                    );

                    relay(conn, transfer);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if std::time::Instant::now() - last_check_time > std::time::Duration::from_secs(5) {
                    match channel.write("KEEPALIVE".as_bytes()) {
                        // 通知客户端发起了连接
                        Ok(0) | Err(_) => {
                            println!("KEEPALIVE：检测到后端服务器 {} 掉线", channel_addr);
                            break; // 结束线程，释放端口，后端主动检测，掉线后重新申请绑定
                        }
                        _ => (),
                    }
                    last_check_time = std::time::Instant::now();
                }
            }
            _ => break,
        }
    }
    println!("释放端口 {}", server_addr.port());
}

/// 非阻塞地响应accept，超时返回None
fn await_connect(transfer: std::net::TcpListener) -> Option<std::net::TcpStream> {
    transfer.set_nonblocking(true).unwrap();
    let start_time = std::time::Instant::now();
    let (transfer, _) = loop {
        match transfer.accept() {
            Ok((conn, addr)) => break (conn, addr),
            _ => {
                let time = std::time::Instant::now() - start_time;
                if time > std::time::Duration::from_secs(3) {
                    return None;
                }
                println!("等待响应, time: {:?}", time);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    };
    Some(transfer)
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
