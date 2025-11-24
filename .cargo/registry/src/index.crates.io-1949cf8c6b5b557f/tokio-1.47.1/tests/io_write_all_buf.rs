#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_test::{assert_err, assert_ok};

use bytes::{Buf, Bytes, BytesMut};
use std::cmp;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[tokio::test]
async fn write_all_buf() {
    struct Wr {
        buf: BytesMut,
        cnt: usize,
    }

    impl AsyncWrite for Wr {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let n = cmp::min(4, buf.len());
            dbg!(buf);
            let buf = &buf[0..n];

            self.cnt += 1;
            self.buf.extend(buf);
            Ok(buf.len()).into()
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }
    }

    let mut wr = Wr {
        buf: BytesMut::with_capacity(64),
        cnt: 0,
    };

    let mut buf = Bytes::from_static(b"hello").chain(Bytes::from_static(b"world"));

    assert_ok!(wr.write_all_buf(&mut buf).await);
    assert_eq!(wr.buf, b"helloworld"[..]);
    // expect 4 writes, [hell],[o],[worl],[d]
    assert_eq!(wr.cnt, 4);
    assert!(!buf.has_remaining());
}

#[tokio::test]
async fn write_buf_err() {
    /// Error out after writing the first 4 bytes
    struct Wr {
        cnt: usize,
    }

    impl AsyncWrite for Wr {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.cnt += 1;
            if self.cnt == 2 {
                return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "whoops")));
            }
            Poll::Ready(Ok(4))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }
    }

    let mut wr = Wr { cnt: 0 };

    let mut buf = Bytes::from_static(b"hello").chain(Bytes::from_static(b"world"));

    assert_err!(wr.write_all_buf(&mut buf).await);
    assert_eq!(
        buf.copy_to_bytes(buf.remaining()),
        Bytes::from_static(b"oworld")
    );
}

#[tokio::test]
async fn write_all_buf_vectored() {
    struct Wr {
        buf: BytesMut,
    }
    impl AsyncWrite for Wr {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            // When executing `write_all_buf` with this writer,
            // `poll_write` is not called.
            panic!("shouldn't be called")
        }
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }
        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }
        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[io::IoSlice<'_>],
        ) -> Poll<Result<usize, io::Error>> {
            for buf in bufs {
                self.buf.extend_from_slice(buf);
            }
            let n = self.buf.len();
            Ok(n).into()
        }
        fn is_write_vectored(&self) -> bool {
            // Enable vectored write.
            true
        }
    }

    let mut wr = Wr {
        buf: BytesMut::with_capacity(64),
    };
    let mut buf = Bytes::from_static(b"hello")
        .chain(Bytes::from_static(b" "))
        .chain(Bytes::from_static(b"world"));

    wr.write_all_buf(&mut buf).await.unwrap();
    assert_eq!(&wr.buf[..], b"hello world");
}
