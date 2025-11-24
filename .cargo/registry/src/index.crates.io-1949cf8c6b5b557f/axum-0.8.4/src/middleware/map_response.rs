use crate::response::{IntoResponse, Response};
use axum_core::extract::FromRequestParts;
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

/// Create a middleware from an async function that transforms a response.
///
/// This differs from [`tower::util::MapResponse`] in that it allows you to easily run axum-specific
/// extractors.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::map_response,
///     response::Response,
/// };
///
/// async fn set_header<B>(mut response: Response<B>) -> Response<B> {
///     response.headers_mut().insert("x-foo", "foo".parse().unwrap());
///     response
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .layer(map_response(set_header));
/// # let _: Router = app;
/// ```
///
/// # Running extractors
///
/// It is also possible to run extractors that implement [`FromRequestParts`]. These will be run
/// before calling the handler.
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::map_response,
///     extract::Path,
///     response::Response,
/// };
/// use std::collections::HashMap;
///
/// async fn log_path_params<B>(
///     Path(path_params): Path<HashMap<String, String>>,
///     response: Response<B>,
/// ) -> Response<B> {
///     tracing::debug!(?path_params);
///     response
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .layer(map_response(log_path_params));
/// # let _: Router = app;
/// ```
///
/// Note that to access state you must use either [`map_response_with_state`].
///
/// # Returning any `impl IntoResponse`
///
/// It is also possible to return anything that implements [`IntoResponse`]
///
/// ```
/// use axum::{
///     Router,
///     routing::get,
///     middleware::map_response,
///     response::{Response, IntoResponse},
/// };
/// use std::collections::HashMap;
///
/// async fn set_header(response: Response) -> impl IntoResponse {
///     (
///         [("x-foo", "foo")],
///         response,
///     )
/// }
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .layer(map_response(set_header));
/// # let _: Router = app;
/// ```
pub fn map_response<F, T>(f: F) -> MapResponseLayer<F, (), T> {
    map_response_with_state((), f)
}

/// Create a middleware from an async function that transforms a response, with the given state.
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
///     response::Response,
///     middleware::map_response_with_state,
///     extract::State,
/// };
///
/// #[derive(Clone)]
/// struct AppState { /* ... */ }
///
/// async fn my_middleware<B>(
///     State(state): State<AppState>,
///     // you can add more extractors here but they must
///     // all implement `FromRequestParts`
///     // `FromRequest` is not allowed
///     response: Response<B>,
/// ) -> Response<B> {
///     // do something with `state` and `response`...
///     response
/// }
///
/// let state = AppState { /* ... */ };
///
/// let app = Router::new()
///     .route("/", get(|| async { /* ... */ }))
///     .route_layer(map_response_with_state(state.clone(), my_middleware))
///     .with_state(state);
/// # let _: axum::Router = app;
/// ```
pub fn map_response_with_state<F, S, T>(state: S, f: F) -> MapResponseLayer<F, S, T> {
    MapResponseLayer {
        f,
        state,
        _extractor: PhantomData,
    }
}

/// A [`tower::Layer`] from an async function that transforms a response.
///
/// Created with [`map_response`]. See that function for more details.
#[must_use]
pub struct MapResponseLayer<F, S, T> {
    f: F,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, T> Clone for MapResponseLayer<F, S, T>
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

impl<S, I, F, T> Layer<I> for MapResponseLayer<F, S, T>
where
    F: Clone,
    S: Clone,
{
    type Service = MapResponse<F, S, I, T>;

    fn layer(&self, inner: I) -> Self::Service {
        MapResponse {
            f: self.f.clone(),
            state: self.state.clone(),
            inner,
            _extractor: PhantomData,
        }
    }
}

impl<F, S, T> fmt::Debug for MapResponseLayer<F, S, T>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponseLayer")
            // Write out the type name, without quoting it as `&type_name::<F>()` would
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("state", &self.state)
            .finish()
    }
}

/// A middleware created from an async function that transforms a response.
///
/// Created with [`map_response`]. See that function for more details.
pub struct MapResponse<F, S, I, T> {
    f: F,
    inner: I,
    state: S,
    _extractor: PhantomData<fn() -> T>,
}

impl<F, S, I, T> Clone for MapResponse<F, S, I, T>
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
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, I, B, ResBody, $($ty,)*> Service<Request<B>> for MapResponse<F, S, I, ($($ty,)*)>
        where
            F: FnMut($($ty,)* Response<ResBody>) -> Fut + Clone + Send + 'static,
            $( $ty: FromRequestParts<S> + Send, )*
            Fut: Future + Send + 'static,
            Fut::Output: IntoResponse + Send + 'static,
            I: Service<Request<B>, Response = Response<ResBody>, Error = Infallible>
                + Clone
                + Send
                + 'static,
            I::Future: Send + 'static,
            B: Send + 'static,
            ResBody: Send + 'static,
            S: Clone + Send + Sync + 'static,
        {
            type Response = Response;
            type Error = Infallible;
            type Future = ResponseFuture;

            fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                self.inner.poll_ready(cx)
            }


            fn call(&mut self, req: Request<B>) -> Self::Future {
                let not_ready_inner = self.inner.clone();
                let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

                let mut f = self.f.clone();
                let _state = self.state.clone();
                let (mut parts, body) = req.into_parts();

                let future = Box::pin(async move {
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &_state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    match ready_inner.call(req).await {
                        Ok(res) => {
                            f($($ty,)* res).await.into_response()
                        }
                        Err(err) => match err {}
                    }
                });

                ResponseFuture {
                    inner: future
                }
            }
        }
    };
}

impl_service!();
impl_service!(T1);
impl_service!(T1, T2);
impl_service!(T1, T2, T3);
impl_service!(T1, T2, T3, T4);
impl_service!(T1, T2, T3, T4, T5);
impl_service!(T1, T2, T3, T4, T5, T6);
impl_service!(T1, T2, T3, T4, T5, T6, T7);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_service!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);

impl<F, S, I, T> fmt::Debug for MapResponse<F, S, I, T>
where
    S: fmt::Debug,
    I: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponse")
            .field("f", &format_args!("{}", type_name::<F>()))
            .field("inner", &self.inner)
            .field("state", &self.state)
            .finish()
    }
}

/// Response future for [`MapResponse`].
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
    #[allow(unused_imports)]
    use super::*;
    use crate::{test_helpers::TestClient, Router};

    #[crate::test]
    async fn works() {
        async fn add_header<B>(mut res: Response<B>) -> Response<B> {
            res.headers_mut().insert("x-foo", "foo".parse().unwrap());
            res
        }

        let app = Router::new().layer(map_response(add_header));
        let client = TestClient::new(app);

        let res = client.get("/").await;

        assert_eq!(res.headers()["x-foo"], "foo");
    }
}
