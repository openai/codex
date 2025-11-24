#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::{join, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, Join, ReadBuf};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

struct R;

impl AsyncRead for R {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        buf.put_slice(b"z");
        Poll::Ready(Ok(()))
    }
}

struct W;

impl AsyncWrite for W {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Poll::Ready(Ok(1))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        Poll::Ready(Ok(2))
    }

    fn is_write_vectored(&self) -> bool {
        true
    }
}

#[test]
fn is_send_and_sync() {
    fn assert_bound<T: Send + Sync>() {}

    assert_bound::<Join<W, R>>();
}

#[test]
fn method_delegation() {
    let mut rw = join(R, W);
    let mut buf = [0; 1];

    tokio_test::block_on(async move {
        assert_eq!(1, rw.read(&mut buf).await.unwrap());
        assert_eq!(b'z', buf[0]);

        assert_eq!(1, rw.write(b"x").await.unwrap());
        assert_eq!(
            2,
            rw.write_vectored(&[io::IoSlice::new(b"x")]).await.unwrap()
        );
        assert!(rw.is_write_vectored());

        assert!(rw.flush().await.is_ok());
        assert!(rw.shutdown().await.is_ok());
    });
}
