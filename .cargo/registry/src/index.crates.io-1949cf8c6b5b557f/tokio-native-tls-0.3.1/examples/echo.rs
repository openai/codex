#![warn(rust_2018_idioms)]

// A tiny async TLS echo server with Tokio
use native_tls::Identity;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/**
an example to setup a tls server.
how to test:
wget https://127.0.0.1:12345 --no-check-certificate
*/
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind the server's socket
    let addr = "127.0.0.1:12345".to_string();
    let tcp: TcpListener = TcpListener::bind(&addr).await?;

    // Create the TLS acceptor.
    let der = include_bytes!("identity.p12");
    let cert = Identity::from_pkcs12(der, "mypass")?;
    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?);
    loop {
        // Asynchronously wait for an inbound socket.
        let (socket, remote_addr) = tcp.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        println!("accept connection from {}", remote_addr);
        tokio::spawn(async move {
            // Accept the TLS connection.
            let mut tls_stream = tls_acceptor.accept(socket).await.expect("accept error");
            // In a loop, read data from the socket and write the data back.

            let mut buf = [0; 1024];
            let n = tls_stream
                .read(&mut buf)
                .await
                .expect("failed to read data from socket");

            if n == 0 {
                return;
            }
            println!("read={}", unsafe {
                String::from_utf8_unchecked(buf[0..n].into())
            });
            tls_stream
                .write_all(&buf[0..n])
                .await
                .expect("failed to write data to socket");
        });
    }
}
