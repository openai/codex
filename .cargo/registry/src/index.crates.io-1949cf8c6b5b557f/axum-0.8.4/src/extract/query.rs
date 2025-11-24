use super::{rejection::*, FromRequestParts};
use http::{request::Parts, Uri};
use serde::de::DeserializeOwned;

/// Extractor that deserializes query strings into some type.
///
/// `T` is expected to implement [`serde::Deserialize`].
///
/// # Examples
///
/// ```rust,no_run
/// use axum::{
///     extract::Query,
///     routing::get,
///     Router,
/// };
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Pagination {
///     page: usize,
///     per_page: usize,
/// }
///
/// // This will parse query strings like `?page=2&per_page=30` into `Pagination`
/// // structs.
/// async fn list_things(pagination: Query<Pagination>) {
///     let pagination: Pagination = pagination.0;
///
///     // ...
/// }
///
/// let app = Router::new().route("/list_things", get(list_things));
/// # let _: Router = app;
/// ```
///
/// If the query string cannot be parsed it will reject the request with a `400
/// Bad Request` response.
///
/// For handling values being empty vs missing see the [query-params-with-empty-strings][example]
/// example.
///
/// [example]: https://github.com/tokio-rs/axum/blob/main/examples/query-params-with-empty-strings/src/main.rs
///
/// For handling multiple values for the same query parameter, in a `?foo=1&foo=2&foo=3`
/// fashion, use [`axum_extra::extract::Query`] instead.
///
/// [`axum_extra::extract::Query`]: https://docs.rs/axum-extra/latest/axum_extra/extract/struct.Query.html
#[cfg_attr(docsrs, doc(cfg(feature = "query")))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Self::try_from_uri(&parts.uri)
    }
}

impl<T> Query<T>
where
    T: DeserializeOwned,
{
    /// Attempts to construct a [`Query`] from a reference to a [`Uri`].
    ///
    /// # Example
    /// ```
    /// use axum::extract::Query;
    /// use http::Uri;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct ExampleParams {
    ///     foo: String,
    ///     bar: u32,
    /// }
    ///
    /// let uri: Uri = "http://example.com/path?foo=hello&bar=42".parse().unwrap();
    /// let result: Query<ExampleParams> = Query::try_from_uri(&uri).unwrap();
    /// assert_eq!(result.foo, String::from("hello"));
    /// assert_eq!(result.bar, 42);
    /// ```
    pub fn try_from_uri(value: &Uri) -> Result<Self, QueryRejection> {
        let query = value.query().unwrap_or_default();
        let deserializer =
            serde_urlencoded::Deserializer::new(form_urlencoded::parse(query.as_bytes()));
        let params = serde_path_to_error::deserialize(deserializer)
            .map_err(FailedToDeserializeQueryString::from_err)?;
        Ok(Query(params))
    }
}

axum_core::__impl_deref!(Query);

#[cfg(test)]
mod tests {
    use crate::{routing::get, test_helpers::TestClient, Router};

    use super::*;
    use axum_core::{body::Body, extract::FromRequest};
    use http::{Request, StatusCode};
    use serde::Deserialize;
    use std::fmt::Debug;

    async fn check<T>(uri: impl AsRef<str>, value: T)
    where
        T: DeserializeOwned + PartialEq + Debug,
    {
        let req = Request::builder()
            .uri(uri.as_ref())
            .body(Body::empty())
            .unwrap();
        assert_eq!(Query::<T>::from_request(req, &()).await.unwrap().0, value);
    }

    #[crate::test]
    async fn test_query() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Pagination {
            size: Option<u64>,
            page: Option<u64>,
        }

        check(
            "http://example.com/test",
            Pagination {
                size: None,
                page: None,
            },
        )
        .await;

        check(
            "http://example.com/test?size=10",
            Pagination {
                size: Some(10),
                page: None,
            },
        )
        .await;

        check(
            "http://example.com/test?size=10&page=20",
            Pagination {
                size: Some(10),
                page: Some(20),
            },
        )
        .await;
    }

    #[crate::test]
    async fn correct_rejection_status_code() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct Params {
            n: i32,
        }

        async fn handler(_: Query<Params>) {}

        let app = Router::new().route("/", get(handler));
        let client = TestClient::new(app);

        let res = client.get("/?n=hi").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            res.text().await,
            "Failed to deserialize query string: n: invalid digit found in string"
        );
    }

    #[test]
    fn test_try_from_uri() {
        #[derive(Deserialize)]
        struct TestQueryParams {
            foo: String,
            bar: u32,
        }
        let uri: Uri = "http://example.com/path?foo=hello&bar=42".parse().unwrap();
        let result: Query<TestQueryParams> = Query::try_from_uri(&uri).unwrap();
        assert_eq!(result.foo, String::from("hello"));
        assert_eq!(result.bar, 42);
    }

    #[test]
    fn test_try_from_uri_with_invalid_query() {
        #[derive(Deserialize)]
        struct TestQueryParams {
            _foo: String,
            _bar: u32,
        }
        let uri: Uri = "http://example.com/path?foo=hello&bar=invalid"
            .parse()
            .unwrap();
        let result: Result<Query<TestQueryParams>, _> = Query::try_from_uri(&uri);

        assert!(result.is_err());
    }
}
