use std::io::ErrorKind;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::log::d;

#[derive(Debug)]
pub enum Command {
    Bind {
        port: u16,
    },
    Accept {
        /// 远程绑定的端口号
        port: u16,
        /// 会话ID
        id: String,
        /// 访问者地址
        addr: String,
    },
    P2pRequest {
        port: u16,
        udp_addr: String, // 访问者Udp公网地址
    },
    AcceptP2P {
        addr: String, // 访问者Tcp公网地址
        udp_addr: String, // Udp公网地址
    },
    Nothing,
    Success,
    Failure {
        reason: String,
    },
    /// Error(e), 只在读取命令时产生，无法作为命令写入
    Error(std::io::Error),
}

impl Command {
    pub fn invalid_data() -> Command {
        Command::Error(ErrorKind::InvalidData.into())
    }
    pub fn permission_denied() -> Command {
        Command::Error(ErrorKind::PermissionDenied.into())
    }
    pub fn success() -> Command {
        Command::Success
    }
    pub fn failure(reason: String) -> Command {
        Command::Failure { reason }
    }
}

async fn get_length(tcp: &mut TcpStream) -> Result<(u8, u8), Command> {
    let mut len = [0u8; 2];
    for i in 0..2 {
        match tcp.read_u8().await {
            Ok(v) => {
                len[i] = v;
            }
            Err(e) => return Err(Command::Error(e)),
        }
    }
    Ok((len[0], len[1]))
}

/// 不需要认证时（如客户端读取服务器发送的命令），passwrod设为空串
pub async fn read_cmd(tcp: &mut TcpStream, password: &str) -> Command {
    // magic number check
    let mut buf = vec![0u8; 2];
    match tcp.read_exact(&mut buf).await {
        Ok(v) if buf[0] == 0xba && buf[1] == 0xbe => v,
        Ok(_) => return Command::invalid_data(),
        Err(e) => return Command::Error(e),
    };
    // d!("Magic correct");
    // length
    let (len_password, len_cmd) = match get_length(tcp).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    // password check
    let mut incorrect = false;
    if password.len() != 0 {
        let mut buf = vec![0; len_password.into()];
        match tcp.read_exact(&mut buf).await {
            Ok(_) if buf == password.as_bytes() => (),
            Ok(_) => incorrect = true,
            Err(e) => return Command::Error(e),
        };
    }
    // cmd
    let mut buf = vec![0; len_cmd.into()];
    match tcp.read_exact(&mut buf).await {
        Ok(v) => v,
        Err(e) => return Command::Error(e),
    };
    let cmd: String = String::from_utf8_lossy(&buf[..]).into();
    let mut cmds = cmd.split_whitespace();
    // match cmd
    match cmds.next() {
        Some("p2p_request") => {
            let mut r = Command::invalid_data();
            if let (Some(port), Some(addr)) = (cmds.next(), cmds.next()) {
                if let Ok(port) = port.parse::<u16>() {
                    r = Command::P2pRequest { port: port, udp_addr: addr.to_string() }
                }
            }
            r
        }
        // 允许上面的指令不检测密码
        _ if incorrect => return Command::permission_denied(),
        Some("bind") => {
            let mut r = Command::invalid_data();
            if let Some(port) = cmds.next() {
                if let Ok(port) = port.parse::<u16>() {
                    r = Command::Bind { port }
                }
            }
            r
        }
        Some("accept") => {
            let mut r = Command::invalid_data();
            if let (Some(port), Some(id), addr) = (cmds.next(), cmds.next(), cmds.next().unwrap_or_default()) {
                if let Ok(port) = port.parse::<u16>() {
                    r = Command::Accept { port, id: id.into(), addr: addr.into() }
                }
            }
            r
        }
        Some("accept_p2p") => {
            Command::AcceptP2P { addr: cmds.next().unwrap_or_default().to_string(), udp_addr: cmds.next().unwrap_or_default().to_string() }
        }
        Some("success") => Command::Success,
        Some("failure") => Command::Failure {
            reason: cmds.into_iter().collect::<Vec<&str>>().join(" "),
        },
        _ => Command::Nothing,
    }
}

/// 0xba, 0xbe, len_password, len_cmd, password, cmd
/// 不需要认证时（如服务器向客户端发送命令），passwrod设为空串
pub async fn write_cmd(tcp: &mut TcpStream, cmd: Command, password: &str) -> std::io::Result<()> {
    let cmd = match cmd {
        Command::Bind { port } => {
            format!("bind {port}")
        }
        Command::Accept { port, id, addr} => {
            format!("accept {port} {id} {addr}")
        }
        Command::P2pRequest { port, udp_addr } => {
            format!("p2p_request {port} {udp_addr}")
        }
        Command::AcceptP2P { addr, udp_addr } => {
            format!("accept_p2p {addr} {udp_addr}")
        }
        Command::Success => {
            format!("success")
        }
        Command::Failure { reason } => {
            format!("failure {reason}")
        }
        _ => "".into(),
    };
    let len_password = password.len() as u8;
    let len_cmd = cmd.len() as u8;
    tcp.write_all(&[0xba, 0xbe, len_password, len_cmd]).await?;
    tcp.write_all(password.as_bytes()).await?;
    tcp.write_all(cmd.as_bytes()).await?;
    Ok(())
}
