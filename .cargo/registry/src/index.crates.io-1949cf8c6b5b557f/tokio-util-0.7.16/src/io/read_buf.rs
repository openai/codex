use bytes::BufMut;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncRead;

/// Read data from an `AsyncRead` into an implementer of the [`BufMut`] trait.
///
/// [`BufMut`]: bytes::BufMut
///
/// # Example
///
/// ```
/// use bytes::{Bytes, BytesMut};
/// use tokio_stream as stream;
/// use tokio::io::Result;
/// use tokio_util::io::{StreamReader, read_buf};
/// # #[tokio::main]
/// # async fn main() -> std::io::Result<()> {
///
/// // Create a reader from an iterator. This particular reader will always be
/// // ready.
/// let mut read = StreamReader::new(stream::iter(vec![Result::Ok(Bytes::from_static(&[0, 1, 2, 3]))]));
///
/// let mut buf = BytesMut::new();
/// let mut reads = 0;
///
/// loop {
///     reads += 1;
///     let n = read_buf(&mut read, &mut buf).await?;
///
///     if n == 0 {
///         break;
///     }
/// }
///
/// // one or more reads might be necessary.
/// assert!(reads >= 1);
/// assert_eq!(&buf[..], &[0, 1, 2, 3]);
/// # Ok(())
/// # }
/// ```
pub async fn read_buf<R, B>(read: &mut R, buf: &mut B) -> io::Result<usize>
where
    R: AsyncRead + Unpin,
    B: BufMut,
{
    return ReadBufFn(read, buf).await;

    struct ReadBufFn<'a, R, B>(&'a mut R, &'a mut B);

    impl<'a, R, B> Future for ReadBufFn<'a, R, B>
    where
        R: AsyncRead + Unpin,
        B: BufMut,
    {
        type Output = io::Result<usize>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = &mut *self;
            crate::util::poll_read_buf(Pin::new(this.0), cx, this.1)
        }
    }
}
