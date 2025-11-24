use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_collect`](super::TryStreamExt::try_collect) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryCollect<St, C> {
        #[pin]
        stream: St,
        items: C,
    }
}

impl<St: TryStream, C: Default> TryCollect<St, C> {
    pub(super) fn new(s: St) -> Self {
        Self { stream: s, items: Default::default() }
    }
}

impl<St, C> FusedFuture for TryCollect<St, C>
where
    St: TryStream + FusedStream,
    C: Default + Extend<St::Ok>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for TryCollect<St, C>
where
    St: TryStream,
    C: Default + Extend<St::Ok>,
{
    type Output = Result<C, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        Poll::Ready(Ok(loop {
            match ready!(this.stream.as_mut().try_poll_next(cx)?) {
                Some(x) => this.items.extend(Some(x)),
                None => break mem::take(this.items),
            }
        }))
    }
}
