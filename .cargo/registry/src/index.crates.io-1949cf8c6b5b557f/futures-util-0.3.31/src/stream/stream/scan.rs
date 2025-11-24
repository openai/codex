use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

struct StateFn<S, F> {
    state: S,
    f: F,
}

pin_project! {
    /// Stream for the [`scan`](super::StreamExt::scan) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Scan<St: Stream, S, Fut, F> {
        #[pin]
        stream: St,
        state_f: Option<StateFn<S, F>>,
        #[pin]
        future: Option<Fut>,
    }
}

impl<St, S, Fut, F> fmt::Debug for Scan<St, S, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    S: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scan")
            .field("stream", &self.stream)
            .field("state", &self.state_f.as_ref().map(|s| &s.state))
            .field("future", &self.future)
            .field("done_taking", &self.is_done_taking())
            .finish()
    }
}

impl<St: Stream, S, Fut, F> Scan<St, S, Fut, F> {
    /// Checks if internal state is `None`.
    fn is_done_taking(&self) -> bool {
        self.state_f.is_none()
    }
}

impl<B, St, S, Fut, F> Scan<St, S, Fut, F>
where
    St: Stream,
    F: FnMut(&mut S, St::Item) -> Fut,
    Fut: Future<Output = Option<B>>,
{
    pub(super) fn new(stream: St, initial_state: S, f: F) -> Self {
        Self { stream, state_f: Some(StateFn { state: initial_state, f }), future: None }
    }

    delegate_access_inner!(stream, St, ());
}

impl<B, St, S, Fut, F> Stream for Scan<St, S, Fut, F>
where
    St: Stream,
    F: FnMut(&mut S, St::Item) -> Fut,
    Fut: Future<Output = Option<B>>,
{
    type Item = B;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<B>> {
        if self.is_done_taking() {
            return Poll::Ready(None);
        }

        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                let item = ready!(fut.poll(cx));
                this.future.set(None);

                if item.is_none() {
                    *this.state_f = None;
                }

                break item;
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                let state_f = this.state_f.as_mut().unwrap();
                this.future.set(Some((state_f.f)(&mut state_f.state, item)))
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_done_taking() {
            (0, Some(0))
        } else {
            self.stream.size_hint() // can't know a lower bound, due to the predicate
        }
    }
}

impl<B, St, S, Fut, F> FusedStream for Scan<St, S, Fut, F>
where
    St: FusedStream,
    F: FnMut(&mut S, St::Item) -> Fut,
    Fut: Future<Output = Option<B>>,
{
    fn is_terminated(&self) -> bool {
        self.is_done_taking() || self.future.is_none() && self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, S, Fut, F, Item> Sink<Item> for Scan<St, S, Fut, F>
where
    St: Stream + Sink<Item>,
{
    type Error = St::Error;

    delegate_sink!(stream, Item);
}
