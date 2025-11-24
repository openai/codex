use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncWrite};
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;

/// Creates a future which copies all the bytes from one object to another.
///
/// The returned future will copy all the bytes read from this `AsyncBufRead` into the
/// `writer` specified. This future will only complete once the `reader` has hit
/// EOF and all bytes have been written to and flushed from the `writer`
/// provided.
///
/// On success the number of bytes is returned.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::io::{self, AsyncWriteExt, Cursor};
///
/// let reader = Cursor::new([1, 2, 3, 4]);
/// let mut writer = Cursor::new(vec![0u8; 5]);
///
/// let bytes = io::copy_buf(reader, &mut writer).await?;
/// writer.close().await?;
///
/// assert_eq!(bytes, 4);
/// assert_eq!(writer.into_inner(), [1, 2, 3, 4, 0]);
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
pub fn copy_buf<R, W>(reader: R, writer: &mut W) -> CopyBuf<'_, R, W>
where
    R: AsyncBufRead,
    W: AsyncWrite + Unpin + ?Sized,
{
    CopyBuf { reader, writer, amt: 0 }
}

pin_project! {
    /// Future for the [`copy_buf()`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CopyBuf<'a, R, W: ?Sized> {
        #[pin]
        reader: R,
        writer: &'a mut W,
        amt: u64,
    }
}

impl<R, W> Future for CopyBuf<'_, R, W>
where
    R: AsyncBufRead,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            let buffer = ready!(this.reader.as_mut().poll_fill_buf(cx))?;
            if buffer.is_empty() {
                ready!(Pin::new(&mut this.writer).poll_flush(cx))?;
                return Poll::Ready(Ok(*this.amt));
            }

            let i = ready!(Pin::new(&mut this.writer).poll_write(cx, buffer))?;
            if i == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            *this.amt += i as u64;
            this.reader.as_mut().consume(i);
        }
    }
}
