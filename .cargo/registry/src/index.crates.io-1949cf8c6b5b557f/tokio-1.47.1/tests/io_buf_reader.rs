#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

// https://github.com/rust-lang/futures-rs/blob/1803948ff091b4eabf7f3bf39e16bbbdefca5cc8/futures/tests/io_buf_reader.rs

use futures::task::{noop_waker_ref, Context, Poll};
use std::cmp;
use std::io::{self, Cursor};
use std::pin::Pin;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWriteExt,
    BufReader, ReadBuf, SeekFrom,
};
use tokio_test::task::spawn;
use tokio_test::{assert_pending, assert_ready};

macro_rules! run_fill_buf {
    ($reader:expr) => {{
        let mut cx = Context::from_waker(noop_waker_ref());
        loop {
            if let Poll::Ready(x) = Pin::new(&mut $reader).poll_fill_buf(&mut cx) {
                break x;
            }
        }
    }};
}

struct MaybePending<'a> {
    inner: &'a [u8],
    ready_read: bool,
    ready_fill_buf: bool,
}

impl<'a> MaybePending<'a> {
    fn new(inner: &'a [u8]) -> Self {
        Self {
            inner,
            ready_read: false,
            ready_fill_buf: false,
        }
    }
}

impl AsyncRead for MaybePending<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.ready_read {
            self.ready_read = false;
            Pin::new(&mut self.inner).poll_read(cx, buf)
        } else {
            self.ready_read = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl AsyncBufRead for MaybePending<'_> {
    fn poll_fill_buf(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        if self.ready_fill_buf {
            self.ready_fill_buf = false;
            if self.inner.is_empty() {
                return Poll::Ready(Ok(&[]));
            }
            let len = cmp::min(2, self.inner.len());
            Poll::Ready(Ok(&self.inner[0..len]))
        } else {
            self.ready_fill_buf = true;
            Poll::Pending
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.inner = &self.inner[amt..];
    }
}

#[tokio::test]
async fn test_buffered_reader() {
    let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
    let mut reader = BufReader::with_capacity(2, inner);

    let mut buf = [0, 0, 0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 3);
    assert_eq!(buf, [5, 6, 7]);
    assert_eq!(reader.buffer(), []);

    let mut buf = [0, 0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 2);
    assert_eq!(buf, [0, 1]);
    assert_eq!(reader.buffer(), []);

    let mut buf = [0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 1);
    assert_eq!(buf, [2]);
    assert_eq!(reader.buffer(), [3]);

    let mut buf = [0, 0, 0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 1);
    assert_eq!(buf, [3, 0, 0]);
    assert_eq!(reader.buffer(), []);

    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 1);
    assert_eq!(buf, [4, 0, 0]);
    assert_eq!(reader.buffer(), []);

    assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
}

#[tokio::test]
async fn test_buffered_reader_seek() {
    let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
    let mut reader = BufReader::with_capacity(2, Cursor::new(inner));

    assert_eq!(reader.seek(SeekFrom::Start(3)).await.unwrap(), 3);
    assert_eq!(run_fill_buf!(reader).unwrap(), &[0, 1][..]);
    assert!(reader.seek(SeekFrom::Current(i64::MIN)).await.is_err());
    assert_eq!(run_fill_buf!(reader).unwrap(), &[0, 1][..]);
    assert_eq!(reader.seek(SeekFrom::Current(1)).await.unwrap(), 4);
    assert_eq!(run_fill_buf!(reader).unwrap(), &[1, 2][..]);
    Pin::new(&mut reader).consume(1);
    assert_eq!(reader.seek(SeekFrom::Current(-2)).await.unwrap(), 3);
}

#[tokio::test]
async fn test_buffered_reader_seek_underflow() {
    // gimmick reader that yields its position modulo 256 for each byte
    struct PositionReader {
        pos: u64,
    }
    impl AsyncRead for PositionReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let b = buf.initialize_unfilled();
            let len = b.len();
            for x in b {
                *x = self.pos as u8;
                self.pos = self.pos.wrapping_add(1);
            }
            buf.advance(len);
            Poll::Ready(Ok(()))
        }
    }
    impl AsyncSeek for PositionReader {
        fn start_seek(mut self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
            match pos {
                SeekFrom::Start(n) => {
                    self.pos = n;
                }
                SeekFrom::Current(n) => {
                    self.pos = self.pos.wrapping_add(n as u64);
                }
                SeekFrom::End(n) => {
                    self.pos = u64::MAX.wrapping_add(n as u64);
                }
            }
            Ok(())
        }
        fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
            Poll::Ready(Ok(self.pos))
        }
    }

    let mut reader = BufReader::with_capacity(5, PositionReader { pos: 0 });
    assert_eq!(run_fill_buf!(reader).unwrap(), &[0, 1, 2, 3, 4][..]);
    assert_eq!(reader.seek(SeekFrom::End(-5)).await.unwrap(), u64::MAX - 5);
    assert_eq!(run_fill_buf!(reader).unwrap().len(), 5);
    // the following seek will require two underlying seeks
    let expected = 9_223_372_036_854_775_802;
    assert_eq!(
        reader.seek(SeekFrom::Current(i64::MIN)).await.unwrap(),
        expected
    );
    assert_eq!(run_fill_buf!(reader).unwrap().len(), 5);
    // seeking to 0 should empty the buffer.
    assert_eq!(reader.seek(SeekFrom::Current(0)).await.unwrap(), expected);
    assert_eq!(reader.get_ref().pos, expected);
}

