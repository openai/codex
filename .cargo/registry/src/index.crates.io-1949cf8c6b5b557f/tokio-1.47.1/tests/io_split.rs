#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi does not support panic recovery

use tokio::io::{
    split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf, ReadHalf, WriteHalf,
};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

struct RW;

impl AsyncRead for RW {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        buf.put_slice(b"z");
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for RW {
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

    assert_bound::<ReadHalf<RW>>();
    assert_bound::<WriteHalf<RW>>();
}

#[test]
fn split_stream_id() {
    let (r1, w1) = split(RW);
    let (r2, w2) = split(RW);
    assert!(r1.is_pair_of(&w1));
    assert!(!r1.is_pair_of(&w2));
    assert!(r2.is_pair_of(&w2));
    assert!(!r2.is_pair_of(&w1));
}

#[test]
fn unsplit_ok() {
    let (r, w) = split(RW);
    r.unsplit(w);
}

#[test]
#[should_panic]
fn unsplit_err1() {
    let (r, _) = split(RW);
    let (_, w) = split(RW);
    r.unsplit(w);
}

#[test]
#[should_panic]
fn unsplit_err2() {
    let (_, w) = split(RW);
    let (r, _) = split(RW);
    r.unsplit(w);
}

#[test]
fn method_delegation() {
    let (mut r, mut w) = split(RW);
    let mut buf = [0; 1];

    tokio_test::block_on(async move {
        assert_eq!(1, r.read(&mut buf).await.unwrap());
        assert_eq!(b'z', buf[0]);

        assert_eq!(1, w.write(b"x").await.unwrap());
        assert_eq!(
            2,
            w.write_vectored(&[io::IoSlice::new(b"x")]).await.unwrap()
        );
        assert!(w.is_write_vectored());

        assert!(w.flush().await.is_ok());
        assert!(w.shutdown().await.is_ok());
    });
}
