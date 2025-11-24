use super::Handler;
use crate::body::{Body, Bytes, HttpBody};
#[cfg(feature = "tokio")]
use crate::extract::connect_info::IntoMakeServiceWithConnectInfo;
use crate::response::Response;
use crate::routing::IntoMakeService;
use crate::BoxError;
use http::Request;
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    task::{Context, Poll},
};
use tower_service::Service;

/// An adapter that makes a [`Handler`] into a [`Service`].
///
/// Created with [`Handler::with_state`] or [`HandlerWithoutStateExt::into_service`].
///
/// [`HandlerWithoutStateExt::into_service`]: super::HandlerWithoutStateExt::into_service
pub struct HandlerService<H, T, S> {
    handler: H,
    state: S,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T, S> HandlerService<H, T, S> {
    /// Get a reference to the state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Convert the handler into a [`MakeService`].
    ///
    /// This allows you to serve a single handler if you don't need any routing:
    ///
    /// ```rust
    /// use axum::{
    ///     handler::Handler,
    ///     extract::State,
    ///     http::{Uri, Method},
    ///     response::IntoResponse,
    /// };
    /// use std::net::SocketAddr;
    ///
    /// #[derive(Clone)]
    /// struct AppState {}
    ///
    /// async fn handler(State(state): State<AppState>) {
    ///     // ...
    /// }
    ///
    /// let app = handler.with_state(AppState {});
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(listener, app.into_make_service()).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    pub fn into_make_service(self) -> IntoMakeService<HandlerService<H, T, S>> {
        IntoMakeService::new(self)
    }

    /// Convert the handler into a [`MakeService`] which stores information
    /// about the incoming connection.
    ///
    /// See [`Router::into_make_service_with_connect_info`] for more details.
    ///
    /// ```rust
    /// use axum::{
    ///     handler::Handler,
    ///     response::IntoResponse,
    ///     extract::{ConnectInfo, State},
    /// };
    /// use std::net::SocketAddr;
    ///
    /// #[derive(Clone)]
    /// struct AppState {};
    ///
    /// async fn handler(
    ///     ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ///     State(state): State<AppState>,
    /// ) -> String {
    ///     format!("Hello {addr}")
    /// }
    ///
    /// let app = handler.with_state(AppState {});
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    /// axum::serve(
    ///     listener,
    ///     app.into_make_service_with_connect_info::<SocketAddr>(),
    /// ).await.unwrap();
    /// # };
    /// ```
    ///
    /// [`MakeService`]: tower::make::MakeService
    /// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
    #[cfg(feature = "tokio")]
    pub fn into_make_service_with_connect_info<C>(
        self,
    ) -> IntoMakeServiceWithConnectInfo<HandlerService<H, T, S>, C> {
        IntoMakeServiceWithConnectInfo::new(self)
    }
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<HandlerService<(), NotSendSync, ()>>();
    assert_sync::<HandlerService<(), NotSendSync, ()>>();
}

impl<H, T, S> HandlerService<H, T, S> {
    pub(super) fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}

impl<H, T, S> fmt::Debug for HandlerService<H, T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoService").finish_non_exhaustive()
    }
}

impl<H, T, S> Clone for HandlerService<H, T, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T, S, B> Service<Request<B>> for HandlerService<H, T, S>
where
    H: Handler<T, S> + Clone + Send + 'static,
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
    S: Clone + Send + Sync,
{
    type Response = Response;
    type Error = Infallible;
    type Future = super::future::IntoServiceFuture<H::Future>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `IntoService` can only be constructed from async functions which are always ready, or
        // from `Layered` which buffers in `<Layered as Handler>::call` and is therefore
        // also always ready.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        use futures_util::future::FutureExt;

        let req = req.map(Body::new);

        let handler = self.handler.clone();
        let future = Handler::call(handler, req, self.state.clone());
        let future = future.map(Ok as _);

        super::future::IntoServiceFuture::new(future)
    }
}

// for `axum::serve(listener, handler)`
#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
const _: () = {
    use crate::serve;

    impl<H, T, S, L> Service<serve::IncomingStream<'_, L>> for HandlerService<H, T, S>
    where
        H: Clone,
        S: Clone,
        L: serve::Listener,
    {
        type Response = Self;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: serve::IncomingStream<'_, L>) -> Self::Future {
            std::future::ready(Ok(self.clone()))
        }
    }
};
