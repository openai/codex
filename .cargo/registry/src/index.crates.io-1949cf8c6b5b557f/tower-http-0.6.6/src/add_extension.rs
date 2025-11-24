//! Middleware that clones a value into each request's [extensions].
//!
//! [extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
//!
//! # Example
//!
//! ```
//! use tower_http::add_extension::AddExtensionLayer;
//! use tower::{Service, ServiceExt, ServiceBuilder, service_fn};
//! use http::{Request, Response};
//! use bytes::Bytes;
//! use http_body_util::Full;
//! use std::{sync::Arc, convert::Infallible};
//!
//! # struct DatabaseConnectionPool;
//! # impl DatabaseConnectionPool {
//! #     fn new() -> DatabaseConnectionPool { DatabaseConnectionPool }
//! # }
//! #
//! // Shared state across all request handlers --- in this case, a pool of database connections.
//! struct State {
//!     pool: DatabaseConnectionPool,
//! }
//!
//! async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     // Grab the state from the request extensions.
//!     let state = req.extensions().get::<Arc<State>>().unwrap();
//!
//!     Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Construct the shared state.
//! let state = State {
//!     pool: DatabaseConnectionPool::new(),
//! };
//!
//! let mut service = ServiceBuilder::new()
//!     // Share an `Arc<State>` with all requests.
//!     .layer(AddExtensionLayer::new(Arc::new(state)))
//!     .service_fn(handle);
//!
//! // Call the service.
//! let response = service
//!     .ready()
//!     .await?
//!     .call(Request::new(Full::default()))
//!     .await?;
//! # Ok(())
//! # }
//! ```

use http::{Request, Response};
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

/// [`Layer`] for adding some shareable value to [request extensions].
///
/// See the [module docs](crate::add_extension) for more details.
///
/// [request extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
#[derive(Clone, Copy, Debug)]
pub struct AddExtensionLayer<T> {
    value: T,
}

impl<T> AddExtensionLayer<T> {
    /// Create a new [`AddExtensionLayer`].
    pub fn new(value: T) -> Self {
        AddExtensionLayer { value }
    }
}

impl<S, T> Layer<S> for AddExtensionLayer<T>
where
    T: Clone,
{
    type Service = AddExtension<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        AddExtension {
            inner,
            value: self.value.clone(),
        }
    }
}

/// Middleware for adding some shareable value to [request extensions].
///
/// See the [module docs](crate::add_extension) for more details.
///
/// [request extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
#[derive(Clone, Copy, Debug)]
pub struct AddExtension<S, T> {
    inner: S,
    value: T,
}

impl<S, T> AddExtension<S, T> {
    /// Create a new [`AddExtension`].
    pub fn new(inner: S, value: T) -> Self {
        Self { inner, value }
    }

    define_inner_service_accessors!();

    /// Returns a new [`Layer`] that wraps services with a `AddExtension` middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(value: T) -> AddExtensionLayer<T> {
        AddExtensionLayer::new(value)
    }
}

impl<ResBody, ReqBody, S, T> Service<Request<ReqBody>> for AddExtension<S, T>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    T: Clone + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        req.extensions_mut().insert(self.value.clone());
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::test_helpers::Body;
    use http::Response;
    use std::{convert::Infallible, sync::Arc};
    use tower::{service_fn, ServiceBuilder, ServiceExt};

    struct State(i32);

    #[tokio::test]
    async fn basic() {
        let state = Arc::new(State(1));

        let svc = ServiceBuilder::new()
            .layer(AddExtensionLayer::new(state))
            .service(service_fn(|req: Request<Body>| async move {
                let state = req.extensions().get::<Arc<State>>().unwrap();
                Ok::<_, Infallible>(Response::new(state.0))
            }));

        let res = svc
            .oneshot(Request::new(Body::empty()))
            .await
            .unwrap()
            .into_body();

        assert_eq!(1, res);
    }
}
