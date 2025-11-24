//! Middleware that normalizes paths.
//!
//! # Example
//!
//! ```
//! use tower_http::normalize_path::NormalizePathLayer;
//! use http::{Request, Response, StatusCode};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use std::{iter::once, convert::Infallible};
//! use tower::{ServiceBuilder, Service, ServiceExt};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     // `req.uri().path()` will not have trailing slashes
//!     # Ok(Response::new(Full::default()))
//! }
//!
//! let mut service = ServiceBuilder::new()
//!     // trim trailing slashes from paths
//!     .layer(NormalizePathLayer::trim_trailing_slash())
//!     .service_fn(handle);
//!
//! // call the service
//! let request = Request::builder()
//!     // `handle` will see `/foo`
//!     .uri("/foo/")
//!     .body(Full::default())?;
//!
//! service.ready().await?.call(request).await?;
//! #
//! # Ok(())
//! # }
//! ```

use http::{Request, Response, Uri};
use std::{
    borrow::Cow,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Different modes of normalizing paths
#[derive(Debug, Copy, Clone)]
enum NormalizeMode {
    /// Normalizes paths by trimming the trailing slashes, e.g. /foo/ -> /foo
    Trim,
    /// Normalizes paths by appending trailing slash, e.g. /foo -> /foo/
    Append,
}

/// Layer that applies [`NormalizePath`] which normalizes paths.
///
/// See the [module docs](self) for more details.
#[derive(Debug, Copy, Clone)]
pub struct NormalizePathLayer {
    mode: NormalizeMode,
}

impl NormalizePathLayer {
    /// Create a new [`NormalizePathLayer`].
    ///
    /// Any trailing slashes from request paths will be removed. For example, a request with `/foo/`
    /// will be changed to `/foo` before reaching the inner service.
    pub fn trim_trailing_slash() -> Self {
        NormalizePathLayer {
            mode: NormalizeMode::Trim,
        }
    }

    /// Create a new [`NormalizePathLayer`].
    ///
    /// Request paths without trailing slash will be appended with a trailing slash. For example, a request with `/foo`
    /// will be changed to `/foo/` before reaching the inner service.
    pub fn append_trailing_slash() -> Self {
        NormalizePathLayer {
            mode: NormalizeMode::Append,
        }
    }
}

impl<S> Layer<S> for NormalizePathLayer {
    type Service = NormalizePath<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NormalizePath {
            mode: self.mode,
            inner,
        }
    }
}

/// Middleware that normalizes paths.
///
/// See the [module docs](self) for more details.
#[derive(Debug, Copy, Clone)]
pub struct NormalizePath<S> {
    mode: NormalizeMode,
    inner: S,
}

impl<S> NormalizePath<S> {
    /// Construct a new [`NormalizePath`] with trim mode.
    pub fn trim_trailing_slash(inner: S) -> Self {
        Self {
            mode: NormalizeMode::Trim,
            inner,
        }
    }

    /// Construct a new [`NormalizePath`] with append mode.
    pub fn append_trailing_slash(inner: S) -> Self {
        Self {
            mode: NormalizeMode::Append,
            inner,
        }
    }

    define_inner_service_accessors!();
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for NormalizePath<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        match self.mode {
            NormalizeMode::Trim => trim_trailing_slash(req.uri_mut()),
            NormalizeMode::Append => append_trailing_slash(req.uri_mut()),
        }
        self.inner.call(req)
    }
}

fn trim_trailing_slash(uri: &mut Uri) {
    if !uri.path().ends_with('/') && !uri.path().starts_with("//") {
        return;
    }

    let new_path = format!("/{}", uri.path().trim_matches('/'));

    let mut parts = uri.clone().into_parts();

    let new_path_and_query = if let Some(path_and_query) = &parts.path_and_query {
        let new_path_and_query = if let Some(query) = path_and_query.query() {
            Cow::Owned(format!("{}?{}", new_path, query))
        } else {
            new_path.into()
        }
        .parse()
        .unwrap();

        Some(new_path_and_query)
    } else {
        None
    };

    parts.path_and_query = new_path_and_query;
    if let Ok(new_uri) = Uri::from_parts(parts) {
        *uri = new_uri;
    }
}

