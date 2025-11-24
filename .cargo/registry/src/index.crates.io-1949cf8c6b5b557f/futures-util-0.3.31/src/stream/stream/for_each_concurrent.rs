use crate::stream::{FuturesUnordered, StreamExt};
use core::fmt;
use core::num::NonZeroUsize;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`for_each_concurrent`](super::StreamExt::for_each_concurrent)
    /// method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ForEachConcurrent<St, Fut, F> {
        #[pin]
        stream: Option<St>,
        f: F,
        futures: FuturesUnordered<Fut>,
        limit: Option<NonZeroUsize>,
    }
}

impl<St, Fut, F> fmt::Debug for ForEachConcurrent<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEachConcurrent")
            .field("stream", &self.stream)
            .field("futures", &self.futures)
            .field("limit", &self.limit)
            .finish()
    }
}

impl<St, Fut, F> ForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> Self {
        Self {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
        }
    }
}

impl<St, Fut, F> FusedFuture for ForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_none() && self.futures.is_empty()
    }
}

impl<St, Fut, F> Future for ForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut this = self.project();
        loop {
            let mut made_progress_this_iter = false;

            // Check if we've already created a number of futures greater than `limit`
            if this.limit.map(|limit| limit.get() > this.futures.len()).unwrap_or(true) {
                let mut stream_completed = false;
                let elem = if let Some(stream) = this.stream.as_mut().as_pin_mut() {
                    match stream.poll_next(cx) {
                        Poll::Ready(Some(elem)) => {
                            made_progress_this_iter = true;
                            Some(elem)
                        }
                        Poll::Ready(None) => {
                            stream_completed = true;
                            None
                        }
                        Poll::Pending => None,
                    }
                } else {
                    None
                };
                if stream_completed {
                    this.stream.set(None);
                }
                if let Some(elem) = elem {
                    this.futures.push((this.f)(elem));
                }
            }

            match this.futures.poll_next_unpin(cx) {
                Poll::Ready(Some(())) => made_progress_this_iter = true,
                Poll::Ready(None) => {
                    if this.stream.is_none() {
                        return Poll::Ready(());
                    }
                }
                Poll::Pending => {}
            }

            if !made_progress_this_iter {
                return Poll::Pending;
            }
        }
    }
}
