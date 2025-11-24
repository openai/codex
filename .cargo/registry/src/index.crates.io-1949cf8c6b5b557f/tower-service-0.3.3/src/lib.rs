#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![forbid(unsafe_code)]
// `rustdoc::broken_intra_doc_links` is checked on CI

//! Definition of the core `Service` trait to Tower
//!
//! The [`Service`] trait provides the necessary abstractions for defining
//! request / response clients and servers. It is simple but powerful and is
//! used as the foundation for the rest of Tower.

use std::future::Future;
use std::task::{Context, Poll};

/// An asynchronous function from a `Request` to a `Response`.
///
/// The `Service` trait is a simplified interface making it easy to write
/// network applications in a modular and reusable way, decoupled from the
/// underlying protocol. It is one of Tower's fundamental abstractions.
///
/// # Functional
///
/// A `Service` is a function of a `Request`. It immediately returns a
/// `Future` representing the eventual completion of processing the
/// request. The actual request processing may happen at any time in the
/// future, on any thread or executor. The processing may depend on calling
/// other services. At some point in the future, the processing will complete,
/// and the `Future` will resolve to a response or error.
///
/// At a high level, the `Service::call` function represents an RPC request. The
/// `Service` value can be a server or a client.
///
/// # Server
///
/// An RPC server *implements* the `Service` trait. Requests received by the
/// server over the network are deserialized and then passed as an argument to the
/// server value. The returned response is sent back over the network.
///
/// As an example, here is how an HTTP request is processed by a server:
///
/// ```rust
/// # use std::pin::Pin;
/// # use std::task::{Poll, Context};
/// # use std::future::Future;
/// # use tower_service::Service;
/// use http::{Request, Response, StatusCode};
///
/// struct HelloWorld;
///
/// impl Service<Request<Vec<u8>>> for HelloWorld {
///     type Response = Response<Vec<u8>>;
///     type Error = http::Error;
///     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
///
///     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
///         Poll::Ready(Ok(()))
///     }
///
///     fn call(&mut self, req: Request<Vec<u8>>) -> Self::Future {
///         // create the body
///         let body: Vec<u8> = "hello, world!\n"
///             .as_bytes()
///             .to_owned();
///         // Create the HTTP response
///         let resp = Response::builder()
///             .status(StatusCode::OK)
///             .body(body)
///             .expect("Unable to create `http::Response`");
///
///         // create a response in a future.
///         let fut = async {
///             Ok(resp)
///         };
///
///         // Return the response as an immediate future
///         Box::pin(fut)
///     }
/// }
/// ```
///
/// # Client
///
/// A client consumes a service by using a `Service` value. The client may
/// issue requests by invoking `call` and passing the request as an argument.
/// It then receives the response by waiting for the returned future.
///
/// As an example, here is how a Redis request would be issued:
///
/// ```rust,ignore
/// let client = redis::Client::new()
///     .connect("127.0.0.1:6379".parse().unwrap())
///     .unwrap();
///
/// let resp = client.call(Cmd::set("foo", "this is the value of foo")).await?;
///
/// // Wait for the future to resolve
/// println!("Redis response: {:?}", resp);
/// ```
///
/// # Middleware / Layer
///
/// More often than not, all the pieces needed for writing robust, scalable
/// network applications are the same no matter the underlying protocol. By
/// unifying the API for both clients and servers in a protocol agnostic way,
/// it is possible to write middleware that provide these pieces in a
/// reusable way.
///
/// Take timeouts as an example:
///
/// ```rust
/// use tower_service::Service;
/// use tower_layer::Layer;
/// use futures::FutureExt;
/// use std::future::Future;
/// use std::task::{Context, Poll};
/// use std::time::Duration;
/// use std::pin::Pin;
/// use std::fmt;
/// use std::error::Error;
///
/// // Our timeout service, which wraps another service and
/// // adds a timeout to its response future.
/// pub struct Timeout<T> {
///     inner: T,
///     timeout: Duration,
/// }
///
/// impl<T> Timeout<T> {
///     pub const fn new(inner: T, timeout: Duration) -> Timeout<T> {
///         Timeout {
///             inner,
///             timeout
///         }
///     }
/// }
///
/// // The error returned if processing a request timed out
/// #[derive(Debug)]
/// pub struct Expired;
///
/// impl fmt::Display for Expired {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "expired")
///     }
/// }
///
/// impl Error for Expired {}
///
/// // We can implement `Service` for `Timeout<T>` if `T` is a `Service`
/// impl<T, Request> Service<Request> for Timeout<T>
/// where
///     T: Service<Request>,
///     T::Future: 'static,
///     T::Error: Into<Box<dyn Error + Send + Sync>> + 'static,
///     T::Response: 'static,
/// {
///     // `Timeout` doesn't modify the response type, so we use `T`'s response type
///     type Response = T::Response;
///     // Errors may be either `Expired` if the timeout expired, or the inner service's
///     // `Error` type. Therefore, we return a boxed `dyn Error + Send + Sync` trait object to erase
///     // the error's type.
///     type Error = Box<dyn Error + Send + Sync>;
///     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
///
///     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
///         // Our timeout service is ready if the inner service is ready.
///         // This is how backpressure can be propagated through a tree of nested services.
///        self.inner.poll_ready(cx).map_err(Into::into)
///     }
///
///     fn call(&mut self, req: Request) -> Self::Future {
///         // Create a future that completes after `self.timeout`
///         let timeout = tokio::time::sleep(self.timeout);
///
///         // Call the inner service and get a future that resolves to the response
///         let fut = self.inner.call(req);
///
///         // Wrap those two futures in another future that completes when either one completes
///         //
///         // If the inner service is too slow the `sleep` future will complete first
///         // And an error will be returned and `fut` will be dropped and not polled again
///         //
///         // We have to box the errors so the types match
///         let f = async move {
///             tokio::select! {
///                 res = fut => {
///                     res.map_err(|err| err.into())
///                 },
///                 _ = timeout => {
///                     Err(Box::new(Expired) as Box<dyn Error + Send + Sync>)
///                 },
///             }
///         };
///
///         Box::pin(f)
///     }
/// }
///
/// // A layer for wrapping services in `Timeout`
/// pub struct TimeoutLayer(Duration);
///
/// impl TimeoutLayer {
///     pub const fn new(delay: Duration) -> Self {
///         TimeoutLayer(delay)
///     }
/// }
///
/// impl<S> Layer<S> for TimeoutLayer {
///     type Service = Timeout<S>;
///
///     fn layer(&self, service: S) -> Timeout<S> {
///         Timeout::new(service, self.0)
///     }
/// }
/// ```
///
/// The above timeout implementation is decoupled from the underlying protocol
/// and is also decoupled from client or server concerns. In other words, the
/// same timeout middleware could be used in either a client or a server.
///
/// # Backpressure
///
/// Calling a `Service` which is at capacity (i.e., it is temporarily unable to process a
/// request) should result in an error. The caller is responsible for ensuring
/// that the service is ready to receive the request before calling it.
///
/// `Service` provides a mechanism by which the caller is able to coordinate
/// readiness. `Service::poll_ready` returns `Ready` if the service expects that
/// it is able to process a request.
///
/// # Be careful when cloning inner services
///
/// Services are permitted to panic if `call` is invoked without obtaining `Poll::Ready(Ok(()))`
/// from `poll_ready`. You should therefore be careful when cloning services for example to move
/// them into boxed futures. Even though the original service is ready, the clone might not be.
///
/// Therefore this kind of code is wrong and might panic:
///
/// ```rust
/// # use std::pin::Pin;
/// # use std::task::{Poll, Context};
/// # use std::future::Future;
/// # use tower_service::Service;
/// #
/// struct Wrapper<S> {
///     inner: S,
/// }
///
/// impl<R, S> Service<R> for Wrapper<S>
/// where
///     S: Service<R> + Clone + 'static,
///     R: 'static,
/// {
///     type Response = S::Response;
///     type Error = S::Error;
///     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
///
///     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
///         self.inner.poll_ready(cx)
///     }
///
///     fn call(&mut self, req: R) -> Self::Future {
///         let mut inner = self.inner.clone();
///         Box::pin(async move {
///             // `inner` might not be ready since its a clone
///             inner.call(req).await
///         })
///     }
/// }
/// ```
///
/// You should instead use [`std::mem::replace`] to take the service that was ready:
///
/// ```rust
/// # use std::pin::Pin;
/// # use std::task::{Poll, Context};
/// # use std::future::Future;
/// # use tower_service::Service;
/// #
/// struct Wrapper<S> {
///     inner: S,
/// }
///
/// impl<R, S> Service<R> for Wrapper<S>
/// where
///     S: Service<R> + Clone + 'static,
///     R: 'static,
/// {
///     type Response = S::Response;
///     type Error = S::Error;
///     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
///
///     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
///         self.inner.poll_ready(cx)
///     }
///
///     fn call(&mut self, req: R) -> Self::Future {
///         let clone = self.inner.clone();
///         // take the service that was ready
///         let mut inner = std::mem::replace(&mut self.inner, clone);
///         Box::pin(async move {
///             inner.call(req).await
///         })
///     }
/// }
/// ```
pub trait Service<Request> {
    /// Responses given by the service.
    type Response;

