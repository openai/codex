use axum_core::extract::{FromRequest, FromRequestParts, Request};
use futures_util::future::BoxFuture;
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower::util::BoxCloneSyncService;
use tower_layer::Layer;
use tower_service::Service;

use crate::{
    response::{IntoResponse, Response},
    util::MapIntoResponse,
};

/// Create a middleware from an async function.
///
/// `from_fn` requires the function given to
///
/// 1. Be an `async fn`.
/// 2. Take zero or more [`FromRequestParts`] extractors.
/// 3. Take exactly one [`FromRequest`] extractor as the second to last argument.
/// 4. Take [`Next`](Next) as the last argument.
/// 5. Return something that implements [`IntoResponse`].
///
/// Note that this function doesn't support extracting [`State`]. For that, use [`from_fn_with_state`].
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     http,
///     routing::get,
///     response::Response,
///     middleware::{self, Next},
///     extract::Request,
/// };
///
/// async fn my_middleware(
///     request: Request,
///     next: Next,
/// ) -> Response {
///     // do something with `request`...
///
///     let response = next.run(request).await;
///
///     // do something with `response`...
///
///     response
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .layer(middleware::from_fn(my_middleware));
/// # let app: Router = app;
/// ```
///
/// # Running extractors
///
/// ```rust
/// use axum::{
///     Router,
///     extract::Request,
///     http::{StatusCode, HeaderMap},
///     middleware::{self, Next},
///     response::Response,
///     routing::get,
/// };
///
/// async fn auth(
///     // run the `HeaderMap` extractor
///     headers: HeaderMap,
///     // you can also add more extractors here but the last
///     // extractor must implement `FromRequest` which
///     // `Request` does
///     request: Request,
///     next: Next,
/// ) -> Result<Response, StatusCode> {
///     match get_token(&headers) {
///         Some(token) if token_is_valid(token) => {
///             let response = next.run(request).await;
///             Ok(response)
///         }
///         _ => {
///             Err(StatusCode::UNAUTHORIZED)
///         }
///     }
/// }
///
/// fn get_token(headers: &HeaderMap) -> Option<&str> {
///     // ...
///     # None
/// }
///
/// fn token_is_valid(token: &str) -> bool {
///     // ...
///     # false
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(middleware::from_fn(auth));
/// # let app: Router = app;
/// ```
///
/// [extractors]: crate::extract::FromRequest
/// [`State`]: crate::extract::State
pub fn from_fn<F, T>(f: F) -> FromFnLayer<F, (), T> {
    from_fn_with_state((), f)
}

/// Create a middleware from an async function with the given state.
///
/// For the requirements for the function supplied see [`from_fn`].
///
/// See [`State`](crate::extract::State) for more details about accessing state.
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     http::StatusCode,
///     routing::get,
///     response::{IntoResponse, Response},
///     middleware::{self, Next},
///     extract::{Request, State},
/// };
///
/// #[derive(Clone)]
/// struct AppState { /* ... */ }
///
/// async fn my_middleware(
///     State(state): State<AppState>,
///     // you can add more extractors here but the last
///     // extractor must implement `FromRequest` which
///     // `Request` does
///     request: Request,
///     next: Next,
/// ) -> Response {
///     // do something with `request`...
///
///     let response = next.run(request).await;
///
///     // do something with `response`...
///
///     response
/// }
///
/// let state = AppState { /* ... */ };
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(middleware::from_fn_with_state(state.clone(), my_middleware))
///     .with_state(state);
/// # let _: axum::Router = app;
/// ```
pub fn from_fn_with_state<F, S, T>(state: S, f: F) -> FromFnLayer<F, S, T> {
    FromFnLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}

/// A [`tower::Layer`] from an async function.
///
/// [`tower::Layer`] is used to apply middleware to [`Router`](crate::Router)'s.
///
/// Created with [`from_fn`] or [`from_fn_with_state`]. See those functions for more details.
#[must_use]
pub struct FromFnLayer<F, S, T> {
    f: F,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, T> Clone for FromFnLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            state: self.state.clone(),
            _extractor: self._extractor,
        }
    }
}

impl<S, I, F, T> Layer<I> for FromFnLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    type Service = FromFn<F, S, I, T>;

    fn layer(&self, inner: I) -> Self::Service {
        FromFn {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}

impl<F, S, T> fmt::Debug for FromFnLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            // Write out the type name, without quoting it as `&type_name::<F>()` would
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}

/// A middleware created from an async function.
///
/// Created with [`from_fn`] or [`from_fn_with_state`]. See those functions for more details.
pub struct FromFn<F, S, I, T> {
    f: F,
    inner: I,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, I, T> Clone for FromFn<F, S, I, T>
where
    F: Clone,
    I: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            inner: self.inner.clone(),
            state: self.state.clone(),
            _extractor: self._extractor,
        }
    }
}

macro_rules! impl_service {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, Out, S, I, $($ty,)* $last> Service<Request> for FromFn<F, S, I, ($($ty,)* $last,)>
        where
            F: FnMut($($ty,)* $last, Next) -> Fut + Clone + Send + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S> + Send,
            Fut: Future<Output = Out> + Send + 'static,
            Out: IntoResponse + 'static,
            I: Service<Request, Error = Infallible>
                + Clone
                + Send
                + Sync
                + 'static,
            I::Response: IntoResponse,
            I::Future: Send + 'static,
            S: Clone + Send + Sync + 'static,
        {
            type Response = Response;
            type Error = Infallible;
            type Future = ResponseFuture;

            fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                self.inner.poll_ready(cx)
            }

            fn call(&mut self, req: Request) -> Self::Future {
                let not_ready_inner = self.inner.clone();
                let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

                let mut f = self.f.clone();
                let state = self.state.clone();
                let (mut parts, body) = req.into_parts();

                let future = Box::pin(async move {
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let $last = match $last::from_request(req, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };

                    let inner = BoxCloneSyncService::new(MapIntoResponse::new(ready_inner));
                    let next = Next { inner };

                    f($($ty,)* $last, next).await.into_response()
                });

                ResponseFuture {
                    inner: future
                }
            }
        }
    };
}

all_the_tuples!(impl_service);

impl<F, S, I, T> fmt::Debug for FromFn<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFnLayer")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .field("state", &self.state)
            .finish()
    }
}

/// The remainder of a middleware stack, including the handler.
#[derive(Debug, Clone)]
pub struct Next {
    inner: BoxCloneSyncService<Request, Response, Infallible>,
}

impl Next {
    /// Execute the remaining middleware stack.
    pub async fn run(mut self, req: Request) -> Response {
        match self.inner.call(req).await {
            Ok(res) => res,
            Err(err) => match err {},
        }
    }
}

impl Service<Request> for Next {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call(req)
    }
}

/// Response future for [`FromFn`].
pub struct ResponseFuture {
    inner: BoxFuture<'static, Response>,
}

impl Future for ResponseFuture {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx).map(Ok)
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{body::Body, routing::get, Router};
    use http::{HeaderMap, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[crate::test]
    async fn basic() {
        async fn insert_header(mut req: Request, next: Next) -> impl IntoResponse {
            req.headers_mut()
                .insert("x-axum-test", "ok".parse().unwrap());

            next.run(req).await
        }

        async fn handle(headers: HeaderMap) -> String {
            headers["x-axum-test"].to_str().unwrap().to_owned()
        }

        let app = Router::new()
            .route("/", get(handle))
            .layer(from_fn(insert_header));

        let res = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"ok");
    }
}
