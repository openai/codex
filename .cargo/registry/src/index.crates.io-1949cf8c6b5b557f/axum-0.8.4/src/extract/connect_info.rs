//! Extractor for getting connection information from a client.
//!
//! See [`Router::into_make_service_with_connect_info`] for more details.
//!
//! [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info

use crate::extension::AddExtension;

use super::{Extension, FromRequestParts};
use http::request::Parts;
use std::{
    convert::Infallible,
    fmt,
    future::ready,
    marker::PhantomData,
    net::SocketAddr,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// A [`MakeService`] created from a router.
///
/// See [`Router::into_make_service_with_connect_info`] for more details.
///
/// [`MakeService`]: tower::make::MakeService
/// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
pub struct IntoMakeServiceWithConnectInfo<S, C> {
    svc: S,
    _connect_info: PhantomData<fn() -> C>,
}

#[test]
fn traits() {
    use crate::test_helpers::*;
    assert_send::<IntoMakeServiceWithConnectInfo<(), NotSendSync>>();
}

impl<S, C> IntoMakeServiceWithConnectInfo<S, C> {
    pub(crate) fn new(svc: S) -> Self {
        Self {
            svc,
            _connect_info: PhantomData,
        }
    }
}

impl<S, C> fmt::Debug for IntoMakeServiceWithConnectInfo<S, C>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoMakeServiceWithConnectInfo")
            .field("svc", &self.svc)
            .finish()
    }
}

impl<S, C> Clone for IntoMakeServiceWithConnectInfo<S, C>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            svc: self.svc.clone(),
            _connect_info: PhantomData,
        }
    }
}

/// Trait that connected IO resources implement and use to produce information
/// about the connection.
///
/// The goal for this trait is to allow users to implement custom IO types that
/// can still provide the same connection metadata.
///
/// See [`Router::into_make_service_with_connect_info`] for more details.
///
/// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
pub trait Connected<T>: Clone + Send + Sync + 'static {
    /// Create type holding information about the connection.
    fn connect_info(stream: T) -> Self;
}

#[cfg(all(feature = "tokio", any(feature = "http1", feature = "http2")))]
const _: () = {
    use crate::serve;
    use tokio::net::TcpListener;

    impl Connected<serve::IncomingStream<'_, TcpListener>> for SocketAddr {
        fn connect_info(stream: serve::IncomingStream<'_, TcpListener>) -> Self {
            *stream.remote_addr()
        }
    }

    impl<'a, L, F> Connected<serve::IncomingStream<'a, serve::TapIo<L, F>>> for L::Addr
    where
        L: serve::Listener,
        L::Addr: Clone + Sync + 'static,
        F: FnMut(&mut L::Io) + Send + 'static,
    {
        fn connect_info(stream: serve::IncomingStream<'a, serve::TapIo<L, F>>) -> Self {
            stream.remote_addr().clone()
        }
    }
};

impl Connected<SocketAddr> for SocketAddr {
    fn connect_info(remote_addr: SocketAddr) -> Self {
        remote_addr
    }
}

impl<S, C, T> Service<T> for IntoMakeServiceWithConnectInfo<S, C>
where
    S: Clone,
    C: Connected<T>,
{
    type Response = AddExtension<S, ConnectInfo<C>>;
    type Error = Infallible;
    type Future = ResponseFuture<S, C>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: T) -> Self::Future {
        let connect_info = ConnectInfo(C::connect_info(target));
        let svc = Extension(connect_info).layer(self.svc.clone());
        ResponseFuture::new(ready(Ok(svc)))
    }
}

opaque_future! {
    /// Response future for [`IntoMakeServiceWithConnectInfo`].
    pub type ResponseFuture<S, C> =
        std::future::Ready<Result<AddExtension<S, ConnectInfo<C>>, Infallible>>;
}

/// Extractor for getting connection information produced by a [`Connected`].
///
/// Note this extractor requires you to use
/// [`Router::into_make_service_with_connect_info`] to run your app
/// otherwise it will fail at runtime.
///
/// See [`Router::into_make_service_with_connect_info`] for more details.
///
/// [`Router::into_make_service_with_connect_info`]: crate::routing::Router::into_make_service_with_connect_info
#[derive(Clone, Copy, Debug)]
pub struct ConnectInfo<T>(pub T);

