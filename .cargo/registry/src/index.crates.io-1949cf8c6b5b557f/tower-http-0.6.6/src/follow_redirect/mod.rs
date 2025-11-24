//! Middleware for following redirections.
//!
//! # Overview
//!
//! The [`FollowRedirect`] middleware retries requests with the inner [`Service`] to follow HTTP
//! redirections.
//!
//! The middleware tries to clone the original [`Request`] when making a redirected request.
//! However, since [`Extensions`][http::Extensions] are `!Clone`, any extensions set by outer
//! middleware will be discarded. Also, the request body cannot always be cloned. When the
//! original body is known to be empty by [`Body::size_hint`], the middleware uses `Default`
//! implementation of the body type to create a new request body. If you know that the body can be
//! cloned in some way, you can tell the middleware to clone it by configuring a [`policy`].
//!
//! # Examples
//!
//! ## Basic usage
//!
//! ```
//! use http::{Request, Response};
//! use bytes::Bytes;
//! use http_body_util::Full;
//! use tower::{Service, ServiceBuilder, ServiceExt};
//! use tower_http::follow_redirect::{FollowRedirectLayer, RequestUri};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), std::convert::Infallible> {
//! # let http_client = tower::service_fn(|req: Request<_>| async move {
//! #     let dest = "https://www.rust-lang.org/";
//! #     let mut res = http::Response::builder();
//! #     if req.uri() != dest {
//! #         res = res
//! #             .status(http::StatusCode::MOVED_PERMANENTLY)
//! #             .header(http::header::LOCATION, dest);
//! #     }
//! #     Ok::<_, std::convert::Infallible>(res.body(Full::<Bytes>::default()).unwrap())
//! # });
//! let mut client = ServiceBuilder::new()
//!     .layer(FollowRedirectLayer::new())
//!     .service(http_client);
//!
//! let request = Request::builder()
//!     .uri("https://rust-lang.org/")
//!     .body(Full::<Bytes>::default())
//!     .unwrap();
//!
//! let response = client.ready().await?.call(request).await?;
//! // Get the final request URI.
//! assert_eq!(response.extensions().get::<RequestUri>().unwrap().0, "https://www.rust-lang.org/");
//! # Ok(())
//! # }
//! ```
//!
//! ## Customizing the `Policy`
//!
//! You can use a [`Policy`] value to customize how the middleware handles redirections.
//!
//! ```
//! use http::{Request, Response};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use tower::{Service, ServiceBuilder, ServiceExt};
//! use tower_http::follow_redirect::{
//!     policy::{self, PolicyExt},
//!     FollowRedirectLayer,
//! };
//!
//! #[derive(Debug)]
//! enum MyError {
//!     TooManyRedirects,
//!     Other(tower::BoxError),
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), MyError> {
//! # let http_client =
//! #     tower::service_fn(|_: Request<Full<Bytes>>| async { Ok(Response::new(Full::<Bytes>::default())) });
//! let policy = policy::Limited::new(10) // Set the maximum number of redirections to 10.
//!     // Return an error when the limit was reached.
//!     .or::<_, (), _>(policy::redirect_fn(|_| Err(MyError::TooManyRedirects)))
//!     // Do not follow cross-origin redirections, and return the redirection responses as-is.
//!     .and::<_, (), _>(policy::SameOrigin::new());
//!
//! let mut client = ServiceBuilder::new()
//!     .layer(FollowRedirectLayer::with_policy(policy))
//!     .map_err(MyError::Other)
//!     .service(http_client);
//!
//! // ...
//! # let _ = client.ready().await?.call(Request::default()).await?;
//! # Ok(())
//! # }
//! ```

pub mod policy;

use self::policy::{Action, Attempt, Policy, Standard};
use futures_util::future::Either;
use http::{
    header::CONTENT_ENCODING, header::CONTENT_LENGTH, header::CONTENT_TYPE, header::LOCATION,
    header::TRANSFER_ENCODING, HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri,
    Version,
};
use http_body::Body;
use iri_string::types::{UriAbsoluteString, UriReferenceStr};
use pin_project_lite::pin_project;
use std::{
    convert::TryFrom,
    future::Future,
    mem,
    pin::Pin,
    str,
    task::{ready, Context, Poll},
};
use tower::util::Oneshot;
use tower_layer::Layer;
use tower_service::Service;

/// [`Layer`] for retrying requests with a [`Service`] to follow redirection responses.
///
/// See the [module docs](self) for more details.
#[derive(Clone, Copy, Debug, Default)]
pub struct FollowRedirectLayer<P = Standard> {
    policy: P,
}

impl FollowRedirectLayer {
    /// Create a new [`FollowRedirectLayer`] with a [`Standard`] redirection policy.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<P> FollowRedirectLayer<P> {
    /// Create a new [`FollowRedirectLayer`] with the given redirection [`Policy`].
    pub fn with_policy(policy: P) -> Self {
        FollowRedirectLayer { policy }
    }
}

impl<S, P> Layer<S> for FollowRedirectLayer<P>
where
    S: Clone,
    P: Clone,
{
    type Service = FollowRedirect<S, P>;

    fn layer(&self, inner: S) -> Self::Service {
        FollowRedirect::with_policy(inner, self.policy.clone())
    }
}

/// Middleware that retries requests with a [`Service`] to follow redirection responses.
///
/// See the [module docs](self) for more details.
#[derive(Clone, Copy, Debug)]
pub struct FollowRedirect<S, P = Standard> {
    inner: S,
    policy: P,
}

impl<S> FollowRedirect<S> {
    /// Create a new [`FollowRedirect`] with a [`Standard`] redirection policy.
    pub fn new(inner: S) -> Self {
        Self::with_policy(inner, Standard::default())
    }

    /// Returns a new [`Layer`] that wraps services with a `FollowRedirect` middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer() -> FollowRedirectLayer {
        FollowRedirectLayer::new()
    }
}

