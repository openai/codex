use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`count`](super::StreamExt::count) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Count<St> {
        #[pin]
        stream: St,
        count: usize
    }
}

impl<St> fmt::Debug for Count<St>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Count").field("stream", &self.stream).field("count", &self.count).finish()
    }
}

impl<St: Stream> Count<St> {
    pub(super) fn new(stream: St) -> Self {
        Self { stream, count: 0 }
    }
}

impl<St: FusedStream> FusedFuture for Count<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: Stream> Future for Count<St> {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        Poll::Ready(loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(_) => *this.count += 1,
                None => break *this.count,
            }
        })
    }
}
