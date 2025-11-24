//! Set a header on the request.
//!
//! The header value to be set may be provided as a fixed value when the
//! middleware is constructed, or determined dynamically based on the request
//! by a closure. See the [`MakeHeaderValue`] trait for details.
//!
//! # Example
//!
//! Setting a header from a fixed value provided when the middleware is constructed:
//!
//! ```
//! use http::{Request, Response, header::{self, HeaderValue}};
//! use tower::{Service, ServiceExt, ServiceBuilder};
//! use tower_http::set_header::SetRequestHeaderLayer;
//! use http_body_util::Full;
//! use bytes::Bytes;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let http_client = tower::service_fn(|_: Request<Full<Bytes>>| async move {
//! #     Ok::<_, std::convert::Infallible>(Response::new(Full::<Bytes>::default()))
//! # });
//! #
//! let mut svc = ServiceBuilder::new()
//!     .layer(
//!         // Layer that sets `User-Agent: my very cool app` on requests.
//!         //
//!         // `if_not_present` will only insert the header if it does not already
//!         // have a value.
//!         SetRequestHeaderLayer::if_not_present(
//!             header::USER_AGENT,
//!             HeaderValue::from_static("my very cool app"),
//!         )
//!     )
//!     .service(http_client);
//!
//! let request = Request::new(Full::default());
//!
//! let response = svc.ready().await?.call(request).await?;
//! #
//! # Ok(())
//! # }
//! ```
//!
//! Setting a header based on a value determined dynamically from the request:
//!
//! ```
//! use http::{Request, Response, header::{self, HeaderValue}};
//! use tower::{Service, ServiceExt, ServiceBuilder};
//! use tower_http::set_header::SetRequestHeaderLayer;
//! use bytes::Bytes;
//! use http_body_util::Full;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let http_client = tower::service_fn(|_: Request<Full<Bytes>>| async move {
//! #     Ok::<_, std::convert::Infallible>(Response::new(Full::<Bytes>::default()))
//! # });
//! fn date_header_value() -> HeaderValue {
//!     // ...
//!     # HeaderValue::from_static("now")
//! }
//!
//! let mut svc = ServiceBuilder::new()
//!     .layer(
//!         // Layer that sets `Date` to the current date and time.
//!         //
//!         // `overriding` will insert the header and override any previous values it
//!         // may have.
//!         SetRequestHeaderLayer::overriding(
//!             header::DATE,
//!             |request: &Request<Full<Bytes>>| {
//!                 Some(date_header_value())
//!             }
//!         )
//!     )
//!     .service(http_client);
//!
//! let request = Request::new(Full::default());
//!
//! let response = svc.ready().await?.call(request).await?;
//! #
//! # Ok(())
//! # }
//! ```

use super::{InsertHeaderMode, MakeHeaderValue};
use http::{header::HeaderName, Request, Response};
use std::{
    fmt,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Layer that applies [`SetRequestHeader`] which adds a request header.
///
/// See [`SetRequestHeader`] for more details.
pub struct SetRequestHeaderLayer<M> {
    header_name: HeaderName,
    make: M,
    mode: InsertHeaderMode,
}

impl<M> fmt::Debug for SetRequestHeaderLayer<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetRequestHeaderLayer")
            .field("header_name", &self.header_name)
            .field("mode", &self.mode)
            .field("make", &std::any::type_name::<M>())
            .finish()
    }
}

impl<M> SetRequestHeaderLayer<M> {
    /// Create a new [`SetRequestHeaderLayer`].
    ///
    /// If a previous value exists for the same header, it is removed and replaced with the new
    /// header value.
    pub fn overriding(header_name: HeaderName, make: M) -> Self {
        Self::new(header_name, make, InsertHeaderMode::Override)
    }

    /// Create a new [`SetRequestHeaderLayer`].
    ///
    /// The new header is always added, preserving any existing values. If previous values exist,
    /// the header will have multiple values.
    pub fn appending(header_name: HeaderName, make: M) -> Self {
        Self::new(header_name, make, InsertHeaderMode::Append)
    }

    /// Create a new [`SetRequestHeaderLayer`].
    ///
    /// If a previous value exists for the header, the new value is not inserted.
    pub fn if_not_present(header_name: HeaderName, make: M) -> Self {
        Self::new(header_name, make, InsertHeaderMode::IfNotPresent)
    }

    fn new(header_name: HeaderName, make: M, mode: InsertHeaderMode) -> Self {
        Self {
            make,
            header_name,
            mode,
        }
    }
}

impl<S, M> Layer<S> for SetRequestHeaderLayer<M>
where
    M: Clone,
{
    type Service = SetRequestHeader<S, M>;

    fn layer(&self, inner: S) -> Self::Service {
        SetRequestHeader {
            inner,
            header_name: self.header_name.clone(),
            make: self.make.clone(),
            mode: self.mode,
        }
    }
}

impl<M> Clone for SetRequestHeaderLayer<M>
where
    M: Clone,
{
    fn clone(&self) -> Self {
        Self {
            make: self.make.clone(),
            header_name: self.header_name.clone(),
            mode: self.mode,
        }
    }
}

/// Middleware that sets a header on the request.
#[derive(Clone)]
pub struct SetRequestHeader<S, M> {
    inner: S,
    header_name: HeaderName,
    make: M,
    mode: InsertHeaderMode,
}

impl<S, M> SetRequestHeader<S, M> {
    /// Create a new [`SetRequestHeader`].
    ///
    /// If a previous value exists for the same header, it is removed and replaced with the new
    /// header value.
    pub fn overriding(inner: S, header_name: HeaderName, make: M) -> Self {
        Self::new(inner, header_name, make, InsertHeaderMode::Override)
    }

    /// Create a new [`SetRequestHeader`].
    ///
    /// The new header is always added, preserving any existing values. If previous values exist,
    /// the header will have multiple values.
    pub fn appending(inner: S, header_name: HeaderName, make: M) -> Self {
        Self::new(inner, header_name, make, InsertHeaderMode::Append)
    }

    /// Create a new [`SetRequestHeader`].
    ///
    /// If a previous value exists for the header, the new value is not inserted.
    pub fn if_not_present(inner: S, header_name: HeaderName, make: M) -> Self {
        Self::new(inner, header_name, make, InsertHeaderMode::IfNotPresent)
    }

    fn new(inner: S, header_name: HeaderName, make: M, mode: InsertHeaderMode) -> Self {
        Self {
            inner,
            header_name,
            make,
            mode,
        }
    }

    define_inner_service_accessors!();
}

impl<S, M> fmt::Debug for SetRequestHeader<S, M>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetRequestHeader")
            .field("inner", &self.inner)
            .field("header_name", &self.header_name)
            .field("mode", &self.mode)
            .field("make", &std::any::type_name::<M>())
            .finish()
    }
}

impl<ReqBody, ResBody, S, M> Service<Request<ReqBody>> for SetRequestHeader<S, M>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    M: MakeHeaderValue<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        self.mode.apply(&self.header_name, &mut req, &mut self.make);
        self.inner.call(req)
    }
}