    /// Errors produced by the service.
    type Error;

    /// The future response value.
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    /// Returns `Poll::Ready(Ok(()))` when the service is able to process requests.
    ///
    /// If the service is at capacity, then `Poll::Pending` is returned and the task
    /// is notified when the service becomes ready again. This function is
    /// expected to be called while on a task. Generally, this can be done with
    /// a simple `futures::future::poll_fn` call.
    ///
    /// If `Poll::Ready(Err(_))` is returned, the service is no longer able to service requests
    /// and the caller should discard the service instance.
    ///
    /// Once `poll_ready` returns `Poll::Ready(Ok(()))`, a request may be dispatched to the
    /// service using `call`. Until a request is dispatched, repeated calls to
    /// `poll_ready` must return either `Poll::Ready(Ok(()))` or `Poll::Ready(Err(_))`.
    ///
    /// Note that `poll_ready` may reserve shared resources that are consumed in a subsequent
    /// invocation of `call`. Thus, it is critical for implementations to not assume that `call`
    /// will always be invoked and to ensure that such resources are released if the service is
    /// dropped before `call` is invoked or the future returned by `call` is dropped before it
    /// is polled.
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Process the request and return the response asynchronously.
    ///
    /// This function is expected to be callable off task. As such,
    /// implementations should take care to not call `poll_ready`.
    ///
    /// Before dispatching a request, `poll_ready` must be called and return
    /// `Poll::Ready(Ok(()))`.
    ///
    /// # Panics
    ///
    /// Implementations are permitted to panic if `call` is invoked without
    /// obtaining `Poll::Ready(Ok(()))` from `poll_ready`.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    fn call(&mut self, req: Request) -> Self::Future;
}

impl<'a, S, Request> Service<Request> for &'a mut S
where
    S: Service<Request> + 'a,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        (**self).poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> S::Future {
        (**self).call(request)
    }
}

impl<S, Request> Service<Request> for Box<S>
where
    S: Service<Request> + ?Sized,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        (**self).poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> S::Future {
        (**self).call(request)
    }
}
