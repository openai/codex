use super::ReadBuf;
use std::io;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Reads bytes from a source.
///
/// This trait is analogous to the [`std::io::Read`] trait, but integrates with
/// the asynchronous task system. In particular, the [`poll_read`] method,
/// unlike [`Read::read`], will automatically queue the current task for wakeup
/// and return if data is not yet available, rather than blocking the calling
/// thread.
///
/// Specifically, this means that the `poll_read` function will return one of
/// the following:
///
/// * `Poll::Ready(Ok(()))` means that data was immediately read and placed into
///   the output buffer. The amount of data read can be determined by the
///   increase in the length of the slice returned by `ReadBuf::filled`. If the
///   difference is 0, either EOF has been reached, or the output buffer had zero
///   capacity (i.e. `buf.remaining()` == 0).
///
/// * `Poll::Pending` means that no data was read into the buffer
///   provided. The I/O object is not currently readable but may become readable
///   in the future. Most importantly, **the current future's task is scheduled
///   to get unparked when the object is readable**. This means that like
///   `Future::poll` you'll receive a notification when the I/O object is
///   readable again.
///
/// * `Poll::Ready(Err(e))` for other errors are standard I/O errors coming from the
///   underlying object.
///
/// This trait importantly means that the `read` method only works in the
/// context of a future's task. The object may panic if used outside of a task.
///
/// Utilities for working with `AsyncRead` values are provided by
/// [`AsyncReadExt`].
///
/// [`poll_read`]: AsyncRead::poll_read
/// [`std::io::Read`]: std::io::Read
/// [`Read::read`]: std::io::Read::read
/// [`AsyncReadExt`]: crate::io::AsyncReadExt
pub trait AsyncRead {
    /// Attempts to read from the `AsyncRead` into `buf`.
    ///
    /// On success, returns `Poll::Ready(Ok(()))` and places data in the
    /// unfilled portion of `buf`. If no data was read (`buf.filled().len()` is
    /// unchanged), it implies that EOF has been reached.
    ///
    /// If no data is available for reading, the method returns `Poll::Pending`
    /// and arranges for the current task (via `cx.waker()`) to receive a
    /// notification when the object becomes readable or is closed.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>;
}

macro_rules! deref_async_read {
    () => {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut **self).poll_read(cx, buf)
        }
    };
}

impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for Box<T> {
    deref_async_read!();
}

impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for &mut T {
    deref_async_read!();
}

impl<P> AsyncRead for Pin<P>
where
    P: DerefMut,
    P::Target: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        crate::util::pin_as_deref_mut(self).poll_read(cx, buf)
    }
}

impl AsyncRead for &[u8] {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let amt = std::cmp::min(self.len(), buf.remaining());
        let (a, b) = self.split_at(amt);
        buf.put_slice(a);
        *self = b;
        Poll::Ready(Ok(()))
    }
}

impl<T: AsRef<[u8]> + Unpin> AsyncRead for io::Cursor<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let pos = self.position();
        let slice: &[u8] = (*self).get_ref().as_ref();

        // The position could technically be out of bounds, so don't panic...
        if pos > slice.len() as u64 {
            return Poll::Ready(Ok(()));
        }

        let start = pos as usize;
        let amt = std::cmp::min(slice.len() - start, buf.remaining());
        // Add won't overflow because of pos check above.
        let end = start + amt;
        buf.put_slice(&slice[start..end]);
        self.set_position(end as u64);

        Poll::Ready(Ok(()))
    }
}
