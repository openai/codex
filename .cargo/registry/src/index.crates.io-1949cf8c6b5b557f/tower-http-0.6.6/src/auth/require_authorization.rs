//! Authorize requests using [`ValidateRequest`].
//!
//! [`Authorization`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization
//!
//! # Example
//!
//! ```
//! use tower_http::validate_request::{ValidateRequest, ValidateRequestHeader, ValidateRequestHeaderLayer};
//! use http::{Request, Response, StatusCode, header::AUTHORIZATION};
//! use tower::{Service, ServiceExt, ServiceBuilder, service_fn, BoxError};
//! use bytes::Bytes;
//! use http_body_util::Full;
//!
//! async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, BoxError> {
//!     Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), BoxError> {
//! let mut service = ServiceBuilder::new()
//!     // Require the `Authorization` header to be `Bearer passwordlol`
//!     .layer(ValidateRequestHeaderLayer::bearer("passwordlol"))
//!     .service_fn(handle);
//!
//! // Requests with the correct token are allowed through
//! let request = Request::builder()
//!     .header(AUTHORIZATION, "Bearer passwordlol")
//!     .body(Full::default())
//!     .unwrap();
//!
//! let response = service
//!     .ready()
//!     .await?
//!     .call(request)
//!     .await?;
//!
//! assert_eq!(StatusCode::OK, response.status());
//!
//! // Requests with an invalid token get a `401 Unauthorized` response
//! let request = Request::builder()
//!     .body(Full::default())
//!     .unwrap();
//!
//! let response = service
//!     .ready()
//!     .await?
//!     .call(request)
//!     .await?;
//!
//! assert_eq!(StatusCode::UNAUTHORIZED, response.status());
//! # Ok(())
//! # }
//! ```
//!
//! Custom validation can be made by implementing [`ValidateRequest`].

use crate::validate_request::{ValidateRequest, ValidateRequestHeader, ValidateRequestHeaderLayer};
use base64::Engine as _;
use http::{
    header::{self, HeaderValue},
    Request, Response, StatusCode,
};
use std::{fmt, marker::PhantomData};

const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

impl<S, ResBody> ValidateRequestHeader<S, Basic<ResBody>> {
    /// Authorize requests using a username and password pair.
    ///
    /// The `Authorization` header is required to be `Basic {credentials}` where `credentials` is
    /// `base64_encode("{username}:{password}")`.
    ///
    /// Since the username and password is sent in clear text it is recommended to use HTTPS/TLS
    /// with this method. However use of HTTPS/TLS is not enforced by this middleware.
    pub fn basic(inner: S, username: &str, value: &str) -> Self
    where
        ResBody: Default,
    {
        Self::custom(inner, Basic::new(username, value))
    }
}

impl<ResBody> ValidateRequestHeaderLayer<Basic<ResBody>> {
    /// Authorize requests using a username and password pair.
    ///
    /// The `Authorization` header is required to be `Basic {credentials}` where `credentials` is
    /// `base64_encode("{username}:{password}")`.
    ///
    /// Since the username and password is sent in clear text it is recommended to use HTTPS/TLS
    /// with this method. However use of HTTPS/TLS is not enforced by this middleware.
    pub fn basic(username: &str, password: &str) -> Self
    where
        ResBody: Default,
    {
        Self::custom(Basic::new(username, password))
    }
}

impl<S, ResBody> ValidateRequestHeader<S, Bearer<ResBody>> {
    /// Authorize requests using a "bearer token". Commonly used for OAuth 2.
    ///
    /// The `Authorization` header is required to be `Bearer {token}`.
    ///
    /// # Panics
    ///
    /// Panics if the token is not a valid [`HeaderValue`].
    pub fn bearer(inner: S, token: &str) -> Self
    where
        ResBody: Default,
    {
        Self::custom(inner, Bearer::new(token))
    }
}

impl<ResBody> ValidateRequestHeaderLayer<Bearer<ResBody>> {
    /// Authorize requests using a "bearer token". Commonly used for OAuth 2.
    ///
    /// The `Authorization` header is required to be `Bearer {token}`.
    ///
    /// # Panics
    ///
    /// Panics if the token is not a valid [`HeaderValue`].
    pub fn bearer(token: &str) -> Self
    where
        ResBody: Default,
    {
        Self::custom(Bearer::new(token))
    }
}

/// Type that performs "bearer token" authorization.
///
/// See [`ValidateRequestHeader::bearer`] for more details.
pub struct Bearer<ResBody> {
    header_value: HeaderValue,
    _ty: PhantomData<fn() -> ResBody>,
}