impl<S, P> FollowRedirect<S, P>
where
    P: Clone,
{
    /// Create a new [`FollowRedirect`] with the given redirection [`Policy`].
    pub fn with_policy(inner: S, policy: P) -> Self {
        FollowRedirect { inner, policy }
    }

    /// Returns a new [`Layer`] that wraps services with a `FollowRedirect` middleware
    /// with the given redirection [`Policy`].
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer_with_policy(policy: P) -> FollowRedirectLayer<P> {
        FollowRedirectLayer::with_policy(policy)
    }

    define_inner_service_accessors!();
}

impl<ReqBody, ResBody, S, P> Service<Request<ReqBody>> for FollowRedirect<S, P>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    ReqBody: Body + Default,
    P: Policy<ReqBody, S::Error> + Clone,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = ResponseFuture<S, ReqBody, P>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let service = self.inner.clone();
        let mut service = mem::replace(&mut self.inner, service);
        let mut policy = self.policy.clone();
        let mut body = BodyRepr::None;
        body.try_clone_from(req.body(), &policy);
        policy.on_request(&mut req);
        ResponseFuture {
            method: req.method().clone(),
            uri: req.uri().clone(),
            version: req.version(),
            headers: req.headers().clone(),
            body,
            future: Either::Left(service.call(req)),
            service,
            policy,
        }
    }
}

pin_project! {
    /// Response future for [`FollowRedirect`].
    #[derive(Debug)]
    pub struct ResponseFuture<S, B, P>
    where
        S: Service<Request<B>>,
    {
        #[pin]
        future: Either<S::Future, Oneshot<S, Request<B>>>,
        service: S,
        policy: P,
        method: Method,
        uri: Uri,
        version: Version,
        headers: HeaderMap<HeaderValue>,
        body: BodyRepr<B>,
    }
}

impl<S, ReqBody, ResBody, P> Future for ResponseFuture<S, ReqBody, P>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    ReqBody: Body + Default,
    P: Policy<ReqBody, S::Error>,
{
    type Output = Result<Response<ResBody>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut res = ready!(this.future.as_mut().poll(cx)?);
        res.extensions_mut().insert(RequestUri(this.uri.clone()));

        let drop_payload_headers = |headers: &mut HeaderMap| {
            for header in &[
                CONTENT_TYPE,
                CONTENT_LENGTH,
                CONTENT_ENCODING,
                TRANSFER_ENCODING,
            ] {
                headers.remove(header);
            }
        };
        match res.status() {
            StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND => {
                // User agents MAY change the request method from POST to GET
                // (RFC 7231 section 6.4.2. and 6.4.3.).
                if *this.method == Method::POST {
                    *this.method = Method::GET;
                    *this.body = BodyRepr::Empty;
                    drop_payload_headers(this.headers);
                }
            }
            StatusCode::SEE_OTHER => {
                // A user agent can perform a GET or HEAD request (RFC 7231 section 6.4.4.).
                if *this.method != Method::HEAD {
                    *this.method = Method::GET;
                }
                *this.body = BodyRepr::Empty;
                drop_payload_headers(this.headers);
            }
            StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT => {}
            _ => return Poll::Ready(Ok(res)),
        };

        let body = if let Some(body) = this.body.take() {
            body
        } else {
            return Poll::Ready(Ok(res));
        };

        let location = res
            .headers()
            .get(&LOCATION)
            .and_then(|loc| resolve_uri(str::from_utf8(loc.as_bytes()).ok()?, this.uri));
        let location = if let Some(loc) = location {
            loc
        } else {
            return Poll::Ready(Ok(res));
        };

        let attempt = Attempt {
            status: res.status(),
            location: &location,
            previous: this.uri,
        };
        match this.policy.redirect(&attempt)? {
            Action::Follow => {
                *this.uri = location;
                this.body.try_clone_from(&body, &this.policy);

                let mut req = Request::new(body);
                *req.uri_mut() = this.uri.clone();
                *req.method_mut() = this.method.clone();
                *req.version_mut() = *this.version;
                *req.headers_mut() = this.headers.clone();
                this.policy.on_request(&mut req);
                this.future
                    .set(Either::Right(Oneshot::new(this.service.clone(), req)));

                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Action::Stop => Poll::Ready(Ok(res)),
        }
    }
}

