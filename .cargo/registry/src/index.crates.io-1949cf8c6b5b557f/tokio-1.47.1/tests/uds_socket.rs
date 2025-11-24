#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]

use futures::future::try_join;
use std::io;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixSocket,
};

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
async fn datagram_echo_server() -> io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    let server_socket = {
        let socket = UnixSocket::new_datagram()?;
        socket.bind(&server_path)?;
        socket.datagram()?
    };

    tokio::spawn(async move {
        let mut recv_buf = vec![0u8; 1024];
        loop {
            let (len, peer_addr) = server_socket.recv_from(&mut recv_buf[..]).await?;
            if let Some(path) = peer_addr.as_pathname() {
                server_socket.send_to(&recv_buf[..len], path).await?;
            }
        }

        #[allow(unreachable_code)]
        Ok::<(), io::Error>(())
    });

    {
        let socket = UnixSocket::new_datagram()?;
        socket.bind(&client_path).unwrap();
        let socket = socket.datagram()?;

        socket.connect(server_path)?;
        socket.send(b"ECHO").await?;

        let mut recv_buf = [0u8; 16];
        let len = socket.recv(&mut recv_buf[..]).await?;
        assert_eq!(&recv_buf[..len], b"ECHO");
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
async fn listen_and_stream() -> std::io::Result<()> {
    let dir = tempfile::Builder::new().tempdir().unwrap();
    let sock_path = dir.path().join("connect.sock");
    let peer_path = dir.path().join("peer.sock");

    let listener = {
        let sock = UnixSocket::new_stream()?;
        sock.bind(&sock_path)?;
        sock.listen(1024)?
    };

    let accept = listener.accept();
    let connect = {
        let sock = UnixSocket::new_stream()?;
        sock.bind(&peer_path)?;
        sock.connect(&sock_path)
    };

    let ((mut server, _), mut client) = try_join(accept, connect).await?;

    assert_eq!(
        server.peer_addr().unwrap().as_pathname().unwrap(),
        &peer_path
    );

    // Write to the client.
    client.write_all(b"hello").await?;
    drop(client);

    // Read from the server.
    let mut buf = vec![];
    server.read_to_end(&mut buf).await?;
    assert_eq!(&buf, b"hello");
    let len = server.read(&mut buf).await?;
    assert_eq!(len, 0);
    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
async fn assert_usage() -> std::io::Result<()> {
    let datagram_socket = UnixSocket::new_datagram()?;
    let result = datagram_socket
        .connect(std::path::PathBuf::new().join("invalid.sock"))
        .await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "connect cannot be called on a datagram socket"
    );

    let datagram_socket = UnixSocket::new_datagram()?;
    let result = datagram_socket.listen(1024);
    assert_eq!(
        result.unwrap_err().to_string(),
        "listen cannot be called on a datagram socket"
    );

    let stream_socket = UnixSocket::new_stream()?;
    let result = stream_socket.datagram();
    assert_eq!(
        result.unwrap_err().to_string(),
        "datagram cannot be called on a stream socket"
    );

    Ok(())
}
