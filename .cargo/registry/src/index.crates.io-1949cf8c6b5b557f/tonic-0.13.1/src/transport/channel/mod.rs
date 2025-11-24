//! Client implementation and builder.

mod endpoint;
pub(crate) mod service;
#[cfg(feature = "_tls-any")]
mod tls;
mod uds_connector;

pub use self::service::Change;
pub use endpoint::Endpoint;
#[cfg(feature = "_tls-any")]
pub use tls::ClientTlsConfig;

use self::service::{Connection, DynamicServiceStream, Executor, SharedExec};
use crate::body::Body;
use bytes::Bytes;
use http::{
    uri::{InvalidUri, Uri},
    Request, Response,
};
use std::{
    fmt,
    future::Future,
    hash::Hash,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::{channel, Sender};

use hyper::rt;
use tower::balance::p2c::Balance;
use tower::{
    buffer::{future::ResponseFuture as BufferResponseFuture, Buffer},
    discover::Discover,
    util::BoxService,
    Service,
};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

const DEFAULT_BUFFER_SIZE: usize = 1024;

/// A default batteries included `transport` channel.
///
/// This provides a fully featured http2 gRPC client based on `hyper`
/// and `tower` services.
///
/// # Multiplexing requests
///
/// Sending a request on a channel requires a `&mut self` and thus can only send
/// one request in flight. This is intentional and is required to follow the `Service`
/// contract from the `tower` library which this channel implementation is built on
/// top of.
///
/// `tower` itself has a concept of `poll_ready` which is the main mechanism to apply
/// back pressure. `poll_ready` takes a `&mut self` and when it returns `Poll::Ready`
/// we know the `Service` is able to accept only one request before we must `poll_ready`
/// again. Due to this fact any `async fn` that wants to poll for readiness and submit
/// the request must have a `&mut self` reference.
///
/// To work around this and to ease the use of the channel, `Channel` provides a
/// `Clone` implementation that is _cheap_. This is because at the very top level
/// the channel is backed by a `tower_buffer::Buffer` which runs the connection
/// in a background task and provides a `mpsc` channel interface. Due to this
/// cloning the `Channel` type is cheap and encouraged.
#[derive(Clone)]
pub struct Channel {
    svc: Buffer<Request<Body>, BoxFuture<'static, Result<Response<Body>, crate::BoxError>>>,
}

/// A future that resolves to an HTTP response.
///
/// This is returned by the `Service::call` on [`Channel`].
pub struct ResponseFuture {
    inner: BufferResponseFuture<BoxFuture<'static, Result<Response<Body>, crate::BoxError>>>,
}

impl Channel {
    /// Create an [`Endpoint`] builder that can create [`Channel`]s.
    pub fn builder(uri: Uri) -> Endpoint {
        Endpoint::from(uri)
    }

    /// Create an [`Endpoint`] from a static string.
    ///
    /// ```
    /// # use tonic::transport::Channel;
    /// Channel::from_static("https://example.com");
    /// ```
    pub fn from_static(s: &'static str) -> Endpoint {
        let uri = Uri::from_static(s);
        Self::builder(uri)
    }

    /// Create an [`Endpoint`] from shared bytes.
    ///
    /// ```
    /// # use tonic::transport::Channel;
    /// Channel::from_shared("https://example.com");
    /// ```
    pub fn from_shared(s: impl Into<Bytes>) -> Result<Endpoint, InvalidUri> {
        let uri = Uri::from_maybe_shared(s.into())?;
        Ok(Self::builder(uri))
    }

    /// Balance a list of [`Endpoint`]'s.
    ///
    /// This creates a [`Channel`] that will load balance across all the
    /// provided endpoints.
    pub fn balance_list(list: impl Iterator<Item = Endpoint>) -> Self {
        let (channel, tx) = Self::balance_channel(DEFAULT_BUFFER_SIZE);
        list.for_each(|endpoint| {
            tx.try_send(Change::Insert(endpoint.uri.clone(), endpoint))
                .unwrap();
        });

        channel
    }

    /// Balance a list of [`Endpoint`]'s.
    ///
    /// This creates a [`Channel`] that will listen to a stream of change events and will add or remove provided endpoints.
    pub fn balance_channel<K>(capacity: usize) -> (Self, Sender<Change<K, Endpoint>>)
    where
        K: Hash + Eq + Send + Clone + 'static,
    {
        Self::balance_channel_with_executor(capacity, SharedExec::tokio())
    }

    /// Balance a list of [`Endpoint`]'s.
    ///
    /// This creates a [`Channel`] that will listen to a stream of change events and will add or remove provided endpoints.
    ///
    /// The [`Channel`] will use the given executor to spawn async tasks.
    pub fn balance_channel_with_executor<K, E>(
        capacity: usize,
        executor: E,
    ) -> (Self, Sender<Change<K, Endpoint>>)
    where
        K: Hash + Eq + Send + Clone + 'static,
        E: Executor<Pin<Box<dyn Future<Output = ()> + Send>>> + Send + Sync + 'static,
    {
        let (tx, rx) = channel(capacity);
        let list = DynamicServiceStream::new(rx);
        (Self::balance(list, DEFAULT_BUFFER_SIZE, executor), tx)
    }

    /// Create a new [`Channel`] using a custom connector to the provided [Endpoint].
    ///
    /// This is a lower level API, prefer to use [`Endpoint::connect_lazy`] if you are not using a custom connector.
    pub fn new<C>(connector: C, endpoint: Endpoint) -> Self
    where
        C: Service<Uri> + Send + 'static,
        C::Error: Into<crate::BoxError> + Send,
        C::Future: Send,
        C::Response: rt::Read + rt::Write + Unpin + Send + 'static,
    {
        let buffer_size = endpoint.buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE);
        let executor = endpoint.executor.clone();

        let svc = Connection::lazy(connector, endpoint);
        let (svc, worker) = Buffer::pair(svc, buffer_size);

        executor.execute(worker);

        Channel { svc }
    }

    /// Connect to the provided [`Endpoint`] using the provided connector, and return a new [`Channel`].
    ///
    /// This is a lower level API, prefer to use [`Endpoint::connect`] if you are not using a custom connector.
    pub async fn connect<C>(connector: C, endpoint: Endpoint) -> Result<Self, super::Error>
    where
        C: Service<Uri> + Send + 'static,
        C::Error: Into<crate::BoxError> + Send,
        C::Future: Unpin + Send,
        C::Response: rt::Read + rt::Write + Unpin + Send + 'static,
    {
        let buffer_size = endpoint.buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE);
        let executor = endpoint.executor.clone();

        let svc = Connection::connect(connector, endpoint)
            .await
            .map_err(super::Error::from_source)?;
        let (svc, worker) = Buffer::pair(svc, buffer_size);
        executor.execute(worker);

        Ok(Channel { svc })
    }

    pub(crate) fn balance<D, E>(discover: D, buffer_size: usize, executor: E) -> Self
    where
        D: Discover<Service = Connection> + Unpin + Send + 'static,
        D::Error: Into<crate::BoxError>,
        D::Key: Hash + Send + Clone,
        E: Executor<BoxFuture<'static, ()>> + Send + Sync + 'static,
    {
        let svc = Balance::new(discover);

        let svc = BoxService::new(svc);
        let (svc, worker) = Buffer::pair(svc, buffer_size);
        executor.execute(Box::pin(worker));

        Channel { svc }
    }
}

impl Service<http::Request<Body>> for Channel {
    type Response = http::Response<Body>;
    type Error = super::Error;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(&mut self.svc, cx).map_err(super::Error::from_source)
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let inner = Service::call(&mut self.svc, request);

        ResponseFuture { inner }
    }
}

impl Future for ResponseFuture {
    type Output = Result<Response<Body>, super::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner)
            .poll(cx)
            .map_err(super::Error::from_source)
    }
}

impl fmt::Debug for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Channel").finish()
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}
