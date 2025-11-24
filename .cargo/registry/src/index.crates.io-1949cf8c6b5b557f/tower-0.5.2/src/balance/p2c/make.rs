use super::Balance;
use crate::discover::Discover;
use futures_core::ready;
use pin_project_lite::pin_project;
use std::hash::Hash;
use std::marker::PhantomData;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

/// Constructs load balancers over dynamic service sets produced by a wrapped "inner" service.
///
/// This is effectively an implementation of [`MakeService`] except that it forwards the service
/// descriptors (`Target`) to an inner service (`S`), and expects that service to produce a
/// service set in the form of a [`Discover`]. It then wraps the service set in a [`Balance`]
/// before returning it as the "made" service.
///
/// See the [module-level documentation](crate::balance) for details on load balancing.
///
/// [`MakeService`]: crate::MakeService
/// [`Discover`]: crate::discover::Discover
/// [`Balance`]: crate::balance::p2c::Balance
pub struct MakeBalance<S, Req> {
    inner: S,
    _marker: PhantomData<fn(Req)>,
}

pin_project! {
    /// A [`Balance`] in the making.
    ///
    /// [`Balance`]: crate::balance::p2c::Balance
    pub struct MakeFuture<F, Req> {
        #[pin]
        inner: F,
        _marker: PhantomData<fn(Req)>,
    }
}

impl<S, Req> MakeBalance<S, Req> {
    /// Build balancers using operating system entropy.
    pub const fn new(make_discover: S) -> Self {
        Self {
            inner: make_discover,
            _marker: PhantomData,
        }
    }
}

impl<S, Req> Clone for MakeBalance<S, Req>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<S, Target, Req> Service<Target> for MakeBalance<S, Req>
where
    S: Service<Target>,
    S::Response: Discover,
    <S::Response as Discover>::Key: Hash,
    <S::Response as Discover>::Service: Service<Req>,
    <<S::Response as Discover>::Service as Service<Req>>::Error: Into<crate::BoxError>,
{
    type Response = Balance<S::Response, Req>;
    type Error = S::Error;
    type Future = MakeFuture<S::Future, Req>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, target: Target) -> Self::Future {
        MakeFuture {
            inner: self.inner.call(target),
            _marker: PhantomData,
        }
    }
}

impl<S, Req> fmt::Debug for MakeBalance<S, Req>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { inner, _marker } = self;
        f.debug_struct("MakeBalance").field("inner", inner).finish()
    }
}

impl<F, T, E, Req> Future for MakeFuture<F, Req>
where
    F: Future<Output = Result<T, E>>,
    T: Discover,
    <T as Discover>::Key: Hash,
    <T as Discover>::Service: Service<Req>,
    <<T as Discover>::Service as Service<Req>>::Error: Into<crate::BoxError>,
{
    type Output = Result<Balance<T, Req>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let inner = ready!(this.inner.poll(cx))?;
        let svc = Balance::new(inner);
        Poll::Ready(Ok(svc))
    }
}

impl<F, Req> fmt::Debug for MakeFuture<F, Req>
where
    F: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { inner, _marker } = self;
        f.debug_struct("MakeFuture").field("inner", inner).finish()
    }
}
