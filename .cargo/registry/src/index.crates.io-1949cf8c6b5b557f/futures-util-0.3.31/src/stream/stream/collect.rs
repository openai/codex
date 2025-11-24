use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`collect`](super::StreamExt::collect) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Collect<St, C> {
        #[pin]
        stream: St,
        collection: C,
    }
}

impl<St: Stream, C: Default> Collect<St, C> {
    fn finish(self: Pin<&mut Self>) -> C {
        mem::take(self.project().collection)
    }

    pub(super) fn new(stream: St) -> Self {
        Self { stream, collection: Default::default() }
    }
}

impl<St, C> FusedFuture for Collect<St, C>
where
    St: FusedStream,
    C: Default + Extend<St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for Collect<St, C>
where
    St: Stream,
    C: Default + Extend<St::Item>,
{
    type Output = C;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<C> {
        let mut this = self.as_mut().project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(e) => this.collection.extend(Some(e)),
                None => return Poll::Ready(self.finish()),
            }
        }
    }
}
