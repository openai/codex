use bytes::{Bytes, BytesMut};
use futures_core::stream::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncRead;

const DEFAULT_CAPACITY: usize = 4096;

pin_project! {
    /// Convert an [`AsyncRead`] into a [`Stream`] of byte chunks.
    ///
    /// This stream is fused. It performs the inverse operation of
    /// [`StreamReader`].
    ///
    /// # Example
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() -> std::io::Result<()> {
    /// use tokio_stream::StreamExt;
    /// use tokio_util::io::ReaderStream;
    ///
    /// // Create a stream of data.
    /// let data = b"hello, world!";
    /// let mut stream = ReaderStream::new(&data[..]);
    ///
    /// // Read all of the chunks into a vector.
    /// let mut stream_contents = Vec::new();
    /// while let Some(chunk) = stream.next().await {
    ///    stream_contents.extend_from_slice(&chunk?);
    /// }
    ///
    /// // Once the chunks are concatenated, we should have the
    /// // original data.
    /// assert_eq!(stream_contents, data);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`AsyncRead`]: tokio::io::AsyncRead
    /// [`StreamReader`]: crate::io::StreamReader
    /// [`Stream`]: futures_core::Stream
    #[derive(Debug)]
    pub struct ReaderStream<R> {
        // Reader itself.
        //
        // This value is `None` if the stream has terminated.
        #[pin]
        reader: Option<R>,
        // Working buffer, used to optimize allocations.
        buf: BytesMut,
        capacity: usize,
    }
}

impl<R: AsyncRead> ReaderStream<R> {
    /// Convert an [`AsyncRead`] into a [`Stream`] with item type
    /// `Result<Bytes, std::io::Error>`.
    ///
    /// [`AsyncRead`]: tokio::io::AsyncRead
    /// [`Stream`]: futures_core::Stream
    pub fn new(reader: R) -> Self {
        ReaderStream {
            reader: Some(reader),
            buf: BytesMut::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }

    /// Convert an [`AsyncRead`] into a [`Stream`] with item type
    /// `Result<Bytes, std::io::Error>`,
    /// with a specific read buffer initial capacity.
    ///
    /// [`AsyncRead`]: tokio::io::AsyncRead
    /// [`Stream`]: futures_core::Stream
    pub fn with_capacity(reader: R, capacity: usize) -> Self {
        ReaderStream {
            reader: Some(reader),
            buf: BytesMut::with_capacity(capacity),
            capacity,
        }
    }
}

impl<R: AsyncRead> Stream for ReaderStream<R> {
    type Item = std::io::Result<Bytes>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use crate::util::poll_read_buf;

        let mut this = self.as_mut().project();

        let reader = match this.reader.as_pin_mut() {
            Some(r) => r,
            None => return Poll::Ready(None),
        };

        if this.buf.capacity() == 0 {
            this.buf.reserve(*this.capacity);
        }

        match poll_read_buf(reader, cx, &mut this.buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(err)) => {
                self.project().reader.set(None);
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(Ok(0)) => {
                self.project().reader.set(None);
                Poll::Ready(None)
            }
            Poll::Ready(Ok(_)) => {
                let chunk = this.buf.split();
                Poll::Ready(Some(Ok(chunk.freeze())))
            }
        }
    }
}
