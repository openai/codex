//! A body backed by a channel.

use std::{
    fmt::Display,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Buf;
use http::HeaderMap;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use tokio::sync::{mpsc, oneshot};

pin_project! {
    /// A body backed by a channel.
    pub struct Channel<D, E = std::convert::Infallible> {
        rx_frame: mpsc::Receiver<Frame<D>>,
        #[pin]
        rx_error: oneshot::Receiver<E>,
    }
}

impl<D, E> Channel<D, E> {
    /// Create a new channel body.
    ///
    /// The channel will buffer up to the provided number of messages. Once the buffer is full,
    /// attempts to send new messages will wait until a message is received from the channel. The
    /// provided buffer capacity must be at least 1.
    pub fn new(buffer: usize) -> (Sender<D, E>, Self) {
        let (tx_frame, rx_frame) = mpsc::channel(buffer);
        let (tx_error, rx_error) = oneshot::channel();
        (Sender { tx_frame, tx_error }, Self { rx_frame, rx_error })
    }
}

impl<D, E> Body for Channel<D, E>
where
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        match this.rx_frame.poll_recv(cx) {
            Poll::Ready(frame @ Some(_)) => return Poll::Ready(frame.map(Ok)),
            Poll::Ready(None) | Poll::Pending => {}
        }

        use core::future::Future;
        match this.rx_error.poll(cx) {
            Poll::Ready(Ok(error)) => return Poll::Ready(Some(Err(error))),
            Poll::Ready(Err(_)) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        Poll::Pending
    }
}

impl<D, E: std::fmt::Debug> std::fmt::Debug for Channel<D, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("rx_frame", &self.rx_frame)
            .field("rx_error", &self.rx_error)
            .finish()
    }
}

/// A sender half created through [`Channel::new`].
pub struct Sender<D, E = std::convert::Infallible> {
    tx_frame: mpsc::Sender<Frame<D>>,
    tx_error: oneshot::Sender<E>,
}

impl<D, E> Sender<D, E> {
    /// Send a frame on the channel.
    pub async fn send(&mut self, frame: Frame<D>) -> Result<(), SendError> {
        self.tx_frame.send(frame).await.map_err(|_| SendError)
    }

    /// Send data on data channel.
    pub async fn send_data(&mut self, buf: D) -> Result<(), SendError> {
        self.send(Frame::data(buf)).await
    }

    /// Send trailers on trailers channel.
    pub async fn send_trailers(&mut self, trailers: HeaderMap) -> Result<(), SendError> {
        self.send(Frame::trailers(trailers)).await
    }

    /// Attempts to send a frame on this channel.
    ///
    /// This function returns the unsent frame back as an `Err(_)` if the channel could not
    /// (currently) accept another frame.
    ///
    /// # Note
    ///
    /// This is mostly useful for when trying to send a frame from outside of an asynchronous
    /// context. If in an async context, prefer [`Sender::send_data()`] instead.
    pub fn try_send(&mut self, frame: Frame<D>) -> Result<(), Frame<D>> {
        let Self {
            tx_frame,
            tx_error: _,
        } = self;

        tx_frame
            .try_send(frame)
            .map_err(tokio::sync::mpsc::error::TrySendError::into_inner)
    }

    /// Returns the current capacity of the channel.
    ///
    /// The capacity goes down when [`Frame<T>`]s are sent. The capacity goes up when these frames
    /// are received by the corresponding [`Channel<D, E>`]. This is distinct from
    /// [`max_capacity()`][Self::max_capacity], which always returns the buffer capacity initially
    /// specified when [`Channel::new()`][Channel::new] was called.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use http_body_util::{BodyExt, channel::Channel};
    /// use std::convert::Infallible;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///    let (mut tx, mut body) = Channel::<Bytes, Infallible>::new(4);
    ///    assert_eq!(tx.capacity(), 4);
    ///
    ///    // Sending a value decreases the available capacity.
    ///    tx.send_data(Bytes::from("Hel")).await.unwrap();
    ///    assert_eq!(tx.capacity(), 3);
    ///
    ///    // Reading a value increases the available capacity.
    ///    let _ = body.frame().await;
    ///    assert_eq!(tx.capacity(), 4);
    /// }
    /// ```
    pub fn capacity(&mut self) -> usize {
        self.tx_frame.capacity()
    }

    /// Returns the maximum capacity of the channel.
    ///
    /// This function always returns the buffer capacity initially specified when
    /// [`Channel::new()`][Channel::new] was called. This is distinct from
    /// [`capacity()`][Self::capacity], which returns the currently available capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use http_body_util::{BodyExt, channel::Channel};
    /// use std::convert::Infallible;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///    let (mut tx, mut body) = Channel::<Bytes, Infallible>::new(4);
    ///    assert_eq!(tx.max_capacity(), 4);
    ///
    ///    // Sending a value buffers it, but does not affect the maximum capacity reported.
    ///    tx.send_data(Bytes::from("Hel")).await.unwrap();
    ///    assert_eq!(tx.max_capacity(), 4);
    /// }
    /// ```
    pub fn max_capacity(&mut self) -> usize {
        self.tx_frame.max_capacity()
    }

    /// Aborts the body in an abnormal fashion.
    pub fn abort(self, error: E) {
        self.tx_error.send(error).ok();
    }
}

