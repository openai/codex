use futures::executor::block_on;
use futures::future::{Future, FutureExt};
use futures::io::{AsyncBufReadExt, Cursor};
use futures::stream::{self, StreamExt, TryStreamExt};
use futures::task::Poll;
use futures::AsyncRead;
use futures_test::io::AsyncReadTestExt;
use futures_test::task::noop_context;

fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
            return x;
        }
    }
}

struct IOErrorRead(bool);

impl AsyncRead for IOErrorRead {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        b: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.0 {
            Poll::Ready(Err(std::io::ErrorKind::InvalidInput.into()))
        } else {
            self.0 = true;
            b[..16].fill(b'x');
            Ok(16).into()
        }
    }
}

#[test]
fn read_line() {
    let mut buf = Cursor::new(b"12");
    let mut v = String::new();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 2);
    assert_eq!(v, "12");

    let mut buf = Cursor::new(b"12\n\n");
    let mut v = String::new();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 3);
    assert_eq!(v, "12\n");
    v.clear();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 1);
    assert_eq!(v, "\n");
    v.clear();
    assert_eq!(block_on(buf.read_line(&mut v)).unwrap(), 0);
    assert_eq!(v, "");
}

#[test]
fn read_line_drop() {
    // string contents should be preserved if the future is dropped
    let mut buf = Cursor::new(b"12\n\n");
    let mut v = String::from("abc");
    drop(buf.read_line(&mut v));
    assert_eq!(v, "abc");
}

#[test]
fn read_line_io_error() {
    let mut r = futures::io::BufReader::new(IOErrorRead(false));
    let _ = block_on(r.read_line(&mut String::new()));
}

#[test]
fn read_line_utf8_error() {
    let mut buf = Cursor::new(b"12\xFF\n\n");
    let mut v = String::from("abc");
    let res = block_on(buf.read_line(&mut v));
    assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(v, "abc");
}

#[test]
fn maybe_pending() {
    let mut buf = b"12".interleave_pending();
    let mut v = String::new();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 2);
    assert_eq!(v, "12");

    let mut buf =
        stream::iter(vec![&b"12"[..], &b"\n\n"[..]]).map(Ok).into_async_read().interleave_pending();
    let mut v = String::new();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 3);
    assert_eq!(v, "12\n");
    v.clear();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 1);
    assert_eq!(v, "\n");
    v.clear();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 0);
    assert_eq!(v, "");
    v.clear();
    assert_eq!(run(buf.read_line(&mut v)).unwrap(), 0);
    assert_eq!(v, "");
}
