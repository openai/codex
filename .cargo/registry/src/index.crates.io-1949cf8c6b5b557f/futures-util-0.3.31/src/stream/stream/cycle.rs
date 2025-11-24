use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`cycle`](super::StreamExt::cycle) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Cycle<St> {
        orig: St,
        #[pin]
        stream: St,
    }
}

impl<St> Cycle<St>
where
    St: Clone + Stream,
{
    pub(super) fn new(stream: St) -> Self {
        Self { orig: stream.clone(), stream }
    }
}

impl<St> Stream for Cycle<St>
where
    St: Clone + Stream,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match ready!(this.stream.as_mut().poll_next(cx)) {
            None => {
                this.stream.set(this.orig.clone());
                this.stream.poll_next(cx)
            }
            item => Poll::Ready(item),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // the cycle stream is either empty or infinite
        match self.orig.size_hint() {
            size @ (0, Some(0)) => size,
            (0, _) => (0, None),
            _ => (usize::MAX, None),
        }
    }
}

impl<St> FusedStream for Cycle<St>
where
    St: Clone + Stream,
{
    fn is_terminated(&self) -> bool {
        // the cycle stream is either empty or infinite
        if let (0, Some(0)) = self.size_hint() {
            true
        } else {
            false
        }
    }
}
