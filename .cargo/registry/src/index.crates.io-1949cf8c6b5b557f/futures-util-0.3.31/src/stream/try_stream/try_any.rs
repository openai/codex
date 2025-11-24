use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_any`](super::TryStreamExt::try_any) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryAny<St, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        done: bool,
        #[pin]
        future: Option<Fut>,
    }
}

impl<St, Fut, F> fmt::Debug for TryAny<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryAny")
            .field("stream", &self.stream)
            .field("done", &self.done)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> TryAny<St, Fut, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> Fut,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, done: false, future: None }
    }
}

impl<St, Fut, F> FusedFuture for TryAny<St, Fut, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> Fut,
    Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.done && self.future.is_none()
    }
}

impl<St, Fut, F> Future for TryAny<St, Fut, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> Fut,
    Fut: Future<Output = bool>,
{
    type Output = Result<bool, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<bool, St::Error>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                // we're currently processing a future to produce a new value
                let acc = ready!(fut.poll(cx));
                this.future.set(None);
                if acc {
                    *this.done = true;
                    break Ok(true);
                } // early exit
            } else if !*this.done {
                // we're waiting on a new item from the stream
                match ready!(this.stream.as_mut().try_poll_next(cx)) {
                    Some(Ok(item)) => {
                        this.future.set(Some((this.f)(item)));
                    }
                    Some(Err(err)) => {
                        *this.done = true;
                        break Err(err);
                    }
                    None => {
                        *this.done = true;
                        break Ok(false);
                    }
                }
            } else {
                panic!("TryAny polled after completion")
            }
        })
    }
}
