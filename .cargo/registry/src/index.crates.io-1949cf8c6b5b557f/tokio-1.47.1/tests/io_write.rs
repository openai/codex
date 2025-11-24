#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_test::assert_ok;

use bytes::BytesMut;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[tokio::test]
async fn write() {
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
            assert_eq!(self.cnt, 0);
            self.buf.extend(&buf[0..4]);
            Ok(4).into()
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

    let n = assert_ok!(wr.write(b"hello world").await);
    assert_eq!(n, 4);
    assert_eq!(wr.buf, b"hell"[..]);
}

#[tokio::test]
async fn write_cursor() {
    use std::io::Cursor;

    let mut wr = Cursor::new(Vec::new());

    let n = assert_ok!(wr.write(b"hello world").await);
    assert_eq!(n, 11);
    assert_eq!(wr.get_ref().as_slice(), &b"hello world"[..]);
}
