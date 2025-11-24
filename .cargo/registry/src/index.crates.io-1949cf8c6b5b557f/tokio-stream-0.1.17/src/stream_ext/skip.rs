use crate::Stream;

use core::fmt;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`skip`](super::StreamExt::skip) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Skip<St> {
        #[pin]
        stream: St,
        remaining: usize,
    }
}

impl<St> fmt::Debug for Skip<St>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Skip")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St> Skip<St> {
    pub(super) fn new(stream: St, remaining: usize) -> Self {
        Self { stream, remaining }
    }
}

impl<St> Stream for Skip<St>
where
    St: Stream,
{
    type Item = St::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match ready!(self.as_mut().project().stream.poll_next(cx)) {
                Some(e) => {
                    if self.remaining == 0 {
                        return Poll::Ready(Some(e));
                    }
                    *self.as_mut().project().remaining -= 1;
                }
                None => return Poll::Ready(None),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.stream.size_hint();

        let lower = lower.saturating_sub(self.remaining);
        let upper = upper.map(|x| x.saturating_sub(self.remaining));

        (lower, upper)
    }
}
