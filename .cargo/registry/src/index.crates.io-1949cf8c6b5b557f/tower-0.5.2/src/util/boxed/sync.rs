use crate::ServiceExt;
use tower_layer::{layer_fn, LayerFn};
use tower_service::Service;

use sync_wrapper::SyncWrapper;

use std::fmt;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// A boxed `Service + Send` trait object.
///
/// [`BoxService`] turns a service into a trait object, allowing the response
/// future type to be dynamic. This type requires both the service and the
/// response future to be [`Send`].
///
/// If you need a boxed [`Service`] that implements [`Clone`] consider using
/// [`BoxCloneService`](crate::util::BoxCloneService).
///
/// Dynamically dispatched [`Service`] objects allow for erasing the underlying
/// [`Service`] type and using the `Service` instances as opaque handles. This can
/// be useful when the service instance cannot be explicitly named for whatever
/// reason.
///
/// # Examples
///
/// ```
/// use futures_util::future::ready;
/// # use tower_service::Service;
/// # use tower::util::{BoxService, service_fn};
/// // Respond to requests using a closure, but closures cannot be named...
/// # pub fn main() {
/// let svc = service_fn(|mut request: String| {
///     request.push_str(" response");
///     ready(Ok(request))
/// });
///
/// let service: BoxService<String, String, ()> = BoxService::new(svc);
/// # drop(service);
/// }
/// ```
///
/// [`Service`]: crate::Service
/// [`Rc`]: std::rc::Rc
pub struct BoxService<T, U, E> {
    inner:
        SyncWrapper<Box<dyn Service<T, Response = U, Error = E, Future = BoxFuture<U, E>> + Send>>,
}

/// A boxed `Future + Send` trait object.
///
/// This type alias represents a boxed future that is [`Send`] and can be moved
/// across threads.
type BoxFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

impl<T, U, E> BoxService<T, U, E> {
    #[allow(missing_docs)]
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Send + 'static,
        S::Future: Send + 'static,
    {
        // rust can't infer the type
        let inner: Box<dyn Service<T, Response = U, Error = E, Future = BoxFuture<U, E>> + Send> =
            Box::new(inner.map_future(|f: S::Future| Box::pin(f) as _));
        let inner = SyncWrapper::new(inner);
        BoxService { inner }
    }

    /// Returns a [`Layer`] for wrapping a [`Service`] in a [`BoxService`]
    /// middleware.
    ///
    /// [`Layer`]: crate::Layer
    pub fn layer<S>() -> LayerFn<fn(S) -> Self>
    where
        S: Service<T, Response = U, Error = E> + Send + 'static,
        S::Future: Send + 'static,
    {
        layer_fn(Self::new)
    }
}

impl<T, U, E> Service<T> for BoxService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = BoxFuture<U, E>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        self.inner.get_mut().poll_ready(cx)
    }

    fn call(&mut self, request: T) -> BoxFuture<U, E> {
        self.inner.get_mut().call(request)
    }
}

impl<T, U, E> fmt::Debug for BoxService<T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BoxService").finish()
    }
}

#[test]
fn is_sync() {
    fn assert_sync<T: Sync>() {}

    assert_sync::<BoxService<(), (), ()>>();
}
