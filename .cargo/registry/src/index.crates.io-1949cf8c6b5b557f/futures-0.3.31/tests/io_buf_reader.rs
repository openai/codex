use futures::executor::block_on;
use futures::future::{Future, FutureExt};
use futures::io::{
    AllowStdIo, AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt,
    BufReader, SeekFrom,
};
use futures::pin_mut;
use futures::task::{Context, Poll};
use futures_test::task::noop_context;
use pin_project::pin_project;
use std::cmp;
use std::io;
use std::pin::Pin;

// helper for maybe_pending_* tests
fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
            return x;
        }
    }
}

// https://github.com/rust-lang/futures-rs/pull/2489#discussion_r697865719
#[pin_project(!Unpin)]
struct Cursor<T> {
    #[pin]
    inner: futures::io::Cursor<T>,
}

impl<T> Cursor<T> {
    fn new(inner: T) -> Self {
        Self { inner: futures::io::Cursor::new(inner) }
    }
}

impl AsyncRead for Cursor<&[u8]> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_read(cx, buf)
    }
}

impl AsyncBufRead for Cursor<&[u8]> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.project().inner.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.project().inner.consume(amt)
    }
}

impl AsyncSeek for Cursor<&[u8]> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        self.project().inner.poll_seek(cx, pos)
    }
}

struct MaybePending<'a> {
    inner: &'a [u8],
    ready_read: bool,
    ready_fill_buf: bool,
}

impl<'a> MaybePending<'a> {
    fn new(inner: &'a [u8]) -> Self {
        Self { inner, ready_read: false, ready_fill_buf: false }
    }
}

impl AsyncRead for MaybePending<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if self.ready_read {
            self.ready_read = false;
            Pin::new(&mut self.inner).poll_read(cx, buf)
        } else {
            self.ready_read = true;
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

#[test]
fn test_buffered_reader() {
    block_on(async {
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
    });
}

#[test]
fn test_buffered_reader_seek() {
    block_on(async {
        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let reader = BufReader::with_capacity(2, Cursor::new(inner));
        pin_mut!(reader);

        assert_eq!(reader.seek(SeekFrom::Start(3)).await.unwrap(), 3);
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[0, 1][..]);
        assert!(reader.seek(SeekFrom::Current(i64::MIN)).await.is_err());
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[0, 1][..]);
        assert_eq!(reader.seek(SeekFrom::Current(1)).await.unwrap(), 4);
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[1, 2][..]);
        reader.as_mut().consume(1);
        assert_eq!(reader.seek(SeekFrom::Current(-2)).await.unwrap(), 3);
    });
}

#[test]
fn test_buffered_reader_seek_relative() {
    block_on(async {
        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let reader = BufReader::with_capacity(2, Cursor::new(inner));
        pin_mut!(reader);

        assert!(reader.as_mut().seek_relative(3).await.is_ok());
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[0, 1][..]);
        assert!(reader.as_mut().seek_relative(0).await.is_ok());
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[0, 1][..]);
        assert!(reader.as_mut().seek_relative(1).await.is_ok());
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[1][..]);
        assert!(reader.as_mut().seek_relative(-1).await.is_ok());
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[0, 1][..]);
        assert!(reader.as_mut().seek_relative(2).await.is_ok());
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[2, 3][..]);
    });
}

#[test]
fn test_buffered_reader_invalidated_after_read() {
    block_on(async {
        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let reader = BufReader::with_capacity(3, Cursor::new(inner));
        pin_mut!(reader);

        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[5, 6, 7][..]);
        reader.as_mut().consume(3);

        let mut buffer = [0, 0, 0, 0, 0];
        assert_eq!(reader.read(&mut buffer).await.unwrap(), 5);
        assert_eq!(buffer, [0, 1, 2, 3, 4]);

        assert!(reader.as_mut().seek_relative(-2).await.is_ok());
        let mut buffer = [0, 0];
        assert_eq!(reader.read(&mut buffer).await.unwrap(), 2);
        assert_eq!(buffer, [3, 4]);
    });
}

#[test]
fn test_buffered_reader_invalidated_after_seek() {
    block_on(async {
        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let reader = BufReader::with_capacity(3, Cursor::new(inner));
        pin_mut!(reader);

        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[5, 6, 7][..]);
        reader.as_mut().consume(3);

        assert!(reader.seek(SeekFrom::Current(5)).await.is_ok());

        assert!(reader.as_mut().seek_relative(-2).await.is_ok());
        let mut buffer = [0, 0];
        assert_eq!(reader.read(&mut buffer).await.unwrap(), 2);
        assert_eq!(buffer, [3, 4]);
    });
}

#[test]
fn test_buffered_reader_seek_underflow() {
    // gimmick reader that yields its position modulo 256 for each byte
    struct PositionReader {
        pos: u64,
    }
    impl io::Read for PositionReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let len = buf.len();
            for x in buf {
                *x = self.pos as u8;
                self.pos = self.pos.wrapping_add(1);
            }
            Ok(len)
        }
    }
    impl io::Seek for PositionReader {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
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
            Ok(self.pos)
        }
    }

    block_on(async {
        let reader = BufReader::with_capacity(5, AllowStdIo::new(PositionReader { pos: 0 }));
        pin_mut!(reader);
        assert_eq!(reader.as_mut().fill_buf().await.unwrap(), &[0, 1, 2, 3, 4][..]);
        assert_eq!(reader.seek(SeekFrom::End(-5)).await.unwrap(), u64::MAX - 5);
        assert_eq!(reader.as_mut().fill_buf().await.unwrap().len(), 5);
        // the following seek will require two underlying seeks
        let expected = 9_223_372_036_854_775_802;
        assert_eq!(reader.seek(SeekFrom::Current(i64::MIN)).await.unwrap(), expected);
        assert_eq!(reader.as_mut().fill_buf().await.unwrap().len(), 5);
        // seeking to 0 should empty the buffer.
        assert_eq!(reader.seek(SeekFrom::Current(0)).await.unwrap(), expected);
        assert_eq!(reader.get_ref().get_ref().pos, expected);
    });
}

