#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

// https://github.com/rust-lang/futures-rs/blob/1803948ff091b4eabf7f3bf39e16bbbdefca5cc8/futures/tests/io_buf_writer.rs

use futures::task::{Context, Poll};
use std::io::{self, Cursor};
use std::pin::Pin;
use tokio::io::{AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufWriter, SeekFrom};

use futures::future;
use tokio_test::assert_ok;

use std::cmp;
use std::io::IoSlice;

mod support {
    pub(crate) mod io_vec;
}
use support::io_vec::IoBufs;

struct MaybePending {
    inner: Vec<u8>,
    ready: bool,
}

impl MaybePending {
    fn new(inner: Vec<u8>) -> Self {
        Self {
            inner,
            ready: false,
        }
    }
}

impl AsyncWrite for MaybePending {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.ready {
            self.ready = false;
            Pin::new(&mut self.inner).poll_write(cx, buf)
        } else {
            self.ready = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

async fn write_vectored<W>(writer: &mut W, bufs: &[IoSlice<'_>]) -> io::Result<usize>
where
    W: AsyncWrite + Unpin,
{
    let mut writer = Pin::new(writer);
    future::poll_fn(|cx| writer.as_mut().poll_write_vectored(cx, bufs)).await
}

#[tokio::test]
async fn buf_writer() {
    let mut writer = BufWriter::with_capacity(2, Vec::new());

    assert_eq!(writer.write(&[0, 1]).await.unwrap(), 2);
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1]);

    assert_eq!(writer.write(&[2]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [2]);
    assert_eq!(*writer.get_ref(), [0, 1]);

    assert_eq!(writer.write(&[3]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [2, 3]);
    assert_eq!(*writer.get_ref(), [0, 1]);

    writer.flush().await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3]);

    assert_eq!(writer.write(&[4]).await.unwrap(), 1);
    assert_eq!(writer.write(&[5]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [4, 5]);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3]);

    assert_eq!(writer.write(&[6]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [6]);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5]);

    assert_eq!(writer.write(&[7, 8]).await.unwrap(), 2);
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8]);

    assert_eq!(writer.write(&[9, 10, 11]).await.unwrap(), 3);
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

    writer.flush().await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
}

#[tokio::test]
async fn buf_writer_inner_flushes() {
    let mut w = BufWriter::with_capacity(3, Vec::new());
    assert_eq!(w.write(&[0, 1]).await.unwrap(), 2);
    assert_eq!(*w.get_ref(), []);
    w.flush().await.unwrap();
    let w = w.into_inner();
    assert_eq!(w, [0, 1]);
}

#[tokio::test]
async fn buf_writer_seek() {
    let mut w = BufWriter::with_capacity(3, Cursor::new(Vec::new()));
    w.write_all(&[0, 1, 2, 3, 4, 5]).await.unwrap();
    w.write_all(&[6, 7]).await.unwrap();
    assert_eq!(w.seek(SeekFrom::Current(0)).await.unwrap(), 8);
    assert_eq!(&w.get_ref().get_ref()[..], &[0, 1, 2, 3, 4, 5, 6, 7][..]);
    assert_eq!(w.seek(SeekFrom::Start(2)).await.unwrap(), 2);
    w.write_all(&[8, 9]).await.unwrap();
    w.flush().await.unwrap();
    assert_eq!(&w.into_inner().into_inner()[..], &[0, 1, 8, 9, 4, 5, 6, 7]);
}

