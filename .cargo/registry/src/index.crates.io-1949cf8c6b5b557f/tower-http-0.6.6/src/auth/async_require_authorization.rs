//! Authorize requests using the [`Authorization`] header asynchronously.
//!
//! [`Authorization`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization
//!
//! # Example
//!
//! ```
//! use tower_http::auth::{AsyncRequireAuthorizationLayer, AsyncAuthorizeRequest};
//! use http::{Request, Response, StatusCode, header::AUTHORIZATION};
//! use tower::{Service, ServiceExt, ServiceBuilder, service_fn, BoxError};
//! use futures_core::future::BoxFuture;
//! use bytes::Bytes;
//! use http_body_util::Full;
//!
//! #[derive(Clone, Copy)]
//! struct MyAuth;
//!
//! impl<B> AsyncAuthorizeRequest<B> for MyAuth
//! where
//!     B: Send + Sync + 'static,
//! {
//!     type RequestBody = B;
//!     type ResponseBody = Full<Bytes>;
//!     type Future = BoxFuture<'static, Result<Request<B>, Response<Self::ResponseBody>>>;
//!
//!     fn authorize(&mut self, mut request: Request<B>) -> Self::Future {
//!         Box::pin(async {
//!             if let Some(user_id) = check_auth(&request).await {
//!                 // Set `user_id` as a request extension so it can be accessed by other
//!                 // services down the stack.
//!                 request.extensions_mut().insert(user_id);
//!
//!                 Ok(request)
//!             } else {
//!                 let unauthorized_response = Response::builder()
//!                     .status(StatusCode::UNAUTHORIZED)
//!                     .body(Full::<Bytes>::default())
//!                     .unwrap();
//!
//!                 Err(unauthorized_response)
//!             }
//!         })
//!     }
//! }
//!
//! async fn check_auth<B>(request: &Request<B>) -> Option<UserId> {
//!     // ...
//!     # None
//! }
//!
//! #[derive(Debug, Clone)]
//! struct UserId(String);
//!
//! async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, BoxError> {
//!     // Access the `UserId` that was set in `on_authorized`. If `handle` gets called the
//!     // request was authorized and `UserId` will be present.
//!     let user_id = request
//!         .extensions()
//!         .get::<UserId>()
//!         .expect("UserId will be there if request was authorized");
//!
//!     println!("request from {:?}", user_id);
//!
//!     Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), BoxError> {
//! let service = ServiceBuilder::new()
//!     // Authorize requests using `MyAuth`
//!     .layer(AsyncRequireAuthorizationLayer::new(MyAuth))
//!     .service_fn(handle);
//! # Ok(())
//! # }
//! ```
//!
//! Or using a closure:
//!
//! ```
//! use tower_http::auth::{AsyncRequireAuthorizationLayer, AsyncAuthorizeRequest};
//! use http::{Request, Response, StatusCode};
//! use tower::{Service, ServiceExt, ServiceBuilder, BoxError};
//! use futures_core::future::BoxFuture;
//! use http_body_util::Full;
//! use bytes::Bytes;
//!
//! async fn check_auth<B>(request: &Request<B>) -> Option<UserId> {
//!     // ...
//!     # None
//! }
//!
//! #[derive(Debug)]
//! struct UserId(String);
//!
//! async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, BoxError> {
//!     # todo!();
//!     // ...
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), BoxError> {
//! let service = ServiceBuilder::new()
//!     .layer(AsyncRequireAuthorizationLayer::new(|request: Request<Full<Bytes>>| async move {
//!         if let Some(user_id) = check_auth(&request).await {
//!             Ok(request)
//!         } else {
//!             let unauthorized_response = Response::builder()
//!                 .status(StatusCode::UNAUTHORIZED)
//!                 .body(Full::<Bytes>::default())
//!                 .unwrap();
//!
//!             Err(unauthorized_response)
//!         }
//!     }))
//!     .service_fn(handle);
//! # Ok(())
//! # }
//! ```

use http::{Request, Response};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    mem,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Layer that applies [`AsyncRequireAuthorization`] which authorizes all requests using the
/// [`Authorization`] header.
///
/// See the [module docs](crate::auth::async_require_authorization) for an example.
///
/// [`Authorization`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization
#[derive(Debug, Clone)]
pub struct AsyncRequireAuthorizationLayer<T> {
    auth: T,
}

impl<T> AsyncRequireAuthorizationLayer<T> {
    /// Authorize requests using a custom scheme.
    pub fn new(auth: T) -> AsyncRequireAuthorizationLayer<T> {
        Self { auth }
    }
}

impl<S, T> Layer<S> for AsyncRequireAuthorizationLayer<T>
where
    T: Clone,
{
    type Service = AsyncRequireAuthorization<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        AsyncRequireAuthorization::new(inner, self.auth.clone())
    }
}

/// Middleware that authorizes all requests using the [`Authorization`] header.
///
/// See the [module docs](crate::auth::async_require_authorization) for an example.
///
/// [`Authorization`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization
#[derive(Clone, Debug)]
pub struct AsyncRequireAuthorization<S, T> {
    inner: S,
    auth: T,
}

impl<S, T> AsyncRequireAuthorization<S, T> {
    define_inner_service_accessors!();
}

