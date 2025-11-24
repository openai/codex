#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support bind
                                                                   // No socket on `miri`.

use std::io::Read;
use std::io::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;

#[tokio::test]
async fn tcp_into_std() -> Result<()> {
    let mut data = [0u8; 12];
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr().unwrap().to_string();

    let handle = tokio::spawn(async {
        let stream: TcpStream = TcpStream::connect(addr).await.unwrap();
        stream
    });

    let (tokio_tcp_stream, _) = listener.accept().await?;
    let mut std_tcp_stream = tokio_tcp_stream.into_std()?;
    std_tcp_stream
        .set_nonblocking(false)
        .expect("set_nonblocking call failed");

    let mut client = handle.await.expect("The task being joined has panicked");
    client.write_all(b"Hello world!").await?;

    std_tcp_stream
        .read_exact(&mut data)
        .expect("std TcpStream read failed!");
    assert_eq!(b"Hello world!", &data);

    // test back to tokio stream
    std_tcp_stream
        .set_nonblocking(true)
        .expect("set_nonblocking call failed");
    let mut tokio_tcp_stream = TcpStream::from_std(std_tcp_stream)?;
    client.write_all(b"Hello tokio!").await?;
    let _size = tokio_tcp_stream.read_exact(&mut data).await?;
    assert_eq!(b"Hello tokio!", &data);

    Ok(())
}