#[tokio::test]
async fn maybe_pending_buf_writer() {
    let mut writer = BufWriter::with_capacity(2, MaybePending::new(Vec::new()));

    assert_eq!(writer.write(&[0, 1]).await.unwrap(), 2);
    assert_eq!(writer.buffer(), []);
    assert_eq!(&writer.get_ref().inner, &[0, 1]);

    assert_eq!(writer.write(&[2]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [2]);
    assert_eq!(&writer.get_ref().inner, &[0, 1]);

    assert_eq!(writer.write(&[3]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [2, 3]);
    assert_eq!(&writer.get_ref().inner, &[0, 1]);

    writer.flush().await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(&writer.get_ref().inner, &[0, 1, 2, 3]);

    assert_eq!(writer.write(&[4]).await.unwrap(), 1);
    assert_eq!(writer.write(&[5]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [4, 5]);
    assert_eq!(&writer.get_ref().inner, &[0, 1, 2, 3]);

    assert_eq!(writer.write(&[6]).await.unwrap(), 1);
    assert_eq!(writer.buffer(), [6]);
    assert_eq!(writer.get_ref().inner, &[0, 1, 2, 3, 4, 5]);

    assert_eq!(writer.write(&[7, 8]).await.unwrap(), 2);
    assert_eq!(writer.buffer(), []);
    assert_eq!(writer.get_ref().inner, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);

    assert_eq!(writer.write(&[9, 10, 11]).await.unwrap(), 3);
    assert_eq!(writer.buffer(), []);
    assert_eq!(
        writer.get_ref().inner,
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );

    writer.flush().await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(
        &writer.get_ref().inner,
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
}

#[tokio::test]
async fn maybe_pending_buf_writer_inner_flushes() {
    let mut w = BufWriter::with_capacity(3, MaybePending::new(Vec::new()));
    assert_eq!(w.write(&[0, 1]).await.unwrap(), 2);
    assert_eq!(&w.get_ref().inner, &[]);
    w.flush().await.unwrap();
    let w = w.into_inner().inner;
    assert_eq!(w, [0, 1]);
}

#[tokio::test]
async fn maybe_pending_buf_writer_seek() {
    struct MaybePendingSeek {
        inner: Cursor<Vec<u8>>,
        ready_write: bool,
        ready_seek: bool,
        seek_res: Option<io::Result<()>>,
    }

    impl MaybePendingSeek {
        fn new(inner: Vec<u8>) -> Self {
            Self {
                inner: Cursor::new(inner),
                ready_write: false,
                ready_seek: false,
                seek_res: None,
            }
        }
    }

    impl AsyncWrite for MaybePendingSeek {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.ready_write {
                self.ready_write = false;
                Pin::new(&mut self.inner).poll_write(cx, buf)
            } else {
                self.ready_write = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    impl AsyncSeek for MaybePendingSeek {
        fn start_seek(mut self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
            self.seek_res = Some(Pin::new(&mut self.inner).start_seek(pos));
            Ok(())
        }
        fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
            if self.ready_seek {
                self.ready_seek = false;
                self.seek_res.take().unwrap_or(Ok(()))?;
                Pin::new(&mut self.inner).poll_complete(cx)
            } else {
                self.ready_seek = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    let mut w = BufWriter::with_capacity(3, MaybePendingSeek::new(Vec::new()));
    w.write_all(&[0, 1, 2, 3, 4, 5]).await.unwrap();
    w.write_all(&[6, 7]).await.unwrap();
    assert_eq!(w.seek(SeekFrom::Current(0)).await.unwrap(), 8);
    assert_eq!(
        &w.get_ref().inner.get_ref()[..],
        &[0, 1, 2, 3, 4, 5, 6, 7][..]
    );
    assert_eq!(w.seek(SeekFrom::Start(2)).await.unwrap(), 2);
    w.write_all(&[8, 9]).await.unwrap();
    w.flush().await.unwrap();
    assert_eq!(
        &w.into_inner().inner.into_inner()[..],
        &[0, 1, 8, 9, 4, 5, 6, 7]
    );
}

struct MockWriter {
    data: Vec<u8>,
    write_len: usize,
    vectored: bool,
}

impl MockWriter {
    fn new(write_len: usize) -> Self {
        MockWriter {
            data: Vec::new(),
            write_len,
            vectored: false,
        }
    }

    fn vectored(write_len: usize) -> Self {
        MockWriter {
            data: Vec::new(),
            write_len,
            vectored: true,
        }
    }

    fn write_up_to(&mut self, buf: &[u8], limit: usize) -> usize {
        let len = cmp::min(buf.len(), limit);
        self.data.extend_from_slice(&buf[..len]);
        len
    }
}

impl AsyncWrite for MockWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.get_mut();
        let n = this.write_up_to(buf, this.write_len);
        Ok(n).into()
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.get_mut();
        let mut total_written = 0;
        for buf in bufs {
            let n = this.write_up_to(buf, this.write_len - total_written);
            total_written += n;
            if total_written == this.write_len {
                break;
            }
        }
        Ok(total_written).into()
    }

    fn is_write_vectored(&self) -> bool {
        self.vectored
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Ok(()).into()
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Ok(()).into()
    }
}

#[tokio::test]
async fn write_vectored_empty_on_non_vectored() {
    let mut w = BufWriter::new(MockWriter::new(4));
    let n = assert_ok!(write_vectored(&mut w, &[]).await);
    assert_eq!(n, 0);

    let io_vec = [IoSlice::new(&[]); 3];
    let n = assert_ok!(write_vectored(&mut w, &io_vec).await);
    assert_eq!(n, 0);

    assert_ok!(w.flush().await);
    assert!(w.get_ref().data.is_empty());
}

#[tokio::test]
async fn write_vectored_empty_on_vectored() {
    let mut w = BufWriter::new(MockWriter::vectored(4));
    let n = assert_ok!(write_vectored(&mut w, &[]).await);
    assert_eq!(n, 0);

    let io_vec = [IoSlice::new(&[]); 3];
    let n = assert_ok!(write_vectored(&mut w, &io_vec).await);
    assert_eq!(n, 0);

    assert_ok!(w.flush().await);
    assert!(w.get_ref().data.is_empty());
}

#[tokio::test]
async fn write_vectored_basic_on_non_vectored() {
    let msg = b"foo bar baz";
    let bufs = [
        IoSlice::new(&msg[0..4]),
        IoSlice::new(&msg[4..8]),
        IoSlice::new(&msg[8..]),
    ];
    let mut w = BufWriter::new(MockWriter::new(4));
    let n = assert_ok!(write_vectored(&mut w, &bufs).await);
    assert_eq!(n, msg.len());
    assert!(w.buffer() == &msg[..]);
    assert_ok!(w.flush().await);
    assert_eq!(w.get_ref().data, msg);
}

#[tokio::test]
async fn write_vectored_basic_on_vectored() {
    let msg = b"foo bar baz";
    let bufs = [
        IoSlice::new(&msg[0..4]),
        IoSlice::new(&msg[4..8]),
        IoSlice::new(&msg[8..]),
    ];
    let mut w = BufWriter::new(MockWriter::vectored(4));
    let n = assert_ok!(write_vectored(&mut w, &bufs).await);
    assert_eq!(n, msg.len());
    assert!(w.buffer() == &msg[..]);
    assert_ok!(w.flush().await);
    assert_eq!(w.get_ref().data, msg);
}

#[tokio::test]
async fn write_vectored_large_total_on_non_vectored() {
    let msg = b"foo bar baz";
    let mut bufs = [
        IoSlice::new(&msg[0..4]),
        IoSlice::new(&msg[4..8]),
        IoSlice::new(&msg[8..]),
    ];
    let io_vec = IoBufs::new(&mut bufs);
    let mut w = BufWriter::with_capacity(8, MockWriter::new(4));
    let n = assert_ok!(write_vectored(&mut w, &io_vec).await);
    assert_eq!(n, 8);
    assert!(w.buffer() == &msg[..8]);
    let io_vec = io_vec.advance(n);
    let n = assert_ok!(write_vectored(&mut w, &io_vec).await);
    assert_eq!(n, 3);
    assert!(w.get_ref().data.as_slice() == &msg[..8]);
    assert!(w.buffer() == &msg[8..]);
}

#[tokio::test]
async fn write_vectored_large_total_on_vectored() {
    let msg = b"foo bar baz";
    let mut bufs = [
        IoSlice::new(&msg[0..4]),
        IoSlice::new(&msg[4..8]),
        IoSlice::new(&msg[8..]),
    ];
    let io_vec = IoBufs::new(&mut bufs);
    let mut w = BufWriter::with_capacity(8, MockWriter::vectored(10));
    let n = assert_ok!(write_vectored(&mut w, &io_vec).await);
    assert_eq!(n, 10);
    assert!(w.buffer().is_empty());
    let io_vec = io_vec.advance(n);
    let n = assert_ok!(write_vectored(&mut w, &io_vec).await);
    assert_eq!(n, 1);
    assert!(w.get_ref().data.as_slice() == &msg[..10]);
    assert!(w.buffer() == &msg[10..]);
}

struct VectoredWriteHarness {
    writer: BufWriter<MockWriter>,
    buf_capacity: usize,
}

impl VectoredWriteHarness {
    fn new(buf_capacity: usize) -> Self {
        VectoredWriteHarness {
            writer: BufWriter::with_capacity(buf_capacity, MockWriter::new(4)),
            buf_capacity,
        }
    }

    fn with_vectored_backend(buf_capacity: usize) -> Self {
        VectoredWriteHarness {
            writer: BufWriter::with_capacity(buf_capacity, MockWriter::vectored(4)),
            buf_capacity,
        }
    }

    async fn write_all<'a, 'b>(&mut self, mut io_vec: IoBufs<'a, 'b>) -> usize {
        let mut total_written = 0;
        while !io_vec.is_empty() {
            let n = assert_ok!(write_vectored(&mut self.writer, &io_vec).await);
            assert!(n != 0);
            assert!(self.writer.buffer().len() <= self.buf_capacity);
            total_written += n;
            io_vec = io_vec.advance(n);
        }
        total_written
    }

    async fn flush(&mut self) -> &[u8] {
        assert_ok!(self.writer.flush().await);
        &self.writer.get_ref().data
    }
}

#[tokio::test]
async fn write_vectored_odd_on_non_vectored() {
    let msg = b"foo bar baz";
    let mut bufs = [
        IoSlice::new(&msg[0..4]),
        IoSlice::new(&[]),
        IoSlice::new(&msg[4..9]),
        IoSlice::new(&msg[9..]),
    ];
    let mut h = VectoredWriteHarness::new(8);
    let bytes_written = h.write_all(IoBufs::new(&mut bufs)).await;
    assert_eq!(bytes_written, msg.len());
    assert_eq!(h.flush().await, msg);
}

#[tokio::test]
async fn write_vectored_odd_on_vectored() {
    let msg = b"foo bar baz";
    let mut bufs = [
        IoSlice::new(&msg[0..4]),
        IoSlice::new(&[]),
        IoSlice::new(&msg[4..9]),
        IoSlice::new(&msg[9..]),
    ];
    let mut h = VectoredWriteHarness::with_vectored_backend(8);
    let bytes_written = h.write_all(IoBufs::new(&mut bufs)).await;
    assert_eq!(bytes_written, msg.len());
    assert_eq!(h.flush().await, msg);
}

#[tokio::test]
async fn write_vectored_large_slice_on_non_vectored() {
    let msg = b"foo bar baz";
    let mut bufs = [
        IoSlice::new(&[]),
        IoSlice::new(&msg[..9]),
        IoSlice::new(&msg[9..]),
    ];
    let mut h = VectoredWriteHarness::new(8);
    let bytes_written = h.write_all(IoBufs::new(&mut bufs)).await;
    assert_eq!(bytes_written, msg.len());
    assert_eq!(h.flush().await, msg);
}

#[tokio::test]
async fn write_vectored_large_slice_on_vectored() {
    let msg = b"foo bar baz";
    let mut bufs = [
        IoSlice::new(&[]),
        IoSlice::new(&msg[..9]),
        IoSlice::new(&msg[9..]),
    ];
    let mut h = VectoredWriteHarness::with_vectored_backend(8);
    let bytes_written = h.write_all(IoBufs::new(&mut bufs)).await;
    assert_eq!(bytes_written, msg.len());
    assert_eq!(h.flush().await, msg);
}
