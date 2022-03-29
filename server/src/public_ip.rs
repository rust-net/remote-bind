/// Get the public IP address of this machine.
pub fn public_ip() -> String {
    let ip = "0.0.0.0".to_string();
    let mut box_ip = Box::new(ip);
    let apis = vec!["ifconfig.me", "api.ipify.org"];
    let _ = apis.iter().try_for_each(|it| -> std::io::Result<()> {
        let addr = std::net::ToSocketAddrs::to_socket_addrs(&format!("{}:80", it));
        if addr.is_err() {
            return Ok(())
        }
        let addr = addr.unwrap().next().unwrap();
        let timeout = std::time::Duration::from_millis(2000);
        match std::net::TcpStream::connect_timeout(&addr, timeout) {
            Ok(mut stream) => {
                let msg = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", it);
                let msg = msg.as_bytes();
                std::io::Write::write(&mut stream, msg).unwrap();
                let mut buf = [0u8; 1024];
                let le = std::io::Read::read(&mut stream, &mut buf).unwrap();
                let resp = std::str::from_utf8(&buf[0..le]).unwrap();
                let resp = resp.split("\r\n\r\n").nth(1).unwrap().to_string();
                box_ip = Box::new(resp);
                return Err(std::io::ErrorKind::Interrupted.into()) // interrupt the loop
            }
            _ => ()
        }
        Ok(())
    });
    *box_ip
}