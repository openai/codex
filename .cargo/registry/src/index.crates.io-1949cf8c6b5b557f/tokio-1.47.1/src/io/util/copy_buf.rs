use crate::io::{AsyncBufRead, AsyncWrite};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

cfg_io_util! {
    /// A future that asynchronously copies the entire contents of a reader into a
    /// writer.
    ///
    /// This struct is generally created by calling [`copy_buf`][copy_buf]. Please
    /// see the documentation of `copy_buf()` for more details.
    ///
    /// [copy_buf]: copy_buf()
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    struct CopyBuf<'a, R: ?Sized, W: ?Sized> {
        reader: &'a mut R,
        writer: &'a mut W,
        amt: u64,
    }

    /// Asynchronously copies the entire contents of a reader into a writer.
    ///
    /// This function returns a future that will continuously read data from
    /// `reader` and then write it into `writer` in a streaming fashion until
    /// `reader` returns EOF or fails.
    ///
    /// On success, the total number of bytes that were copied from `reader` to
    /// `writer` is returned.
    ///
    /// This is a [`tokio::io::copy`] alternative for [`AsyncBufRead`] readers
    /// with no extra buffer allocation, since [`AsyncBufRead`] allow access
    /// to the reader's inner buffer.
    ///
    /// [`tokio::io::copy`]: crate::io::copy
    /// [`AsyncBufRead`]: crate::io::AsyncBufRead
    ///
    /// # Errors
    ///
    /// The returned future will finish with an error will return an error
    /// immediately if any call to `poll_fill_buf` or `poll_write` returns an
    /// error.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut reader: &[u8] = b"hello";
    /// let mut writer: Vec<u8> = vec![];
    ///
    /// io::copy_buf(&mut reader, &mut writer).await?;
    ///
    /// assert_eq!(b"hello", &writer[..]);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy_buf<'a, R, W>(reader: &'a mut R, writer: &'a mut W) -> io::Result<u64>
    where
        R: AsyncBufRead + Unpin + ?Sized,
        W: AsyncWrite + Unpin + ?Sized,
    {
        CopyBuf {
            reader,
            writer,
            amt: 0,
        }.await
    }
}

impl<R, W> Future for CopyBuf<'_, R, W>
where
    R: AsyncBufRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let me = &mut *self;
            let buffer = ready!(Pin::new(&mut *me.reader).poll_fill_buf(cx))?;
            if buffer.is_empty() {
                ready!(Pin::new(&mut self.writer).poll_flush(cx))?;
                return Poll::Ready(Ok(self.amt));
            }

            let i = ready!(Pin::new(&mut *me.writer).poll_write(cx, buffer))?;
            if i == 0 {
                return Poll::Ready(Err(std::io::ErrorKind::WriteZero.into()));
            }
            self.amt += i as u64;
            Pin::new(&mut *self.reader).consume(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_unpin() {
        use std::marker::PhantomPinned;
        crate::is_unpin::<CopyBuf<'_, PhantomPinned, PhantomPinned>>();
    }
}
