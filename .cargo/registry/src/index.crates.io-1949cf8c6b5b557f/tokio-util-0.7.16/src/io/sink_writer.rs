use futures_sink::Sink;

use futures_core::stream::Stream;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};

pin_project! {
    /// Convert a [`Sink`] of byte chunks into an [`AsyncWrite`].
    ///
    /// Whenever you write to this [`SinkWriter`], the supplied bytes are
    /// forwarded to the inner [`Sink`]. When `shutdown` is called on this
    /// [`SinkWriter`], the inner sink is closed.
    ///
    /// This adapter takes a `Sink<&[u8]>` and provides an [`AsyncWrite`] impl
    /// for it. Because of the lifetime, this trait is relatively rarely
    /// implemented. The main ways to get a `Sink<&[u8]>` that you can use with
    /// this type are:
    ///
    ///  * With the codec module by implementing the [`Encoder`]`<&[u8]>` trait.
    ///  * By wrapping a `Sink<Bytes>` in a [`CopyToBytes`].
    ///  * Manually implementing `Sink<&[u8]>` directly.
    ///
    /// The opposite conversion of implementing `Sink<_>` for an [`AsyncWrite`]
    /// is done using the [`codec`] module.
    ///
    /// # Example
    ///
    /// ```
    /// use bytes::Bytes;
    /// use futures_util::SinkExt;
    /// use std::io::{Error, ErrorKind};
    /// use tokio::io::AsyncWriteExt;
    /// use tokio_util::io::{SinkWriter, CopyToBytes};
    /// use tokio_util::sync::PollSender;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Error> {
    /// // We use an mpsc channel as an example of a `Sink<Bytes>`.
    /// let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(1);
    /// let sink = PollSender::new(tx).sink_map_err(|_| Error::from(ErrorKind::BrokenPipe));
    ///
    /// // Wrap it in `CopyToBytes` to get a `Sink<&[u8]>`.
    /// let mut writer = SinkWriter::new(CopyToBytes::new(sink));
    ///
    /// // Write data to our interface...
    /// let data: [u8; 4] = [1, 2, 3, 4];
    /// let _ = writer.write(&data).await?;
    ///
    /// // ... and receive it.
    /// assert_eq!(data.as_slice(), &*rx.recv().await.unwrap());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`AsyncWrite`]: tokio::io::AsyncWrite
    /// [`CopyToBytes`]: crate::io::CopyToBytes
    /// [`Encoder`]: crate::codec::Encoder
    /// [`Sink`]: futures_sink::Sink
    /// [`codec`]: crate::codec
    #[derive(Debug)]
    pub struct SinkWriter<S> {
        #[pin]
        inner: S,
    }
}

impl<S> SinkWriter<S> {
    /// Creates a new [`SinkWriter`].
    pub fn new(sink: S) -> Self {
        Self { inner: sink }
    }

    /// Gets a reference to the underlying sink.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Gets a mutable reference to the underlying sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Consumes this [`SinkWriter`], returning the underlying sink.
    pub fn into_inner(self) -> S {
        self.inner
    }
}
impl<S, E> AsyncWrite for SinkWriter<S>
where
    for<'a> S: Sink<&'a [u8], Error = E>,
    E: Into<io::Error>,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut this = self.project();

        ready!(this.inner.as_mut().poll_ready(cx).map_err(Into::into))?;
        match this.inner.as_mut().start_send(buf) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(e) => Poll::Ready(Err(e.into())),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().inner.poll_flush(cx).map_err(Into::into)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().inner.poll_close(cx).map_err(Into::into)
    }
}

impl<S: Stream> Stream for SinkWriter<S> {
    type Item = S::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl<S: AsyncRead> AsyncRead for SinkWriter<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.project().inner.poll_read(cx, buf)
    }
}
