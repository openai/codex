use core::fmt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Sink for the [`with`](super::SinkExt::with) method.
    #[must_use = "sinks do nothing unless polled"]
    pub struct With<Si, Item, U, Fut, F> {
        #[pin]
        sink: Si,
        f: F,
        #[pin]
        state: Option<Fut>,
        _phantom: PhantomData<fn(U) -> Item>,
    }
}

impl<Si, Item, U, Fut, F> fmt::Debug for With<Si, Item, U, Fut, F>
where
    Si: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("With").field("sink", &self.sink).field("state", &self.state).finish()
    }
}

impl<Si, Item, U, Fut, F> With<Si, Item, U, Fut, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future,
{
    pub(super) fn new<E>(sink: Si, f: F) -> Self
    where
        Fut: Future<Output = Result<Item, E>>,
        E: From<Si::Error>,
    {
        Self { state: None, sink, f, _phantom: PhantomData }
    }
}

impl<Si, Item, U, Fut, F> Clone for With<Si, Item, U, Fut, F>
where
    Si: Clone,
    F: Clone,
    Fut: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            sink: self.sink.clone(),
            f: self.f.clone(),
            _phantom: PhantomData,
        }
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item, U, Fut, F> Stream for With<S, Item, U, Fut, F>
where
    S: Stream + Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future,
{
    type Item = S::Item;

    delegate_stream!(sink);
}

impl<Si, Item, U, Fut, F, E> With<Si, Item, U, Fut, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Item, E>>,
    E: From<Si::Error>,
{
    delegate_access_inner!(sink, Si, ());

    /// Completes the processing of previous item if any.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        let mut this = self.project();

        let item = match this.state.as_mut().as_pin_mut() {
            None => return Poll::Ready(Ok(())),
            Some(fut) => ready!(fut.poll(cx))?,
        };
        this.state.set(None);
        this.sink.start_send(item)?;
        Poll::Ready(Ok(()))
    }
}

impl<Si, Item, U, Fut, F, E> Sink<U> for With<Si, Item, U, Fut, F>
where
    Si: Sink<Item>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Item, E>>,
    E: From<Si::Error>,
{
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().sink.poll_ready(cx)?);
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: U) -> Result<(), Self::Error> {
        let mut this = self.project();

        assert!(this.state.is_none());
        this.state.set(Some((this.f)(item)));
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().sink.poll_flush(cx)?);
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().sink.poll_close(cx)?);
        Poll::Ready(Ok(()))
    }
}
