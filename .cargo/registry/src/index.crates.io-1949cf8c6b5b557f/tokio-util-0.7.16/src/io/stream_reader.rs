use bytes::Buf;
use futures_core::stream::Stream;
use futures_sink::Sink;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

/// Convert a [`Stream`] of byte chunks into an [`AsyncRead`].
///
/// This type performs the inverse operation of [`ReaderStream`].
///
/// This type also implements the [`AsyncBufRead`] trait, so you can use it
/// to read a `Stream` of byte chunks line-by-line. See the examples below.
///
/// # Example
///
/// ```
/// use bytes::Bytes;
/// use tokio::io::{AsyncReadExt, Result};
/// use tokio_util::io::StreamReader;
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
///
/// // Create a stream from an iterator.
/// let stream = tokio_stream::iter(vec![
///     Result::Ok(Bytes::from_static(&[0, 1, 2, 3])),
///     Result::Ok(Bytes::from_static(&[4, 5, 6, 7])),
///     Result::Ok(Bytes::from_static(&[8, 9, 10, 11])),
/// ]);
///
/// // Convert it to an AsyncRead.
/// let mut read = StreamReader::new(stream);
///
/// // Read five bytes from the stream.
/// let mut buf = [0; 5];
/// read.read_exact(&mut buf).await?;
/// assert_eq!(buf, [0, 1, 2, 3, 4]);
///
/// // Read the rest of the current chunk.
/// assert_eq!(read.read(&mut buf).await?, 3);
/// assert_eq!(&buf[..3], [5, 6, 7]);
///
/// // Read the next chunk.
/// assert_eq!(read.read(&mut buf).await?, 4);
/// assert_eq!(&buf[..4], [8, 9, 10, 11]);
///
/// // We have now reached the end.
/// assert_eq!(read.read(&mut buf).await?, 0);
///
/// # Ok(())
/// # }
/// ```
///
/// If the stream produces errors which are not [`std::io::Error`],
/// the errors can be converted using [`StreamExt`] to map each
/// element.
///
/// ```
/// use bytes::Bytes;
/// use tokio::io::AsyncReadExt;
/// use tokio_util::io::StreamReader;
/// use tokio_stream::StreamExt;
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
///
/// // Create a stream from an iterator, including an error.
/// let stream = tokio_stream::iter(vec![
///     Result::Ok(Bytes::from_static(&[0, 1, 2, 3])),
///     Result::Ok(Bytes::from_static(&[4, 5, 6, 7])),
///     Result::Err("Something bad happened!")
/// ]);
///
/// // Use StreamExt to map the stream and error to a std::io::Error
/// let stream = stream.map(|result| result.map_err(|err| {
///     std::io::Error::new(std::io::ErrorKind::Other, err)
/// }));
///
/// // Convert it to an AsyncRead.
/// let mut read = StreamReader::new(stream);
///
/// // Read five bytes from the stream.
/// let mut buf = [0; 5];
/// read.read_exact(&mut buf).await?;
/// assert_eq!(buf, [0, 1, 2, 3, 4]);
///
/// // Read the rest of the current chunk.
/// assert_eq!(read.read(&mut buf).await?, 3);
/// assert_eq!(&buf[..3], [5, 6, 7]);
///
/// // Reading the next chunk will produce an error
/// let error = read.read(&mut buf).await.unwrap_err();
/// assert_eq!(error.kind(), std::io::ErrorKind::Other);
/// assert_eq!(error.into_inner().unwrap().to_string(), "Something bad happened!");
///
/// // We have now reached the end.
/// assert_eq!(read.read(&mut buf).await?, 0);
///
/// # Ok(())
/// # }
/// ```
///
/// Using the [`AsyncBufRead`] impl, you can read a `Stream` of byte chunks
/// line-by-line. Note that you will usually also need to convert the error
/// type when doing this. See the second example for an explanation of how
/// to do this.
///
/// ```
/// use tokio::io::{Result, AsyncBufReadExt};
/// use tokio_util::io::StreamReader;
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
///
/// // Create a stream of byte chunks.
/// let stream = tokio_stream::iter(vec![
///     Result::Ok(b"The first line.\n".as_slice()),
///     Result::Ok(b"The second line.".as_slice()),
///     Result::Ok(b"\nThe third".as_slice()),
///     Result::Ok(b" line.\nThe fourth line.\nThe fifth line.\n".as_slice()),
/// ]);
///
/// // Convert it to an AsyncRead.
/// let mut read = StreamReader::new(stream);
///
/// // Loop through the lines from the `StreamReader`.
/// let mut line = String::new();
/// let mut lines = Vec::new();
/// loop {
///     line.clear();
///     let len = read.read_line(&mut line).await?;
///     if len == 0 { break; }
///     lines.push(line.clone());
/// }
///
/// // Verify that we got the lines we expected.
/// assert_eq!(
///     lines,
///     vec![
///         "The first line.\n",
///         "The second line.\n",
///         "The third line.\n",
///         "The fourth line.\n",
///         "The fifth line.\n",
///     ]
/// );
/// # Ok(())
/// # }
/// ```
///
/// [`AsyncRead`]: tokio::io::AsyncRead
/// [`AsyncBufRead`]: tokio::io::AsyncBufRead
/// [`Stream`]: futures_core::Stream
/// [`ReaderStream`]: crate::io::ReaderStream
/// [`StreamExt`]: https://docs.rs/tokio-stream/latest/tokio_stream/trait.StreamExt.html
#[derive(Debug)]
pub struct StreamReader<S, B> {
    // This field is pinned.
    inner: S,
    // This field is not pinned.
    chunk: Option<B>,
}

