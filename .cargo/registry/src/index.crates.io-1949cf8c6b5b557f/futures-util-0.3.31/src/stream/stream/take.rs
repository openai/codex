use core::cmp;
use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`take`](super::StreamExt::take) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Take<St> {
        #[pin]
        stream: St,
        remaining: usize,
    }
}

impl<St: Stream> Take<St> {
    pub(super) fn new(stream: St, n: usize) -> Self {
        Self { stream, remaining: n }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St> Stream for Take<St>
where
    St: Stream,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        if self.remaining == 0 {
            Poll::Ready(None)
        } else {
            let this = self.project();
            let next = ready!(this.stream.poll_next(cx));
            if next.is_some() {
                *this.remaining -= 1;
            } else {
                *this.remaining = 0;
            }
            Poll::Ready(next)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.remaining == 0 {
            return (0, Some(0));
        }

        let (lower, upper) = self.stream.size_hint();

        let lower = cmp::min(lower, self.remaining);

        let upper = match upper {
            Some(x) if x < self.remaining => Some(x),
            _ => Some(self.remaining),
        };

        (lower, upper)
    }
}

impl<St> FusedStream for Take<St>
where
    St: FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.remaining == 0 || self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Take<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