impl<S, T> FromRequestParts<S> for ConnectInfo<T>
where
    S: Send + Sync,
    T: Clone + Send + Sync + 'static,
{
    type Rejection = <Extension<Self> as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Extension::<Self>::from_request_parts(parts, state).await {
            Ok(Extension(connect_info)) => Ok(connect_info),
            Err(err) => match parts.extensions.get::<MockConnectInfo<T>>() {
                Some(MockConnectInfo(connect_info)) => Ok(Self(connect_info.clone())),
                None => Err(err),
            },
        }
    }
}

axum_core::__impl_deref!(ConnectInfo);

/// Middleware used to mock [`ConnectInfo`] during tests.
///
/// If you're accidentally using [`MockConnectInfo`] and
/// [`Router::into_make_service_with_connect_info`] at the same time then
/// [`Router::into_make_service_with_connect_info`] takes precedence.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     extract::connect_info::{MockConnectInfo, ConnectInfo},
///     body::Body,
///     routing::get,
///     http::{Request, StatusCode},
/// };
/// use std::net::SocketAddr;
/// use tower::ServiceExt;
///
/// async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) {}
///
/// // this router you can run with `app.into_make_service_with_connect_info::<SocketAddr>()`
/// fn app() -> Router {
///     Router::new().route("/", get(handler))
/// }
///
/// // use this router for tests
/// fn test_app() -> Router {
///     app().layer(MockConnectInfo(SocketAddr::from(([0, 0, 0, 0], 1337))))
/// }
///
/// // #[tokio::test]
/// async fn some_test() {
///     let app = test_app();
///
///     let request = Request::new(Body::empty());
///     let response = app.oneshot(request).await.unwrap();
///     assert_eq!(response.status(), StatusCode::OK);
/// }
/// #
/// # #[tokio::main]
/// # async fn main() {
/// #     some_test().await;
/// # }
/// ```
///
/// [`Router::into_make_service_with_connect_info`]: crate::Router::into_make_service_with_connect_info
#[derive(Clone, Copy, Debug)]
pub struct MockConnectInfo<T>(pub T);

impl<S, T> Layer<S> for MockConnectInfo<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Service = <Extension<Self> as Layer<S>>::Service;

    fn layer(&self, inner: S) -> Self::Service {
        Extension(self.clone()).layer(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::get, serve::IncomingStream, test_helpers::TestClient, Router};
    use tokio::net::TcpListener;

    #[crate::test]
    async fn socket_addr() {
        async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
            format!("{addr}")
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let app = Router::new().route("/", get(handler));
            tx.send(()).unwrap();
            crate::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        rx.await.unwrap();

        let client = reqwest::Client::new();

        let res = client.get(format!("http://{addr}")).send().await.unwrap();
        let body = res.text().await.unwrap();
        assert!(body.starts_with("127.0.0.1:"));
    }

    #[crate::test]
    async fn custom() {
        #[derive(Clone, Debug)]
        struct MyConnectInfo {
            value: &'static str,
        }

        impl Connected<IncomingStream<'_, TcpListener>> for MyConnectInfo {
            fn connect_info(_target: IncomingStream<'_, TcpListener>) -> Self {
                Self {
                    value: "it worked!",
                }
            }
        }

        async fn handler(ConnectInfo(addr): ConnectInfo<MyConnectInfo>) -> &'static str {
            addr.value
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let app = Router::new().route("/", get(handler));
            tx.send(()).unwrap();
            crate::serve(
                listener,
                app.into_make_service_with_connect_info::<MyConnectInfo>(),
            )
            .await
            .unwrap();
        });
        rx.await.unwrap();

        let client = reqwest::Client::new();

        let res = client.get(format!("http://{addr}")).send().await.unwrap();
        let body = res.text().await.unwrap();
        assert_eq!(body, "it worked!");
    }

    #[crate::test]
    async fn mock_connect_info() {
        async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
            format!("{addr}")
        }

        let app = Router::new()
            .route("/", get(handler))
            .layer(MockConnectInfo(SocketAddr::from(([0, 0, 0, 0], 1337))));

        let client = TestClient::new(app);

        let res = client.get("/").await;
        let body = res.text().await;
        assert!(body.starts_with("0.0.0.0:1337"));
    }

    #[crate::test]
    async fn both_mock_and_real_connect_info() {
        async fn handler(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> String {
            format!("{addr}")
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let app = Router::new()
                .route("/", get(handler))
                .layer(MockConnectInfo(SocketAddr::from(([0, 0, 0, 0], 1337))));

            crate::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });

        let client = reqwest::Client::new();

        let res = client.get(format!("http://{addr}")).send().await.unwrap();
        let body = res.text().await.unwrap();
        assert!(body.starts_with("127.0.0.1:"));
    }
}
