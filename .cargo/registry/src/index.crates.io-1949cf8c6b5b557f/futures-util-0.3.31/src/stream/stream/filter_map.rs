use crate::fns::FnMut1;
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`filter_map`](super::StreamExt::filter_map) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct FilterMap<St, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending: Option<Fut>,
    }
}

impl<St, Fut, F> fmt::Debug for FilterMap<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilterMap")
            .field("stream", &self.stream)
            .field("pending", &self.pending)
            .finish()
    }
}

impl<St, Fut, F> FilterMap<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, pending: None }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F, T> FusedStream for FilterMap<St, Fut, F>
where
    St: Stream + FusedStream,
    F: FnMut1<St::Item, Output = Fut>,
    Fut: Future<Output = Option<T>>,
{
    fn is_terminated(&self) -> bool {
        self.pending.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F, T> Stream for FilterMap<St, Fut, F>
where
    St: Stream,
    F: FnMut1<St::Item, Output = Fut>,
    Fut: Future<Output = Option<T>>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let mut this = self.project();
        Poll::Ready(loop {
            if let Some(p) = this.pending.as_mut().as_pin_mut() {
                // We have an item in progress, poll that until it's done
                let item = ready!(p.poll(cx));
                this.pending.set(None);
                if item.is_some() {
                    break item;
                }
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                // No item in progress, but the stream is still going
                this.pending.set(Some(this.f.call_mut(item)));
            } else {
                // The stream is done
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let pending_len = usize::from(self.pending.is_some());
        let (_, upper) = self.stream.size_hint();
        let upper = match upper {
            Some(x) => x.checked_add(pending_len),
            None => None,
        };
        (0, upper) // can't know a lower bound, due to the predicate
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for FilterMap<S, Fut, F>
where
    S: Stream + Sink<Item>,
    F: FnMut1<S::Item, Output = Fut>,
    Fut: Future,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
