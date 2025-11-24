use axum_core::extract::{FromRequest, Request};
use bytes::Bytes;
use http::Method;

use super::{
    has_content_type,
    rejection::{InvalidFormContentType, RawFormRejection},
};

/// Extractor that extracts raw form requests.
///
/// For `GET` requests it will extract the raw query. For other methods it extracts the raw
/// `application/x-www-form-urlencoded` encoded request body.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{
///     extract::RawForm,
///     routing::get,
///     Router
/// };
///
/// async fn handler(RawForm(form): RawForm) {}
///
/// let app = Router::new().route("/", get(handler));
/// # let _: Router = app;
/// ```
#[derive(Debug)]
pub struct RawForm(pub Bytes);

impl<S> FromRequest<S> for RawForm
where
    S: Send + Sync,
{
    type Rejection = RawFormRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        if req.method() == Method::GET {
            if let Some(query) = req.uri().query() {
                return Ok(Self(Bytes::copy_from_slice(query.as_bytes())));
            }

            Ok(Self(Bytes::new()))
        } else {
            if !has_content_type(req.headers(), &mime::APPLICATION_WWW_FORM_URLENCODED) {
                return Err(InvalidFormContentType.into());
            }

            Ok(Self(Bytes::from_request(req, state).await?))
        }
    }
}

#[cfg(test)]
mod tests {
    use axum_core::body::Body;
    use http::{header::CONTENT_TYPE, Request};

    use super::{InvalidFormContentType, RawForm, RawFormRejection};

    use crate::extract::FromRequest;

    async fn check_query(uri: &str, value: &[u8]) {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

        assert_eq!(RawForm::from_request(req, &()).await.unwrap().0, value);
    }

    async fn check_body(body: &'static [u8]) {
        let req = Request::post("http://example.com/test")
            .header(CONTENT_TYPE, mime::APPLICATION_WWW_FORM_URLENCODED.as_ref())
            .body(Body::from(body))
            .unwrap();

        assert_eq!(RawForm::from_request(req, &()).await.unwrap().0, body);
    }

    #[crate::test]
    async fn test_from_query() {
        check_query("http://example.com/test", b"").await;

        check_query("http://example.com/test?page=0&size=10", b"page=0&size=10").await;
    }

    #[crate::test]
    async fn test_from_body() {
        check_body(b"").await;

        check_body(b"username=user&password=secure%20password").await;
    }

    #[crate::test]
    async fn test_incorrect_content_type() {
        let req = Request::post("http://example.com/test")
            .body(Body::from("page=0&size=10"))
            .unwrap();

        assert!(matches!(
            RawForm::from_request(req, &()).await.unwrap_err(),
            RawFormRejection::InvalidFormContentType(InvalidFormContentType)
        ))
    }
}
