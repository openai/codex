use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::AsyncBufRead;
use std::io;
use std::pin::Pin;
use std::slice;

/// Future for the [`fill_buf`](super::AsyncBufReadExt::fill_buf) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct FillBuf<'a, R: ?Sized> {
    reader: Option<&'a mut R>,
}

impl<R: ?Sized> Unpin for FillBuf<'_, R> {}

impl<'a, R: AsyncBufRead + ?Sized + Unpin> FillBuf<'a, R> {
    pub(super) fn new(reader: &'a mut R) -> Self {
        Self { reader: Some(reader) }
    }
}

impl<'a, R> Future for FillBuf<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    type Output = io::Result<&'a [u8]>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        let reader = this.reader.take().expect("Polled FillBuf after completion");

        match Pin::new(&mut *reader).poll_fill_buf(cx) {
            Poll::Ready(Ok(slice)) => {
                // With polonius it is possible to remove this lifetime transmutation and just have
                // the correct lifetime of the reference inferred based on which branch is taken
                let slice: &'a [u8] = unsafe { slice::from_raw_parts(slice.as_ptr(), slice.len()) };
                Poll::Ready(Ok(slice))
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => {
                this.reader = Some(reader);
                Poll::Pending
            }
        }
    }
}
