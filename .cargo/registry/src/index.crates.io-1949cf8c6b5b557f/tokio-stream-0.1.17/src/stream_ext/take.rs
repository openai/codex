use crate::Stream;

use core::cmp;
use core::fmt;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`take`](super::StreamExt::take) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Take<St> {
        #[pin]
        stream: St,
        remaining: usize,
    }
}

impl<St> fmt::Debug for Take<St>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Take")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St> Take<St> {
    pub(super) fn new(stream: St, remaining: usize) -> Self {
        Self { stream, remaining }
    }
}

impl<St> Stream for Take<St>
where
    St: Stream,
{
    type Item = St::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.as_mut().project().remaining > 0 {
            self.as_mut().project().stream.poll_next(cx).map(|ready| {
                match &ready {
                    Some(_) => {
                        *self.as_mut().project().remaining -= 1;
                    }
                    None => {
                        *self.as_mut().project().remaining = 0;
                    }
                }
                ready
            })
        } else {
            Poll::Ready(None)
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
