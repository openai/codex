use crate::future::{IntoFuture, TryFutureExt};
use crate::stream::{Fuse, FuturesOrdered, IntoStream, StreamExt};
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_buffered`](super::TryStreamExt::try_buffered) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryBuffered<St>
    where
        St: TryStream,
        St::Ok: TryFuture,
    {
        #[pin]
        stream: Fuse<IntoStream<St>>,
        in_progress_queue: FuturesOrdered<IntoFuture<St::Ok>>,
        max: usize,
    }
}

impl<St> TryBuffered<St>
where
    St: TryStream,
    St::Ok: TryFuture,
{
    pub(super) fn new(stream: St, n: usize) -> Self {
        Self {
            stream: IntoStream::new(stream).fuse(),
            in_progress_queue: FuturesOrdered::new(),
            max: n,
        }
    }

    delegate_access_inner!(stream, St, (. .));
}

impl<St> Stream for TryBuffered<St>
where
    St: TryStream,
    St::Ok: TryFuture<Error = St::Error>,
{
    type Item = Result<<St::Ok as TryFuture>::Ok, St::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First up, try to spawn off as many futures as possible by filling up
        // our queue of futures. Propagate errors from the stream immediately.
        while this.in_progress_queue.len() < *this.max {
            match this.stream.as_mut().poll_next(cx)? {
                Poll::Ready(Some(fut)) => this.in_progress_queue.push_back(fut.into_future()),
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Attempt to pull the next value from the in_progress_queue
        match this.in_progress_queue.poll_next_unpin(cx) {
            x @ Poll::Pending | x @ Poll::Ready(Some(_)) => return x,
            Poll::Ready(None) => {}
        }

        // If more values are still coming from the stream, we're not done yet
        if this.stream.is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item, E> Sink<Item> for TryBuffered<S>
where
    S: TryStream + Sink<Item, Error = E>,
    S::Ok: TryFuture<Error = E>,
{
    type Error = E;

    delegate_sink!(stream, Item);
}
