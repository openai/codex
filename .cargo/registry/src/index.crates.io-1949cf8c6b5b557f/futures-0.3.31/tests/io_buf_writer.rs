use futures::executor::block_on;
use futures::future::{Future, FutureExt};
use futures::io::{
    AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufWriter, Cursor, SeekFrom,
};
use futures::task::{Context, Poll};
use futures_test::task::noop_context;
use std::io;
use std::pin::Pin;

struct MaybePending {
    inner: Vec<u8>,
    ready: bool,
}

impl MaybePending {
    fn new(inner: Vec<u8>) -> Self {
        Self { inner, ready: false }
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
            Poll::Pending
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}

fn run<F: Future + Unpin>(mut f: F) -> F::Output {
    let mut cx = noop_context();
    loop {
        if let Poll::Ready(x) = f.poll_unpin(&mut cx) {
            return x;
        }
    }
}

#[test]
fn buf_writer() {
    let mut writer = BufWriter::with_capacity(2, Vec::new());

    block_on(writer.write(&[0, 1])).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1]);

    block_on(writer.write(&[2])).unwrap();
    assert_eq!(writer.buffer(), [2]);
    assert_eq!(*writer.get_ref(), [0, 1]);

    block_on(writer.write(&[3])).unwrap();
    assert_eq!(writer.buffer(), [2, 3]);
    assert_eq!(*writer.get_ref(), [0, 1]);

    block_on(writer.flush()).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3]);

    block_on(writer.write(&[4])).unwrap();
    block_on(writer.write(&[5])).unwrap();
    assert_eq!(writer.buffer(), [4, 5]);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3]);

    block_on(writer.write(&[6])).unwrap();
    assert_eq!(writer.buffer(), [6]);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5]);

    block_on(writer.write(&[7, 8])).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8]);

    block_on(writer.write(&[9, 10, 11])).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

    block_on(writer.flush()).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
}

#[test]
fn buf_writer_inner_flushes() {
    let mut w = BufWriter::with_capacity(3, Vec::new());
    block_on(w.write(&[0, 1])).unwrap();
    assert_eq!(*w.get_ref(), []);
    block_on(w.flush()).unwrap();
    let w = w.into_inner();
    assert_eq!(w, [0, 1]);
}

#[test]
fn buf_writer_seek() {
    // FIXME: when https://github.com/rust-lang/futures-rs/issues/1510 fixed,
    // use `Vec::new` instead of `vec![0; 8]`.
    let mut w = BufWriter::with_capacity(3, Cursor::new(vec![0; 8]));
    block_on(w.write_all(&[0, 1, 2, 3, 4, 5])).unwrap();
    block_on(w.write_all(&[6, 7])).unwrap();
    assert_eq!(block_on(w.seek(SeekFrom::Current(0))).ok(), Some(8));
    assert_eq!(&w.get_ref().get_ref()[..], &[0, 1, 2, 3, 4, 5, 6, 7][..]);
    assert_eq!(block_on(w.seek(SeekFrom::Start(2))).ok(), Some(2));
    block_on(w.write_all(&[8, 9])).unwrap();
    block_on(w.flush()).unwrap();
    assert_eq!(&w.into_inner().into_inner()[..], &[0, 1, 8, 9, 4, 5, 6, 7]);
}

#[test]
fn maybe_pending_buf_writer() {
    let mut writer = BufWriter::with_capacity(2, MaybePending::new(Vec::new()));

    run(writer.write(&[0, 1])).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(&writer.get_ref().inner, &[0, 1]);

    run(writer.write(&[2])).unwrap();
    assert_eq!(writer.buffer(), [2]);
    assert_eq!(&writer.get_ref().inner, &[0, 1]);

    run(writer.write(&[3])).unwrap();
    assert_eq!(writer.buffer(), [2, 3]);
    assert_eq!(&writer.get_ref().inner, &[0, 1]);

    run(writer.flush()).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(&writer.get_ref().inner, &[0, 1, 2, 3]);

    run(writer.write(&[4])).unwrap();
    run(writer.write(&[5])).unwrap();
    assert_eq!(writer.buffer(), [4, 5]);
    assert_eq!(&writer.get_ref().inner, &[0, 1, 2, 3]);

    run(writer.write(&[6])).unwrap();
    assert_eq!(writer.buffer(), [6]);
    assert_eq!(writer.get_ref().inner, &[0, 1, 2, 3, 4, 5]);

    run(writer.write(&[7, 8])).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(writer.get_ref().inner, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);

    run(writer.write(&[9, 10, 11])).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(writer.get_ref().inner, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

    run(writer.flush()).unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(&writer.get_ref().inner, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
}

#[test]
fn maybe_pending_buf_writer_inner_flushes() {
    let mut w = BufWriter::with_capacity(3, MaybePending::new(Vec::new()));
    run(w.write(&[0, 1])).unwrap();
    assert_eq!(&w.get_ref().inner, &[]);
    run(w.flush()).unwrap();
    let w = w.into_inner().inner;
    assert_eq!(w, [0, 1]);
}

#[test]
fn maybe_pending_buf_writer_seek() {
    struct MaybePendingSeek {
        inner: Cursor<Vec<u8>>,
        ready_write: bool,
        ready_seek: bool,
    }

    impl MaybePendingSeek {
        fn new(inner: Vec<u8>) -> Self {
            Self { inner: Cursor::new(inner), ready_write: false, ready_seek: false }
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
                Poll::Pending
            }
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_close(cx)
        }
    }

    impl AsyncSeek for MaybePendingSeek {
        fn poll_seek(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<io::Result<u64>> {
            if self.ready_seek {
                self.ready_seek = false;
                Pin::new(&mut self.inner).poll_seek(cx, pos)
            } else {
                self.ready_seek = true;
                Poll::Pending
            }
        }
    }

    // FIXME: when https://github.com/rust-lang/futures-rs/issues/1510 fixed,
    // use `Vec::new` instead of `vec![0; 8]`.
    let mut w = BufWriter::with_capacity(3, MaybePendingSeek::new(vec![0; 8]));
    run(w.write_all(&[0, 1, 2, 3, 4, 5])).unwrap();
    run(w.write_all(&[6, 7])).unwrap();
    assert_eq!(run(w.seek(SeekFrom::Current(0))).ok(), Some(8));
    assert_eq!(&w.get_ref().inner.get_ref()[..], &[0, 1, 2, 3, 4, 5, 6, 7][..]);
    assert_eq!(run(w.seek(SeekFrom::Start(2))).ok(), Some(2));
    run(w.write_all(&[8, 9])).unwrap();
    run(w.flush()).unwrap();
    assert_eq!(&w.into_inner().inner.into_inner()[..], &[0, 1, 8, 9, 4, 5, 6, 7]);
}
