use crate::stream::{Fuse, FuturesUnordered, StreamExt};
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`buffer_unordered`](super::StreamExt::buffer_unordered)
    /// method.
    #[must_use = "streams do nothing unless polled"]
    pub struct BufferUnordered<St>
    where
        St: Stream,
    {
        #[pin]
        stream: Fuse<St>,
        in_progress_queue: FuturesUnordered<St::Item>,
        max: usize,
    }
}

impl<St> fmt::Debug for BufferUnordered<St>
where
    St: Stream + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferUnordered")
            .field("stream", &self.stream)
            .field("in_progress_queue", &self.in_progress_queue)
            .field("max", &self.max)
            .finish()
    }
}

impl<St> BufferUnordered<St>
where
    St: Stream,
    St::Item: Future,
{
    pub(super) fn new(stream: St, n: usize) -> Self {
        Self {
            stream: super::Fuse::new(stream),
            in_progress_queue: FuturesUnordered::new(),
            max: n,
        }
    }

    delegate_access_inner!(stream, St, (.));
}

impl<St> Stream for BufferUnordered<St>
where
    St: Stream,
    St::Item: Future,
{
    type Item = <St::Item as Future>::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First up, try to spawn off as many futures as possible by filling up
        // our queue of futures.
        while this.in_progress_queue.len() < *this.max {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(fut)) => this.in_progress_queue.push(fut),
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

    fn size_hint(&self) -> (usize, Option<usize>) {
        let queue_len = self.in_progress_queue.len();
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(queue_len);
        let upper = match upper {
            Some(x) => x.checked_add(queue_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St> FusedStream for BufferUnordered<St>
where
    St: Stream,
    St::Item: Future,
{
    fn is_terminated(&self) -> bool {
        self.in_progress_queue.is_terminated() && self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for BufferUnordered<S>
where
    S: Stream + Sink<Item>,
    S::Item: Future,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