impl<D, E: std::fmt::Debug> std::fmt::Debug for Sender<D, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender")
            .field("tx_frame", &self.tx_frame)
            .field("tx_error", &self.tx_error)
            .finish()
    }
}

/// The error returned if [`Sender`] fails to send because the receiver is closed.
#[derive(Debug)]
#[non_exhaustive]
pub struct SendError;

impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to send frame")
    }
}

impl std::error::Error for SendError {}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::{HeaderName, HeaderValue};

    use crate::BodyExt;

    use super::*;

    #[tokio::test]
    async fn empty() {
        let (tx, body) = Channel::<Bytes>::new(1024);
        drop(tx);

        let collected = body.collect().await.unwrap();
        assert!(collected.trailers().is_none());
        assert!(collected.to_bytes().is_empty());
    }

    #[tokio::test]
    async fn can_send_data() {
        let (mut tx, body) = Channel::<Bytes>::new(1024);

        tokio::spawn(async move {
            tx.send_data(Bytes::from("Hel")).await.unwrap();
            tx.send_data(Bytes::from("lo!")).await.unwrap();
        });

        let collected = body.collect().await.unwrap();
        assert!(collected.trailers().is_none());
        assert_eq!(collected.to_bytes(), "Hello!");
    }

    #[tokio::test]
    async fn can_send_trailers() {
        let (mut tx, body) = Channel::<Bytes>::new(1024);

        tokio::spawn(async move {
            let mut trailers = HeaderMap::new();
            trailers.insert(
                HeaderName::from_static("foo"),
                HeaderValue::from_static("bar"),
            );
            tx.send_trailers(trailers).await.unwrap();
        });

        let collected = body.collect().await.unwrap();
        assert_eq!(collected.trailers().unwrap()["foo"], "bar");
        assert!(collected.to_bytes().is_empty());
    }

    #[tokio::test]
    async fn can_send_both_data_and_trailers() {
        let (mut tx, body) = Channel::<Bytes>::new(1024);

        tokio::spawn(async move {
            tx.send_data(Bytes::from("Hel")).await.unwrap();
            tx.send_data(Bytes::from("lo!")).await.unwrap();
            let mut trailers = HeaderMap::new();
            trailers.insert(
                HeaderName::from_static("foo"),
                HeaderValue::from_static("bar"),
            );
            tx.send_trailers(trailers).await.unwrap();
        });

        let collected = body.collect().await.unwrap();
        assert_eq!(collected.trailers().unwrap()["foo"], "bar");
        assert_eq!(collected.to_bytes(), "Hello!");
    }

    #[tokio::test]
    async fn try_send_works() {
        let (mut tx, mut body) = Channel::<Bytes>::new(2);

        // Send two messages, filling the channel's buffer.
        tx.try_send(Frame::data(Bytes::from("one")))
            .expect("can send one message");
        tx.try_send(Frame::data(Bytes::from("two")))
            .expect("can send two messages");

        // Sending a value to a full channel should return it back to us.
        match tx.try_send(Frame::data(Bytes::from("three"))) {
            Err(frame) => assert_eq!(frame.into_data().unwrap(), "three"),
            Ok(()) => panic!("synchronously sending a value to a full channel should fail"),
        };

        // Read the messages out of the body.
        assert_eq!(
            body.frame()
                .await
                .expect("yields result")
                .expect("yields frame")
                .into_data()
                .expect("yields data"),
            "one"
        );
        assert_eq!(
            body.frame()
                .await
                .expect("yields result")
                .expect("yields frame")
                .into_data()
                .expect("yields data"),
            "two"
        );

        // Drop the body.
        drop(body);

        // Sending a value to a closed channel should return it back to us.
        match tx.try_send(Frame::data(Bytes::from("closed"))) {
            Err(frame) => assert_eq!(frame.into_data().unwrap(), "closed"),
            Ok(()) => panic!("synchronously sending a value to a closed channel should fail"),
        };
    }

    /// A stand-in for an error type, for unit tests.
    type Error = &'static str;
    /// An example error message.
    const MSG: Error = "oh no";

    #[tokio::test]
    async fn aborts_before_trailers() {
        let (mut tx, body) = Channel::<Bytes, Error>::new(1024);

        tokio::spawn(async move {
            tx.send_data(Bytes::from("Hel")).await.unwrap();
            tx.send_data(Bytes::from("lo!")).await.unwrap();
            tx.abort(MSG);
        });

        let err = body.collect().await.unwrap_err();
        assert_eq!(err, MSG);
    }

    #[tokio::test]
    async fn aborts_after_trailers() {
        let (mut tx, body) = Channel::<Bytes, Error>::new(1024);

        tokio::spawn(async move {
            tx.send_data(Bytes::from("Hel")).await.unwrap();
            tx.send_data(Bytes::from("lo!")).await.unwrap();
            let mut trailers = HeaderMap::new();
            trailers.insert(
                HeaderName::from_static("foo"),
                HeaderValue::from_static("bar"),
            );
            tx.send_trailers(trailers).await.unwrap();
            tx.abort(MSG);
        });

        let err = body.collect().await.unwrap_err();
        assert_eq!(err, MSG);
    }
}
