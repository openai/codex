use std::io::{self, SeekFrom};
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Seek bytes asynchronously.
///
/// This trait is analogous to the [`std::io::Seek`] trait, but integrates
/// with the asynchronous task system. In particular, the `start_seek`
/// method, unlike [`Seek::seek`], will not block the calling thread.
///
/// Utilities for working with `AsyncSeek` values are provided by
/// [`AsyncSeekExt`].
///
/// [`std::io::Seek`]: std::io::Seek
/// [`Seek::seek`]: std::io::Seek::seek()
/// [`AsyncSeekExt`]: crate::io::AsyncSeekExt
pub trait AsyncSeek {
    /// Attempts to seek to an offset, in bytes, in a stream.
    ///
    /// A seek beyond the end of a stream is allowed, but behavior is defined
    /// by the implementation.
    ///
    /// If this function returns successfully, then the job has been submitted.
    /// To find out when it completes, call `poll_complete`.
    ///
    /// # Errors
    ///
    /// This function can return [`io::ErrorKind::Other`] in case there is
    /// another seek in progress. To avoid this, it is advisable that any call
    /// to `start_seek` is preceded by a call to `poll_complete` to ensure all
    /// pending seeks have completed.
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()>;

    /// Waits for a seek operation to complete.
    ///
    /// If the seek operation completed successfully, this method returns the
    /// new position from the start of the stream. That position can be used
    /// later with [`SeekFrom::Start`].
    ///
    /// The position returned by calling this method can only be relied on right
    /// after `start_seek`. If you have changed the position by e.g. reading or
    /// writing since calling `start_seek`, then it is unspecified whether the
    /// returned position takes that position change into account. Similarly, if
    /// `start_seek` has never been called, then it is unspecified whether
    /// `poll_complete` returns the actual position or some other placeholder
    /// value (such as 0).
    ///
    /// # Errors
    ///
    /// Seeking to a negative offset is considered an error.
    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>>;
}

macro_rules! deref_async_seek {
    () => {
        fn start_seek(mut self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
            Pin::new(&mut **self).start_seek(pos)
        }

        fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
            Pin::new(&mut **self).poll_complete(cx)
        }
    };
}

impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for Box<T> {
    deref_async_seek!();
}

impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for &mut T {
    deref_async_seek!();
}

impl<P> AsyncSeek for Pin<P>
where
    P: DerefMut,
    P::Target: AsyncSeek,
{
    fn start_seek(self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
        crate::util::pin_as_deref_mut(self).start_seek(pos)
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        crate::util::pin_as_deref_mut(self).poll_complete(cx)
    }
}

impl<T: AsRef<[u8]> + Unpin> AsyncSeek for io::Cursor<T> {
    fn start_seek(mut self: Pin<&mut Self>, pos: SeekFrom) -> io::Result<()> {
        io::Seek::seek(&mut *self, pos).map(drop)
    }
    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Poll::Ready(Ok(self.get_mut().position()))
    }
}
