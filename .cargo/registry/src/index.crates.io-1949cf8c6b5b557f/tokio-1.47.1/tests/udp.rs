#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi does not support bind or UDP
                                                                   // No `socket` on miri.

use std::future::poll_fn;
use std::io;
use std::sync::Arc;
use tokio::{io::ReadBuf, net::UdpSocket};
use tokio_test::assert_ok;

const MSG: &[u8] = b"hello";
const MSG_LEN: usize = MSG.len();

#[tokio::test]
async fn send_recv() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    sender.connect(receiver.local_addr()?).await?;
    receiver.connect(sender.local_addr()?).await?;

    sender.send(MSG).await?;

    let mut recv_buf = [0u8; 32];
    let len = receiver.recv(&mut recv_buf[..]).await?;

    assert_eq!(&recv_buf[..len], MSG);
    Ok(())
}

#[tokio::test]
async fn send_recv_poll() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    sender.connect(receiver.local_addr()?).await?;
    receiver.connect(sender.local_addr()?).await?;

    poll_fn(|cx| sender.poll_send(cx, MSG)).await?;

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    poll_fn(|cx| receiver.poll_recv(cx, &mut read)).await?;

    assert_eq!(read.filled(), MSG);
    Ok(())
}

#[tokio::test]
async fn send_to_recv_from() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let receiver_addr = receiver.local_addr()?;
    sender.send_to(MSG, &receiver_addr).await?;

    let mut recv_buf = [0u8; 32];
    let (len, addr) = receiver.recv_from(&mut recv_buf[..]).await?;

    assert_eq!(&recv_buf[..len], MSG);
    assert_eq!(addr, sender.local_addr()?);
    Ok(())
}

#[tokio::test]
async fn send_to_recv_from_poll() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let receiver_addr = receiver.local_addr()?;
    poll_fn(|cx| sender.poll_send_to(cx, MSG, receiver_addr)).await?;

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    let addr = poll_fn(|cx| receiver.poll_recv_from(cx, &mut read)).await?;

    assert_eq!(read.filled(), MSG);
    assert_eq!(addr, sender.local_addr()?);
    Ok(())
}

#[tokio::test]
async fn send_to_peek_from() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let receiver_addr = receiver.local_addr()?;
    poll_fn(|cx| sender.poll_send_to(cx, MSG, receiver_addr)).await?;

    // peek
    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.peek_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.local_addr()?);

    // peek
    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.peek_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.local_addr()?);

    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.recv_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.local_addr()?);

    Ok(())
}

#[tokio::test]
async fn send_to_try_peek_from() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let receiver_addr = receiver.local_addr()?;
    poll_fn(|cx| sender.poll_send_to(cx, MSG, receiver_addr)).await?;

    // peek
    let mut recv_buf = [0u8; 32];

    loop {
        match receiver.try_peek_from(&mut recv_buf) {
            Ok((n, addr)) => {
                assert_eq!(&recv_buf[..n], MSG);
                assert_eq!(addr, sender.local_addr()?);
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                receiver.readable().await?;
            }
            Err(e) => return Err(e),
        }
    }

    // peek
    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.peek_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.local_addr()?);

    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.recv_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.local_addr()?);

    Ok(())
}

#[tokio::test]
async fn send_to_peek_from_poll() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let receiver_addr = receiver.local_addr()?;
    poll_fn(|cx| sender.poll_send_to(cx, MSG, receiver_addr)).await?;

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    let addr = poll_fn(|cx| receiver.poll_peek_from(cx, &mut read)).await?;

    assert_eq!(read.filled(), MSG);
    assert_eq!(addr, sender.local_addr()?);

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    poll_fn(|cx| receiver.poll_peek_from(cx, &mut read)).await?;

    assert_eq!(read.filled(), MSG);
    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);

    poll_fn(|cx| receiver.poll_recv_from(cx, &mut read)).await?;
    assert_eq!(read.filled(), MSG);
    Ok(())
}

#[tokio::test]
async fn peek_sender() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let sender_addr = sender.local_addr()?;
    let receiver_addr = receiver.local_addr()?;

    let msg = b"Hello, world!";
    sender.send_to(msg, receiver_addr).await?;

    let peeked_sender = receiver.peek_sender().await?;
    assert_eq!(peeked_sender, sender_addr);

    // Assert that `peek_sender()` returns the right sender but
    // doesn't remove from the receive queue.
    let mut recv_buf = [0u8; 32];
    let (read, received_sender) = receiver.recv_from(&mut recv_buf).await?;

    assert_eq!(&recv_buf[..read], msg);
    assert_eq!(received_sender, peeked_sender);

    Ok(())
}

