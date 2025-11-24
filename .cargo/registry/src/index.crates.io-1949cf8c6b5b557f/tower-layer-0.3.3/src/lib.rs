#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![forbid(unsafe_code)]
// `rustdoc::broken_intra_doc_links` is checked on CI

//! Layer traits and extensions.
//!
//! A layer decorates an service and provides additional functionality. It
//! allows other services to be composed with the service that implements layer.
//!
//! A middleware implements the [`Layer`] and [`Service`] trait.
//!
//! [`Service`]: https://docs.rs/tower/*/tower/trait.Service.html

mod identity;
mod layer_fn;
mod stack;
mod tuple;

pub use self::{
    identity::Identity,
    layer_fn::{layer_fn, LayerFn},
    stack::Stack,
};

/// Decorates a [`Service`], transforming either the request or the response.
///
/// Often, many of the pieces needed for writing network applications can be
/// reused across multiple services. The `Layer` trait can be used to write
/// reusable components that can be applied to very different kinds of services;
/// for example, it can be applied to services operating on different protocols,
/// and to both the client and server side of a network transaction.
///
/// # Log
///
/// Take request logging as an example:
///
/// ```rust
/// # use tower_service::Service;
/// # use std::task::{Poll, Context};
/// # use tower_layer::Layer;
/// # use std::fmt;
///
/// pub struct LogLayer {
///     target: &'static str,
/// }
///
/// impl<S> Layer<S> for LogLayer {
///     type Service = LogService<S>;
///
///     fn layer(&self, service: S) -> Self::Service {
///         LogService {
///             target: self.target,
///             service
///         }
///     }
/// }
///
/// // This service implements the Log behavior
/// pub struct LogService<S> {
///     target: &'static str,
///     service: S,
/// }
///
/// impl<S, Request> Service<Request> for LogService<S>
/// where
///     S: Service<Request>,
///     Request: fmt::Debug,
/// {
///     type Response = S::Response;
///     type Error = S::Error;
///     type Future = S::Future;
///
///     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
///         self.service.poll_ready(cx)
///     }
///
///     fn call(&mut self, request: Request) -> Self::Future {
///         // Insert log statement here or other functionality
///         println!("request = {:?}, target = {:?}", request, self.target);
///         self.service.call(request)
///     }
/// }
/// ```
///
/// The above log implementation is decoupled from the underlying protocol and
/// is also decoupled from client or server concerns. In other words, the same
/// log middleware could be used in either a client or a server.
///
/// [`Service`]: https://docs.rs/tower/*/tower/trait.Service.html
pub trait Layer<S> {
    /// The wrapped service
    type Service;
    /// Wrap the given service with the middleware, returning a new service
    /// that has been decorated with the middleware.
    fn layer(&self, inner: S) -> Self::Service;
}

impl<'a, T, S> Layer<S> for &'a T
where
    T: ?Sized + Layer<S>,
{
    type Service = T::Service;

    fn layer(&self, inner: S) -> Self::Service {
        (**self).layer(inner)
    }
}
