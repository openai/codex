// #![warn(rust_2018_idioms)]

use native_tls::TlsConnector;
use std::error::Error;
use std::net::ToSocketAddrs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let addr = "www.rust-lang.org:443"
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve www.rust-lang.org")?;

    let socket = TcpStream::connect(&addr).await?;
    let cx = TlsConnector::builder().build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);

    let mut socket = cx.connect("www.rust-lang.org", socket).await?;

    socket
        .write_all(
            "\
             GET / HTTP/1.0\r\n\
             Host: www.rust-lang.org\r\n\
             \r\n\
             "
            .as_bytes(),
        )
        .await?;

    let mut data = Vec::new();
    socket.read_to_end(&mut data).await?;

    // println!("data: {:?}", &data);
    println!("{}", String::from_utf8_lossy(&data[..]));
    Ok(())
}