#[test]
fn test_short_reads() {
    /// A dummy reader intended at testing short-reads propagation.
    struct ShortReader {
        lengths: Vec<usize>,
    }

    impl io::Read for ShortReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            if self.lengths.is_empty() {
                Ok(0)
            } else {
                Ok(self.lengths.remove(0))
            }
        }
    }

    block_on(async {
        let inner = ShortReader { lengths: vec![0, 1, 2, 0, 1, 0] };
        let mut reader = BufReader::new(AllowStdIo::new(inner));
        let mut buf = [0, 0];
        assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
        assert_eq!(reader.read(&mut buf).await.unwrap(), 1);
        assert_eq!(reader.read(&mut buf).await.unwrap(), 2);
        assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
        assert_eq!(reader.read(&mut buf).await.unwrap(), 1);
        assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
        assert_eq!(reader.read(&mut buf).await.unwrap(), 0);
    });
}

#[test]
fn maybe_pending() {
    let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
    let mut reader = BufReader::with_capacity(2, MaybePending::new(inner));

    let mut buf = [0, 0, 0];
    let nread = run(reader.read(&mut buf));
    assert_eq!(nread.unwrap(), 3);
    assert_eq!(buf, [5, 6, 7]);
    assert_eq!(reader.buffer(), []);

    let mut buf = [0, 0];
    let nread = run(reader.read(&mut buf));
    assert_eq!(nread.unwrap(), 2);
    assert_eq!(buf, [0, 1]);
    assert_eq!(reader.buffer(), []);

    let mut buf = [0];
    let nread = run(reader.read(&mut buf));
    assert_eq!(nread.unwrap(), 1);
    assert_eq!(buf, [2]);
    assert_eq!(reader.buffer(), [3]);

    let mut buf = [0, 0, 0];
    let nread = run(reader.read(&mut buf));
    assert_eq!(nread.unwrap(), 1);
    assert_eq!(buf, [3, 0, 0]);
    assert_eq!(reader.buffer(), []);

    let nread = run(reader.read(&mut buf));
    assert_eq!(nread.unwrap(), 1);
    assert_eq!(buf, [4, 0, 0]);
    assert_eq!(reader.buffer(), []);

    assert_eq!(run(reader.read(&mut buf)).unwrap(), 0);
}

#[test]
fn maybe_pending_buf_read() {
    let inner = MaybePending::new(&[0, 1, 2, 3, 1, 0]);
    let mut reader = BufReader::with_capacity(2, inner);
    let mut v = Vec::new();
    run(reader.read_until(3, &mut v)).unwrap();
    assert_eq!(v, [0, 1, 2, 3]);
    v.clear();
    run(reader.read_until(1, &mut v)).unwrap();
    assert_eq!(v, [1]);
    v.clear();
    run(reader.read_until(8, &mut v)).unwrap();
    assert_eq!(v, [0]);
    v.clear();
    run(reader.read_until(9, &mut v)).unwrap();
    assert_eq!(v, []);
}

// https://github.com/rust-lang/futures-rs/pull/1573#discussion_r281162309
#[test]
fn maybe_pending_seek() {
    #[pin_project]
    struct MaybePendingSeek<'a> {
        #[pin]
        inner: Cursor<&'a [u8]>,
        ready: bool,
    }

    impl<'a> MaybePendingSeek<'a> {
        fn new(inner: &'a [u8]) -> Self {
            Self { inner: Cursor::new(inner), ready: true }
        }
    }

    impl AsyncRead for MaybePendingSeek<'_> {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            self.project().inner.poll_read(cx, buf)
        }
    }

    impl AsyncBufRead for MaybePendingSeek<'_> {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
            self.project().inner.poll_fill_buf(cx)
        }

        fn consume(self: Pin<&mut Self>, amt: usize) {
            self.project().inner.consume(amt)
        }
    }

    impl AsyncSeek for MaybePendingSeek<'_> {
        fn poll_seek(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<io::Result<u64>> {
            if self.ready {
                *self.as_mut().project().ready = false;
                self.project().inner.poll_seek(cx, pos)
            } else {
                *self.project().ready = true;
                Poll::Pending
            }
        }
    }

    let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
    let reader = BufReader::with_capacity(2, MaybePendingSeek::new(inner));
    pin_mut!(reader);

    assert_eq!(run(reader.seek(SeekFrom::Current(3))).ok(), Some(3));
    assert_eq!(run(reader.as_mut().fill_buf()).ok(), Some(&[0, 1][..]));
    assert_eq!(run(reader.seek(SeekFrom::Current(i64::MIN))).ok(), None);
    assert_eq!(run(reader.as_mut().fill_buf()).ok(), Some(&[0, 1][..]));
    assert_eq!(run(reader.seek(SeekFrom::Current(1))).ok(), Some(4));
    assert_eq!(run(reader.as_mut().fill_buf()).ok(), Some(&[1, 2][..]));
    Pin::new(&mut reader).consume(1);
    assert_eq!(run(reader.seek(SeekFrom::Current(-2))).ok(), Some(3));
}
