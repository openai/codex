use crate::Stream;

use core::fmt;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`map`](super::StreamExt::map) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Map<St, F> {
        #[pin]
        stream: St,
        f: F,
    }
}

impl<St, F> fmt::Debug for Map<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Map").field("stream", &self.stream).finish()
    }
}

impl<St, F> Map<St, F> {
    pub(super) fn new(stream: St, f: F) -> Self {
        Map { stream, f }
    }
}

impl<St, F, T> Stream for Map<St, F>
where
    St: Stream,
    F: FnMut(St::Item) -> T,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.as_mut()
            .project()
            .stream
            .poll_next(cx)
            .map(|opt| opt.map(|x| (self.as_mut().project().f)(x)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
