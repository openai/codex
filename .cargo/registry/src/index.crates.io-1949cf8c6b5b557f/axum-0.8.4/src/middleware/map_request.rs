use crate::body::{Body, Bytes, HttpBody};
use crate::response::{IntoResponse, Response};
use crate::BoxError;
use axum_core::extract::{FromRequest, FromRequestParts};
use futures_util::future::BoxFuture;
use http::Request;
use std::{
    any::type_name,
    convert::Infallible,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Create a middleware from an async function that transforms a request.
///
/// This differs from [`tower::util::MapRequest`] in that it allows you to easily run axum-specific
/// extractors.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::map_request,
///     http::Request,
/// };
///
/// async fn set_header<B>(mut request: Request<B>) -> Request<B> {
///     request.headers_mut().insert("x-foo", "foo".parse().unwrap());
///     request
/// }
///
/// async fn handler<B>(request: Request<B>) {
///     // `request` will have an `x-foo` header
/// }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(map_request(set_header));
/// # let _: Router = app;
/// ```
///
/// # Rejecting the request
///
/// The function given to `map_request` is allowed to also return a `Result` which can be used to
/// reject the request and return a response immediately, without calling the remaining
/// middleware.
///
/// Specifically the valid return types are:
///
/// - `Request<B>`
/// - `Result<Request<B>, E> where E:  IntoResponse`
///
/// ```
/// use axum::{
///     Router,
///     http::{Request, StatusCode},
///     routing::get,
///     middleware::map_request,
/// };
///
/// async fn auth<B>(request: Request<B>) -> Result<Request<B>, StatusCode> {
///     let auth_header = request.headers()
///         .get(http::header::AUTHORIZATION)
///         .and_then(|header| header.to_str().ok());
///
///     match auth_header {
///         Some(auth_header) if token_is_valid(auth_header) => Ok(request),
///         _ => Err(StatusCode::UNAUTHORIZED),
///     }
/// }
///
/// fn token_is_valid(token: &str) -> bool {
///     // ...
///     # false
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(map_request(auth));
/// # let app: Router = app;
/// ```
///
/// # Running extractors
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::map_request,
///     extract::Path,
///     http::Request,
/// };
/// use std::collections::HashMap;
///
/// async fn log_path_params<B>(
///     Path(path_params): Path<HashMap<String, String>>,
///     request: Request<B>,
/// ) -> Request<B> {
///     tracing::debug!(?path_params);
///     request
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .layer(map_request(log_path_params));
/// # let _: Router = app;
/// ```
///
/// Note that to access state you must use either [`map_request_with_state`].
pub fn map_request<F, T>(f: F) -> MapRequestLayer<F, (), T> {
    map_request_with_state((), f)
}

/// Create a middleware from an async function that transforms a request, with the given state.
///
/// See [`State`](crate::extract::State) for more details about accessing state.
///
/// # Example
///
/// ```rust
/// use axum::{
///     Router,
///     http::{Request, StatusCode},
///     routing::get,
///     response::IntoResponse,
///     middleware::map_request_with_state,
///     extract::State,
/// };
///
/// #[derive(Clone)]
/// struct AppState { /* ... */ }
///
/// async fn my_middleware<B>(
///     State(state): State<AppState>,
///     // you can add more extractors here but the last
///     // extractor must implement `FromRequest` which
///     // `Request` does
///     request: Request<B>,
/// ) -> Request<B> {
///     // do something with `state` and `request`...
///     request
/// }
///
/// let state = AppState { /* ... */ };
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(map_request_with_state(state.clone(), my_middleware))
///     .with_state(state);
/// # let _: axum::Router = app;
/// ```
pub fn map_request_with_state<F, S, T>(state: S, f: F) -> MapRequestLayer<F, S, T> {
    MapRequestLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}

/// A [`tower::Layer`] from an async function that transforms a request.
///
/// Created with [`map_request`]. See that function for more details.
#[must_use]
pub struct MapRequestLayer<F, S, T> {
    f: F,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, T> Clone for MapRequestLayer<F, S, T>
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

impl<S, I, F, T> Layer<I> for MapRequestLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    type Service = MapRequest<F, S, I, T>;

    fn layer(&self, inner: I) -> Self::Service {
        MapRequest {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}

impl<F, S, T> fmt::Debug for MapRequestLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequestLayer")
            // Write out the type name, without quoting it as `&type_name::<F>()` would
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}

/// A middleware created from an async function that transforms a request.
///
/// Created with [`map_request`]. See that function for more details.
pub struct MapRequest<F, S, I, T> {
    f: F,
    inner: I,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, I, T> Clone for MapRequest<F, S, I, T>
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
        impl<F, Fut, S, I, B, $($ty,)* $last> Service<Request<B>> for MapRequest<F, S, I, ($($ty,)* $last,)>
        where
            F: FnMut($($ty,)* $last) -> Fut + Clone + Send + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
            $last: FromRequest<S> + Send,
            Fut: Future + Send + 'static,
            Fut::Output: IntoMapRequestResult<B> + Send + 'static,
            I: Service<Request<B>, Error = Infallible>
                + Clone
                + Send
                + 'static,
            I::Response: IntoResponse,
            I::Future: Send + 'static,
            B: HttpBody<Data = Bytes> + Send + 'static,
            B::Error: Into<BoxError>,
            S: Clone + Send + Sync + 'static,
        {
            type Response = Response;
            type Error = Infallible;
            type Future = ResponseFuture;

            fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                self.inner.poll_ready(cx)
            }

            fn call(&mut self, req: Request<B>) -> Self::Future {
                let req = req.map(Body::new);

                let not_ready_inner = self.inner.clone();
                let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

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

                    match f($($ty,)* $last).await.into_map_request_result() {
                        Ok(req) => {
                            ready_inner.call(req).await.into_response()
                        }
                        Err(res) => {
                            res
                        }
                    }
                });

                ResponseFuture {
                    inner: future
                }
            }
        }
    };
}

