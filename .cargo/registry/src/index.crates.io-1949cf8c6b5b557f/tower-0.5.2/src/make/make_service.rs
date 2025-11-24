//! Contains [`MakeService`] which is a trait alias for a [`Service`] of [`Service`]s.

use crate::sealed::Sealed;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::task::{Context, Poll};
use tower_service::Service;

pub(crate) mod shared;

/// Creates new [`Service`] values.
///
/// Acts as a service factory. This is useful for cases where new [`Service`]
/// values must be produced. One case is a TCP server listener. The listener
/// accepts new TCP streams, obtains a new [`Service`] value using the
/// [`MakeService`] trait, and uses that new [`Service`] value to process inbound
/// requests on that new TCP stream.
///
/// This is essentially a trait alias for a [`Service`] of [`Service`]s.
pub trait MakeService<Target, Request>: Sealed<(Target, Request)> {
    /// Responses given by the service
    type Response;

    /// Errors produced by the service
    type Error;

    /// The [`Service`] value created by this factory
    type Service: Service<Request, Response = Self::Response, Error = Self::Error>;

    /// Errors produced while building a service.
    type MakeError;

    /// The future of the [`Service`] instance.
    type Future: Future<Output = Result<Self::Service, Self::MakeError>>;

    /// Returns [`Poll::Ready`] when the factory is able to create more services.
    ///
    /// If the service is at capacity, then [`Poll::Pending`] is returned and the task
    /// is notified when the service becomes ready again. This function is
    /// expected to be called while on a task.
    ///
    /// [`Poll::Ready`]: std::task::Poll::Ready
    /// [`Poll::Pending`]: std::task::Poll::Pending
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::MakeError>>;

    /// Create and return a new service value asynchronously.
    fn make_service(&mut self, target: Target) -> Self::Future;

    /// Consume this [`MakeService`] and convert it into a [`Service`].
    ///
    /// # Example
    /// ```
    /// use std::convert::Infallible;
    /// use tower::Service;
    /// use tower::make::MakeService;
    /// use tower::service_fn;
    ///
    /// # fn main() {
    /// # async {
    /// // A `MakeService`
    /// let make_service = service_fn(|make_req: ()| async {
    ///     Ok::<_, Infallible>(service_fn(|req: String| async {
    ///         Ok::<_, Infallible>(req)
    ///     }))
    /// });
    ///
    /// // Convert the `MakeService` into a `Service`
    /// let mut svc = make_service.into_service();
    ///
    /// // Make a new service
    /// let mut new_svc = svc.call(()).await.unwrap();
    ///
    /// // Call the service
    /// let res = new_svc.call("foo".to_string()).await.unwrap();
    /// # };
    /// # }
    /// ```
    fn into_service(self) -> IntoService<Self, Request>
    where
        Self: Sized,
    {
        IntoService {
            make: self,
            _marker: PhantomData,
        }
    }

    /// Convert this [`MakeService`] into a [`Service`] without consuming the original [`MakeService`].
    ///
    /// # Example
    /// ```
    /// use std::convert::Infallible;
    /// use tower::Service;
    /// use tower::make::MakeService;
    /// use tower::service_fn;
    ///
    /// # fn main() {
    /// # async {
    /// // A `MakeService`
    /// let mut make_service = service_fn(|make_req: ()| async {
    ///     Ok::<_, Infallible>(service_fn(|req: String| async {
    ///         Ok::<_, Infallible>(req)
    ///     }))
    /// });
    ///
    /// // Convert the `MakeService` into a `Service`
    /// let mut svc = make_service.as_service();
    ///
    /// // Make a new service
    /// let mut new_svc = svc.call(()).await.unwrap();
    ///
    /// // Call the service
    /// let res = new_svc.call("foo".to_string()).await.unwrap();
    ///
    /// // The original `MakeService` is still accessible
    /// let new_svc = make_service.make_service(()).await.unwrap();
    /// # };
    /// # }
    /// ```
    fn as_service(&mut self) -> AsService<Self, Request>
    where
        Self: Sized,
    {
        AsService {
            make: self,
            _marker: PhantomData,
        }
    }
}

impl<M, S, Target, Request> Sealed<(Target, Request)> for M
where
    M: Service<Target, Response = S>,
    S: Service<Request>,
{
}

impl<M, S, Target, Request> MakeService<Target, Request> for M
where
    M: Service<Target, Response = S>,
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Service = S;
    type MakeError = M::Error;
    type Future = M::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::MakeError>> {
        Service::poll_ready(self, cx)
    }

    fn make_service(&mut self, target: Target) -> Self::Future {
        Service::call(self, target)
    }
}

/// Service returned by [`MakeService::into_service`][into].
///
/// See the documentation on [`into_service`][into] for details.
///
/// [into]: MakeService::into_service
pub struct IntoService<M, Request> {
    make: M,
    _marker: PhantomData<Request>,
}

impl<M, Request> Clone for IntoService<M, Request>
where
    M: Clone,
{
    fn clone(&self) -> Self {
        Self {
            make: self.make.clone(),
            _marker: PhantomData,
        }
    }
}

impl<M, Request> fmt::Debug for IntoService<M, Request>
where
    M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoService")
            .field("make", &self.make)
            .finish()
    }
}

impl<M, S, Target, Request> Service<Target> for IntoService<M, Request>
where
    M: Service<Target, Response = S>,
    S: Service<Request>,
{
    type Response = M::Response;
    type Error = M::Error;
    type Future = M::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.make.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, target: Target) -> Self::Future {
        self.make.make_service(target)
    }
}

/// Service returned by [`MakeService::as_service`][as].
///
/// See the documentation on [`as_service`][as] for details.
///
/// [as]: MakeService::as_service
pub struct AsService<'a, M, Request> {
    make: &'a mut M,
    _marker: PhantomData<Request>,
}

impl<M, Request> fmt::Debug for AsService<'_, M, Request>
where
    M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsService")
            .field("make", &self.make)
            .finish()
    }
}

impl<M, S, Target, Request> Service<Target> for AsService<'_, M, Request>
where
    M: Service<Target, Response = S>,
    S: Service<Request>,
{
    type Response = M::Response;
    type Error = M::Error;
    type Future = M::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.make.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, target: Target) -> Self::Future {
        self.make.make_service(target)
    }
}