#[tokio::test]
async fn poll_peek_sender() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let sender_addr = sender.local_addr()?;
    let receiver_addr = receiver.local_addr()?;

    let msg = b"Hello, world!";
    poll_fn(|cx| sender.poll_send_to(cx, msg, receiver_addr)).await?;

    let peeked_sender = poll_fn(|cx| receiver.poll_peek_sender(cx)).await?;
    assert_eq!(peeked_sender, sender_addr);

    // Assert that `poll_peek_sender()` returns the right sender but
    // doesn't remove from the receive queue.
    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    let received_sender = poll_fn(|cx| receiver.poll_recv_from(cx, &mut read)).await?;

    assert_eq!(read.filled(), msg);
    assert_eq!(received_sender, peeked_sender);

    Ok(())
}

#[tokio::test]
async fn try_peek_sender() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    let sender_addr = sender.local_addr()?;
    let receiver_addr = receiver.local_addr()?;

    let msg = b"Hello, world!";
    sender.send_to(msg, receiver_addr).await?;

    let peeked_sender = loop {
        match receiver.try_peek_sender() {
            Ok(peeked_sender) => break peeked_sender,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                receiver.readable().await?;
            }
            Err(e) => return Err(e),
        }
    };

    assert_eq!(peeked_sender, sender_addr);

    // Assert that `try_peek_sender()` returns the right sender but
    // didn't remove from the receive queue.
    let mut recv_buf = [0u8; 32];
    // We already peeked the sender so there must be data in the receive queue.
    let (read, received_sender) = receiver.try_recv_from(&mut recv_buf).unwrap();

    assert_eq!(&recv_buf[..read], msg);
    assert_eq!(received_sender, peeked_sender);

    Ok(())
}

#[tokio::test]
async fn split() -> std::io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let s = Arc::new(socket);
    let r = s.clone();

    let addr = s.local_addr()?;
    tokio::spawn(async move {
        s.send_to(MSG, &addr).await.unwrap();
    });
    let mut recv_buf = [0u8; 32];
    let (len, _) = r.recv_from(&mut recv_buf[..]).await?;
    assert_eq!(&recv_buf[..len], MSG);
    Ok(())
}

#[tokio::test]
async fn split_chan() -> std::io::Result<()> {
    // setup UdpSocket that will echo all sent items
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let addr = socket.local_addr().unwrap();
    let s = Arc::new(socket);
    let r = s.clone();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(Vec<u8>, std::net::SocketAddr)>(1_000);
    tokio::spawn(async move {
        while let Some((bytes, addr)) = rx.recv().await {
            s.send_to(&bytes, &addr).await.unwrap();
        }
    });

    tokio::spawn(async move {
        let mut buf = [0u8; 32];
        loop {
            let (len, addr) = r.recv_from(&mut buf).await.unwrap();
            tx.send((buf[..len].to_vec(), addr)).await.unwrap();
        }
    });

    // test that we can send a value and get back some response
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    sender.send_to(MSG, addr).await?;
    let mut recv_buf = [0u8; 32];
    let (len, _) = sender.recv_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..len], MSG);
    Ok(())
}

#[tokio::test]
async fn split_chan_poll() -> std::io::Result<()> {
    // setup UdpSocket that will echo all sent items
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let addr = socket.local_addr().unwrap();
    let s = Arc::new(socket);
    let r = s.clone();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(Vec<u8>, std::net::SocketAddr)>(1_000);
    tokio::spawn(async move {
        while let Some((bytes, addr)) = rx.recv().await {
            poll_fn(|cx| s.poll_send_to(cx, &bytes, addr))
                .await
                .unwrap();
        }
    });

    tokio::spawn(async move {
        let mut recv_buf = [0u8; 32];
        let mut read = ReadBuf::new(&mut recv_buf);
        loop {
            let addr = poll_fn(|cx| r.poll_recv_from(cx, &mut read)).await.unwrap();
            tx.send((read.filled().to_vec(), addr)).await.unwrap();
        }
    });

    // test that we can send a value and get back some response
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    poll_fn(|cx| sender.poll_send_to(cx, MSG, addr)).await?;

    let mut recv_buf = [0u8; 32];
    let mut read = ReadBuf::new(&mut recv_buf);
    let _ = poll_fn(|cx| sender.poll_recv_from(cx, &mut read)).await?;
    assert_eq!(read.filled(), MSG);
    Ok(())
}

