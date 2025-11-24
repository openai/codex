#![warn(rust_2018_idioms)]

use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio_stream::StreamExt;

/// produces at most `remaining` zeros, that returns error.
/// each time it reads at most 31 byte.
struct Reader {
    remaining: usize,
}

impl AsyncRead for Reader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = Pin::into_inner(self);
        assert_ne!(buf.remaining(), 0);
        if this.remaining > 0 {
            let n = std::cmp::min(this.remaining, buf.remaining());
            let n = std::cmp::min(n, 31);
            for x in &mut buf.initialize_unfilled_to(n)[..n] {
                *x = 0;
            }
            buf.advance(n);
            this.remaining -= n;
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(std::io::Error::from_raw_os_error(22)))
        }
    }
}

#[tokio::test]
async fn correct_behavior_on_errors() {
    let reader = Reader { remaining: 8000 };
    let mut stream = tokio_util::io::ReaderStream::new(reader);
    let mut zeros_received = 0;
    let mut had_error = false;
    loop {
        let item = stream.next().await.unwrap();
        println!("{item:?}");
        match item {
            Ok(bytes) => {
                let bytes = &*bytes;
                for byte in bytes {
                    assert_eq!(*byte, 0);
                    zeros_received += 1;
                }
            }
            Err(_) => {
                assert!(!had_error);
                had_error = true;
                break;
            }
        }
    }

    assert!(had_error);
    assert_eq!(zeros_received, 8000);
    assert!(stream.next().await.is_none());
}
