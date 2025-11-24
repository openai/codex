use core::fmt;
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_skip_while`](super::TryStreamExt::try_skip_while)
    /// method.
    #[must_use = "streams do nothing unless polled"]
    pub struct TrySkipWhile<St, Fut, F> where St: TryStream {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending_fut: Option<Fut>,
        pending_item: Option<St::Ok>,
        done_skipping: bool,
    }
}

impl<St, Fut, F> fmt::Debug for TrySkipWhile<St, Fut, F>
where
    St: TryStream + fmt::Debug,
    St::Ok: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrySkipWhile")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .field("done_skipping", &self.done_skipping)
            .finish()
    }
}

impl<St, Fut, F> TrySkipWhile<St, Fut, F>
where
    St: TryStream,
    F: FnMut(&St::Ok) -> Fut,
    Fut: TryFuture<Ok = bool, Error = St::Error>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, pending_fut: None, pending_item: None, done_skipping: false }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> Stream for TrySkipWhile<St, Fut, F>
where
    St: TryStream,
    F: FnMut(&St::Ok) -> Fut,
    Fut: TryFuture<Ok = bool, Error = St::Error>,
{
    type Item = Result<St::Ok, St::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.done_skipping {
            return this.stream.try_poll_next(cx);
        }

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let res = ready!(fut.try_poll(cx));
                this.pending_fut.set(None);
                let skipped = res?;
                let item = this.pending_item.take();
                if !skipped {
                    *this.done_skipping = true;
                    break item.map(Ok);
                }
            } else if let Some(item) = ready!(this.stream.as_mut().try_poll_next(cx)?) {
                this.pending_fut.set(Some((this.f)(&item)));
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

impl<St, Fut, F> FusedStream for TrySkipWhile<St, Fut, F>
where
    St: TryStream + FusedStream,
    F: FnMut(&St::Ok) -> Fut,
    Fut: TryFuture<Ok = bool, Error = St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.pending_item.is_none() && self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item, E> Sink<Item> for TrySkipWhile<S, Fut, F>
where
    S: TryStream + Sink<Item, Error = E>,
{
    type Error = E;

    delegate_sink!(stream, Item);
}
