use crate::BoxError;
use http_body::Body;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};
use tokio::time::{sleep, Sleep};

pin_project! {
    /// Middleware that applies a timeout to request and response bodies.
    ///
    /// Wrapper around a [`Body`][`http_body::Body`] to time out if data is not ready within the specified duration.
    /// The timeout is enforced between consecutive [`Frame`][`http_body::Frame`] polls, and it
    /// resets after each poll.
    /// The total time to produce a [`Body`][`http_body::Body`] could exceed the timeout duration without
    /// timing out, as long as no single interval between polls exceeds the timeout.
    ///
    /// If the [`Body`][`http_body::Body`] does not produce a requested data frame within the timeout period, it will return a [`TimeoutError`].
    ///
    /// # Differences from [`Timeout`][crate::timeout::Timeout]
    ///
    /// [`Timeout`][crate::timeout::Timeout] applies a timeout to the request future, not body.
    /// That timeout is not reset when bytes are handled, whether the request is active or not.
    /// Bodies are handled asynchronously outside of the tower stack's future and thus needs an additional timeout.
    ///
    /// # Example
    ///
    /// ```
    /// use http::{Request, Response};
    /// use bytes::Bytes;
    /// use http_body_util::Full;
    /// use std::time::Duration;
    /// use tower::ServiceBuilder;
    /// use tower_http::timeout::RequestBodyTimeoutLayer;
    ///
    /// async fn handle(_: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    ///     // ...
    ///     # todo!()
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let svc = ServiceBuilder::new()
    ///     // Timeout bodies after 30 seconds of inactivity
    ///     .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
    ///     .service_fn(handle);
    /// # Ok(())
    /// # }
    /// ```
    pub struct TimeoutBody<B> {
        timeout: Duration,
        #[pin]
        sleep: Option<Sleep>,
        #[pin]
        body: B,
    }
}

impl<B> TimeoutBody<B> {
    /// Creates a new [`TimeoutBody`].
    pub fn new(timeout: Duration, body: B) -> Self {
        TimeoutBody {
            timeout,
            sleep: None,
            body,
        }
    }
}

impl<B> Body for TimeoutBody<B>
where
    B: Body,
    B::Error: Into<BoxError>,
{
    type Data = B::Data;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();

        // Start the `Sleep` if not active.
        let sleep_pinned = if let Some(some) = this.sleep.as_mut().as_pin_mut() {
            some
        } else {
            this.sleep.set(Some(sleep(*this.timeout)));
            this.sleep.as_mut().as_pin_mut().unwrap()
        };

        // Error if the timeout has expired.
        if let Poll::Ready(()) = sleep_pinned.poll(cx) {
            return Poll::Ready(Some(Err(Box::new(TimeoutError(())))));
        }

        // Check for body data.
        let frame = ready!(this.body.poll_frame(cx));
        // A frame is ready. Reset the `Sleep`...
        this.sleep.set(None);

        Poll::Ready(frame.transpose().map_err(Into::into).transpose())
    }
}

/// Error for [`TimeoutBody`].
#[derive(Debug)]
pub struct TimeoutError(());

impl std::error::Error for TimeoutError {}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "data was not received within the designated timeout")
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use http_body::Frame;
    use http_body_util::BodyExt;
    use pin_project_lite::pin_project;
    use std::{error::Error, fmt::Display};

    #[derive(Debug)]
    struct MockError;

    impl Error for MockError {}

    impl Display for MockError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "mock error")
        }
    }

    pin_project! {
        struct MockBody {
            #[pin]
            sleep: Sleep
        }
    }

    impl Body for MockBody {
        type Data = Bytes;
        type Error = MockError;

        fn poll_frame(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            let this = self.project();
            this.sleep
                .poll(cx)
                .map(|_| Some(Ok(Frame::data(vec![].into()))))
        }
    }

    #[tokio::test]
    async fn test_body_available_within_timeout() {
        let mock_sleep = Duration::from_secs(1);
        let timeout_sleep = Duration::from_secs(2);

        let mock_body = MockBody {
            sleep: sleep(mock_sleep),
        };
        let timeout_body = TimeoutBody::new(timeout_sleep, mock_body);

        assert!(timeout_body
            .boxed()
            .frame()
            .await
            .expect("no frame")
            .is_ok());
    }

    #[tokio::test]
    async fn test_body_unavailable_within_timeout_error() {
        let mock_sleep = Duration::from_secs(2);
        let timeout_sleep = Duration::from_secs(1);

        let mock_body = MockBody {
            sleep: sleep(mock_sleep),
        };
        let timeout_body = TimeoutBody::new(timeout_sleep, mock_body);

        assert!(timeout_body.boxed().frame().await.unwrap().is_err());
    }
}
