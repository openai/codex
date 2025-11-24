use crate::Stream;

use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`then`](super::StreamExt::then) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Then<St, Fut, F> {
        #[pin]
        stream: St,
        #[pin]
        future: Option<Fut>,
        f: F,
    }
}

impl<St, Fut, F> fmt::Debug for Then<St, Fut, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Then")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, Fut, F> Then<St, Fut, F> {
    pub(super) fn new(stream: St, f: F) -> Self {
        Then {
            stream,
            future: None,
            f,
        }
    }
}

impl<St, F, Fut> Stream for Then<St, Fut, F>
where
    St: Stream,
    Fut: Future,
    F: FnMut(St::Item) -> Fut,
{
    type Item = Fut::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Fut::Output>> {
        let mut me = self.project();

        loop {
            if let Some(future) = me.future.as_mut().as_pin_mut() {
                match future.poll(cx) {
                    Poll::Ready(item) => {
                        me.future.set(None);
                        return Poll::Ready(Some(item));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            match me.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    me.future.set(Some((me.f)(item)));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let future_len = usize::from(self.future.is_some());
        let (lower, upper) = self.stream.size_hint();

        let lower = lower.saturating_add(future_len);
        let upper = upper.and_then(|upper| upper.checked_add(future_len));

        (lower, upper)
    }
}
