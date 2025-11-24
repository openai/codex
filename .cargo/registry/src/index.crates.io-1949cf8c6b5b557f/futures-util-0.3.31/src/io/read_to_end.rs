use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncRead;
use std::io;
use std::pin::Pin;
use std::vec::Vec;

/// Future for the [`read_to_end`](super::AsyncReadExt::read_to_end) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToEnd<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
    start_len: usize,
}

impl<R: ?Sized + Unpin> Unpin for ReadToEnd<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadToEnd<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut Vec<u8>) -> Self {
        let start_len = buf.len();
        Self { reader, buf, start_len }
    }
}

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than this if the reader has a very small
// amount of data to return.
//
// Because we're extending the buffer with uninitialized data for trusted
// readers, we need to make sure to truncate that if any of this panics.
pub(super) fn read_to_end_internal<R: AsyncRead + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
    start_len: usize,
) -> Poll<io::Result<usize>> {
    let mut g = Guard { len: buf.len(), buf };
    loop {
        if g.len == g.buf.len() {
            unsafe {
                g.buf.reserve(32);
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);
                super::initialize(&rd, &mut g.buf[g.len..]);
            }
        }

        let buf = &mut g.buf[g.len..];
        match ready!(rd.as_mut().poll_read(cx, buf)) {
            Ok(0) => return Poll::Ready(Ok(g.len - start_len)),
            Ok(n) => {
                // We can't allow bogus values from read. If it is too large, the returned vec could have its length
                // set past its capacity, or if it overflows the vec could be shortened which could create an invalid
                // string if this is called via read_to_string.
                assert!(n <= buf.len());
                g.len += n;
            }
            Err(e) => return Poll::Ready(Err(e)),
        }
    }
}

impl<A> Future for ReadToEnd<'_, A>
where
    A: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        read_to_end_internal(Pin::new(&mut this.reader), cx, this.buf, this.start_len)
    }
}
