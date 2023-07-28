use std::io;

use tokio::net::tcp::{ReadHalf, WriteHalf};

#[cfg(feature = "dump")]
async fn copy(r: &mut tokio::net::tcp::ReadHalf<'_>, w: &mut tokio::net::tcp::WriteHalf<'_>) -> io::Result<u64> {
    use std::io::Write;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use uuid::Uuid;
    use crate::time::get_time;

    let time = get_time();
    let path = format!("{time} {}.txt", Uuid::new_v4().to_string());
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path.replace(":", "-"))
        .unwrap();
    let mut count = 0u64;
    let mut buf = [0; 8 * 1024]; // 8 KB
    loop {
        match r.read(&mut buf).await {
            Ok(n) if n > 0 => {
                let _ = file.write_all(&buf[..n]);
                match w.write_all(&buf[..n]).await {
                    Err(_) => {
                        break Ok(count)
                    }
                    _ => ()
                }
                count += n as u64;
            }
            _ => {
                break Ok(count)
            }
        };
    }
}

#[cfg(not(feature = "dump"))]
async fn copy(r: &mut tokio::net::tcp::ReadHalf<'_>, w: &mut tokio::net::tcp::WriteHalf<'_>) -> io::Result<u64> {
    tokio::io::copy(r, w).await
}

pub async fn a2b(a: (ReadHalf<'_>, WriteHalf<'_>), b: (ReadHalf<'_>, WriteHalf<'_>)) {
    let (mut ar, mut aw) = a;
    let (mut br, mut bw) = b;
    let a = copy(&mut ar, &mut bw);
    let b = copy(&mut br, &mut aw);
    tokio::select! {
        _ = a => {}
        _ = b => {}
    }
}
