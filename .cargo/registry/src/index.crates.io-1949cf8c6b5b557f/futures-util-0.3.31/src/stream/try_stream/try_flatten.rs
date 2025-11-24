use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_flatten`](super::TryStreamExt::try_flatten) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryFlatten<St>
    where
        St: TryStream,
    {
        #[pin]
        stream: St,
        #[pin]
        next: Option<St::Ok>,
    }
}

impl<St> TryFlatten<St>
where
    St: TryStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    pub(super) fn new(stream: St) -> Self {
        Self { stream, next: None }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St> FusedStream for TryFlatten<St>
where
    St: TryStream + FusedStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.next.is_none() && self.stream.is_terminated()
    }
}

impl<St> Stream for TryFlatten<St>
where
    St: TryStream,
    St::Ok: TryStream,
    <St::Ok as TryStream>::Error: From<St::Error>,
{
    type Item = Result<<St::Ok as TryStream>::Ok, <St::Ok as TryStream>::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(s) = this.next.as_mut().as_pin_mut() {
                if let Some(item) = ready!(s.try_poll_next(cx)?) {
                    break Some(Ok(item));
                } else {
                    this.next.set(None);
                }
            } else if let Some(s) = ready!(this.stream.as_mut().try_poll_next(cx)?) {
                this.next.set(Some(s));
            } else {
                break None;
            }
        })
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for TryFlatten<S>
where
    S: TryStream + Sink<Item>,
{
    type Error = <S as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}
