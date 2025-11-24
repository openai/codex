use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`unzip`](super::StreamExt::unzip) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Unzip<St, FromA, FromB> {
        #[pin]
        stream: St,
        left: FromA,
        right: FromB,
    }
}

impl<St: Stream, FromA: Default, FromB: Default> Unzip<St, FromA, FromB> {
    fn finish(self: Pin<&mut Self>) -> (FromA, FromB) {
        let this = self.project();
        (mem::take(this.left), mem::take(this.right))
    }

    pub(super) fn new(stream: St) -> Self {
        Self { stream, left: Default::default(), right: Default::default() }
    }
}

impl<St, A, B, FromA, FromB> FusedFuture for Unzip<St, FromA, FromB>
where
    St: FusedStream<Item = (A, B)>,
    FromA: Default + Extend<A>,
    FromB: Default + Extend<B>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, A, B, FromA, FromB> Future for Unzip<St, FromA, FromB>
where
    St: Stream<Item = (A, B)>,
    FromA: Default + Extend<A>,
    FromB: Default + Extend<B>,
{
    type Output = (FromA, FromB);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<(FromA, FromB)> {
        let mut this = self.as_mut().project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(e) => {
                    this.left.extend(Some(e.0));
                    this.right.extend(Some(e.1));
                }
                None => return Poll::Ready(self.finish()),
            }
        }
    }
}
