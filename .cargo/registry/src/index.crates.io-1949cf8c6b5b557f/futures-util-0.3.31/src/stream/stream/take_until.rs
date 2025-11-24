use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

// FIXME: docs, tests

pin_project! {
    /// Stream for the [`take_until`](super::StreamExt::take_until) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct TakeUntil<St: Stream, Fut: Future> {
        #[pin]
        stream: St,
        // Contains the inner Future on start and None once the inner Future is resolved
        // or taken out by the user.
        #[pin]
        fut: Option<Fut>,
        // Contains fut's return value once fut is resolved
        fut_result: Option<Fut::Output>,
        // Whether the future was taken out by the user.
        free: bool,
    }
}

impl<St, Fut> fmt::Debug for TakeUntil<St, Fut>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: Future + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TakeUntil").field("stream", &self.stream).field("fut", &self.fut).finish()
    }
}

impl<St, Fut> TakeUntil<St, Fut>
where
    St: Stream,
    Fut: Future,
{
    pub(super) fn new(stream: St, fut: Fut) -> Self {
        Self { stream, fut: Some(fut), fut_result: None, free: false }
    }

    delegate_access_inner!(stream, St, ());

    /// Extract the stopping future out of the combinator.
    /// The future is returned only if it isn't resolved yet, ie. if the stream isn't stopped yet.
    /// Taking out the future means the combinator will be yielding
    /// elements from the wrapped stream without ever stopping it.
    pub fn take_future(&mut self) -> Option<Fut> {
        if self.fut.is_some() {
            self.free = true;
        }

        self.fut.take()
    }

    /// Once the stopping future is resolved, this method can be used
    /// to extract the value returned by the stopping future.
    ///
    /// This may be used to retrieve arbitrary data from the stopping
    /// future, for example a reason why the stream was stopped.
    ///
    /// This method will return `None` if the future isn't resolved yet,
    /// or if the result was already taken out.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    /// use futures::task::Poll;
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let mut i = 0;
    /// let stop_fut = future::poll_fn(|_cx| {
    ///     i += 1;
    ///     if i <= 5 {
    ///         Poll::Pending
    ///     } else {
    ///         Poll::Ready("reason")
    ///     }
    /// });
    ///
    /// let mut stream = stream.take_until(stop_fut);
    /// let _ = stream.by_ref().collect::<Vec<_>>().await;
    ///
    /// let result = stream.take_result().unwrap();
    /// assert_eq!(result, "reason");
    /// # });
    /// ```
    pub fn take_result(&mut self) -> Option<Fut::Output> {
        self.fut_result.take()
    }

    /// Whether the stream was stopped yet by the stopping future
    /// being resolved.
    pub fn is_stopped(&self) -> bool {
        !self.free && self.fut.is_none()
    }
}

impl<St, Fut> Stream for TakeUntil<St, Fut>
where
    St: Stream,
    Fut: Future,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        let mut this = self.project();

        if let Some(f) = this.fut.as_mut().as_pin_mut() {
            if let Poll::Ready(result) = f.poll(cx) {
                this.fut.set(None);
                *this.fut_result = Some(result);
            }
        }

        if !*this.free && this.fut.is_none() {
            // Future resolved, inner stream stopped
            Poll::Ready(None)
        } else {
            // Future either not resolved yet or taken out by the user
            let item = ready!(this.stream.poll_next(cx));
            if item.is_none() {
                this.fut.set(None);
            }
            Poll::Ready(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_stopped() {
            return (0, Some(0));
        }

        self.stream.size_hint()
    }
}

impl<St, Fut> FusedStream for TakeUntil<St, Fut>
where
    St: Stream,
    Fut: Future,
{
    fn is_terminated(&self) -> bool {
        self.is_stopped()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, Item> Sink<Item> for TakeUntil<S, Fut>
where
    S: Stream + Sink<Item>,
    Fut: Future,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
