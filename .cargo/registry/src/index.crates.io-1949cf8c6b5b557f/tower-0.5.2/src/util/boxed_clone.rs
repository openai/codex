use super::ServiceExt;
use futures_util::future::BoxFuture;
use std::{
    fmt,
    task::{Context, Poll},
};
use tower_layer::{layer_fn, LayerFn};
use tower_service::Service;

/// A [`Clone`] + [`Send`] boxed [`Service`].
///
/// [`BoxCloneService`] turns a service into a trait object, allowing the
/// response future type to be dynamic, and allowing the service to be cloned.
///
/// This is similar to [`BoxService`](super::BoxService) except the resulting
/// service implements [`Clone`].
///
/// # Example
///
/// ```
/// use tower::{Service, ServiceBuilder, BoxError, util::BoxCloneService};
/// use std::time::Duration;
/// #
/// # struct Request;
/// # struct Response;
/// # impl Response {
/// #     fn new() -> Self { Self }
/// # }
///
/// // This service has a complex type that is hard to name
/// let service = ServiceBuilder::new()
///     .map_request(|req| {
///         println!("received request");
///         req
///     })
///     .map_response(|res| {
///         println!("response produced");
///         res
///     })
///     .load_shed()
///     .concurrency_limit(64)
///     .timeout(Duration::from_secs(10))
///     .service_fn(|req: Request| async {
///         Ok::<_, BoxError>(Response::new())
///     });
/// # let service = assert_service(service);
///
/// // `BoxCloneService` will erase the type so it's nameable
/// let service: BoxCloneService<Request, Response, BoxError> = BoxCloneService::new(service);
/// # let service = assert_service(service);
///
/// // And we can still clone the service
/// let cloned_service = service.clone();
/// #
/// # fn assert_service<S, R>(svc: S) -> S
/// # where S: Service<R> { svc }
/// ```
pub struct BoxCloneService<T, U, E>(
    Box<
        dyn CloneService<T, Response = U, Error = E, Future = BoxFuture<'static, Result<U, E>>>
            + Send,
    >,
);

impl<T, U, E> BoxCloneService<T, U, E> {
    /// Create a new `BoxCloneService`.
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        BoxCloneService(Box::new(inner))
    }

    /// Returns a [`Layer`] for wrapping a [`Service`] in a [`BoxCloneService`]
    /// middleware.
    ///
    /// [`Layer`]: crate::Layer
    pub fn layer<S>() -> LayerFn<fn(S) -> Self>
    where
        S: Service<T, Response = U, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        layer_fn(Self::new)
    }
}

impl<T, U, E> Service<T> for BoxCloneService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxFuture<'static, Result<U, E>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        self.0.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, request: T) -> Self::Future {
        self.0.call(request)
    }
}

impl<T, U, E> Clone for BoxCloneService<T, U, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait CloneService<R>: Service<R> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneService<R, Response = Self::Response, Error = Self::Error, Future = Self::Future>
            + Send,
    >;
}

impl<R, T> CloneService<R> for T
where
    T: Service<R> + Send + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<dyn CloneService<R, Response = T::Response, Error = T::Error, Future = T::Future> + Send>
    {
        Box::new(self.clone())
    }
}

impl<T, U, E> fmt::Debug for BoxCloneService<T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BoxCloneService").finish()
    }
}
