use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`concat`](super::StreamExt::concat) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Concat<St: Stream> {
        #[pin]
        stream: St,
        accum: Option<St::Item>,
    }
}

impl<St> Concat<St>
where
    St: Stream,
    St::Item: Extend<<St::Item as IntoIterator>::Item> + IntoIterator + Default,
{
    pub(super) fn new(stream: St) -> Self {
        Self { stream, accum: None }
    }
}

impl<St> Future for Concat<St>
where
    St: Stream,
    St::Item: Extend<<St::Item as IntoIterator>::Item> + IntoIterator + Default,
{
    type Output = St::Item;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                None => return Poll::Ready(this.accum.take().unwrap_or_default()),
                Some(e) => {
                    if let Some(a) = this.accum {
                        a.extend(e)
                    } else {
                        *this.accum = Some(e)
                    }
                }
            }
        }
    }
}

impl<St> FusedFuture for Concat<St>
where
    St: FusedStream,
    St::Item: Extend<<St::Item as IntoIterator>::Item> + IntoIterator + Default,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.stream.is_terminated()
    }
}
