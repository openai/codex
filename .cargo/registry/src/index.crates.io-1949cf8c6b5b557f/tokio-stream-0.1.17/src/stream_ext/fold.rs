use crate::Stream;

use core::future::Future;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future returned by the [`fold`](super::StreamExt::fold) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct FoldFuture<St, B, F> {
        #[pin]
        stream: St,
        acc: Option<B>,
        f: F,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

impl<St, B, F> FoldFuture<St, B, F> {
    pub(super) fn new(stream: St, init: B, f: F) -> Self {
        Self {
            stream,
            acc: Some(init),
            f,
            _pin: PhantomPinned,
        }
    }
}

impl<St, B, F> Future for FoldFuture<St, B, F>
where
    St: Stream,
    F: FnMut(B, St::Item) -> B,
{
    type Output = B;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            let next = ready!(me.stream.as_mut().poll_next(cx));

            match next {
                Some(v) => {
                    let old = me.acc.take().unwrap();
                    let new = (me.f)(old, v);
                    *me.acc = Some(new);
                }
                None => return Poll::Ready(me.acc.take().unwrap()),
            }
        }
    }
}
