use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`into_stream`](super::TryStreamExt::into_stream) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct IntoStream<St> {
        #[pin]
        stream: St,
    }
}

impl<St> IntoStream<St> {
    #[inline]
    pub(super) fn new(stream: St) -> Self {
        Self { stream }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St: TryStream + FusedStream> FusedStream for IntoStream<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: TryStream> Stream for IntoStream<St> {
    type Item = Result<St::Ok, St::Error>;

    #[inline]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.try_poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S: Sink<Item>, Item> Sink<Item> for IntoStream<S> {
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
