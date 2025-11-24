use super::{Extension, FromRequestParts};
use http::{request::Parts, Uri};
use std::convert::Infallible;

/// Extractor that gets the original request URI regardless of nesting.
///
/// This is necessary since [`Uri`](http::Uri), when used as an extractor, will
/// have the prefix stripped if used in a nested service.
///
/// # Example
///
/// ```
/// use axum::{
///     routing::get,
///     Router,
///     extract::OriginalUri,
///     http::Uri
/// };
///
/// let api_routes = Router::new()
///     .route(
///         "/users",
///         get(|uri: Uri, OriginalUri(original_uri): OriginalUri| async {
///             // `uri` is `/users`
///             // `original_uri` is `/api/users`
///         }),
///     );
///
/// let app = Router::new().nest("/api", api_routes);
/// # let _: Router = app;
/// ```
///
/// # Extracting via request extensions
///
/// `OriginalUri` can also be accessed from middleware via request extensions.
/// This is useful for example with [`Trace`](tower_http::trace::Trace) to
/// create a span that contains the full path, if your service might be nested:
///
/// ```
/// use axum::{
///     Router,
///     extract::OriginalUri,
///     http::Request,
///     routing::get,
/// };
/// use tower_http::trace::TraceLayer;
///
/// let api_routes = Router::new()
///     .route("/users/{id}", get(|| async { /* ... */ }))
///     .layer(
///         TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
///             let path = if let Some(path) = req.extensions().get::<OriginalUri>() {
///                 // This will include `/api`
///                 path.0.path().to_owned()
///             } else {
///                 // The `OriginalUri` extension will always be present if using
///                 // `Router` unless another extractor or middleware has removed it
///                 req.uri().path().to_owned()
///             };
///             tracing::info_span!("http-request", %path)
///         }),
///     );
///
/// let app = Router::new().nest("/api", api_routes);
/// # let _: Router = app;
/// ```
#[derive(Debug, Clone)]
pub struct OriginalUri(pub Uri);

impl<S> FromRequestParts<S> for OriginalUri
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let uri = Extension::<Self>::from_request_parts(parts, state)
            .await
            .unwrap_or_else(|_| Extension(OriginalUri(parts.uri.clone())))
            .0;
        Ok(uri)
    }
}

axum_core::__impl_deref!(OriginalUri: Uri);
