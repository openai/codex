use core::pin::Pin;
use futures::io::{AsyncBufRead, AsyncRead};
use futures::stream::{self, TryStreamExt};
use futures::task::Poll;
use futures_test::{stream::StreamTestExt, task::noop_context};

macro_rules! assert_read {
    ($reader:expr, $buf:expr, $item:expr) => {
        let mut cx = noop_context();
        loop {
            match Pin::new(&mut $reader).poll_read(&mut cx, $buf) {
                Poll::Ready(Ok(x)) => {
                    assert_eq!(x, $item);
                    break;
                }
                Poll::Ready(Err(err)) => {
                    panic!("assertion failed: expected value but got {}", err);
                }
                Poll::Pending => {
                    continue;
                }
            }
        }
    };
}

macro_rules! assert_fill_buf {
    ($reader:expr, $buf:expr) => {
        let mut cx = noop_context();
        loop {
            match Pin::new(&mut $reader).poll_fill_buf(&mut cx) {
                Poll::Ready(Ok(x)) => {
                    assert_eq!(x, $buf);
                    break;
                }
                Poll::Ready(Err(err)) => {
                    panic!("assertion failed: expected value but got {}", err);
                }
                Poll::Pending => {
                    continue;
                }
            }
        }
    };
}

#[test]
fn test_into_async_read() {
    let stream = stream::iter((1..=3).flat_map(|_| vec![Ok(vec![]), Ok(vec![1, 2, 3, 4, 5])]));
    let mut reader = stream.interleave_pending().into_async_read();
    let mut buf = vec![0; 3];

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 3);
    assert_eq!(&buf, &[1, 2, 3]);

    assert_read!(reader, &mut buf, 2);
    assert_eq!(&buf[..2], &[4, 5]);

    assert_read!(reader, &mut buf, 0);
}

#[test]
fn test_into_async_bufread() {
    let stream = stream::iter((1..=2).flat_map(|_| vec![Ok(vec![]), Ok(vec![1, 2, 3, 4, 5])]));
    let mut reader = stream.interleave_pending().into_async_read();

    let mut reader = Pin::new(&mut reader);

    assert_fill_buf!(reader, &[1, 2, 3, 4, 5][..]);
    reader.as_mut().consume(3);

    assert_fill_buf!(reader, &[4, 5][..]);
    reader.as_mut().consume(2);

    assert_fill_buf!(reader, &[1, 2, 3, 4, 5][..]);
    reader.as_mut().consume(2);

    assert_fill_buf!(reader, &[3, 4, 5][..]);
    reader.as_mut().consume(3);

    assert_fill_buf!(reader, &[][..]);
}
