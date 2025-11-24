use core::fmt;
use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

use crate::fns::FnMut1;

pin_project! {
    /// Stream for the [`map`](super::StreamExt::map) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Map<St, F> {
        #[pin]
        stream: St,
        f: F,
    }
}

impl<St, F> fmt::Debug for Map<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Map").field("stream", &self.stream).finish()
    }
}

impl<St, F> Map<St, F> {
    pub(crate) fn new(stream: St, f: F) -> Self {
        Self { stream, f }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, F> FusedStream for Map<St, F>
where
    St: FusedStream,
    F: FnMut1<St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F> Stream for Map<St, F>
where
    St: Stream,
    F: FnMut1<St::Item>,
{
    type Item = F::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let res = ready!(this.stream.as_mut().poll_next(cx));
        Poll::Ready(res.map(|x| this.f.call_mut(x)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, F, Item> Sink<Item> for Map<St, F>
where
    St: Stream + Sink<Item>,
    F: FnMut1<St::Item>,
{
    type Error = St::Error;

    delegate_sink!(stream, Item);
}
