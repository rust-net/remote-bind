use tokio::net::tcp::{ReadHalf, WriteHalf};

pub async fn a2b(a: (ReadHalf<'_>, WriteHalf<'_>), b: (ReadHalf<'_>, WriteHalf<'_>)) {
    let (mut ar, mut aw) = a;
    let (mut br, mut bw) = b;
    let a = tokio::io::copy(&mut ar, &mut bw);
    let b = tokio::io::copy(&mut br, &mut aw);
    tokio::select! {
        _ = a => {}
        _ = b => {}
    }
}
