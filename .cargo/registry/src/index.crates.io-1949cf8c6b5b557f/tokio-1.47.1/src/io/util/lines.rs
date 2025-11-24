use crate::io::util::read_line::read_line_internal;
use crate::io::AsyncBufRead;

use pin_project_lite::pin_project;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    /// Reads lines from an [`AsyncBufRead`].
    ///
    /// A `Lines` can be turned into a `Stream` with [`LinesStream`].
    ///
    /// This type is usually created using the [`lines`] method.
    ///
    /// [`AsyncBufRead`]: crate::io::AsyncBufRead
    /// [`LinesStream`]: https://docs.rs/tokio-stream/0.1/tokio_stream/wrappers/struct.LinesStream.html
    /// [`lines`]: crate::io::AsyncBufReadExt::lines
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
    pub struct Lines<R> {
        #[pin]
        reader: R,
        buf: String,
        bytes: Vec<u8>,
        read: usize,
    }
}

pub(crate) fn lines<R>(reader: R) -> Lines<R>
where
    R: AsyncBufRead,
{
    Lines {
        reader,
        buf: String::new(),
        bytes: Vec::new(),
        read: 0,
    }
}

impl<R> Lines<R>
where
    R: AsyncBufRead + Unpin,
{
    /// Returns the next line in the stream.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::AsyncBufRead;
    /// use tokio::io::AsyncBufReadExt;
    ///
    /// # async fn dox(my_buf_read: impl AsyncBufRead + Unpin) -> std::io::Result<()> {
    /// let mut lines = my_buf_read.lines();
    ///
    /// while let Some(line) = lines.next_line().await? {
    ///     println!("length = {}", line.len())
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next_line(&mut self) -> io::Result<Option<String>> {
        use std::future::poll_fn;

        poll_fn(|cx| Pin::new(&mut *self).poll_next_line(cx)).await
    }

    /// Obtains a mutable reference to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Obtains a reference to the underlying reader.
    pub fn get_ref(&mut self) -> &R {
        &self.reader
    }

    /// Unwraps this `Lines<R>`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    /// Therefore, a following read from the underlying reader may lead to data loss.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R> Lines<R>
where
    R: AsyncBufRead,
{
    /// Polls for the next line in the stream.
    ///
    /// This method returns:
    ///
    ///  * `Poll::Pending` if the next line is not yet available.
    ///  * `Poll::Ready(Ok(Some(line)))` if the next line is available.
    ///  * `Poll::Ready(Ok(None))` if there are no more lines in this stream.
    ///  * `Poll::Ready(Err(err))` if an IO error occurred while reading the next line.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided
    /// `Context` is scheduled to receive a wakeup when more bytes become
    /// available on the underlying IO resource.  Note that on multiple calls to
    /// `poll_next_line`, only the `Waker` from the `Context` passed to the most
    /// recent call is scheduled to receive a wakeup.
    pub fn poll_next_line(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Option<String>>> {
        let me = self.project();

        let n = ready!(read_line_internal(me.reader, cx, me.buf, me.bytes, me.read))?;
        debug_assert_eq!(*me.read, 0);

        if n == 0 && me.buf.is_empty() {
            return Poll::Ready(Ok(None));
        }

        if me.buf.ends_with('\n') {
            me.buf.pop();

            if me.buf.ends_with('\r') {
                me.buf.pop();
            }
        }

        Poll::Ready(Ok(Some(mem::take(me.buf))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_unpin() {
        crate::is_unpin::<Lines<()>>();
    }
}
