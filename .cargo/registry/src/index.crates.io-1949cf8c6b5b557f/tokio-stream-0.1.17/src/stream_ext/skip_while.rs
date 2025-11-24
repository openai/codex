use crate::Stream;

use core::fmt;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`skip_while`](super::StreamExt::skip_while) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct SkipWhile<St, F> {
        #[pin]
        stream: St,
        predicate: Option<F>,
    }
}

impl<St, F> fmt::Debug for SkipWhile<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkipWhile")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, F> SkipWhile<St, F> {
    pub(super) fn new(stream: St, predicate: F) -> Self {
        Self {
            stream,
            predicate: Some(predicate),
        }
    }
}

impl<St, F> Stream for SkipWhile<St, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> bool,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        if let Some(predicate) = this.predicate {
            loop {
                match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => {
                        if !(predicate)(&item) {
                            *this.predicate = None;
                            return Poll::Ready(Some(item));
                        }
                    }
                    None => return Poll::Ready(None),
                }
            }
        } else {
            this.stream.poll_next(cx)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.stream.size_hint();

        if self.predicate.is_some() {
            return (0, upper);
        }

        (lower, upper)
    }
}
