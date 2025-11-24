use futures::executor::block_on;
use futures::future::{Future, FutureExt};
use futures::io::{AsyncBufReadExt, Cursor};
use futures::stream::{self, StreamExt, TryStreamExt};
use futures::task::Poll;
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

#[test]
fn read_until() {
    let mut buf = Cursor::new(b"12");
    let mut v = Vec::new();
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 2);
    assert_eq!(v, b"12");

    let mut buf = Cursor::new(b"1233");
    let mut v = Vec::new();
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 3);
    assert_eq!(v, b"123");
    v.truncate(0);
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 1);
    assert_eq!(v, b"3");
    v.truncate(0);
    assert_eq!(block_on(buf.read_until(b'3', &mut v)).unwrap(), 0);
    assert_eq!(v, []);
}

#[test]
fn maybe_pending() {
    let mut buf = b"12".interleave_pending();
    let mut v = Vec::new();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 2);
    assert_eq!(v, b"12");

    let mut buf = stream::iter(vec![&b"12"[..], &b"33"[..], &b"3"[..]])
        .map(Ok)
        .into_async_read()
        .interleave_pending();
    let mut v = Vec::new();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 3);
    assert_eq!(v, b"123");
    v.clear();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 1);
    assert_eq!(v, b"3");
    v.clear();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 1);
    assert_eq!(v, b"3");
    v.clear();
    assert_eq!(run(buf.read_until(b'3', &mut v)).unwrap(), 0);
    assert_eq!(v, []);
}
