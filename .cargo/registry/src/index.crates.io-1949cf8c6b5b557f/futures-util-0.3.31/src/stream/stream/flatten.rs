use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`flatten`](super::StreamExt::flatten) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Flatten<St, U> {
        #[pin]
        stream: St,
        #[pin]
        next: Option<U>,
    }
}

impl<St, U> Flatten<St, U> {
    pub(super) fn new(stream: St) -> Self {
        Self { stream, next: None }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St> FusedStream for Flatten<St, St::Item>
where
    St: FusedStream,
    St::Item: Stream,
{
    fn is_terminated(&self) -> bool {
        self.next.is_none() && self.stream.is_terminated()
    }
}

impl<St> Stream for Flatten<St, St::Item>
where
    St: Stream,
    St::Item: Stream,
{
    type Item = <St::Item as Stream>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        Poll::Ready(loop {
            if let Some(s) = this.next.as_mut().as_pin_mut() {
                if let Some(item) = ready!(s.poll_next(cx)) {
                    break Some(item);
                } else {
                    this.next.set(None);
                }
            } else if let Some(s) = ready!(this.stream.as_mut().poll_next(cx)) {
                this.next.set(Some(s));
            } else {
                break None;
            }
        })
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Flatten<S, S::Item>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
