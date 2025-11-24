use std::{
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use crate::Sse;
use bytes::Bytes;
use futures_util::Stream;
use http_body::{Body, Frame};
use std::future::Future;
pin_project_lite::pin_project! {
    pub struct SseBody<S, T = NeverTimer> {
        #[pin]
        pub event_stream: S,
        #[pin]
        pub keep_alive: Option<KeepAliveStream<T>>,
    }
}

impl<S, E> SseBody<S, NeverTimer>
where
    S: Stream<Item = Result<Sse, E>>,
{
    pub fn new(stream: S) -> Self {
        Self {
            event_stream: stream,
            keep_alive: None,
        }
    }
}

impl<S, E, T> SseBody<S, T>
where
    S: Stream<Item = Result<Sse, E>>,
    T: Timer,
{
    pub fn new_keep_alive(stream: S, keep_alive: KeepAlive) -> Self {
        Self {
            event_stream: stream,
            keep_alive: Some(KeepAliveStream::new(keep_alive)),
        }
    }

    pub fn with_keep_alive<T2: Timer>(self, keep_alive: KeepAlive) -> SseBody<S, T2> {
        SseBody {
            event_stream: self.event_stream,
            keep_alive: Some(KeepAliveStream::new(keep_alive)),
        }
    }
}

impl<S, E, T> Body for SseBody<S, T>
where
    S: Stream<Item = Result<Sse, E>>,
    T: Timer,
{
    type Data = Bytes;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        match this.event_stream.poll_next(cx) {
            Poll::Pending => {
                if let Some(keep_alive) = this.keep_alive.as_pin_mut() {
                    keep_alive.poll_event(cx).map(|e| Some(Ok(Frame::data(e))))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Ok(event))) => {
                if let Some(keep_alive) = this.keep_alive.as_pin_mut() {
                    keep_alive.reset();
                }
                Poll::Ready(Some(Ok(Frame::data(event.into()))))
            }
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}

/// Configure the interval between keep-alive messages, the content
/// of each message, and the associated stream.
#[derive(Debug, Clone)]
#[must_use]
pub struct KeepAlive {
    event: Bytes,
    max_interval: Duration,
}

impl KeepAlive {
    /// Create a new `KeepAlive`.
    pub fn new() -> Self {
        Self {
            event: Bytes::from_static(b":\n\n"),
            max_interval: Duration::from_secs(15),
        }
    }

    /// Customize the interval between keep-alive messages.
    ///
    /// Default is 15 seconds.
    pub fn interval(mut self, time: Duration) -> Self {
        self.max_interval = time;
        self
    }

    /// Customize the event of the keep-alive message.
    ///
    /// Default is an empty comment.
    ///
    /// # Panics
    ///
    /// Panics if `event` contains any newline or carriage returns, as they are not allowed in SSE
    /// comments.
    pub fn event(mut self, event: Sse) -> Self {
        self.event = event.into();
        self
    }

    /// Customize the event of the keep-alive message with a comment
    pub fn comment(mut self, comment: &str) -> Self {
        self.event = format!(": {}\n\n", comment).into();
        self
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Timer: Future<Output = ()> {
    fn reset(self: Pin<&mut Self>, instant: std::time::Instant);
    fn from_duration(duration: Duration) -> Self;
}

pub struct NeverTimer;

impl Future for NeverTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Timer for NeverTimer {
    fn from_duration(_: Duration) -> Self {
        Self
    }

    fn reset(self: Pin<&mut Self>, _: std::time::Instant) {
        // No-op
    }
}

pin_project_lite::pin_project! {
    #[derive(Debug)]
    struct KeepAliveStream<S> {
        keep_alive: KeepAlive,
        #[pin]
        alive_timer: S,
    }
}

impl<S> KeepAliveStream<S>
where
    S: Timer,
{
    fn new(keep_alive: KeepAlive) -> Self {
        Self {
            alive_timer: S::from_duration(keep_alive.max_interval),
            keep_alive,
        }
    }

    fn reset(self: Pin<&mut Self>) {
        let this = self.project();
        this.alive_timer
            .reset(std::time::Instant::now() + this.keep_alive.max_interval);
    }

    fn poll_event(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Bytes> {
        let this = self.as_mut().project();

        ready!(this.alive_timer.poll(cx));

        let event = this.keep_alive.event.clone();

        self.reset();

        Poll::Ready(event)
    }
}