#[tokio::test]
async fn test_short_reads() {
    /// A dummy reader intended at testing short-reads propagation.
    struct ShortReader {
        lengths: Vec<usize>,
    }

    impl AsyncRead for ShortReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            if !self.lengths.is_empty() {
                buf.advance(self.lengths.remove(0));
            }
            Poll::Ready(Ok(()))
        }
    }

    let inner = ShortReader {
        lengths: vec![0, 1, 2, 0, 1, 0],
    };
    let mut reader = BufReader::new(inner);
    let mut buf = [0, 0];
    assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
    assert_eq!(reader.read(&mut buf).await.unwrap(), 1);
    assert_eq!(reader.read(&mut buf).await.unwrap(), 2);
    assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
    assert_eq!(reader.read(&mut buf).await.unwrap(), 1);
    assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
    assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
}

#[tokio::test]
async fn maybe_pending() {
    let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
    let mut reader = BufReader::with_capacity(2, MaybePending::new(inner));

    let mut buf = [0, 0, 0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 3);
    assert_eq!(buf, [5, 6, 7]);
    assert_eq!(reader.buffer(), []);

    let mut buf = [0, 0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 2);
    assert_eq!(buf, [0, 1]);
    assert_eq!(reader.buffer(), []);

    let mut buf = [0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 1);
    assert_eq!(buf, [2]);
    assert_eq!(reader.buffer(), [3]);

    let mut buf = [0, 0, 0];
    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 1);
    assert_eq!(buf, [3, 0, 0]);
    assert_eq!(reader.buffer(), []);

    let nread = reader.read(&mut buf).await.unwrap();
    assert_eq!(nread, 1);
    assert_eq!(buf, [4, 0, 0]);
    assert_eq!(reader.buffer(), []);

    assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
}

#[tokio::test]
async fn maybe_pending_buf_read() {
    let inner = MaybePending::new(&[0, 1, 2, 3, 1, 0]);
    let mut reader = BufReader::with_capacity(2, inner);
    let mut v = Vec::new();
    reader.read_until(3, &mut v).await.unwrap();
    assert_eq!(v, [0, 1, 2, 3]);
    v.clear();
    reader.read_until(1, &mut v).await.unwrap();
    assert_eq!(v, [1]);
    v.clear();
    reader.read_until(8, &mut v).await.unwrap();
    assert_eq!(v, [0]);
    v.clear();
    reader.read_until(9, &mut v).await.unwrap();
    assert_eq!(v, []);
}

// https://github.com/rust-lang/futures-rs/pull/1573#discussion_r281162309
#[tokio::test]
async fn maybe_pending_seek() {
    struct MaybePendingSeek<'a> {
        inner: Cursor<&'a [u8]>,
        ready: bool,
        seek_res: Option<io::Result<()>>,
    }

    impl<'a> MaybePendingSeek<'a> {
        fn new(inner: &'a [u8]) -> Self {
            Self {
                inner: Cursor::new(inner),
                ready: true,
                seek_res: None,
            }
        }
    }

    impl AsyncRead for MaybePendingSeek<'_> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }

    impl AsyncBufRead for MaybePendingSeek<'_> {
        fn poll_fill_buf(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<io::Result<&[u8]>> {
            let this: *mut Self = &mut *self as *mut _;
            Pin::new(&mut unsafe { &mut *this }.inner).poll_fill_buf(cx)
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            Pin::new(&mut self.inner).consume(amt)
        }
    }

    impl AsyncSeek for MaybePendingSeek<'_> {
        fn start_seek(mut self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
            self.seek_res = Some(Pin::new(&mut self.inner).start_seek(pos));
            Ok(())
        }
        fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
            if self.ready {
                self.ready = false;
                self.seek_res.take().unwrap_or(Ok(()))?;
                Pin::new(&mut self.inner).poll_complete(cx)
            } else {
                self.ready = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
    let mut reader = BufReader::with_capacity(2, MaybePendingSeek::new(inner));

    assert_eq!(reader.seek(SeekFrom::Current(3)).await.unwrap(), 3);
    assert_eq!(run_fill_buf!(reader).unwrap(), &[0, 1][..]);
    assert!(reader.seek(SeekFrom::Current(i64::MIN)).await.is_err());
    assert_eq!(run_fill_buf!(reader).unwrap(), &[0, 1][..]);
    assert_eq!(reader.seek(SeekFrom::Current(1)).await.unwrap(), 4);
    assert_eq!(run_fill_buf!(reader).unwrap(), &[1, 2][..]);
    Pin::new(&mut reader).consume(1);
    assert_eq!(reader.seek(SeekFrom::Current(-2)).await.unwrap(), 3);
}

// This tests the AsyncBufReadExt::fill_buf wrapper.
#[tokio::test]
async fn test_fill_buf_wrapper() {
    let (mut write, read) = tokio::io::duplex(16);

    let mut read = BufReader::new(read);
    write.write_all(b"hello world").await.unwrap();

    assert_eq!(read.fill_buf().await.unwrap(), b"hello world");
    read.consume(b"hello ".len());
    assert_eq!(read.fill_buf().await.unwrap(), b"world");
    assert_eq!(read.fill_buf().await.unwrap(), b"world");
    read.consume(b"world".len());

    let mut fill = spawn(read.fill_buf());
    assert_pending!(fill.poll());

    write.write_all(b"foo bar").await.unwrap();
    assert_eq!(assert_ready!(fill.poll()).unwrap(), b"foo bar");
    drop(fill);

    drop(write);
    assert_eq!(read.fill_buf().await.unwrap(), b"foo bar");
    read.consume(b"foo bar".len());
    assert_eq!(read.fill_buf().await.unwrap(), b"");
}