// # Note
//
// This test is purposely written such that each time `sender` sends data on
// the socket, `receiver` awaits the data. On Unix, it would be okay waiting
// until the end of the test to receive all the data. On Windows, this would
// **not** be okay because it's resources are completion based (via IOCP).
// If data is sent and not yet received, attempting to send more data will
// result in `ErrorKind::WouldBlock` until the first operation completes.
#[tokio::test]
async fn try_send_spawn() {
    const MSG2: &[u8] = b"world!";
    const MSG2_LEN: usize = MSG2.len();

    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    receiver
        .connect(sender.local_addr().unwrap())
        .await
        .unwrap();

    sender.writable().await.unwrap();

    let sent = &sender
        .try_send_to(MSG, receiver.local_addr().unwrap())
        .unwrap();
    assert_eq!(sent, &MSG_LEN);
    let mut buf = [0u8; 32];
    let mut received = receiver.recv(&mut buf[..]).await.unwrap();

    sender
        .connect(receiver.local_addr().unwrap())
        .await
        .unwrap();
    let sent = &sender.try_send(MSG2).unwrap();
    assert_eq!(sent, &MSG2_LEN);
    received += receiver.recv(&mut buf[..]).await.unwrap();

    std::thread::spawn(move || {
        let sent = &sender.try_send(MSG).unwrap();
        assert_eq!(sent, &MSG_LEN);
    })
    .join()
    .unwrap();
    received += receiver.recv(&mut buf[..]).await.unwrap();

    assert_eq!(received, MSG_LEN * 2 + MSG2_LEN);
}

#[tokio::test]
async fn try_send_recv() {
    // Create listener
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // Create socket pair
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // Connect the two
    client.connect(server.local_addr().unwrap()).await.unwrap();
    server.connect(client.local_addr().unwrap()).await.unwrap();

    for _ in 0..5 {
        loop {
            client.writable().await.unwrap();

            match client.try_send(b"hello world") {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            server.readable().await.unwrap();

            let mut buf = [0; 512];

            match server.try_recv(&mut buf) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }
}

#[tokio::test]
async fn try_send_to_recv_from() {
    // Create listener
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let saddr = server.local_addr().unwrap();

    // Create socket pair
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let caddr = client.local_addr().unwrap();

    for _ in 0..5 {
        loop {
            client.writable().await.unwrap();

            match client.try_send_to(b"hello world", saddr) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            server.readable().await.unwrap();

            let mut buf = [0; 512];

            match server.try_recv_from(&mut buf) {
                Ok((n, addr)) => {
                    assert_eq!(n, 11);
                    assert_eq!(addr, caddr);
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }
}

#[tokio::test]
async fn try_recv_buf() {
    // Create listener
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // Create socket pair
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // Connect the two
    client.connect(server.local_addr().unwrap()).await.unwrap();
    server.connect(client.local_addr().unwrap()).await.unwrap();

    for _ in 0..5 {
        loop {
            client.writable().await.unwrap();

            match client.try_send(b"hello world") {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            server.readable().await.unwrap();

            let mut buf = Vec::with_capacity(512);

            match server.try_recv_buf(&mut buf) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }
}

#[tokio::test]
async fn recv_buf() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    sender.connect(receiver.local_addr()?).await?;
    receiver.connect(sender.local_addr()?).await?;

    sender.send(MSG).await?;
    let mut recv_buf = Vec::with_capacity(32);
    let len = receiver.recv_buf(&mut recv_buf).await?;

    assert_eq!(len, MSG_LEN);
    assert_eq!(&recv_buf[..len], MSG);
    Ok(())
}

#[tokio::test]
async fn try_recv_buf_from() {
    // Create listener
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let saddr = server.local_addr().unwrap();

    // Create socket pair
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let caddr = client.local_addr().unwrap();

    for _ in 0..5 {
        loop {
            client.writable().await.unwrap();

            match client.try_send_to(b"hello world", saddr) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            server.readable().await.unwrap();

            let mut buf = Vec::with_capacity(512);

            match server.try_recv_buf_from(&mut buf) {
                Ok((n, addr)) => {
                    assert_eq!(n, 11);
                    assert_eq!(addr, caddr);
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }
}

#[tokio::test]
async fn recv_buf_from() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0").await?;
    let receiver = UdpSocket::bind("127.0.0.1:0").await?;

    sender.connect(receiver.local_addr()?).await?;

    sender.send(MSG).await?;
    let mut recv_buf = Vec::with_capacity(32);
    let (len, caddr) = receiver.recv_buf_from(&mut recv_buf).await?;

    assert_eq!(len, MSG_LEN);
    assert_eq!(&recv_buf[..len], MSG);
    assert_eq!(caddr, sender.local_addr()?);
    Ok(())
}

#[tokio::test]
async fn poll_ready() {
    // Create listener
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let saddr = server.local_addr().unwrap();

    // Create socket pair
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let caddr = client.local_addr().unwrap();

    for _ in 0..5 {
        loop {
            assert_ok!(poll_fn(|cx| client.poll_send_ready(cx)).await);

            match client.try_send_to(b"hello world", saddr) {
                Ok(n) => {
                    assert_eq!(n, 11);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }

        loop {
            assert_ok!(poll_fn(|cx| server.poll_recv_ready(cx)).await);

            let mut buf = Vec::with_capacity(512);

            match server.try_recv_buf_from(&mut buf) {
                Ok((n, addr)) => {
                    assert_eq!(n, 11);
                    assert_eq!(addr, caddr);
                    assert_eq!(&buf[0..11], &b"hello world"[..]);
                    break;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => panic!("{e:?}"),
            }
        }
    }
}
