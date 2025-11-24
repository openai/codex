#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi does not support bind()

use std::time::Duration;
use tokio::io::{self, copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

async fn make_socketpair() -> (TcpStream, TcpStream) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let connector = TcpStream::connect(addr);
    let acceptor = listener.accept();

    let (c1, c2) = tokio::join!(connector, acceptor);

    (c1.unwrap(), c2.unwrap().0)
}

async fn block_write(s: &mut TcpStream) -> usize {
    static BUF: [u8; 2048] = [0; 2048];

    let mut copied = 0;
    loop {
        tokio::select! {
            result = s.write(&BUF) => {
                copied += result.expect("write error")
            },
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                break;
            }
        }
    }

    copied
}

async fn symmetric<F, Fut>(mut cb: F)
where
    F: FnMut(JoinHandle<io::Result<(u64, u64)>>, TcpStream, TcpStream) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    // We run the test twice, with streams passed to copy_bidirectional in
    // different orders, in order to ensure that the two arguments are
    // interchangeable.

    let (a, mut a1) = make_socketpair().await;
    let (b, mut b1) = make_socketpair().await;

    let handle = tokio::spawn(async move { copy_bidirectional(&mut a1, &mut b1).await });
    cb(handle, a, b).await;

    let (a, mut a1) = make_socketpair().await;
    let (b, mut b1) = make_socketpair().await;

    let handle = tokio::spawn(async move { copy_bidirectional(&mut b1, &mut a1).await });

    cb(handle, b, a).await;
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
async fn test_basic_transfer() {
    symmetric(|_handle, mut a, mut b| async move {
        a.write_all(b"test").await.unwrap();
        let mut tmp = [0; 4];
        b.read_exact(&mut tmp).await.unwrap();
        assert_eq!(&tmp[..], b"test");
    })
    .await
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
async fn test_transfer_after_close() {
    symmetric(|handle, mut a, mut b| async move {
        AsyncWriteExt::shutdown(&mut a).await.unwrap();
        b.read_to_end(&mut Vec::new()).await.unwrap();

        b.write_all(b"quux").await.unwrap();
        let mut tmp = [0; 4];
        a.read_exact(&mut tmp).await.unwrap();
        assert_eq!(&tmp[..], b"quux");

        // Once both are closed, we should have our handle back
        drop(b);

        assert_eq!(handle.await.unwrap().unwrap(), (0, 4));
    })
    .await
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
async fn blocking_one_side_does_not_block_other() {
    symmetric(|handle, mut a, mut b| async move {
        block_write(&mut a).await;

        b.write_all(b"quux").await.unwrap();
        let mut tmp = [0; 4];
        a.read_exact(&mut tmp).await.unwrap();
        assert_eq!(&tmp[..], b"quux");

        AsyncWriteExt::shutdown(&mut a).await.unwrap();

        let mut buf = Vec::new();
        b.read_to_end(&mut buf).await.unwrap();

        drop(b);

        assert_eq!(handle.await.unwrap().unwrap(), (buf.len() as u64, 4));
    })
    .await
}

#[tokio::test]
async fn immediate_exit_on_write_error() {
    let payload = b"here, take this";
    let error = || io::Error::new(io::ErrorKind::Other, "no thanks!");

    let mut a = tokio_test::io::Builder::new()
        .read(payload)
        .write_error(error())
        .build();

    let mut b = tokio_test::io::Builder::new()
        .read(payload)
        .write_error(error())
        .build();

    assert!(copy_bidirectional(&mut a, &mut b).await.is_err());
}

#[tokio::test]
async fn immediate_exit_on_read_error() {
    let error = || io::Error::new(io::ErrorKind::Other, "got nothing!");

    let mut a = tokio_test::io::Builder::new().read_error(error()).build();

    let mut b = tokio_test::io::Builder::new().read_error(error()).build();

    assert!(copy_bidirectional(&mut a, &mut b).await.is_err());
}

#[tokio::test]
async fn copy_bidirectional_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                let payload = b"here, take this";

                let mut a = tokio_test::io::Builder::new()
                    .read(payload)
                    .write(payload)
                    .build();

                let mut b = tokio_test::io::Builder::new()
                    .read(payload)
                    .write(payload)
                    .build();

                let _ = copy_bidirectional(&mut a, &mut b).await;
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}
