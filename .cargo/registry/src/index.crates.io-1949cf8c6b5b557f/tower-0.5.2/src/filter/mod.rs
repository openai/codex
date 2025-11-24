//! Conditionally dispatch requests to the inner service based on the result of
//! a predicate.
//!
//! A predicate takes some request type and returns a `Result<Request, Error>`.
//! If the predicate returns [`Ok`], the inner service is called with the request
//! returned by the predicate &mdash; which may be the original request or a
//! modified one. If the predicate returns [`Err`], the request is rejected and
//! the inner service is not called.
//!
//! Predicates may either be synchronous (simple functions from a `Request` to
//! a [`Result`]) or asynchronous (functions returning [`Future`]s). Separate
//! traits, [`Predicate`] and [`AsyncPredicate`], represent these two types of
//! predicate. Note that when it is not necessary to await some other
//! asynchronous operation in the predicate, the synchronous predicate should be
//! preferred, as it introduces less overhead.
//!
//! The predicate traits are implemented for closures and function pointers.
//! However, users may also implement them for other types, such as when the
//! predicate requires some state carried between requests. For example,
//! [`Predicate`] could be implemented for a type that rejects a fixed set of
//! requests by checking if they are contained by a a [`HashSet`] or other
//! collection.
//!
//! [`Future`]: std::future::Future
//! [`HashSet`]: std::collections::HashSet
pub mod future;
mod layer;
mod predicate;

pub use self::{
    layer::{AsyncFilterLayer, FilterLayer},
    predicate::{AsyncPredicate, Predicate},
};

use self::future::{AsyncResponseFuture, ResponseFuture};
use crate::BoxError;
use futures_util::{future::Either, TryFutureExt};
use std::task::{Context, Poll};
use tower_service::Service;

/// Conditionally dispatch requests to the inner service based on a [predicate].
///
/// [predicate]: Predicate
#[derive(Clone, Debug)]
pub struct Filter<T, U> {
    inner: T,
    predicate: U,
}

/// Conditionally dispatch requests to the inner service based on an
/// [asynchronous predicate].
///
/// [asynchronous predicate]: AsyncPredicate
#[derive(Clone, Debug)]
pub struct AsyncFilter<T, U> {
    inner: T,
    predicate: U,
}

// ==== impl Filter ====

impl<T, U> Filter<T, U> {
    /// Returns a new [`Filter`] service wrapping `inner`.
    pub const fn new(inner: T, predicate: U) -> Self {
        Self { inner, predicate }
    }

    /// Returns a new [`Layer`] that wraps services with a [`Filter`] service
    /// with the given [`Predicate`].
    ///
    /// [`Layer`]: crate::Layer
    pub fn layer(predicate: U) -> FilterLayer<U> {
        FilterLayer::new(predicate)
    }

    /// Check a `Request` value against this filter's predicate.
    pub fn check<R>(&mut self, request: R) -> Result<U::Request, BoxError>
    where
        U: Predicate<R>,
    {
        self.predicate.check(request)
    }

    /// Get a reference to the inner service
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner service
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consume `self`, returning the inner service
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U, Request> Service<Request> for Filter<T, U>
where
    U: Predicate<Request>,
    T: Service<U::Request>,
    T::Error: Into<BoxError>,
{
    type Response = T::Response;
    type Error = BoxError;
    type Future = ResponseFuture<T::Response, T::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        ResponseFuture::new(match self.predicate.check(request) {
            Ok(request) => Either::Right(self.inner.call(request).err_into()),
            Err(e) => Either::Left(futures_util::future::ready(Err(e))),
        })
    }
}

// ==== impl AsyncFilter ====

impl<T, U> AsyncFilter<T, U> {
    /// Returns a new [`AsyncFilter`] service wrapping `inner`.
    pub const fn new(inner: T, predicate: U) -> Self {
        Self { inner, predicate }
    }

    /// Returns a new [`Layer`] that wraps services with an [`AsyncFilter`]
    /// service with the given [`AsyncPredicate`].
    ///
    /// [`Layer`]: crate::Layer
    pub fn layer(predicate: U) -> FilterLayer<U> {
        FilterLayer::new(predicate)
    }

    /// Check a `Request` value against this filter's predicate.
    pub async fn check<R>(&mut self, request: R) -> Result<U::Request, BoxError>
    where
        U: AsyncPredicate<R>,
    {
        self.predicate.check(request).await
    }

    /// Get a reference to the inner service
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner service
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consume `self`, returning the inner service
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U, Request> Service<Request> for AsyncFilter<T, U>
where
    U: AsyncPredicate<Request>,
    T: Service<U::Request> + Clone,
    T::Error: Into<BoxError>,
{
    type Response = T::Response;
    type Error = BoxError;
    type Future = AsyncResponseFuture<U, T, Request>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        use std::mem;

        let inner = self.inner.clone();
        // In case the inner service has state that's driven to readiness and
        // not tracked by clones (such as `Buffer`), pass the version we have
        // already called `poll_ready` on into the future, and leave its clone
        // behind.
        let inner = mem::replace(&mut self.inner, inner);

        // Check the request
        let check = self.predicate.check(request);

        AsyncResponseFuture::new(check, inner)
    }
}
