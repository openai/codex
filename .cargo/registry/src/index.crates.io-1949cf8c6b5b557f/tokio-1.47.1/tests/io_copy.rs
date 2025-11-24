#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use bytes::BytesMut;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio_test::assert_ok;

use std::pin::Pin;
use std::task::{ready, Context, Poll};

#[tokio::test]
async fn copy() {
    struct Rd(bool);

    impl AsyncRead for Rd {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            if self.0 {
                buf.put_slice(b"hello world");
                self.0 = false;
                Poll::Ready(Ok(()))
            } else {
                Poll::Ready(Ok(()))
            }
        }
    }

    let mut rd = Rd(true);
    let mut wr = Vec::new();

    let n = assert_ok!(io::copy(&mut rd, &mut wr).await);
    assert_eq!(n, 11);
    assert_eq!(wr, b"hello world");
}

#[tokio::test]
async fn proxy() {
    struct BufferedWd {
        buf: BytesMut,
        writer: io::DuplexStream,
    }

    impl AsyncWrite for BufferedWd {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.get_mut().buf.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let this = self.get_mut();

            while !this.buf.is_empty() {
                let n = ready!(Pin::new(&mut this.writer).poll_write(cx, &this.buf))?;
                let _ = this.buf.split_to(n);
            }

            Pin::new(&mut this.writer).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.writer).poll_shutdown(cx)
        }
    }

    let (rd, wd) = io::duplex(1024);
    let mut rd = rd.take(1024);
    let mut wd = BufferedWd {
        buf: BytesMut::new(),
        writer: wd,
    };

    // write start bytes
    assert_ok!(wd.write_all(&[0x42; 512]).await);
    assert_ok!(wd.flush().await);

    let n = assert_ok!(io::copy(&mut rd, &mut wd).await);

    assert_eq!(n, 1024);
}

#[tokio::test]
async fn copy_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                let mut reader: &[u8] = b"hello";
                let mut writer: Vec<u8> = vec![];
                let _ = io::copy(&mut reader, &mut writer).await;
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}