impl<ResBody> Bearer<ResBody> {
    fn new(token: &str) -> Self
    where
        ResBody: Default,
    {
        Self {
            header_value: format!("Bearer {}", token)
                .parse()
                .expect("token is not a valid header value"),
            _ty: PhantomData,
        }
    }
}

impl<ResBody> Clone for Bearer<ResBody> {
    fn clone(&self) -> Self {
        Self {
            header_value: self.header_value.clone(),
            _ty: PhantomData,
        }
    }
}

impl<ResBody> fmt::Debug for Bearer<ResBody> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bearer")
            .field("header_value", &self.header_value)
            .finish()
    }
}

impl<B, ResBody> ValidateRequest<B> for Bearer<ResBody>
where
    ResBody: Default,
{
    type ResponseBody = ResBody;

    fn validate(&mut self, request: &mut Request<B>) -> Result<(), Response<Self::ResponseBody>> {
        match request.headers().get(header::AUTHORIZATION) {
            Some(actual) if actual == self.header_value => Ok(()),
            _ => {
                let mut res = Response::new(ResBody::default());
                *res.status_mut() = StatusCode::UNAUTHORIZED;
                Err(res)
            }
        }
    }
}

/// Type that performs basic authorization.
///
/// See [`ValidateRequestHeader::basic`] for more details.
pub struct Basic<ResBody> {
    header_value: HeaderValue,
    _ty: PhantomData<fn() -> ResBody>,
}

impl<ResBody> Basic<ResBody> {
    fn new(username: &str, password: &str) -> Self
    where
        ResBody: Default,
    {
        let encoded = BASE64.encode(format!("{}:{}", username, password));
        let header_value = format!("Basic {}", encoded).parse().unwrap();
        Self {
            header_value,
            _ty: PhantomData,
        }
    }
}

impl<ResBody> Clone for Basic<ResBody> {
    fn clone(&self) -> Self {
        Self {
            header_value: self.header_value.clone(),
            _ty: PhantomData,
        }
    }
}

impl<ResBody> fmt::Debug for Basic<ResBody> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Basic")
            .field("header_value", &self.header_value)
            .finish()
    }
}

impl<B, ResBody> ValidateRequest<B> for Basic<ResBody>
where
    ResBody: Default,
{
    type ResponseBody = ResBody;

    fn validate(&mut self, request: &mut Request<B>) -> Result<(), Response<Self::ResponseBody>> {
        match request.headers().get(header::AUTHORIZATION) {
            Some(actual) if actual == self.header_value => Ok(()),
            _ => {
                let mut res = Response::new(ResBody::default());
                *res.status_mut() = StatusCode::UNAUTHORIZED;
                res.headers_mut()
                    .insert(header::WWW_AUTHENTICATE, "Basic".parse().unwrap());
                Err(res)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::validate_request::ValidateRequestHeaderLayer;

    #[allow(unused_imports)]
    use super::*;
    use crate::test_helpers::Body;
    use http::header;
    use tower::{BoxError, ServiceBuilder, ServiceExt};
    use tower_service::Service;

    #[tokio::test]
    async fn valid_basic_token() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::basic("foo", "bar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(
                header::AUTHORIZATION,
                format!("Basic {}", BASE64.encode("foo:bar")),
            )
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_basic_token() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::basic("foo", "bar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(
                header::AUTHORIZATION,
                format!("Basic {}", BASE64.encode("wrong:credentials")),
            )
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        let www_authenticate = res.headers().get(header::WWW_AUTHENTICATE).unwrap();
        assert_eq!(www_authenticate, "Basic");
    }

    #[tokio::test]
    async fn valid_bearer_token() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::bearer("foobar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(header::AUTHORIZATION, "Bearer foobar")
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn basic_auth_is_case_sensitive_in_prefix() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::basic("foo", "bar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(
                header::AUTHORIZATION,
                format!("basic {}", BASE64.encode("foo:bar")),
            )
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn basic_auth_is_case_sensitive_in_value() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::basic("foo", "bar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(
                header::AUTHORIZATION,
                format!("Basic {}", BASE64.encode("Foo:bar")),
            )
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn invalid_bearer_token() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::bearer("foobar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(header::AUTHORIZATION, "Bearer wat")
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bearer_token_is_case_sensitive_in_prefix() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::bearer("foobar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(header::AUTHORIZATION, "bearer foobar")
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bearer_token_is_case_sensitive_in_token() {
        let mut service = ServiceBuilder::new()
            .layer(ValidateRequestHeaderLayer::bearer("foobar"))
            .service_fn(echo);

        let request = Request::get("/")
            .header(header::AUTHORIZATION, "Bearer Foobar")
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    async fn echo(req: Request<Body>) -> Result<Response<Body>, BoxError> {
        Ok(Response::new(req.into_body()))
    }
}