impl<S, T> AsyncRequireAuthorization<S, T> {
    /// Authorize requests using a custom scheme.
    ///
    /// The `Authorization` header is required to have the value provided.
    pub fn new(inner: S, auth: T) -> AsyncRequireAuthorization<S, T> {
        Self { inner, auth }
    }

    /// Returns a new [`Layer`] that wraps services with an [`AsyncRequireAuthorizationLayer`]
    /// middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(auth: T) -> AsyncRequireAuthorizationLayer<T> {
        AsyncRequireAuthorizationLayer::new(auth)
    }
}

impl<ReqBody, ResBody, S, Auth> Service<Request<ReqBody>> for AsyncRequireAuthorization<S, Auth>
where
    Auth: AsyncAuthorizeRequest<ReqBody, ResponseBody = ResBody>,
    S: Service<Request<Auth::RequestBody>, Response = Response<ResBody>> + Clone,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = ResponseFuture<Auth, S, ReqBody>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        let authorize = self.auth.authorize(req);
        // mem::swap due to https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        mem::swap(&mut self.inner, &mut inner);

        ResponseFuture {
            state: State::Authorize { authorize },
            service: inner,
        }
    }
}

pin_project! {
    /// Response future for [`AsyncRequireAuthorization`].
    pub struct ResponseFuture<Auth, S, ReqBody>
    where
        Auth: AsyncAuthorizeRequest<ReqBody>,
        S: Service<Request<Auth::RequestBody>>,
    {
        #[pin]
        state: State<Auth::Future, S::Future>,
        service: S,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<A, SFut> {
        Authorize {
            #[pin]
            authorize: A,
        },
        Authorized {
            #[pin]
            fut: SFut,
        },
    }
}

impl<Auth, S, ReqBody, B> Future for ResponseFuture<Auth, S, ReqBody>
where
    Auth: AsyncAuthorizeRequest<ReqBody, ResponseBody = B>,
    S: Service<Request<Auth::RequestBody>, Response = Response<B>>,
{
    type Output = Result<Response<B>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                StateProj::Authorize { authorize } => {
                    let auth = ready!(authorize.poll(cx));
                    match auth {
                        Ok(req) => {
                            let fut = this.service.call(req);
                            this.state.set(State::Authorized { fut })
                        }
                        Err(res) => {
                            return Poll::Ready(Ok(res));
                        }
                    };
                }
                StateProj::Authorized { fut } => {
                    return fut.poll(cx);
                }
            }
        }
    }
}

/// Trait for authorizing requests.
pub trait AsyncAuthorizeRequest<B> {
    /// The type of request body returned by `authorize`.
    ///
    /// Set this to `B` unless you need to change the request body type.
    type RequestBody;

    /// The body type used for responses to unauthorized requests.
    type ResponseBody;

    /// The Future type returned by `authorize`
    type Future: Future<Output = Result<Request<Self::RequestBody>, Response<Self::ResponseBody>>>;

    /// Authorize the request.
    ///
    /// If the future resolves to `Ok(request)` then the request is allowed through, otherwise not.
    fn authorize(&mut self, request: Request<B>) -> Self::Future;
}

impl<B, F, Fut, ReqBody, ResBody> AsyncAuthorizeRequest<B> for F
where
    F: FnMut(Request<B>) -> Fut,
    Fut: Future<Output = Result<Request<ReqBody>, Response<ResBody>>>,
{
    type RequestBody = ReqBody;
    type ResponseBody = ResBody;
    type Future = Fut;

    fn authorize(&mut self, request: Request<B>) -> Self::Future {
        self(request)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::test_helpers::Body;
    use futures_core::future::BoxFuture;
    use http::{header, StatusCode};
    use tower::{BoxError, ServiceBuilder, ServiceExt};

    #[derive(Clone, Copy)]
    struct MyAuth;

    impl<B> AsyncAuthorizeRequest<B> for MyAuth
    where
        B: Send + 'static,
    {
        type RequestBody = B;
        type ResponseBody = Body;
        type Future = BoxFuture<'static, Result<Request<B>, Response<Self::ResponseBody>>>;

        fn authorize(&mut self, request: Request<B>) -> Self::Future {
            Box::pin(async move {
                let authorized = request
                    .headers()
                    .get(header::AUTHORIZATION)
                    .and_then(|auth| auth.to_str().ok()?.strip_prefix("Bearer "))
                    == Some("69420");

                if authorized {
                    Ok(request)
                } else {
                    Err(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Body::empty())
                        .unwrap())
                }
            })
        }
    }

    #[tokio::test]
    async fn require_async_auth_works() {
        let mut service = ServiceBuilder::new()
            .layer(AsyncRequireAuthorizationLayer::new(MyAuth))
            .service_fn(echo);

        let request = Request::get("/")
            .header(header::AUTHORIZATION, "Bearer 69420")
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_async_auth_401() {
        let mut service = ServiceBuilder::new()
            .layer(AsyncRequireAuthorizationLayer::new(MyAuth))
            .service_fn(echo);

        let request = Request::get("/")
            .header(header::AUTHORIZATION, "Bearer deez")
            .body(Body::empty())
            .unwrap();

        let res = service.ready().await.unwrap().call(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    async fn echo(req: Request<Body>) -> Result<Response<Body>, BoxError> {
        Ok(Response::new(req.into_body()))
    }
}
