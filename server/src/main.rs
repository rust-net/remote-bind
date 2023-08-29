mod public_ip;

use core::log::*;
use core::server::Server;
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

fn get_password() -> String {
    String::from_utf8(unsafe { SLICE_PASSWORD.to_vec() }).unwrap()
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
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(boot(host, port, get_password()));
}

async fn boot(host: &str, port: u16, passwd: String) {
    let s = match Server::new(host.into(), port, passwd).await {
        Ok(v) => v,
        Err(e) => {
            return e!("Server start failed: {e}");
        }
    };
    i!("Server started on {}:{}", public_ip(), port);
    s.serv().await;
}