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
    /// Stream for the [`skip_while`](super::StreamExt::skip_while) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct SkipWhile<St, Fut, F> where St: Stream {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending_fut: Option<Fut>,
        pending_item: Option<St::Item>,
        done_skipping: bool,
    }
}

impl<St, Fut, F> fmt::Debug for SkipWhile<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkipWhile")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .field("done_skipping", &self.done_skipping)
            .finish()
    }
}

impl<St, Fut, F> SkipWhile<St, Fut, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, pending_fut: None, pending_item: None, done_skipping: false }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> FusedStream for SkipWhile<St, Fut, F>
where
    St: FusedStream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.pending_item.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for SkipWhile<St, Fut, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        let mut this = self.project();

        if *this.done_skipping {
            return this.stream.poll_next(cx);
        }

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let skipped = ready!(fut.poll(cx));
                let item = this.pending_item.take();
                this.pending_fut.set(None);
                if !skipped {
                    *this.done_skipping = true;
                    break item;
                }
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                this.pending_fut.set(Some((this.f)(&item)));
                *this.pending_item = Some(item);
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.done_skipping {
            self.stream.size_hint()
        } else {
            let pending_len = usize::from(self.pending_item.is_some());
            let (_, upper) = self.stream.size_hint();
            let upper = match upper {
                Some(x) => x.checked_add(pending_len),
                None => None,
            };
            (0, upper) // can't know a lower bound, due to the predicate
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for SkipWhile<S, Fut, F>
where
    S: Stream + Sink<Item>,
    F: FnMut(&S::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
