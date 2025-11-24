use crate::abortable::{AbortHandle, AbortInner, Aborted};
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncWrite};
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Creates a future which copies all the bytes from one object to another, with its `AbortHandle`.
///
/// The returned future will copy all the bytes read from this `AsyncBufRead` into the
/// `writer` specified. This future will only complete once abort has been requested or the `reader` has hit
/// EOF and all bytes have been written to and flushed from the `writer`
/// provided.
///
/// On success the number of bytes is returned. If aborted, `Aborted` is returned. Otherwise, the underlying error is returned.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::io::{self, AsyncWriteExt, Cursor};
/// use futures::future::Aborted;
///
/// let reader = Cursor::new([1, 2, 3, 4]);
/// let mut writer = Cursor::new(vec![0u8; 5]);
///
/// let (fut, abort_handle) = io::copy_buf_abortable(reader, &mut writer);
/// let bytes = fut.await;
/// abort_handle.abort();
/// writer.close().await.unwrap();
/// match bytes {
///     Ok(Ok(n)) => {
///         assert_eq!(n, 4);
///         assert_eq!(writer.into_inner(), [1, 2, 3, 4, 0]);
///         Ok(n)
///     },
///     Ok(Err(a)) => {
///         Err::<u64, Aborted>(a)
///     }
///     Err(e) => panic!("{}", e)
/// }
/// #  }).unwrap();
/// ```
pub fn copy_buf_abortable<R, W>(
    reader: R,
    writer: &mut W,
) -> (CopyBufAbortable<'_, R, W>, AbortHandle)
where
    R: AsyncBufRead,
    W: AsyncWrite + Unpin + ?Sized,
{
    let (handle, reg) = AbortHandle::new_pair();
    (CopyBufAbortable { reader, writer, amt: 0, inner: reg.inner }, handle)
}

pin_project! {
    /// Future for the [`copy_buf_abortable()`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CopyBufAbortable<'a, R, W: ?Sized> {
        #[pin]
        reader: R,
        writer: &'a mut W,
        amt: u64,
        inner: Arc<AbortInner>
    }
}

macro_rules! ready_or_break {
    ($e:expr $(,)?) => {
        match $e {
            $crate::task::Poll::Ready(t) => t,
            $crate::task::Poll::Pending => break,
        }
    };
}

impl<R, W> Future for CopyBufAbortable<'_, R, W>
where
    R: AsyncBufRead,
    W: AsyncWrite + Unpin + Sized,
{
    type Output = Result<Result<u64, Aborted>, io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            // Check if the task has been aborted
            if this.inner.aborted.load(Ordering::Relaxed) {
                return Poll::Ready(Ok(Err(Aborted)));
            }

            // Read some bytes from the reader, and if we have reached EOF, return total bytes read
            let buffer = ready_or_break!(this.reader.as_mut().poll_fill_buf(cx))?;
            if buffer.is_empty() {
                ready_or_break!(Pin::new(&mut this.writer).poll_flush(cx))?;
                return Poll::Ready(Ok(Ok(*this.amt)));
            }

            // Pass the buffer to the writer, and update the amount written
            let i = ready_or_break!(Pin::new(&mut this.writer).poll_write(cx, buffer))?;
            if i == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            *this.amt += i as u64;
            this.reader.as_mut().consume(i);
        }
        // Schedule the task to be woken up again.
        // Never called unless Poll::Pending is returned from io objects.
        this.inner.waker.register(cx.waker());

        // Check to see if the task was aborted between the first check and
        // registration.
        // Checking with  `Relaxed` is sufficient because
        // `register` introduces an `AcqRel` barrier.
        if this.inner.aborted.load(Ordering::Relaxed) {
            return Poll::Ready(Ok(Err(Aborted)));
        }
        Poll::Pending
    }
}
