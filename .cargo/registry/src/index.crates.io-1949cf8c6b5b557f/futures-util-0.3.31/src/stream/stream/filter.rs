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
    /// Stream for the [`filter`](super::StreamExt::filter) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Filter<St, Fut, F>
        where St: Stream,
    {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending_fut: Option<Fut>,
        pending_item: Option<St::Item>,
    }
}

impl<St, Fut, F> fmt::Debug for Filter<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .finish()
    }
}

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<St, Fut, F> Filter<St, Fut, F>
where
    St: Stream,
    F: for<'a> FnMut1<&'a St::Item, Output = Fut>,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, pending_fut: None, pending_item: None }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> FusedStream for Filter<St, Fut, F>
where
    St: Stream + FusedStream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.pending_fut.is_none() && self.stream.is_terminated()
    }
}

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<St, Fut, F> Stream for Filter<St, Fut, F>
where
    St: Stream,
    F: for<'a> FnMut1<&'a St::Item, Output = Fut>,
    Fut: Future<Output = bool>,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        let mut this = self.project();
        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let res = ready!(fut.poll(cx));
                this.pending_fut.set(None);
                if res {
                    break this.pending_item.take();
                }
                *this.pending_item = None;
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                this.pending_fut.set(Some(this.f.call_mut(&item)));
                *this.pending_item = Some(item);
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let pending_len = usize::from(self.pending_item.is_some());
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
impl<S, Fut, F, Item> Sink<Item> for Filter<S, Fut, F>
where
    S: Stream + Sink<Item>,
    F: FnMut(&S::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
