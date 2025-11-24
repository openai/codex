use crate::stream::StreamExt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};

/// Future for the [`into_future`](super::StreamExt::into_future) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct StreamFuture<St> {
    stream: Option<St>,
}

impl<St: Stream + Unpin> StreamFuture<St> {
    pub(super) fn new(stream: St) -> Self {
        Self { stream: Some(stream) }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    ///
    /// This method returns an `Option` to account for the fact that `StreamFuture`'s
    /// implementation of `Future::poll` consumes the underlying stream during polling
    /// in order to return it to the caller of `Future::poll` if the stream yielded
    /// an element.
    pub fn get_ref(&self) -> Option<&St> {
        self.stream.as_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    ///
    /// This method returns an `Option` to account for the fact that `StreamFuture`'s
    /// implementation of `Future::poll` consumes the underlying stream during polling
    /// in order to return it to the caller of `Future::poll` if the stream yielded
    /// an element.
    pub fn get_mut(&mut self) -> Option<&mut St> {
        self.stream.as_mut()
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    ///
    /// This method returns an `Option` to account for the fact that `StreamFuture`'s
    /// implementation of `Future::poll` consumes the underlying stream during polling
    /// in order to return it to the caller of `Future::poll` if the stream yielded
    /// an element.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Option<Pin<&mut St>> {
        self.get_mut().stream.as_mut().map(Pin::new)
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    ///
    /// This method returns an `Option` to account for the fact that `StreamFuture`'s
    /// implementation of `Future::poll` consumes the underlying stream during polling
    /// in order to return it to the caller of `Future::poll` if the stream yielded
    /// an element.
    pub fn into_inner(self) -> Option<St> {
        self.stream
    }
}

impl<St: Stream + Unpin> FusedFuture for StreamFuture<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_none()
    }
}

impl<St: Stream + Unpin> Future for StreamFuture<St> {
    type Output = (Option<St::Item>, St);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = {
            let s = self.stream.as_mut().expect("polling StreamFuture twice");
            ready!(s.poll_next_unpin(cx))
        };
        let stream = self.stream.take().unwrap();
        Poll::Ready((item, stream))
    }
}
