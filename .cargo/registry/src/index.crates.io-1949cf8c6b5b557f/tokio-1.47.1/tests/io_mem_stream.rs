#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn ping_pong() {
    let (mut a, mut b) = duplex(32);

    let mut buf = [0u8; 4];

    a.write_all(b"ping").await.unwrap();
    b.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"ping");

    b.write_all(b"pong").await.unwrap();
    a.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"pong");
}

#[tokio::test]
async fn across_tasks() {
    let (mut a, mut b) = duplex(32);

    let t1 = tokio::spawn(async move {
        a.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        a.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"pong");
    });

    let t2 = tokio::spawn(async move {
        let mut buf = [0u8; 4];
        b.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ping");
        b.write_all(b"pong").await.unwrap();
    });

    t1.await.unwrap();
    t2.await.unwrap();
}

#[tokio::test]
async fn disconnect() {
    let (mut a, mut b) = duplex(32);

    let t1 = tokio::spawn(async move {
        a.write_all(b"ping").await.unwrap();
        // and dropped
    });

    let t2 = tokio::spawn(async move {
        let mut buf = [0u8; 32];
        let n = b.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"ping");

        let n = b.read(&mut buf).await.unwrap();
        assert_eq!(n, 0);
    });

    t1.await.unwrap();
    t2.await.unwrap();
}

#[tokio::test]
async fn disconnect_reader() {
    let (a, mut b) = duplex(2);

    let t1 = tokio::spawn(async move {
        // this will block, as not all data fits into duplex
        b.write_all(b"ping").await.unwrap_err();
    });

    let t2 = tokio::spawn(async move {
        // here we drop the reader side, and we expect the writer in the other
        // task to exit with an error
        drop(a);
    });

    t2.await.unwrap();
    t1.await.unwrap();
}

#[tokio::test]
async fn max_write_size() {
    let (mut a, mut b) = duplex(32);

    let t1 = tokio::spawn(async move {
        let n = a.write(&[0u8; 64]).await.unwrap();
        assert_eq!(n, 32);
        let n = a.write(&[0u8; 64]).await.unwrap();
        assert_eq!(n, 4);
    });

    let mut buf = [0u8; 4];
    b.read_exact(&mut buf).await.unwrap();

    t1.await.unwrap();

    // drop b only after task t1 finishes writing
    drop(b);
}

#[tokio::test]
async fn duplex_is_cooperative() {
    let (mut tx, mut rx) = tokio::io::duplex(1024 * 8);

    tokio::select! {
        biased;

        _ = async {
            loop {
                let buf = [3u8; 4096];
                tx.write_all(&buf).await.unwrap();
                let mut buf = [0u8; 4096];
                let _ = rx.read(&mut buf).await.unwrap();
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}