impl<S, B, E> StreamReader<S, B>
where
    S: Stream<Item = Result<B, E>>,
    B: Buf,
    E: Into<std::io::Error>,
{
    /// Convert a stream of byte chunks into an [`AsyncRead`].
    ///
    /// The item should be a [`Result`] with the ok variant being something that
    /// implements the [`Buf`] trait (e.g. `Cursor<Vec<u8>>` or `Bytes`). The error
    /// should be convertible into an [io error].
    ///
    /// [`Result`]: std::result::Result
    /// [`Buf`]: bytes::Buf
    /// [io error]: std::io::Error
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream,
            chunk: None,
        }
    }

    /// Do we have a chunk and is it non-empty?
    fn has_chunk(&self) -> bool {
        if let Some(ref chunk) = self.chunk {
            chunk.remaining() > 0
        } else {
            false
        }
    }

    /// Consumes this `StreamReader`, returning a Tuple consisting
    /// of the underlying stream and an Option of the internal buffer,
    /// which is Some in case the buffer contains elements.
    pub fn into_inner_with_chunk(self) -> (S, Option<B>) {
        if self.has_chunk() {
            (self.inner, self.chunk)
        } else {
            (self.inner, None)
        }
    }
}

impl<S, B> StreamReader<S, B> {
    /// Gets a reference to the underlying stream.
    ///
    /// It is inadvisable to directly read from the underlying stream.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Gets a mutable reference to the underlying stream.
    ///
    /// It is inadvisable to directly read from the underlying stream.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Gets a pinned mutable reference to the underlying stream.
    ///
    /// It is inadvisable to directly read from the underlying stream.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().inner
    }

    /// Consumes this `BufWriter`, returning the underlying stream.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    /// If you additionally want access to the internal buffer use
    /// [`into_inner_with_chunk`].
    ///
    /// [`into_inner_with_chunk`]: crate::io::StreamReader::into_inner_with_chunk
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S, B, E> AsyncRead for StreamReader<S, B>
where
    S: Stream<Item = Result<B, E>>,
    B: Buf,
    E: Into<std::io::Error>,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        let inner_buf = match self.as_mut().poll_fill_buf(cx) {
            Poll::Ready(Ok(buf)) => buf,
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => return Poll::Pending,
        };
        let len = std::cmp::min(inner_buf.len(), buf.remaining());
        buf.put_slice(&inner_buf[..len]);

        self.consume(len);
        Poll::Ready(Ok(()))
    }
}

impl<S, B, E> AsyncBufRead for StreamReader<S, B>
where
    S: Stream<Item = Result<B, E>>,
    B: Buf,
    E: Into<std::io::Error>,
{
    fn poll_fill_buf(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        loop {
            if self.as_mut().has_chunk() {
                // This unwrap is very sad, but it can't be avoided.
                let buf = self.project().chunk.as_ref().unwrap().chunk();
                return Poll::Ready(Ok(buf));
            } else {
                match self.as_mut().project().inner.poll_next(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        // Go around the loop in case the chunk is empty.
                        *self.as_mut().project().chunk = Some(chunk);
                    }
                    Poll::Ready(Some(Err(err))) => return Poll::Ready(Err(err.into())),
                    Poll::Ready(None) => return Poll::Ready(Ok(&[])),
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
    }
    fn consume(self: Pin<&mut Self>, amt: usize) {
        if amt > 0 {
            self.project()
                .chunk
                .as_mut()
                .expect("No chunk present")
                .advance(amt);
        }
    }
}

// The code below is a manual expansion of the code that pin-project-lite would
// generate. This is done because pin-project-lite fails by hitting the recursion
// limit on this struct. (Every line of documentation is handled recursively by
// the macro.)

impl<S: Unpin, B> Unpin for StreamReader<S, B> {}

struct StreamReaderProject<'a, S, B> {
    inner: Pin<&'a mut S>,
    chunk: &'a mut Option<B>,
}

impl<S, B> StreamReader<S, B> {
    #[inline]
    fn project(self: Pin<&mut Self>) -> StreamReaderProject<'_, S, B> {
        // SAFETY: We define that only `inner` should be pinned when `Self` is
        // and have an appropriate `impl Unpin` for this.
        let me = unsafe { Pin::into_inner_unchecked(self) };
        StreamReaderProject {
            inner: unsafe { Pin::new_unchecked(&mut me.inner) },
            chunk: &mut me.chunk,
        }
    }
}

impl<S: Sink<T, Error = E>, B, E, T> Sink<T> for StreamReader<S, B> {
    type Error = E;
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}
