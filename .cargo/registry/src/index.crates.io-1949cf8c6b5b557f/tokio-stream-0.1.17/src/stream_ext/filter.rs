use crate::Stream;

use core::fmt;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream returned by the [`filter`](super::StreamExt::filter) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Filter<St, F> {
        #[pin]
        stream: St,
        f: F,
    }
}

impl<St, F> fmt::Debug for Filter<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, F> Filter<St, F> {
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f }
    }
}

impl<St, F> Stream for Filter<St, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> bool,
{
    type Item = St::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        loop {
            match ready!(self.as_mut().project().stream.poll_next(cx)) {
                Some(e) => {
                    if (self.as_mut().project().f)(&e) {
                        return Poll::Ready(Some(e));
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.stream.size_hint().1) // can't know a lower bound, due to the predicate
    }
}