fn append_trailing_slash(uri: &mut Uri) {
    if uri.path().ends_with("/") && !uri.path().ends_with("//") {
        return;
    }

    let trimmed = uri.path().trim_matches('/');
    let new_path = if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{trimmed}/")
    };

    let mut parts = uri.clone().into_parts();

    let new_path_and_query = if let Some(path_and_query) = &parts.path_and_query {
        let new_path_and_query = if let Some(query) = path_and_query.query() {
            Cow::Owned(format!("{new_path}?{query}"))
        } else {
            new_path.into()
        }
        .parse()
        .unwrap();

        Some(new_path_and_query)
    } else {
        Some(new_path.parse().unwrap())
    };

    parts.path_and_query = new_path_and_query;
    if let Ok(new_uri) = Uri::from_parts(parts) {
        *uri = new_uri;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use tower::{ServiceBuilder, ServiceExt};

    #[tokio::test]
    async fn trim_works() {
        async fn handle(request: Request<()>) -> Result<Response<String>, Infallible> {
            Ok(Response::new(request.uri().to_string()))
        }

        let mut svc = ServiceBuilder::new()
            .layer(NormalizePathLayer::trim_trailing_slash())
            .service_fn(handle);

        let body = svc
            .ready()
            .await
            .unwrap()
            .call(Request::builder().uri("/foo/").body(()).unwrap())
            .await
            .unwrap()
            .into_body();

        assert_eq!(body, "/foo");
    }

    #[test]
    fn is_noop_if_no_trailing_slash() {
        let mut uri = "/foo".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo");
    }

    #[test]
    fn maintains_query() {
        let mut uri = "/foo/?a=a".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo?a=a");
    }

    #[test]
    fn removes_multiple_trailing_slashes() {
        let mut uri = "/foo////".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo");
    }

    #[test]
    fn removes_multiple_trailing_slashes_even_with_query() {
        let mut uri = "/foo////?a=a".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo?a=a");
    }

    #[test]
    fn is_noop_on_index() {
        let mut uri = "/".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/");
    }

    #[test]
    fn removes_multiple_trailing_slashes_on_index() {
        let mut uri = "////".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/");
    }

    #[test]
    fn removes_multiple_trailing_slashes_on_index_even_with_query() {
        let mut uri = "////?a=a".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/?a=a");
    }

    #[test]
    fn removes_multiple_preceding_slashes_even_with_query() {
        let mut uri = "///foo//?a=a".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo?a=a");
    }

    #[test]
    fn removes_multiple_preceding_slashes() {
        let mut uri = "///foo".parse::<Uri>().unwrap();
        trim_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo");
    }

    #[tokio::test]
    async fn append_works() {
        async fn handle(request: Request<()>) -> Result<Response<String>, Infallible> {
            Ok(Response::new(request.uri().to_string()))
        }

        let mut svc = ServiceBuilder::new()
            .layer(NormalizePathLayer::append_trailing_slash())
            .service_fn(handle);

        let body = svc
            .ready()
            .await
            .unwrap()
            .call(Request::builder().uri("/foo").body(()).unwrap())
            .await
            .unwrap()
            .into_body();

        assert_eq!(body, "/foo/");
    }

    #[test]
    fn is_noop_if_trailing_slash() {
        let mut uri = "/foo/".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo/");
    }

    #[test]
    fn append_maintains_query() {
        let mut uri = "/foo?a=a".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo/?a=a");
    }

    #[test]
    fn append_only_keeps_one_slash() {
        let mut uri = "/foo////".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo/");
    }

    #[test]
    fn append_only_keeps_one_slash_even_with_query() {
        let mut uri = "/foo////?a=a".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo/?a=a");
    }

    #[test]
    fn append_is_noop_on_index() {
        let mut uri = "/".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/");
    }

    #[test]
    fn append_removes_multiple_trailing_slashes_on_index() {
        let mut uri = "////".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/");
    }

    #[test]
    fn append_removes_multiple_trailing_slashes_on_index_even_with_query() {
        let mut uri = "////?a=a".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/?a=a");
    }

    #[test]
    fn append_removes_multiple_preceding_slashes_even_with_query() {
        let mut uri = "///foo//?a=a".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo/?a=a");
    }

    #[test]
    fn append_removes_multiple_preceding_slashes() {
        let mut uri = "///foo".parse::<Uri>().unwrap();
        append_trailing_slash(&mut uri);
        assert_eq!(uri, "/foo/");
    }
}
