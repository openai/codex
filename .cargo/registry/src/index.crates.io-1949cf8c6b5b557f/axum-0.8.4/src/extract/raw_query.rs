use super::FromRequestParts;
use http::request::Parts;
use std::convert::Infallible;

/// Extractor that extracts the raw query string, without parsing it.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::RawQuery,
///     routing::get,
///     Router,
/// };
/// use futures_util::StreamExt;
///
/// async fn handler(RawQuery(query): RawQuery) {
///     // ...
/// }
///
/// let app = Router::new().route("/users", get(handler));
/// # let _: Router = app;
/// ```
#[derive(Debug)]
pub struct RawQuery(pub Option<String>);

impl<S> FromRequestParts<S> for RawQuery
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().map(|query| query.to_owned());
        Ok(Self(query))
    }
}
