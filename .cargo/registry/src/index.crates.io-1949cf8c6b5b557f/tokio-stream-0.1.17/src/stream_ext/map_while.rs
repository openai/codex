use crate::Stream;

use core::fmt;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`map_while`](super::StreamExt::map_while) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct MapWhile<St, F> {
        #[pin]
        stream: St,
        f: F,
    }
}

impl<St, F> fmt::Debug for MapWhile<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapWhile")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, F> MapWhile<St, F> {
    pub(super) fn new(stream: St, f: F) -> Self {
        MapWhile { stream, f }
    }
}

impl<St, F, T> Stream for MapWhile<St, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Option<T>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let me = self.project();
        let f = me.f;
        me.stream.poll_next(cx).map(|opt| opt.and_then(f))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.stream.size_hint();
        (0, upper)
    }
}
