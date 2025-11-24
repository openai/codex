use crate::io::util::poll_proceed_and_make_progress;
use crate::io::AsyncWrite;

use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

cfg_io_util! {
    /// An async writer which will move data into the void.
    ///
    /// This struct is generally created by calling [`sink`][sink]. Please
    /// see the documentation of `sink()` for more details.
    ///
    /// This is an asynchronous version of [`std::io::Sink`][std].
    ///
    /// [sink]: sink()
    /// [std]: std::io::Sink
    pub struct Sink {
        _p: (),
    }

    /// Creates an instance of an async writer which will successfully consume all
    /// data.
    ///
    /// All calls to [`poll_write`] on the returned instance will return
    /// `Poll::Ready(Ok(buf.len()))` and the contents of the buffer will not be
    /// inspected.
    ///
    /// This is an asynchronous version of [`std::io::sink`][std].
    ///
    /// [`poll_write`]: crate::io::AsyncWrite::poll_write()
    /// [std]: std::io::sink
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::{self, AsyncWriteExt};
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let buffer = vec![1, 2, 3, 5, 8];
    ///     let num_bytes = io::sink().write(&buffer).await?;
    ///     assert_eq!(num_bytes, 5);
    ///     Ok(())
    /// }
    /// ```
    pub fn sink() -> Sink {
        Sink { _p: () }
    }
}

impl AsyncWrite for Sink {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        ready!(poll_proceed_and_make_progress(cx));
        Poll::Ready(Ok(()))
    }
}

impl fmt::Debug for Sink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Sink { .. }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_unpin() {
        crate::is_unpin::<Sink>();
    }
}
