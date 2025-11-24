//! [`Stream<Item = Request>`][stream] + [`Service<Request>`] => [`Stream<Item = Response>`][stream].
//!
//! [`Service<Request>`]: crate::Service
//! [stream]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html

use super::common;
use futures_core::Stream;
use futures_util::stream::FuturesOrdered;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

pin_project! {
    /// This is a [`Stream`] of responses resulting from calling the wrapped [`Service`] for each
    /// request received on the wrapped [`Stream`].
    ///
    /// ```rust
    /// # use std::task::{Poll, Context};
    /// # use std::cell::Cell;
    /// # use std::error::Error;
    /// # use std::rc::Rc;
    /// #
    /// use futures::future::{ready, Ready};
    /// use futures::StreamExt;
    /// use futures::channel::mpsc;
    /// use tower_service::Service;
    /// use tower::util::ServiceExt;
    ///
    /// // First, we need to have a Service to process our requests.
    /// #[derive(Debug, Eq, PartialEq)]
    /// struct FirstLetter;
    /// impl Service<&'static str> for FirstLetter {
    ///      type Response = &'static str;
    ///      type Error = Box<dyn Error + Send + Sync>;
    ///      type Future = Ready<Result<Self::Response, Self::Error>>;
    ///
    ///      fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    ///          Poll::Ready(Ok(()))
    ///      }
    ///
    ///      fn call(&mut self, req: &'static str) -> Self::Future {
    ///          ready(Ok(&req[..1]))
    ///      }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Next, we need a Stream of requests.
    // TODO(eliza): when `tokio-util` has a nice way to convert MPSCs to streams,
    //              tokio::sync::mpsc again?
    ///     let (mut reqs, rx) = mpsc::unbounded();
    ///     // Note that we have to help Rust out here by telling it what error type to use.
    ///     // Specifically, it has to be From<Service::Error> + From<Stream::Error>.
    ///     let mut rsps = FirstLetter.call_all(rx);
    ///
    ///     // Now, let's send a few requests and then check that we get the corresponding responses.
    ///     reqs.unbounded_send("one").unwrap();
    ///     reqs.unbounded_send("two").unwrap();
    ///     reqs.unbounded_send("three").unwrap();
    ///     drop(reqs);
    ///
    ///     // We then loop over the response `Stream` that we get back from call_all.
    ///     let mut i = 0usize;
    ///     while let Some(rsp) = rsps.next().await {
    ///         // Each response is a Result (we could also have used TryStream::try_next)
    ///         match (i + 1, rsp.unwrap()) {
    ///             (1, "o") |
    ///             (2, "t") |
    ///             (3, "t") => {}
    ///             (n, i) => {
    ///                 unreachable!("{}. response was '{}'", n, i);
    ///             }
    ///         }
    ///         i += 1;
    ///     }
    ///
    ///     // And at the end, we can get the Service back when there are no more requests.
    ///     assert_eq!(rsps.into_inner(), FirstLetter);
    /// }
    /// ```
    ///
    /// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
    #[derive(Debug)]
    pub struct CallAll<Svc, S>
    where
        Svc: Service<S::Item>,
        S: Stream,
    {
        #[pin]
        inner: common::CallAll<Svc, S, FuturesOrdered<Svc::Future>>,
    }
}

impl<Svc, S> CallAll<Svc, S>
where
    Svc: Service<S::Item>,
    S: Stream,
{
    /// Create new [`CallAll`] combinator.
    ///
    /// Each request yielded by `stream` is passed to `svc`, and the resulting responses are
    /// yielded in the same order by the implementation of [`Stream`] for [`CallAll`].
    ///
    /// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
    pub fn new(service: Svc, stream: S) -> CallAll<Svc, S> {
        CallAll {
            inner: common::CallAll::new(service, stream, FuturesOrdered::new()),
        }
    }

    /// Extract the wrapped [`Service`].
    ///
    /// # Panics
    ///
    /// Panics if [`take_service`] was already called.
    ///
    /// [`take_service`]: crate::util::CallAll::take_service
    pub fn into_inner(self) -> Svc {
        self.inner.into_inner()
    }

    /// Extract the wrapped [`Service`].
    ///
    /// This [`CallAll`] can no longer be used after this function has been called.
    ///
    /// # Panics
    ///
    /// Panics if [`take_service`] was already called.
    ///
    /// [`take_service`]: crate::util::CallAll::take_service
    pub fn take_service(self: Pin<&mut Self>) -> Svc {
        self.project().inner.take_service()
    }

    /// Return responses as they are ready, regardless of the initial order.
    ///
    /// This function must be called before the stream is polled.
    ///
    /// # Panics
    ///
    /// Panics if [`poll`] was called.
    ///
    /// [`poll`]: std::future::Future::poll
    pub fn unordered(self) -> super::CallAllUnordered<Svc, S> {
        self.inner.unordered()
    }
}

impl<Svc, S> Stream for CallAll<Svc, S>
where
    Svc: Service<S::Item>,
    S: Stream,
{
    type Item = Result<Svc::Response, Svc::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl<F: Future> common::Drive<F> for FuturesOrdered<F> {
    fn is_empty(&self) -> bool {
        FuturesOrdered::is_empty(self)
    }

    fn push(&mut self, future: F) {
        FuturesOrdered::push_back(self, future)
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Option<F::Output>> {
        Stream::poll_next(Pin::new(self), cx)
    }
}
