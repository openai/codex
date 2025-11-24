use crate::{extract::rejection::*, response::IntoResponseParts};
use axum_core::extract::OptionalFromRequestParts;
use axum_core::{
    extract::FromRequestParts,
    response::{IntoResponse, Response, ResponseParts},
};
use http::{request::Parts, Extensions, Request};
use std::{
    convert::Infallible,
    task::{Context, Poll},
};
use tower_service::Service;

/// Extractor and response for extensions.
///
/// # As extractor
///
/// This is commonly used to share state across handlers.
///
/// ```rust,no_run
/// use axum::{
///     Router,
///     Extension,
///     routing::get,
/// };
/// use std::sync::Arc;
///
/// // Some shared state used throughout our application
/// struct State {
///     // ...
/// }
///
/// async fn handler(state: Extension<Arc<State>>) {
///     // ...
/// }
///
/// let state = Arc::new(State { /* ... */ });
///
/// let app = Router::new().route("/", get(handler))
///     // Add middleware that inserts the state into all incoming request's
///     // extensions.
///     .layer(Extension(state));
/// # let _: Router = app;
/// ```
///
/// If the extension is missing it will reject the request with a `500 Internal
/// Server Error` response. Alternatively, you can use `Option<Extension<T>>` to
/// make the extension extractor optional.
///
/// # As response
///
/// Response extensions can be used to share state with middleware.
///
/// ```rust
/// use axum::{
///     Extension,
///     response::IntoResponse,
/// };
///
/// async fn handler() -> (Extension<Foo>, &'static str) {
///     (
///         Extension(Foo("foo")),
///         "Hello, World!"
///     )
/// }
///
/// #[derive(Clone)]
/// struct Foo(&'static str);
/// ```
#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Extension<T>(pub T);

impl<T> Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn from_extensions(extensions: &Extensions) -> Option<Self> {
        extensions.get::<T>().cloned().map(Extension)
    }
}

impl<T, S> FromRequestParts<S> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = ExtensionRejection;

    async fn from_request_parts(req: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_extensions(&req.extensions).ok_or_else(|| {
            MissingExtension::from_err(format!(
                "Extension of type `{}` was not found. Perhaps you forgot to add it? See `axum::Extension`.",
                std::any::type_name::<T>()
            ))
        })?)
    }
}

impl<T, S> OptionalFromRequestParts<S> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        req: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(Self::from_extensions(&req.extensions))
    }
}

axum_core::__impl_deref!(Extension);

impl<T> IntoResponseParts for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        res.extensions_mut().insert(self.0);
        Ok(res)
    }
}

impl<T> IntoResponse for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        res.extensions_mut().insert(self.0);
        res
    }
}

impl<S, T> tower_layer::Layer<S> for Extension<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Service = AddExtension<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        AddExtension {
            inner,
            value: self.0.clone(),
        }
    }
}

/// Middleware for adding some shareable value to [request extensions].
///
/// See [Passing state from middleware to handlers](index.html#passing-state-from-middleware-to-handlers)
/// for more details.
///
/// [request extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
#[derive(Clone, Copy, Debug)]
pub struct AddExtension<S, T> {
    pub(crate) inner: S,
    pub(crate) value: T,
}

impl<ResBody, S, T> Service<Request<ResBody>> for AddExtension<S, T>
where
    S: Service<Request<ResBody>>,
    T: Clone + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ResBody>) -> Self::Future {
        req.extensions_mut().insert(self.value.clone());
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::get;
    use crate::test_helpers::TestClient;
    use crate::Router;
    use http::StatusCode;

    #[derive(Clone)]
    struct Foo(String);

    #[derive(Clone)]
    struct Bar(String);

    #[crate::test]
    async fn extension_extractor() {
        async fn requires_foo(Extension(foo): Extension<Foo>) -> String {
            foo.0
        }

        async fn optional_foo(extension: Option<Extension<Foo>>) -> String {
            extension.map(|foo| foo.0 .0).unwrap_or("none".to_owned())
        }

        async fn requires_bar(Extension(bar): Extension<Bar>) -> String {
            bar.0
        }

        async fn optional_bar(extension: Option<Extension<Bar>>) -> String {
            extension.map(|bar| bar.0 .0).unwrap_or("none".to_owned())
        }

        let app = Router::new()
            .route("/requires_foo", get(requires_foo))
            .route("/optional_foo", get(optional_foo))
            .route("/requires_bar", get(requires_bar))
            .route("/optional_bar", get(optional_bar))
            .layer(Extension(Foo("foo".to_owned())));

        let client = TestClient::new(app);

        let response = client.get("/requires_foo").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "foo");

        let response = client.get("/optional_foo").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "foo");

        let response = client.get("/requires_bar").await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.text().await, "Missing request extension: Extension of type `axum::extension::tests::Bar` was not found. Perhaps you forgot to add it? See `axum::Extension`.");

        let response = client.get("/optional_bar").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await, "none");
    }
}
