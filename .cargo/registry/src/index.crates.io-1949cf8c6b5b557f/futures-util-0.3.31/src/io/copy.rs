use super::{copy_buf, BufReader, CopyBuf};
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncRead, AsyncWrite};
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;

/// Creates a future which copies all the bytes from one object to another.
///
/// The returned future will copy all the bytes read from this `AsyncRead` into the
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
/// let bytes = io::copy(reader, &mut writer).await?;
/// writer.close().await?;
///
/// assert_eq!(bytes, 4);
/// assert_eq!(writer.into_inner(), [1, 2, 3, 4, 0]);
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
pub fn copy<R, W>(reader: R, writer: &mut W) -> Copy<'_, R, W>
where
    R: AsyncRead,
    W: AsyncWrite + Unpin + ?Sized,
{
    Copy { inner: copy_buf(BufReader::new(reader), writer) }
}

pin_project! {
    /// Future for the [`copy()`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Copy<'a, R, W: ?Sized> {
        #[pin]
        inner: CopyBuf<'a, BufReader<R>, W>,
    }
}

impl<R: AsyncRead, W: AsyncWrite + Unpin + ?Sized> Future for Copy<'_, R, W> {
    type Output = io::Result<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}