all_the_tuples!(impl_service);

impl<F, S, I, T> fmt::Debug for MapRequest<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequest")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .field("state", &self.state)
            .finish()
    }
}

/// Response future for [`MapRequest`].
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

mod private {
    use crate::{http::Request, response::IntoResponse};

    pub trait Sealed<B> {}
    impl<B, E> Sealed<B> for Result<Request<B>, E> where E: IntoResponse {}
    impl<B> Sealed<B> for Request<B> {}
}

/// Trait implemented by types that can be returned from [`map_request`],
/// [`map_request_with_state`].
///
/// This trait is sealed such that it cannot be implemented outside this crate.
pub trait IntoMapRequestResult<B>: private::Sealed<B> {
    /// Perform the conversion.
    #[allow(clippy::result_large_err)]
    fn into_map_request_result(self) -> Result<Request<B>, Response>;
}

impl<B, E> IntoMapRequestResult<B> for Result<Request<B>, E>
where
    E: IntoResponse,
{
    fn into_map_request_result(self) -> Result<Request<B>, Response> {
        self.map_err(IntoResponse::into_response)
    }
}

impl<B> IntoMapRequestResult<B> for Request<B> {
    fn into_map_request_result(self) -> Result<Request<B>, Response> {
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, test_helpers::TestClient, Router};
    use http::{HeaderMap, StatusCode};

    #[crate::test]
    async fn works() {
        async fn add_header<B>(mut req: Request<B>) -> Request<B> {
            req.headers_mut().insert("x-foo", "foo".parse().unwrap());
            req
        }

        async fn handler(headers: HeaderMap) -> Response {
            headers["x-foo"]
                .to_str()
                .unwrap()
                .to_owned()
                .into_response()
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(map_request(add_header));
        let client = TestClient::new(app);

        let res = client.get("/").await;

        assert_eq!(res.text().await, "foo");
    }

    #[crate::test]
    async fn works_for_short_circutting() {
        async fn add_header<B>(_req: Request<B>) -> Result<Request<B>, (StatusCode, &'static str)> {
            Err((StatusCode::INTERNAL_SERVER_ERROR, "something went wrong"))
        }

        async fn handler(_headers: HeaderMap) -> Response {
            unreachable!()
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(map_request(add_header));
        let client = TestClient::new(app);

        let res = client.get("/").await;

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(res.text().await, "something went wrong");
    }
}
