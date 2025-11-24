use crate::stream_ext::Fuse;
use crate::{Elapsed, Stream};
use tokio::time::Interval;

use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream returned by the [`timeout_repeating`](super::StreamExt::timeout_repeating) method.
    #[must_use = "streams do nothing unless polled"]
    #[derive(Debug)]
    pub struct TimeoutRepeating<S> {
        #[pin]
        stream: Fuse<S>,
        #[pin]
        interval: Interval,
    }
}

impl<S: Stream> TimeoutRepeating<S> {
    pub(super) fn new(stream: S, interval: Interval) -> Self {
        TimeoutRepeating {
            stream: Fuse::new(stream),
            interval,
        }
    }
}

impl<S: Stream> Stream for TimeoutRepeating<S> {
    type Item = Result<S::Item, Elapsed>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut me = self.project();

        match me.stream.poll_next(cx) {
            Poll::Ready(v) => {
                if v.is_some() {
                    me.interval.reset();
                }
                return Poll::Ready(v.map(Ok));
            }
            Poll::Pending => {}
        };

        ready!(me.interval.poll_tick(cx));
        Poll::Ready(Some(Err(Elapsed::new())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, _) = self.stream.size_hint();

        // The timeout stream may insert an error an infinite number of times.
        (lower, None)
    }
}
