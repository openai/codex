use bytes::Bytes;
use futures_core::stream::Stream;
use futures_sink::Sink;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A helper that wraps a [`Sink`]`<`[`Bytes`]`>` and converts it into a
    /// [`Sink`]`<&'a [u8]>` by copying each byte slice into an owned [`Bytes`].
    ///
    /// See the documentation for [`SinkWriter`] for an example.
    ///
    /// [`Bytes`]: bytes::Bytes
    /// [`SinkWriter`]: crate::io::SinkWriter
    /// [`Sink`]: futures_sink::Sink
    #[derive(Debug)]
    pub struct CopyToBytes<S> {
        #[pin]
        inner: S,
    }
}

impl<S> CopyToBytes<S> {
    /// Creates a new [`CopyToBytes`].
    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    /// Gets a reference to the underlying sink.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Gets a mutable reference to the underlying sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Consumes this [`CopyToBytes`], returning the underlying sink.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<'a, S> Sink<&'a [u8]> for CopyToBytes<S>
where
    S: Sink<Bytes>,
{
    type Error = S::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: &'a [u8]) -> Result<(), Self::Error> {
        self.project()
            .inner
            .start_send(Bytes::copy_from_slice(item))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

impl<S: Stream> Stream for CopyToBytes<S> {
    type Item = S::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}
