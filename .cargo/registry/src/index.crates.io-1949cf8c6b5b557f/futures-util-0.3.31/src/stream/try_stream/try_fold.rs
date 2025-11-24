use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::ready;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_fold`](super::TryStreamExt::try_fold) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryFold<St, Fut, T, F> {
        #[pin]
        stream: St,
        f: F,
        accum: Option<T>,
        #[pin]
        future: Option<Fut>,
    }
}

impl<St, Fut, T, F> fmt::Debug for TryFold<St, Fut, T, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryFold")
            .field("stream", &self.stream)
            .field("accum", &self.accum)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, T, F> TryFold<St, Fut, T, F>
where
    St: TryStream,
    F: FnMut(T, St::Ok) -> Fut,
    Fut: TryFuture<Ok = T, Error = St::Error>,
{
    pub(super) fn new(stream: St, f: F, t: T) -> Self {
        Self { stream, f, accum: Some(t), future: None }
    }
}

impl<St, Fut, T, F> FusedFuture for TryFold<St, Fut, T, F>
where
    St: TryStream,
    F: FnMut(T, St::Ok) -> Fut,
    Fut: TryFuture<Ok = T, Error = St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.future.is_none()
    }
}

impl<St, Fut, T, F> Future for TryFold<St, Fut, T, F>
where
    St: TryStream,
    F: FnMut(T, St::Ok) -> Fut,
    Fut: TryFuture<Ok = T, Error = St::Error>,
{
    type Output = Result<T, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                // we're currently processing a future to produce a new accum value
                let res = ready!(fut.try_poll(cx));
                this.future.set(None);
                match res {
                    Ok(a) => *this.accum = Some(a),
                    Err(e) => break Err(e),
                }
            } else if this.accum.is_some() {
                // we're waiting on a new item from the stream
                let res = ready!(this.stream.as_mut().try_poll_next(cx));
                let a = this.accum.take().unwrap();
                match res {
                    Some(Ok(item)) => this.future.set(Some((this.f)(a, item))),
                    Some(Err(e)) => break Err(e),
                    None => break Ok(a),
                }
            } else {
                panic!("Fold polled after completion")
            }
        })
    }
}
