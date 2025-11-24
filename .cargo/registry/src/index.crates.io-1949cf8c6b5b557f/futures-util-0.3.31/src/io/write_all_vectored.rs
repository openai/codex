use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncWrite;
use futures_io::IoSlice;
use std::io;
use std::pin::Pin;

/// Future for the
/// [`write_all_vectored`](super::AsyncWriteExt::write_all_vectored) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteAllVectored<'a, W: ?Sized + Unpin> {
    writer: &'a mut W,
    bufs: &'a mut [IoSlice<'a>],
}

impl<W: ?Sized + Unpin> Unpin for WriteAllVectored<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized + Unpin> WriteAllVectored<'a, W> {
    pub(super) fn new(writer: &'a mut W, mut bufs: &'a mut [IoSlice<'a>]) -> Self {
        IoSlice::advance_slices(&mut bufs, 0);
        Self { writer, bufs }
    }
}

impl<W: AsyncWrite + ?Sized + Unpin> Future for WriteAllVectored<'_, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = &mut *self;
        while !this.bufs.is_empty() {
            let n = ready!(Pin::new(&mut this.writer).poll_write_vectored(cx, this.bufs))?;
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            } else {
                IoSlice::advance_slices(&mut this.bufs, n);
            }
        }

        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::min;
    use std::future::Future;
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::vec;
    use std::vec::Vec;

    use crate::io::{AsyncWrite, AsyncWriteExt, IoSlice};
    use crate::task::noop_waker;

    /// Create a new writer that reads from at most `n_bufs` and reads
    /// `per_call` bytes (in total) per call to write.
    fn test_writer(n_bufs: usize, per_call: usize) -> TestWriter {
        TestWriter { n_bufs, per_call, written: Vec::new() }
    }

    // TODO: maybe move this the future-test crate?
    struct TestWriter {
        n_bufs: usize,
        per_call: usize,
        written: Vec<u8>,
    }

    impl AsyncWrite for TestWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.poll_write_vectored(cx, &[IoSlice::new(buf)])
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            let mut left = self.per_call;
            let mut written = 0;
            for buf in bufs.iter().take(self.n_bufs) {
                let n = min(left, buf.len());
                self.written.extend_from_slice(&buf[0..n]);
                left -= n;
                written += n;
            }
            Poll::Ready(Ok(written))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    // TODO: maybe move this the future-test crate?
    macro_rules! assert_poll_ok {
        ($e:expr, $expected:expr) => {
            let expected = $expected;
            match $e {
                Poll::Ready(Ok(ok)) if ok == expected => {}
                got => {
                    panic!("unexpected result, got: {:?}, wanted: Ready(Ok({:?}))", got, expected)
                }
            }
        };
    }

    #[test]
    fn test_writer_read_from_one_buf() {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        let mut dst = test_writer(1, 2);
        let mut dst = Pin::new(&mut dst);

        assert_poll_ok!(dst.as_mut().poll_write(&mut cx, &[]), 0);
        assert_poll_ok!(dst.as_mut().poll_write_vectored(&mut cx, &[]), 0);

        // Read at most 2 bytes.
        assert_poll_ok!(dst.as_mut().poll_write(&mut cx, &[1, 1, 1]), 2);
        let bufs = &[IoSlice::new(&[2, 2, 2])];
        assert_poll_ok!(dst.as_mut().poll_write_vectored(&mut cx, bufs), 2);

        // Only read from first buf.
        let bufs = &[IoSlice::new(&[3]), IoSlice::new(&[4, 4])];
        assert_poll_ok!(dst.as_mut().poll_write_vectored(&mut cx, bufs), 1);

        assert_eq!(dst.written, &[1, 1, 2, 2, 3]);
    }

    #[test]
    fn test_writer_read_from_multiple_bufs() {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        let mut dst = test_writer(3, 3);
        let mut dst = Pin::new(&mut dst);

        // Read at most 3 bytes from two buffers.
        let bufs = &[IoSlice::new(&[1]), IoSlice::new(&[2, 2, 2])];
        assert_poll_ok!(dst.as_mut().poll_write_vectored(&mut cx, bufs), 3);

        // Read at most 3 bytes from three buffers.
        let bufs = &[IoSlice::new(&[3]), IoSlice::new(&[4]), IoSlice::new(&[5, 5])];
        assert_poll_ok!(dst.as_mut().poll_write_vectored(&mut cx, bufs), 3);

        assert_eq!(dst.written, &[1, 2, 2, 3, 4, 5]);
    }

    #[test]
    fn test_write_all_vectored() {
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        #[rustfmt::skip] // Becomes unreadable otherwise.
        let tests: Vec<(_, &'static [u8])> = vec![
            (vec![], &[]),
            (vec![IoSlice::new(&[]), IoSlice::new(&[])], &[]),
            (vec![IoSlice::new(&[1])], &[1]),
            (vec![IoSlice::new(&[1, 2])], &[1, 2]),
            (vec![IoSlice::new(&[1, 2, 3])], &[1, 2, 3]),
            (vec![IoSlice::new(&[1, 2, 3, 4])], &[1, 2, 3, 4]),
            (vec![IoSlice::new(&[1, 2, 3, 4, 5])], &[1, 2, 3, 4, 5]),
            (vec![IoSlice::new(&[1]), IoSlice::new(&[2])], &[1, 2]),
            (vec![IoSlice::new(&[1, 1]), IoSlice::new(&[2, 2])], &[1, 1, 2, 2]),
            (vec![IoSlice::new(&[1, 1, 1]), IoSlice::new(&[2, 2, 2])], &[1, 1, 1, 2, 2, 2]),
            (vec![IoSlice::new(&[1, 1, 1, 1]), IoSlice::new(&[2, 2, 2, 2])], &[1, 1, 1, 1, 2, 2, 2, 2]),
            (vec![IoSlice::new(&[1]), IoSlice::new(&[2]), IoSlice::new(&[3])], &[1, 2, 3]),
            (vec![IoSlice::new(&[1, 1]), IoSlice::new(&[2, 2]), IoSlice::new(&[3, 3])], &[1, 1, 2, 2, 3, 3]),
            (vec![IoSlice::new(&[1, 1, 1]), IoSlice::new(&[2, 2, 2]), IoSlice::new(&[3, 3, 3])], &[1, 1, 1, 2, 2, 2, 3, 3, 3]),
        ];

        for (mut input, wanted) in tests {
            let mut dst = test_writer(2, 2);
            {
                let mut future = dst.write_all_vectored(&mut *input);
                match Pin::new(&mut future).poll(&mut cx) {
                    Poll::Ready(Ok(())) => {}
                    other => panic!("unexpected result polling future: {:?}", other),
                }
            }
            assert_eq!(&*dst.written, &*wanted);
        }
    }
}
