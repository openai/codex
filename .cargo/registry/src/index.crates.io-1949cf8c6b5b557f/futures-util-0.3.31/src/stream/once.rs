use super::assert_stream;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

/// Creates a stream of a single element.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// let stream = stream::once(async { 17 });
/// let collected = stream.collect::<Vec<i32>>().await;
/// assert_eq!(collected, vec![17]);
/// # });
/// ```
pub fn once<Fut: Future>(future: Fut) -> Once<Fut> {
    assert_stream::<Fut::Output, _>(Once::new(future))
}

pin_project! {
    /// A stream which emits single element and then EOF.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Once<Fut> {
        #[pin]
        future: Option<Fut>
    }
}

impl<Fut> Once<Fut> {
    pub(crate) fn new(future: Fut) -> Self {
        Self { future: Some(future) }
    }
}

impl<Fut: Future> Stream for Once<Fut> {
    type Item = Fut::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let v = match this.future.as_mut().as_pin_mut() {
            Some(fut) => ready!(fut.poll(cx)),
            None => return Poll::Ready(None),
        };

        this.future.set(None);
        Poll::Ready(Some(v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.future.is_some() {
            (1, Some(1))
        } else {
            (0, Some(0))
        }
    }
}

impl<Fut: Future> FusedStream for Once<Fut> {
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}
