use crate::io::AsyncRead;

use std::io;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Reads bytes asynchronously.
///
/// This trait is analogous to [`std::io::BufRead`], but integrates with
/// the asynchronous task system. In particular, the [`poll_fill_buf`] method,
/// unlike [`BufRead::fill_buf`], will automatically queue the current task for wakeup
/// and return if data is not yet available, rather than blocking the calling
/// thread.
///
/// Utilities for working with `AsyncBufRead` values are provided by
/// [`AsyncBufReadExt`].
///
/// [`std::io::BufRead`]: std::io::BufRead
/// [`poll_fill_buf`]: AsyncBufRead::poll_fill_buf
/// [`BufRead::fill_buf`]: std::io::BufRead::fill_buf
/// [`AsyncBufReadExt`]: crate::io::AsyncBufReadExt
pub trait AsyncBufRead: AsyncRead {
    /// Attempts to return the contents of the internal buffer, filling it with more data
    /// from the inner reader if it is empty.
    ///
    /// On success, returns `Poll::Ready(Ok(buf))`.
    ///
    /// If no data is available for reading, the method returns
    /// `Poll::Pending` and arranges for the current task (via
    /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
    /// readable or is closed.
    ///
    /// This function is a lower-level call. It needs to be paired with the
    /// [`consume`] method to function properly. When calling this
    /// method, none of the contents will be "read" in the sense that later
    /// calling [`poll_read`] may return the same contents. As such, [`consume`] must
    /// be called with the number of bytes that are consumed from this buffer to
    /// ensure that the bytes are never returned twice.
    ///
    /// An empty buffer returned indicates that the stream has reached EOF.
    ///
    /// [`poll_read`]: AsyncRead::poll_read
    /// [`consume`]: AsyncBufRead::consume
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>>;

    /// Tells this buffer that `amt` bytes have been consumed from the buffer,
    /// so they should no longer be returned in calls to [`poll_read`].
    ///
    /// This function is a lower-level call. It needs to be paired with the
    /// [`poll_fill_buf`] method to function properly. This function does
    /// not perform any I/O, it simply informs this object that some amount of
    /// its buffer, returned from [`poll_fill_buf`], has been consumed and should
    /// no longer be returned. As such, this function may do odd things if
    /// [`poll_fill_buf`] isn't called before calling it.
    ///
    /// The `amt` must be `<=` the number of bytes in the buffer returned by
    /// [`poll_fill_buf`].
    ///
    /// [`poll_read`]: AsyncRead::poll_read
    /// [`poll_fill_buf`]: AsyncBufRead::poll_fill_buf
    fn consume(self: Pin<&mut Self>, amt: usize);
}

macro_rules! deref_async_buf_read {
    () => {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
            Pin::new(&mut **self.get_mut()).poll_fill_buf(cx)
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            Pin::new(&mut **self).consume(amt)
        }
    };
}

impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for Box<T> {
    deref_async_buf_read!();
}

impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for &mut T {
    deref_async_buf_read!();
}

impl<P> AsyncBufRead for Pin<P>
where
    P: DerefMut,
    P::Target: AsyncBufRead,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        crate::util::pin_as_deref_mut(self).poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        crate::util::pin_as_deref_mut(self).consume(amt);
    }
}

impl AsyncBufRead for &[u8] {
    fn poll_fill_buf(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Poll::Ready(Ok(*self))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        *self = &self[amt..];
    }
}

impl<T: AsRef<[u8]> + Unpin> AsyncBufRead for io::Cursor<T> {
    fn poll_fill_buf(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Poll::Ready(io::BufRead::fill_buf(self.get_mut()))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        io::BufRead::consume(self.get_mut(), amt);
    }
}
