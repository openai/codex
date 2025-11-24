use crate::extract::Request;
use crate::extract::{rejection::*, FromRequest, RawForm};
use axum_core::response::{IntoResponse, Response};
use axum_core::RequestExt;
use http::header::CONTENT_TYPE;
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// URL encoded extractor and response.
///
/// # As extractor
///
/// If used as an extractor, `Form` will deserialize form data from the request,
/// specifically:
///
/// - If the request has a method of `GET` or `HEAD`, the form data will be read
///   from the query string (same as with [`Query`])
/// - If the request has a different method, the form will be read from the body
///   of the request. It must have a `content-type` of
///   `application/x-www-form-urlencoded` for this to work. If you want to parse
///   `multipart/form-data` request bodies, use [`Multipart`] instead.
///
/// This matches how HTML forms are sent by browsers by default.
/// In both cases, the inner type `T` must implement [`serde::Deserialize`].
///
/// ⚠️ Since parsing form data might require consuming the request body, the `Form` extractor must be
/// *last* if there are multiple extractors in a handler. See ["the order of
/// extractors"][order-of-extractors]
///
/// [order-of-extractors]: crate::extract#the-order-of-extractors
///
/// ```rust
/// use axum::Form;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct SignUp {
///     username: String,
///     password: String,
/// }
///
/// async fn accept_form(Form(sign_up): Form<SignUp>) {
///     // ...
/// }
/// ```
///
/// # As response
///
/// `Form` can also be used to encode any type that implements
/// [`serde::Serialize`] as `application/x-www-form-urlencoded`
///
/// ```rust
/// use axum::Form;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Payload {
///     value: String,
/// }
///
/// async fn handler() -> Form<Payload> {
///     Form(Payload { value: "foo".to_owned() })
/// }
/// ```
///
/// [`Query`]: crate::extract::Query
/// [`Multipart`]: crate::extract::Multipart
#[cfg_attr(docsrs, doc(cfg(feature = "form")))]
#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Form<T>(pub T);

impl<T, S> FromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = FormRejection;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let is_get_or_head =
            req.method() == http::Method::GET || req.method() == http::Method::HEAD;

        match req.extract().await {
            Ok(RawForm(bytes)) => {
                let deserializer =
                    serde_urlencoded::Deserializer::new(form_urlencoded::parse(&bytes));
                let value = serde_path_to_error::deserialize(deserializer).map_err(
                    |err| -> FormRejection {
                        if is_get_or_head {
                            FailedToDeserializeForm::from_err(err).into()
                        } else {
                            FailedToDeserializeFormBody::from_err(err).into()
                        }
                    },
                )?;
                Ok(Form(value))
            }
            Err(RawFormRejection::BytesRejection(r)) => Err(FormRejection::BytesRejection(r)),
            Err(RawFormRejection::InvalidFormContentType(r)) => {
                Err(FormRejection::InvalidFormContentType(r))
            }
        }
    }
}

impl<T> IntoResponse for Form<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        // Extracted into separate fn so it's only compiled once for all T.
        fn make_response(ser_result: Result<String, serde_urlencoded::ser::Error>) -> Response {
            match ser_result {
                Ok(body) => (
                    [(CONTENT_TYPE, mime::APPLICATION_WWW_FORM_URLENCODED.as_ref())],
                    body,
                )
                    .into_response(),
                Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
            }
        }

        make_response(serde_urlencoded::to_string(&self.0))
    }
}
axum_core::__impl_deref!(Form);

#[cfg(test)]
mod tests {
    use crate::{
        routing::{on, MethodFilter},
        test_helpers::TestClient,
        Router,
    };

    use super::*;
    use axum_core::body::Body;
    use http::{Method, Request};
    use mime::APPLICATION_WWW_FORM_URLENCODED;
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Pagination {
        size: Option<u64>,
        page: Option<u64>,
    }

    async fn check_query<T: DeserializeOwned + PartialEq + Debug>(uri: impl AsRef<str>, value: T) {
        let req = Request::builder()
            .uri(uri.as_ref())
            .body(Body::empty())
            .unwrap();
        assert_eq!(Form::<T>::from_request(req, &()).await.unwrap().0, value);
    }

    async fn check_body<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: T) {
        let req = Request::builder()
            .uri("http://example.com/test")
            .method(Method::POST)
            .header(CONTENT_TYPE, APPLICATION_WWW_FORM_URLENCODED.as_ref())
            .body(Body::from(serde_urlencoded::to_string(&value).unwrap()))
            .unwrap();
        assert_eq!(Form::<T>::from_request(req, &()).await.unwrap().0, value);
    }

    #[crate::test]
    async fn test_form_query() {
        check_query(
            "http://example.com/test",
            Pagination {
                size: None,
                page: None,
            },
        )
        .await;

        check_query(
            "http://example.com/test?size=10",
            Pagination {
                size: Some(10),
                page: None,
            },
        )
        .await;

        check_query(
            "http://example.com/test?size=10&page=20",
            Pagination {
                size: Some(10),
                page: Some(20),
            },
        )
        .await;
    }

    #[crate::test]
    async fn test_form_body() {
        check_body(Pagination {
            size: None,
            page: None,
        })
        .await;

        check_body(Pagination {
            size: Some(10),
            page: None,
        })
        .await;

        check_body(Pagination {
            size: Some(10),
            page: Some(20),
        })
        .await;
    }

    #[crate::test]
    async fn test_incorrect_content_type() {
        let req = Request::builder()
            .uri("http://example.com/test")
            .method(Method::POST)
            .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(
                serde_urlencoded::to_string(&Pagination {
                    size: Some(10),
                    page: None,
                })
                .unwrap(),
            ))
            .unwrap();
        assert!(matches!(
            Form::<Pagination>::from_request(req, &())
                .await
                .unwrap_err(),
            FormRejection::InvalidFormContentType(InvalidFormContentType)
        ));
    }

    #[tokio::test]
    async fn deserialize_error_status_codes() {
        #[allow(dead_code)]
        #[derive(Deserialize)]
        struct Payload {
            a: i32,
        }

        let app = Router::new().route(
            "/",
            on(
                MethodFilter::GET.or(MethodFilter::POST),
                |_: Form<Payload>| async {},
            ),
        );

        let client = TestClient::new(app);

        let res = client.get("/?a=false").await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            res.text().await,
            "Failed to deserialize form: a: invalid digit found in string"
        );

        let res = client
            .post("/")
            .header(CONTENT_TYPE, APPLICATION_WWW_FORM_URLENCODED.as_ref())
            .body("a=false")
            .await;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            res.text().await,
            "Failed to deserialize form body: a: invalid digit found in string"
        );
    }
}
