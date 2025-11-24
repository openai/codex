#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]

use tokio::io::ReadBuf;
use tokio::net::UnixDatagram;
use tokio::try_join;

use std::future::poll_fn;
use std::io;
use std::sync::Arc;

async fn echo_server(socket: UnixDatagram) -> io::Result<()> {
    let mut recv_buf = vec![0u8; 1024];
    loop {
        let (len, peer_addr) = socket.recv_from(&mut recv_buf[..]).await?;
        if let Some(path) = peer_addr.as_pathname() {
            socket.send_to(&recv_buf[..len], path).await?;
        }
    }
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn echo() -> io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    let server_socket = UnixDatagram::bind(server_path.clone())?;

    tokio::spawn(async move {
        let _ = echo_server(server_socket).await;
    });

    {
        let socket = UnixDatagram::bind(&client_path).unwrap();
        socket.connect(server_path)?;
        socket.send(b"ECHO").await?;
        let mut recv_buf = [0u8; 16];
        let len = socket.recv(&mut recv_buf[..]).await?;
        assert_eq!(&recv_buf[..len], b"ECHO");
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn echo_from() -> io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    let server_socket = UnixDatagram::bind(server_path.clone())?;

    tokio::spawn(async move {
        let _ = echo_server(server_socket).await;
    });

    {
        let socket = UnixDatagram::bind(&client_path).unwrap();
        socket.connect(&server_path)?;
        socket.send(b"ECHO").await?;
        let mut recv_buf = [0u8; 16];
        let (len, addr) = socket.recv_from(&mut recv_buf[..]).await?;
        assert_eq!(&recv_buf[..len], b"ECHO");
        assert_eq!(addr.as_pathname(), Some(server_path.as_path()));
    }

    Ok(())
}

// Even though we use sync non-blocking io we still need a reactor.
#[tokio::test]
#[cfg_attr(miri, ignore)] // No SOCK_DGRAM for `socketpair` in miri.
async fn try_send_recv_never_block() -> io::Result<()> {
    let mut recv_buf = [0u8; 16];
    let payload = b"PAYLOAD";
    let mut count = 0;

    let (dgram1, dgram2) = UnixDatagram::pair()?;

    // Send until we hit the OS `net.unix.max_dgram_qlen`.
    loop {
        dgram1.writable().await.unwrap();

        match dgram1.try_send(payload) {
            Err(err) => match (err.kind(), err.raw_os_error()) {
                (io::ErrorKind::WouldBlock, _) => break,
                (_, Some(libc::ENOBUFS)) => break,
                _ => {
                    panic!("unexpected error {err:?}");
                }
            },
            Ok(len) => {
                assert_eq!(len, payload.len());
            }
        }
        count += 1;
    }

    // Read every dgram we sent.
    while count > 0 {
        dgram2.readable().await.unwrap();
        let len = dgram2.try_recv(&mut recv_buf[..])?;
        assert_eq!(len, payload.len());
        assert_eq!(payload, &recv_buf[..len]);
        count -= 1;
    }

    let err = dgram2.try_recv(&mut recv_buf[..]).unwrap_err();
    match err.kind() {
        io::ErrorKind::WouldBlock => (),
        _ => unreachable!("unexpected error {:?}", err),
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn split() -> std::io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("split.sock");
    let s = Arc::new(UnixDatagram::bind(path.clone())?);
    let r = s.clone();

    let msg = b"hello";
    let ((), ()) = try_join! {
        async {
            s.send_to(msg, path).await?;
            io::Result::Ok(())
        },
        async {
            let mut recv_buf = [0u8; 32];
            let (len, _) = r.recv_from(&mut recv_buf[..]).await?;
            assert_eq!(&recv_buf[..len], msg);
            Ok(())
        },
    }?;

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn send_to_recv_from_poll() -> std::io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let sender_path = dir.path().join("sender.sock");
    let receiver_path = dir.path().join("receiver.sock");

    let sender = UnixDatagram::bind(&sender_path)?;
    let receiver = UnixDatagram::bind(&receiver_path)?;

    let msg = b"hello";
    poll_fn(|cx| sender.poll_send_to(cx, msg, &receiver_path)).await?;

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    let addr = poll_fn(|cx| receiver.poll_recv_from(cx, &mut read)).await?;

    assert_eq!(read.filled(), msg);
    assert_eq!(addr.as_pathname(), Some(sender_path.as_ref()));
    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn send_recv_poll() -> std::io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let sender_path = dir.path().join("sender.sock");
    let receiver_path = dir.path().join("receiver.sock");

    let sender = UnixDatagram::bind(&sender_path)?;
    let receiver = UnixDatagram::bind(&receiver_path)?;

    sender.connect(&receiver_path)?;
    receiver.connect(&sender_path)?;

    let msg = b"hello";
    poll_fn(|cx| sender.poll_send(cx, msg)).await?;

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    poll_fn(|cx| receiver.poll_recv(cx, &mut read)).await?;

    assert_eq!(read.filled(), msg);
    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn try_send_to_recv_from() -> std::io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    // Create listener
    let server = UnixDatagram::bind(&server_path)?;

    // Create socket pair
    let client = UnixDatagram::bind(&client_path)?;

    for _ in 0..5 {
        loop {
            client.writable().await?;

            match client.try_send_to(b"hello world", &server_path) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            server.readable().await?;

            let mut buf = [0; 512];

            match server.try_recv_from(&mut buf) {
                Ok((n, addr)) => {
                    assert_eq!(n, 11);
                    assert_eq!(addr.as_pathname(), Some(client_path.as_ref()));
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn try_recv_buf_from() -> std::io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    // Create listener
    let server = UnixDatagram::bind(&server_path)?;

    // Create socket pair
    let client = UnixDatagram::bind(&client_path)?;

    for _ in 0..5 {
        loop {
            client.writable().await?;

            match client.try_send_to(b"hello world", &server_path) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            server.readable().await?;

            let mut buf = Vec::with_capacity(512);

            match server.try_recv_buf_from(&mut buf) {
                Ok((n, addr)) => {
                    assert_eq!(n, 11);
                    assert_eq!(addr.as_pathname(), Some(client_path.as_ref()));
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn recv_buf_from() -> std::io::Result<()> {
    let tmp = tempfile::tempdir()?;

    // Bind each socket to a filesystem path
    let tx_path = tmp.path().join("tx");
    let tx = UnixDatagram::bind(&tx_path)?;
    let rx_path = tmp.path().join("rx");
    let rx = UnixDatagram::bind(&rx_path)?;

    let bytes = b"hello world";
    tx.send_to(bytes, &rx_path).await?;

    let mut buf = Vec::with_capacity(24);
    let (size, addr) = rx.recv_buf_from(&mut buf).await?;

    let dgram = &buf[..size];
    assert_eq!(dgram, bytes);
    assert_eq!(addr.as_pathname().unwrap(), &tx_path);
    Ok(())
}

// Even though we use sync non-blocking io we still need a reactor.
#[tokio::test]
#[cfg_attr(miri, ignore)] // No SOCK_DGRAM for `socketpair` in miri.
async fn try_recv_buf_never_block() -> io::Result<()> {
    let payload = b"PAYLOAD";
    let mut count = 0;

    let (dgram1, dgram2) = UnixDatagram::pair()?;

    // Send until we hit the OS `net.unix.max_dgram_qlen`.
    loop {
        dgram1.writable().await.unwrap();

        match dgram1.try_send(payload) {
            Err(err) => match (err.kind(), err.raw_os_error()) {
                (io::ErrorKind::WouldBlock, _) => break,
                (_, Some(libc::ENOBUFS)) => break,
                _ => {
                    panic!("unexpected error {err:?}");
                }
            },
            Ok(len) => {
                assert_eq!(len, payload.len());
            }
        }
        count += 1;
    }

    // Read every dgram we sent.
    while count > 0 {
        let mut recv_buf = Vec::with_capacity(16);

        dgram2.readable().await.unwrap();
        let len = dgram2.try_recv_buf(&mut recv_buf)?;
        assert_eq!(len, payload.len());
        assert_eq!(payload, &recv_buf[..len]);
        count -= 1;
    }

    let mut recv_buf = vec![0; 16];
    let err = dgram2.try_recv_from(&mut recv_buf).unwrap_err();
    match err.kind() {
        io::ErrorKind::WouldBlock => (),
        _ => unreachable!("unexpected error {:?}", err),
    }

    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No SOCK_DGRAM for `socketpair` in miri.
async fn recv_buf() -> std::io::Result<()> {
    // Create the pair of sockets
    let (sock1, sock2) = UnixDatagram::pair()?;

    // Since the sockets are paired, the paired send/recv
    // functions can be used
    let bytes = b"hello world";
    sock1.send(bytes).await?;

    let mut buff = Vec::with_capacity(24);
    let size = sock2.recv_buf(&mut buff).await?;

    let dgram = &buff[..size];
    assert_eq!(dgram, bytes);
    Ok(())
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn poll_ready() -> io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    // Create listener
    let server = UnixDatagram::bind(&server_path)?;

    // Create socket pair
    let client = UnixDatagram::bind(&client_path)?;

    for _ in 0..5 {
        loop {
            poll_fn(|cx| client.poll_send_ready(cx)).await?;

            match client.try_send_to(b"hello world", &server_path) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            poll_fn(|cx| server.poll_recv_ready(cx)).await?;

            let mut buf = Vec::with_capacity(512);

            match server.try_recv_buf_from(&mut buf) {
                Ok((n, addr)) => {
                    assert_eq!(n, 11);
                    assert_eq!(addr.as_pathname(), Some(client_path.as_ref()));
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }

    Ok(())
}
