//! Slow down a stream by enforcing a delay between items.

use crate::Stream;
use tokio::time::{Duration, Instant, Sleep};

use std::future::Future;
use std::pin::Pin;
use std::task::{self, ready, Poll};

use pin_project_lite::pin_project;

pub(super) fn throttle<T>(duration: Duration, stream: T) -> Throttle<T>
where
    T: Stream,
{
    Throttle {
        delay: tokio::time::sleep_until(Instant::now() + duration),
        duration,
        has_delayed: true,
        stream,
    }
}

pin_project! {
    /// Stream for the [`throttle`](throttle) function. This object is `!Unpin`. If you need it to
    /// implement `Unpin` you can pin your throttle like this: `Box::pin(your_throttle)`.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Throttle<T> {
        #[pin]
        delay: Sleep,
        duration: Duration,

        // Set to true when `delay` has returned ready, but `stream` hasn't.
        has_delayed: bool,

        // The stream to throttle
        #[pin]
        stream: T,
    }
}

impl<T> Throttle<T> {
    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &T {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this combinator
    /// is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the stream
    /// which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so care
    /// should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> T {
        self.stream
    }
}

impl<T: Stream> Stream for Throttle<T> {
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let mut me = self.project();
        let dur = *me.duration;

        if !*me.has_delayed && !is_zero(dur) {
            ready!(me.delay.as_mut().poll(cx));
            *me.has_delayed = true;
        }

        let value = ready!(me.stream.poll_next(cx));

        if value.is_some() {
            if !is_zero(dur) {
                me.delay.reset(Instant::now() + dur);
            }

            *me.has_delayed = false;
        }

        Poll::Ready(value)
    }
}

fn is_zero(dur: Duration) -> bool {
    dur == Duration::from_millis(0)
}
