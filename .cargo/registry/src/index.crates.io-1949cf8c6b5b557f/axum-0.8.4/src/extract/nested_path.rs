use std::{
    sync::Arc,
    task::{Context, Poll},
};

use crate::extract::Request;
use axum_core::extract::FromRequestParts;
use http::request::Parts;
use tower_layer::{layer_fn, Layer};
use tower_service::Service;

use super::rejection::NestedPathRejection;

/// Access the path the matched the route is nested at.
///
/// This can for example be used when doing redirects.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     extract::NestedPath,
///     routing::get,
/// };
///
/// let api = Router::new().route(
///     "/users",
///     get(|path: NestedPath| async move {
///         // `path` will be "/api" because that's what this
///         // router is nested at when we build `app`
///         let path = path.as_str();
///     })
/// );
///
/// let app = Router::new().nest("/api", api);
/// # let _: Router = app;
/// ```
#[derive(Debug, Clone)]
pub struct NestedPath(Arc<str>);

impl NestedPath {
    /// Returns a `str` representation of the path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S> FromRequestParts<S> for NestedPath
where
    S: Send + Sync,
{
    type Rejection = NestedPathRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.extensions.get::<Self>() {
            Some(nested_path) => Ok(nested_path.clone()),
            None => Err(NestedPathRejection),
        }
    }
}

#[derive(Clone)]
pub(crate) struct SetNestedPath<S> {
    inner: S,
    path: Arc<str>,
}

impl<S> SetNestedPath<S> {
    pub(crate) fn layer(path: &str) -> impl Layer<S, Service = Self> + Clone {
        let path = Arc::from(path);
        layer_fn(move |inner| Self {
            inner,
            path: Arc::clone(&path),
        })
    }
}

impl<S, B> Service<Request<B>> for SetNestedPath<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some(prev) = req.extensions_mut().get_mut::<NestedPath>() {
            let new_path = if prev.as_str() == "/" {
                Arc::clone(&self.path)
            } else {
                format!("{}{}", prev.as_str().trim_end_matches('/'), self.path).into()
            };
            prev.0 = new_path;
        } else {
            req.extensions_mut()
                .insert(NestedPath(Arc::clone(&self.path)));
        };

        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use axum_core::response::Response;
    use http::StatusCode;

    use crate::{
        extract::{NestedPath, Request},
        middleware::{from_fn, Next},
        routing::get,
        test_helpers::*,
        Router,
    };

    #[crate::test]
    async fn one_level_of_nesting() {
        let api = Router::new().route(
            "/users",
            get(|nested_path: NestedPath| {
                assert_eq!(nested_path.as_str(), "/api");
                async {}
            }),
        );

        let app = Router::new().nest("/api", api);

        let client = TestClient::new(app);

        let res = client.get("/api/users").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[crate::test]
    async fn one_level_of_nesting_with_trailing_slash() {
        let api = Router::new().route(
            "/users",
            get(|nested_path: NestedPath| {
                assert_eq!(nested_path.as_str(), "/api/");
                async {}
            }),
        );

        let app = Router::new().nest("/api/", api);

        let client = TestClient::new(app);

        let res = client.get("/api/users").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[crate::test]
    async fn two_levels_of_nesting() {
        let api = Router::new().route(
            "/users",
            get(|nested_path: NestedPath| {
                assert_eq!(nested_path.as_str(), "/api/v2");
                async {}
            }),
        );

        let app = Router::new().nest("/api", Router::new().nest("/v2", api));

        let client = TestClient::new(app);

        let res = client.get("/api/v2/users").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[crate::test]
    async fn two_levels_of_nesting_with_trailing_slash() {
        let api = Router::new().route(
            "/users",
            get(|nested_path: NestedPath| {
                assert_eq!(nested_path.as_str(), "/api/v2");
                async {}
            }),
        );

        let app = Router::new().nest("/api/", Router::new().nest("/v2", api));

        let client = TestClient::new(app);

        let res = client.get("/api/v2/users").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[crate::test]
    async fn in_fallbacks() {
        let api = Router::new().fallback(get(|nested_path: NestedPath| {
            assert_eq!(nested_path.as_str(), "/api");
            async {}
        }));

        let app = Router::new().nest("/api", api);

        let client = TestClient::new(app);

        let res = client.get("/api/doesnt-exist").await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[crate::test]
    async fn in_middleware() {
        async fn middleware(nested_path: NestedPath, req: Request, next: Next) -> Response {
            assert_eq!(nested_path.as_str(), "/api");
            next.run(req).await
        }

        let api = Router::new()
            .route("/users", get(|| async {}))
            .layer(from_fn(middleware));

        let app = Router::new().nest("/api", api);

        let client = TestClient::new(app);

        let res = client.get("/api/users").await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
