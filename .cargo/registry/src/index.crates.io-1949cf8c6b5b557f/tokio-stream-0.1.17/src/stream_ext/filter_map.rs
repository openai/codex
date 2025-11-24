use crate::Stream;

use core::fmt;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream returned by the [`filter_map`](super::StreamExt::filter_map) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct FilterMap<St, F> {
        #[pin]
        stream: St,
        f: F,
    }
}

impl<St, F> fmt::Debug for FilterMap<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilterMap")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, F> FilterMap<St, F> {
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f }
    }
}

impl<St, F, T> Stream for FilterMap<St, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Option<T>,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        loop {
            match ready!(self.as_mut().project().stream.poll_next(cx)) {
                Some(e) => {
                    if let Some(e) = (self.as_mut().project().f)(e) {
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