/// Response [`Extensions`][http::Extensions] value that represents the effective request URI of
/// a response returned by a [`FollowRedirect`] middleware.
///
/// The value differs from the original request's effective URI if the middleware has followed
/// redirections.
#[derive(Clone)]
pub struct RequestUri(pub Uri);

#[derive(Debug)]
enum BodyRepr<B> {
    Some(B),
    Empty,
    None,
}

impl<B> BodyRepr<B>
where
    B: Body + Default,
{
    fn take(&mut self) -> Option<B> {
        match mem::replace(self, BodyRepr::None) {
            BodyRepr::Some(body) => Some(body),
            BodyRepr::Empty => {
                *self = BodyRepr::Empty;
                Some(B::default())
            }
            BodyRepr::None => None,
        }
    }

    fn try_clone_from<P, E>(&mut self, body: &B, policy: &P)
    where
        P: Policy<B, E>,
    {
        match self {
            BodyRepr::Some(_) | BodyRepr::Empty => {}
            BodyRepr::None => {
                if let Some(body) = clone_body(policy, body) {
                    *self = BodyRepr::Some(body);
                }
            }
        }
    }
}

fn clone_body<P, B, E>(policy: &P, body: &B) -> Option<B>
where
    P: Policy<B, E>,
    B: Body + Default,
{
    if body.size_hint().exact() == Some(0) {
        Some(B::default())
    } else {
        policy.clone_body(body)
    }
}

/// Try to resolve a URI reference `relative` against a base URI `base`.
fn resolve_uri(relative: &str, base: &Uri) -> Option<Uri> {
    let relative = UriReferenceStr::new(relative).ok()?;
    let base = UriAbsoluteString::try_from(base.to_string()).ok()?;
    let uri = relative.resolve_against(&base).to_string();
    Uri::try_from(uri).ok()
}

#[cfg(test)]
mod tests {
    use super::{policy::*, *};
    use crate::test_helpers::Body;
    use http::header::LOCATION;
    use std::convert::Infallible;
    use tower::{ServiceBuilder, ServiceExt};

    #[tokio::test]
    async fn follows() {
        let svc = ServiceBuilder::new()
            .layer(FollowRedirectLayer::with_policy(Action::Follow))
            .buffer(1)
            .service_fn(handle);
        let req = Request::builder()
            .uri("http://example.com/42")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(*res.body(), 0);
        assert_eq!(
            res.extensions().get::<RequestUri>().unwrap().0,
            "http://example.com/0"
        );
    }

    #[tokio::test]
    async fn stops() {
        let svc = ServiceBuilder::new()
            .layer(FollowRedirectLayer::with_policy(Action::Stop))
            .buffer(1)
            .service_fn(handle);
        let req = Request::builder()
            .uri("http://example.com/42")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(*res.body(), 42);
        assert_eq!(
            res.extensions().get::<RequestUri>().unwrap().0,
            "http://example.com/42"
        );
    }

    #[tokio::test]
    async fn limited() {
        let svc = ServiceBuilder::new()
            .layer(FollowRedirectLayer::with_policy(Limited::new(10)))
            .buffer(1)
            .service_fn(handle);
        let req = Request::builder()
            .uri("http://example.com/42")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(*res.body(), 42 - 10);
        assert_eq!(
            res.extensions().get::<RequestUri>().unwrap().0,
            "http://example.com/32"
        );
    }

    /// A server with an endpoint `GET /{n}` which redirects to `/{n-1}` unless `n` equals zero,
    /// returning `n` as the response body.
    async fn handle<B>(req: Request<B>) -> Result<Response<u64>, Infallible> {
        let n: u64 = req.uri().path()[1..].parse().unwrap();
        let mut res = Response::builder();
        if n > 0 {
            res = res
                .status(StatusCode::MOVED_PERMANENTLY)
                .header(LOCATION, format!("/{}", n - 1));
        }
        Ok::<_, Infallible>(res.body(n).unwrap())
    }
}
