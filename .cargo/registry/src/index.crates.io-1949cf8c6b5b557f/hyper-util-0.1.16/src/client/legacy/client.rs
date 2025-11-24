//! The legacy HTTP Client from 0.14.x
//!
//! This `Client` will eventually be deconstructed into more composable parts.
//! For now, to enable people to use hyper 1.0 quicker, this `Client` exists
//! in much the same way it did in hyper 0.14.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{self, Poll};
use std::time::Duration;

use futures_util::future::{self, Either, FutureExt, TryFutureExt};
use http::uri::Scheme;
use hyper::client::conn::TrySendError as ConnTrySendError;
use hyper::header::{HeaderValue, HOST};
use hyper::rt::Timer;
use hyper::{body::Body, Method, Request, Response, Uri, Version};
use tracing::{debug, trace, warn};

use super::connect::capture::CaptureConnectionExtension;
#[cfg(feature = "tokio")]
use super::connect::HttpConnector;
use super::connect::{Alpn, Connect, Connected, Connection};
use super::pool::{self, Ver};

use crate::common::future::poll_fn;
use crate::common::{lazy as hyper_lazy, timer, Exec, Lazy, SyncWrapper};

type BoxSendFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A Client to make outgoing HTTP requests.
///
/// `Client` is cheap to clone and cloning is the recommended way to share a `Client`. The
/// underlying connection pool will be reused.
#[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2"))))]
pub struct Client<C, B> {
    config: Config,
    connector: C,
    exec: Exec,
    #[cfg(feature = "http1")]
    h1_builder: hyper::client::conn::http1::Builder,
    #[cfg(feature = "http2")]
    h2_builder: hyper::client::conn::http2::Builder<Exec>,
    pool: pool::Pool<PoolClient<B>, PoolKey>,
}

#[derive(Clone, Copy, Debug)]
struct Config {
    retry_canceled_requests: bool,
    set_host: bool,
    ver: Ver,
}

/// Client errors
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Send + Sync>>,
    #[cfg(any(feature = "http1", feature = "http2"))]
    connect_info: Option<Connected>,
}

#[derive(Debug)]
enum ErrorKind {
    Canceled,
    ChannelClosed,
    Connect,
    UserUnsupportedRequestMethod,
    UserUnsupportedVersion,
    UserAbsoluteUriRequired,
    SendRequest,
}

macro_rules! e {
    ($kind:ident) => {
        Error {
            kind: ErrorKind::$kind,
            source: None,
            connect_info: None,
        }
    };
    ($kind:ident, $src:expr) => {
        Error {
            kind: ErrorKind::$kind,
            source: Some($src.into()),
            connect_info: None,
        }
    };
}

// We might change this... :shrug:
type PoolKey = (http::uri::Scheme, http::uri::Authority);

enum TrySendError<B> {
    Retryable {
        error: Error,
        req: Request<B>,
        connection_reused: bool,
    },
    Nope(Error),
}

/// A `Future` that will resolve to an HTTP Response.
///
/// This is returned by `Client::request` (and `Client::get`).
#[must_use = "futures do nothing unless polled"]
pub struct ResponseFuture {
    inner: SyncWrapper<
        Pin<Box<dyn Future<Output = Result<Response<hyper::body::Incoming>, Error>> + Send>>,
    >,
}

// ===== impl Client =====

impl Client<(), ()> {
    /// Create a builder to configure a new `Client`.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "tokio")]
    /// # fn run () {
    /// use std::time::Duration;
    /// use hyper_util::client::legacy::Client;
    /// use hyper_util::rt::{TokioExecutor, TokioTimer};
    ///
    /// let client = Client::builder(TokioExecutor::new())
    ///     .pool_timer(TokioTimer::new())
    ///     .pool_idle_timeout(Duration::from_secs(30))
    ///     .http2_only(true)
    ///     .build_http();
    /// # let infer: Client<_, http_body_util::Full<bytes::Bytes>> = client;
    /// # drop(infer);
    /// # }
    /// # fn main() {}
    /// ```
    pub fn builder<E>(executor: E) -> Builder
    where
        E: hyper::rt::Executor<BoxSendFuture> + Send + Sync + Clone + 'static,
    {
        Builder::new(executor)
    }
}

impl<C, B> Client<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    /// Send a `GET` request to the supplied `Uri`.
    ///
    /// # Note
    ///
    /// This requires that the `Body` type have a `Default` implementation.
    /// It *should* return an "empty" version of itself, such that
    /// `Body::is_end_stream` is `true`.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "tokio")]
    /// # fn run () {
    /// use hyper::Uri;
    /// use hyper_util::client::legacy::Client;
    /// use hyper_util::rt::TokioExecutor;
    /// use bytes::Bytes;
    /// use http_body_util::Full;
    ///
    /// let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new()).build_http();
    ///
    /// let future = client.get(Uri::from_static("http://httpbin.org/ip"));
    /// # }
    /// # fn main() {}
    /// ```
    pub fn get(&self, uri: Uri) -> ResponseFuture
    where
        B: Default,
    {
        let body = B::default();
        if !body.is_end_stream() {
            warn!("default Body used for get() does not return true for is_end_stream");
        }

        let mut req = Request::new(body);
        *req.uri_mut() = uri;
        self.request(req)
    }

    /// Send a constructed `Request` using this `Client`.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "tokio")]
    /// # fn run () {
    /// use hyper::{Method, Request};
    /// use hyper_util::client::legacy::Client;
    /// use http_body_util::Full;
    /// use hyper_util::rt::TokioExecutor;
    /// use bytes::Bytes;
    ///
    /// let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new()).build_http();
    ///
    /// let req: Request<Full<Bytes>> = Request::builder()
    ///     .method(Method::POST)
    ///     .uri("http://httpbin.org/post")
    ///     .body(Full::from("Hallo!"))
    ///     .expect("request builder");
    ///
    /// let future = client.request(req);
    /// # }
    /// # fn main() {}
    /// ```
    pub fn request(&self, mut req: Request<B>) -> ResponseFuture {
        let is_http_connect = req.method() == Method::CONNECT;
        match req.version() {
            Version::HTTP_11 => (),
            Version::HTTP_10 => {
                if is_http_connect {
                    warn!("CONNECT is not allowed for HTTP/1.0");
                    return ResponseFuture::new(future::err(e!(UserUnsupportedRequestMethod)));
                }
            }
            Version::HTTP_2 => (),
            // completely unsupported HTTP version (like HTTP/0.9)!
            other => return ResponseFuture::error_version(other),
        };

        let pool_key = match extract_domain(req.uri_mut(), is_http_connect) {
            Ok(s) => s,
            Err(err) => {
                return ResponseFuture::new(future::err(err));
            }
        };

        ResponseFuture::new(self.clone().send_request(req, pool_key))
    }

    async fn send_request(
        self,
        mut req: Request<B>,
        pool_key: PoolKey,
    ) -> Result<Response<hyper::body::Incoming>, Error> {
        let uri = req.uri().clone();

        loop {
            req = match self.try_send_request(req, pool_key.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(TrySendError::Nope(err)) => return Err(err),
                Err(TrySendError::Retryable {
                    mut req,
                    error,
                    connection_reused,
                }) => {
                    if !self.config.retry_canceled_requests || !connection_reused {
                        // if client disabled, don't retry
                        // a fresh connection means we definitely can't retry
                        return Err(error);
                    }

                    trace!(
                        "unstarted request canceled, trying again (reason={:?})",
                        error
                    );
                    *req.uri_mut() = uri.clone();
                    req
                }
            }
        }
    }

    async fn try_send_request(
        &self,
        mut req: Request<B>,
        pool_key: PoolKey,
    ) -> Result<Response<hyper::body::Incoming>, TrySendError<B>> {
        let mut pooled = self
            .connection_for(pool_key)
            .await
            // `connection_for` already retries checkout errors, so if
            // it returns an error, there's not much else to retry
            .map_err(TrySendError::Nope)?;

        if let Some(conn) = req.extensions_mut().get_mut::<CaptureConnectionExtension>() {
            conn.set(&pooled.conn_info);
        }

        if pooled.is_http1() {
            if req.version() == Version::HTTP_2 {
                warn!("Connection is HTTP/1, but request requires HTTP/2");
                return Err(TrySendError::Nope(
                    e!(UserUnsupportedVersion).with_connect_info(pooled.conn_info.clone()),
                ));
            }

            if self.config.set_host {
                let uri = req.uri().clone();
                req.headers_mut().entry(HOST).or_insert_with(|| {
                    let hostname = uri.host().expect("authority implies host");
                    if let Some(port) = get_non_default_port(&uri) {
                        let s = format!("{hostname}:{port}");
                        HeaderValue::from_str(&s)
                    } else {
                        HeaderValue::from_str(hostname)
                    }
                    .expect("uri host is valid header value")
                });
            }

            // CONNECT always sends authority-form, so check it first...
            if req.method() == Method::CONNECT {
                authority_form(req.uri_mut());
            } else if pooled.conn_info.is_proxied {
                absolute_form(req.uri_mut());
            } else {
                origin_form(req.uri_mut());
            }
        } else if req.method() == Method::CONNECT && !pooled.is_http2() {
            authority_form(req.uri_mut());
        }

        let mut res = match pooled.try_send_request(req).await {
            Ok(res) => res,
            Err(mut err) => {
                return if let Some(req) = err.take_message() {
                    Err(TrySendError::Retryable {
                        connection_reused: pooled.is_reused(),
                        error: e!(Canceled, err.into_error())
                            .with_connect_info(pooled.conn_info.clone()),
                        req,
                    })
                } else {
                    Err(TrySendError::Nope(
                        e!(SendRequest, err.into_error())
                            .with_connect_info(pooled.conn_info.clone()),
                    ))
                }
            }
        };

        // If the Connector included 'extra' info, add to Response...
        if let Some(extra) = &pooled.conn_info.extra {
            extra.set(res.extensions_mut());
        }

        // If pooled is HTTP/2, we can toss this reference immediately.
        //
        // when pooled is dropped, it will try to insert back into the
        // pool. To delay that, spawn a future that completes once the
        // sender is ready again.
        //
        // This *should* only be once the related `Connection` has polled
        // for a new request to start.
        //
        // It won't be ready if there is a body to stream.
        if pooled.is_http2() || !pooled.is_pool_enabled() || pooled.is_ready() {
            drop(pooled);
        } else {
            let on_idle = poll_fn(move |cx| pooled.poll_ready(cx)).map(|_| ());
            self.exec.execute(on_idle);
        }

        Ok(res)
    }

    async fn connection_for(
        &self,
        pool_key: PoolKey,
    ) -> Result<pool::Pooled<PoolClient<B>, PoolKey>, Error> {
        loop {
            match self.one_connection_for(pool_key.clone()).await {
                Ok(pooled) => return Ok(pooled),
                Err(ClientConnectError::Normal(err)) => return Err(err),
                Err(ClientConnectError::CheckoutIsClosed(reason)) => {
                    if !self.config.retry_canceled_requests {
                        return Err(e!(Connect, reason));
                    }

                    trace!(
                        "unstarted request canceled, trying again (reason={:?})",
                        reason,
                    );
                    continue;
                }
            };
        }
    }

    async fn one_connection_for(
        &self,
        pool_key: PoolKey,
    ) -> Result<pool::Pooled<PoolClient<B>, PoolKey>, ClientConnectError> {
        // Return a single connection if pooling is not enabled
        if !self.pool.is_enabled() {
            return self
                .connect_to(pool_key)
                .await
                .map_err(ClientConnectError::Normal);
        }

        // This actually races 2 different futures to try to get a ready
        // connection the fastest, and to reduce connection churn.
        //
        // - If the pool has an idle connection waiting, that's used
        //   immediately.
        // - Otherwise, the Connector is asked to start connecting to
        //   the destination Uri.
        // - Meanwhile, the pool Checkout is watching to see if any other
        //   request finishes and tries to insert an idle connection.
        // - If a new connection is started, but the Checkout wins after
        //   (an idle connection became available first), the started
        //   connection future is spawned into the runtime to complete,
        //   and then be inserted into the pool as an idle connection.
        let checkout = self.pool.checkout(pool_key.clone());
        let connect = self.connect_to(pool_key);
        let is_ver_h2 = self.config.ver == Ver::Http2;

        // The order of the `select` is depended on below...

        match future::select(checkout, connect).await {
            // Checkout won, connect future may have been started or not.
            //
            // If it has, let it finish and insert back into the pool,
            // so as to not waste the socket...
            Either::Left((Ok(checked_out), connecting)) => {
                // This depends on the `select` above having the correct
                // order, such that if the checkout future were ready
                // immediately, the connect future will never have been
                // started.
                //
                // If it *wasn't* ready yet, then the connect future will
                // have been started...
                if connecting.started() {
                    let bg = connecting
                        .map_err(|err| {
                            trace!("background connect error: {}", err);
                        })
                        .map(|_pooled| {
                            // dropping here should just place it in
                            // the Pool for us...
                        });
                    // An execute error here isn't important, we're just trying
                    // to prevent a waste of a socket...
                    self.exec.execute(bg);
                }
                Ok(checked_out)
            }
            // Connect won, checkout can just be dropped.
            Either::Right((Ok(connected), _checkout)) => Ok(connected),
            // Either checkout or connect could get canceled:
            //
            // 1. Connect is canceled if this is HTTP/2 and there is
            //    an outstanding HTTP/2 connecting task.
            // 2. Checkout is canceled if the pool cannot deliver an
            //    idle connection reliably.
            //
            // In both cases, we should just wait for the other future.
            Either::Left((Err(err), connecting)) => {
                if err.is_canceled() {
                    connecting.await.map_err(ClientConnectError::Normal)
                } else {
                    Err(ClientConnectError::Normal(e!(Connect, err)))
                }
            }
            Either::Right((Err(err), checkout)) => {
                if err.is_canceled() {
                    checkout.await.map_err(move |err| {
                        if is_ver_h2 && err.is_canceled() {
                            ClientConnectError::CheckoutIsClosed(err)
                        } else {
                            ClientConnectError::Normal(e!(Connect, err))
                        }
                    })
                } else {
                    Err(ClientConnectError::Normal(err))
                }
            }
        }
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    fn connect_to(
        &self,
        pool_key: PoolKey,
    ) -> impl Lazy<Output = Result<pool::Pooled<PoolClient<B>, PoolKey>, Error>> + Send + Unpin
    {
        let executor = self.exec.clone();
        let pool = self.pool.clone();
        #[cfg(feature = "http1")]
        let h1_builder = self.h1_builder.clone();
        #[cfg(feature = "http2")]
        let h2_builder = self.h2_builder.clone();
        let ver = self.config.ver;
        let is_ver_h2 = ver == Ver::Http2;
        let connector = self.connector.clone();
        let dst = domain_as_uri(pool_key.clone());
        hyper_lazy(move || {
            // Try to take a "connecting lock".
            //
            // If the pool_key is for HTTP/2, and there is already a
            // connection being established, then this can't take a
            // second lock. The "connect_to" future is Canceled.
            let connecting = match pool.connecting(&pool_key, ver) {
                Some(lock) => lock,
                None => {
                    let canceled = e!(Canceled);
                    // TODO
                    //crate::Error::new_canceled().with("HTTP/2 connection in progress");
                    return Either::Right(future::err(canceled));
                }
            };
            Either::Left(
                connector
                    .connect(super::connect::sealed::Internal, dst)
                    .map_err(|src| e!(Connect, src))
                    .and_then(move |io| {
                        let connected = io.connected();
                        // If ALPN is h2 and we aren't http2_only already,
                        // then we need to convert our pool checkout into
                        // a single HTTP2 one.
                        let connecting = if connected.alpn == Alpn::H2 && !is_ver_h2 {
                            match connecting.alpn_h2(&pool) {
                                Some(lock) => {
                                    trace!("ALPN negotiated h2, updating pool");
                                    lock
                                }
                                None => {
                                    // Another connection has already upgraded,
                                    // the pool checkout should finish up for us.
                                    let canceled = e!(Canceled, "ALPN upgraded to HTTP/2");
                                    return Either::Right(future::err(canceled));
                                }
                            }
                        } else {
                            connecting
                        };

                        #[cfg_attr(not(feature = "http2"), allow(unused))]
                        let is_h2 = is_ver_h2 || connected.alpn == Alpn::H2;

                        Either::Left(Box::pin(async move {
                            let tx = if is_h2 {
                                #[cfg(feature = "http2")] {
                                    let (mut tx, conn) =
                                        h2_builder.handshake(io).await.map_err(Error::tx)?;

                                    trace!(
                                        "http2 handshake complete, spawning background dispatcher task"
                                    );
                                    executor.execute(
                                        conn.map_err(|e| debug!("client connection error: {}", e))
                                            .map(|_| ()),
                                    );

                                    // Wait for 'conn' to ready up before we
                                    // declare this tx as usable
                                    tx.ready().await.map_err(Error::tx)?;
                                    PoolTx::Http2(tx)
                                }
                                #[cfg(not(feature = "http2"))]
                                panic!("http2 feature is not enabled");
                            } else {
                                #[cfg(feature = "http1")] {
                                    // Perform the HTTP/1.1 handshake on the provided I/O stream.
                                    // Uses the h1_builder to establish a connection, returning a sender (tx) for requests
                                    // and a connection task (conn) that manages the connection lifecycle.
                                    let (mut tx, conn) =
                                        h1_builder.handshake(io).await.map_err(crate::client::legacy::client::Error::tx)?;

                                    // Log that the HTTP/1.1 handshake has completed successfully.
                                    // This indicates the connection is established and ready for request processing.
                                    trace!(
                                        "http1 handshake complete, spawning background dispatcher task"
                                    );
                                    // Create a oneshot channel to communicate errors from the connection task.
                                    // err_tx sends errors from the connection task, and err_rx receives them
                                    // to correlate connection failures with request readiness errors.
                                    let (err_tx, err_rx) = tokio::sync::oneshot::channel();
                                    // Spawn the connection task in the background using the executor.
                                    // The task manages the HTTP/1.1 connection, including upgrades (e.g., WebSocket).
                                    // Errors are sent via err_tx to ensure they can be checked if the sender (tx) fails.
                                    executor.execute(
                                        conn.with_upgrades()
                                            .map_err(|e| {
                                                // Log the connection error at debug level for diagnostic purposes.
                                                debug!("client connection error: {:?}", e);
                                                // Log that the error is being sent to the error channel.
                                                trace!("sending connection error to error channel");
                                                // Send the error via the oneshot channel, ignoring send failures
                                                // (e.g., if the receiver is dropped, which is handled later).
                                                let _ =err_tx.send(e);
                                            })
                                            .map(|_| ()),
                                    );
                                    // Log that the client is waiting for the connection to be ready.
                                    // Readiness indicates the sender (tx) can accept a request without blocking.
                                    trace!("waiting for connection to be ready");
                                    // Check if the sender is ready to accept a request.
                                    // This ensures the connection is fully established before proceeding.
                                    // aka:
                                    // Wait for 'conn' to ready up before we
                                    // declare this tx as usable
                                    match tx.ready().await {
                                        // If ready, the connection is usable for sending requests.
                                        Ok(_) => {
                                            // Log that the connection is ready for use.
                                            trace!("connection is ready");
                                            // Drop the error receiver, as it’s no longer needed since the sender is ready.
                                            // This prevents waiting for errors that won’t occur in a successful case.
                                            drop(err_rx);
                                            // Wrap the sender in PoolTx::Http1 for use in the connection pool.
                                            PoolTx::Http1(tx)
                                        }
                                        // If the sender fails with a closed channel error, check for a specific connection error.
                                        // This distinguishes between a vague ChannelClosed error and an actual connection failure.
                                        Err(e) if e.is_closed() => {
                                            // Log that the channel is closed, indicating a potential connection issue.
                                            trace!("connection channel closed, checking for connection error");
                                            // Check the oneshot channel for a specific error from the connection task.
                                            match err_rx.await {
                                                // If an error was received, it’s a specific connection failure.
                                                Ok(err) => {
                                                     // Log the specific connection error for diagnostics.
                                                    trace!("received connection error: {:?}", err);
                                                    // Return the error wrapped in Error::tx to propagate it.
                                                    return Err(crate::client::legacy::client::Error::tx(err));
                                                }
                                                // If the error channel is closed, no specific error was sent.
                                                // Fall back to the vague ChannelClosed error.
                                                Err(_) => {
                                                    // Log that the error channel is closed, indicating no specific error.
                                                    trace!("error channel closed, returning the vague ChannelClosed error");
                                                    // Return the original error wrapped in Error::tx.
                                                    return Err(crate::client::legacy::client::Error::tx(e));
                                                }
                                            }
                                        }
                                        // For other errors (e.g., timeout, I/O issues), propagate them directly.
                                        // These are not ChannelClosed errors and don’t require error channel checks.
                                        Err(e) => {
                                            // Log the specific readiness failure for diagnostics.
                                            trace!("connection readiness failed: {:?}", e);
                                            // Return the error wrapped in Error::tx to propagate it.
                                            return Err(crate::client::legacy::client::Error::tx(e));
                                        }
                                    }
                                }
                                #[cfg(not(feature = "http1"))] {
                                    panic!("http1 feature is not enabled");
                                }
                            };

                            Ok(pool.pooled(
                                connecting,
                                PoolClient {
                                    conn_info: connected,
                                    tx,
                                },
                            ))
                        }))
                    }),
            )
        })
    }
}

impl<C, B> tower_service::Service<Request<B>> for Client<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Response = Response<hyper::body::Incoming>;
    type Error = Error;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.request(req)
    }
}

impl<C, B> tower_service::Service<Request<B>> for &'_ Client<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Response = Response<hyper::body::Incoming>;
    type Error = Error;
    type Future = ResponseFuture;

    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.request(req)
    }
}

impl<C: Clone, B> Clone for Client<C, B> {
    fn clone(&self) -> Client<C, B> {
        Client {
            config: self.config,
            exec: self.exec.clone(),
            #[cfg(feature = "http1")]
            h1_builder: self.h1_builder.clone(),
            #[cfg(feature = "http2")]
            h2_builder: self.h2_builder.clone(),
            connector: self.connector.clone(),
            pool: self.pool.clone(),
        }
    }
}

impl<C, B> fmt::Debug for Client<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").finish()
    }
}

// ===== impl ResponseFuture =====

impl ResponseFuture {
    fn new<F>(value: F) -> Self
    where
        F: Future<Output = Result<Response<hyper::body::Incoming>, Error>> + Send + 'static,
    {
        Self {
            inner: SyncWrapper::new(Box::pin(value)),
        }
    }

    fn error_version(ver: Version) -> Self {
        warn!("Request has unsupported version \"{:?}\"", ver);
        ResponseFuture::new(Box::pin(future::err(e!(UserUnsupportedVersion))))
    }
}

impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Future<Response>")
    }
}

impl Future for ResponseFuture {
    type Output = Result<Response<hyper::body::Incoming>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.inner.get_mut().as_mut().poll(cx)
    }
}

// ===== impl PoolClient =====

// FIXME: allow() required due to `impl Trait` leaking types to this lint
#[allow(missing_debug_implementations)]
struct PoolClient<B> {
    conn_info: Connected,
    tx: PoolTx<B>,
}

enum PoolTx<B> {
    #[cfg(feature = "http1")]
    Http1(hyper::client::conn::http1::SendRequest<B>),
    #[cfg(feature = "http2")]
    Http2(hyper::client::conn::http2::SendRequest<B>),
}

impl<B> PoolClient<B> {
    fn poll_ready(
        &mut self,
        #[allow(unused_variables)] cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Error>> {
        match self.tx {
            #[cfg(feature = "http1")]
            PoolTx::Http1(ref mut tx) => tx.poll_ready(cx).map_err(Error::closed),
            #[cfg(feature = "http2")]
            PoolTx::Http2(_) => Poll::Ready(Ok(())),
        }
    }

    fn is_http1(&self) -> bool {
        !self.is_http2()
    }

    fn is_http2(&self) -> bool {
        match self.tx {
            #[cfg(feature = "http1")]
            PoolTx::Http1(_) => false,
            #[cfg(feature = "http2")]
            PoolTx::Http2(_) => true,
        }
    }

    fn is_poisoned(&self) -> bool {
        self.conn_info.poisoned.poisoned()
    }

    fn is_ready(&self) -> bool {
        match self.tx {
            #[cfg(feature = "http1")]
            PoolTx::Http1(ref tx) => tx.is_ready(),
            #[cfg(feature = "http2")]
            PoolTx::Http2(ref tx) => tx.is_ready(),
        }
    }
}

impl<B: Body + 'static> PoolClient<B> {
    fn try_send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = Result<Response<hyper::body::Incoming>, ConnTrySendError<Request<B>>>>
    where
        B: Send,
    {
        #[cfg(all(feature = "http1", feature = "http2"))]
        return match self.tx {
            #[cfg(feature = "http1")]
            PoolTx::Http1(ref mut tx) => Either::Left(tx.try_send_request(req)),
            #[cfg(feature = "http2")]
            PoolTx::Http2(ref mut tx) => Either::Right(tx.try_send_request(req)),
        };

        #[cfg(feature = "http1")]
        #[cfg(not(feature = "http2"))]
        return match self.tx {
            #[cfg(feature = "http1")]
            PoolTx::Http1(ref mut tx) => tx.try_send_request(req),
        };

        #[cfg(not(feature = "http1"))]
        #[cfg(feature = "http2")]
        return match self.tx {
            #[cfg(feature = "http2")]
            PoolTx::Http2(ref mut tx) => tx.try_send_request(req),
        };
    }
}

impl<B> pool::Poolable for PoolClient<B>
where
    B: Send + 'static,
{
    fn is_open(&self) -> bool {
        !self.is_poisoned() && self.is_ready()
    }

    fn reserve(self) -> pool::Reservation<Self> {
        match self.tx {
            #[cfg(feature = "http1")]
            PoolTx::Http1(tx) => pool::Reservation::Unique(PoolClient {
                conn_info: self.conn_info,
                tx: PoolTx::Http1(tx),
            }),
            #[cfg(feature = "http2")]
            PoolTx::Http2(tx) => {
                let b = PoolClient {
                    conn_info: self.conn_info.clone(),
                    tx: PoolTx::Http2(tx.clone()),
                };
                let a = PoolClient {
                    conn_info: self.conn_info,
                    tx: PoolTx::Http2(tx),
                };
                pool::Reservation::Shared(a, b)
            }
        }
    }

    fn can_share(&self) -> bool {
        self.is_http2()
    }
}

enum ClientConnectError {
    Normal(Error),
    CheckoutIsClosed(pool::Error),
}

fn origin_form(uri: &mut Uri) {
    let path = match uri.path_and_query() {
        Some(path) if path.as_str() != "/" => {
            let mut parts = ::http::uri::Parts::default();
            parts.path_and_query = Some(path.clone());
            Uri::from_parts(parts).expect("path is valid uri")
        }
        _none_or_just_slash => {
            debug_assert!(Uri::default() == "/");
            Uri::default()
        }
    };
    *uri = path
}

fn absolute_form(uri: &mut Uri) {
    debug_assert!(uri.scheme().is_some(), "absolute_form needs a scheme");
    debug_assert!(
        uri.authority().is_some(),
        "absolute_form needs an authority"
    );
    // If the URI is to HTTPS, and the connector claimed to be a proxy,
    // then it *should* have tunneled, and so we don't want to send
    // absolute-form in that case.
    if uri.scheme() == Some(&Scheme::HTTPS) {
        origin_form(uri);
    }
}

fn authority_form(uri: &mut Uri) {
    if let Some(path) = uri.path_and_query() {
        // `https://hyper.rs` would parse with `/` path, don't
        // annoy people about that...
        if path != "/" {
            warn!("HTTP/1.1 CONNECT request stripping path: {:?}", path);
        }
    }
    *uri = match uri.authority() {
        Some(auth) => {
            let mut parts = ::http::uri::Parts::default();
            parts.authority = Some(auth.clone());
            Uri::from_parts(parts).expect("authority is valid")
        }
        None => {
            unreachable!("authority_form with relative uri");
        }
    };
}

fn extract_domain(uri: &mut Uri, is_http_connect: bool) -> Result<PoolKey, Error> {
    let uri_clone = uri.clone();
    match (uri_clone.scheme(), uri_clone.authority()) {
        (Some(scheme), Some(auth)) => Ok((scheme.clone(), auth.clone())),
        (None, Some(auth)) if is_http_connect => {
            let scheme = match auth.port_u16() {
                Some(443) => {
                    set_scheme(uri, Scheme::HTTPS);
                    Scheme::HTTPS
                }
                _ => {
                    set_scheme(uri, Scheme::HTTP);
                    Scheme::HTTP
                }
            };
            Ok((scheme, auth.clone()))
        }
        _ => {
            debug!("Client requires absolute-form URIs, received: {:?}", uri);
            Err(e!(UserAbsoluteUriRequired))
        }
    }
}

fn domain_as_uri((scheme, auth): PoolKey) -> Uri {
    http::uri::Builder::new()
        .scheme(scheme)
        .authority(auth)
        .path_and_query("/")
        .build()
        .expect("domain is valid Uri")
}

fn set_scheme(uri: &mut Uri, scheme: Scheme) {
    debug_assert!(
        uri.scheme().is_none(),
        "set_scheme expects no existing scheme"
    );
    let old = std::mem::take(uri);
    let mut parts: ::http::uri::Parts = old.into();
    parts.scheme = Some(scheme);
    parts.path_and_query = Some("/".parse().expect("slash is a valid path"));
    *uri = Uri::from_parts(parts).expect("scheme is valid");
}

fn get_non_default_port(uri: &Uri) -> Option<http::uri::Port<&str>> {
    match (uri.port().map(|p| p.as_u16()), is_schema_secure(uri)) {
        (Some(443), true) => None,
        (Some(80), false) => None,
        _ => uri.port(),
    }
}

fn is_schema_secure(uri: &Uri) -> bool {
    uri.scheme_str()
        .map(|scheme_str| matches!(scheme_str, "wss" | "https"))
        .unwrap_or_default()
}

/// A builder to configure a new [`Client`](Client).
///
/// # Example
///
/// ```
/// # #[cfg(feature = "tokio")]
/// # fn run () {
/// use std::time::Duration;
/// use hyper_util::client::legacy::Client;
/// use hyper_util::rt::TokioExecutor;
///
/// let client = Client::builder(TokioExecutor::new())
///     .pool_idle_timeout(Duration::from_secs(30))
///     .http2_only(true)
///     .build_http();
/// # let infer: Client<_, http_body_util::Full<bytes::Bytes>> = client;
/// # drop(infer);
/// # }
/// # fn main() {}
/// ```
#[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2"))))]
#[derive(Clone)]
pub struct Builder {
    client_config: Config,
    exec: Exec,
    #[cfg(feature = "http1")]
    h1_builder: hyper::client::conn::http1::Builder,
    #[cfg(feature = "http2")]
    h2_builder: hyper::client::conn::http2::Builder<Exec>,
    pool_config: pool::Config,
    pool_timer: Option<timer::Timer>,
}

impl Builder {
    /// Construct a new Builder.
    pub fn new<E>(executor: E) -> Self
    where
        E: hyper::rt::Executor<BoxSendFuture> + Send + Sync + Clone + 'static,
    {
        let exec = Exec::new(executor);
        Self {
            client_config: Config {
                retry_canceled_requests: true,
                set_host: true,
                ver: Ver::Auto,
            },
            exec: exec.clone(),
            #[cfg(feature = "http1")]
            h1_builder: hyper::client::conn::http1::Builder::new(),
            #[cfg(feature = "http2")]
            h2_builder: hyper::client::conn::http2::Builder::new(exec),
            pool_config: pool::Config {
                idle_timeout: Some(Duration::from_secs(90)),
                max_idle_per_host: usize::MAX,
            },
            pool_timer: None,
        }
    }
    /// Set an optional timeout for idle sockets being kept-alive.
    /// A `Timer` is required for this to take effect. See `Builder::pool_timer`
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 90 seconds.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "tokio")]
    /// # fn run () {
    /// use std::time::Duration;
    /// use hyper_util::client::legacy::Client;
    /// use hyper_util::rt::{TokioExecutor, TokioTimer};
    ///
    /// let client = Client::builder(TokioExecutor::new())
    ///     .pool_idle_timeout(Duration::from_secs(30))
    ///     .pool_timer(TokioTimer::new())
    ///     .build_http();
    ///
    /// # let infer: Client<_, http_body_util::Full<bytes::Bytes>> = client;
    /// # }
    /// # fn main() {}
    /// ```
    pub fn pool_idle_timeout<D>(&mut self, val: D) -> &mut Self
    where
        D: Into<Option<Duration>>,
    {
        self.pool_config.idle_timeout = val.into();
        self
    }

    #[doc(hidden)]
    #[deprecated(note = "renamed to `pool_max_idle_per_host`")]
    pub fn max_idle_per_host(&mut self, max_idle: usize) -> &mut Self {
        self.pool_config.max_idle_per_host = max_idle;
        self
    }

    /// Sets the maximum idle connection per host allowed in the pool.
    ///
    /// Default is `usize::MAX` (no limit).
    pub fn pool_max_idle_per_host(&mut self, max_idle: usize) -> &mut Self {
        self.pool_config.max_idle_per_host = max_idle;
        self
    }

    // HTTP/1 options

    /// Sets the exact size of the read buffer to *always* use.
    ///
    /// Note that setting this option unsets the `http1_max_buf_size` option.
    ///
    /// Default is an adaptive read buffer.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_read_buf_exact_size(&mut self, sz: usize) -> &mut Self {
        self.h1_builder.read_buf_exact_size(Some(sz));
        self
    }

    /// Set the maximum buffer size for the connection.
    ///
    /// Default is ~400kb.
    ///
    /// Note that setting this option unsets the `http1_read_exact_buf_size` option.
    ///
    /// # Panics
    ///
    /// The minimum value allowed is 8192. This method panics if the passed `max` is less than the minimum.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_max_buf_size(&mut self, max: usize) -> &mut Self {
        self.h1_builder.max_buf_size(max);
        self
    }

    /// Set whether HTTP/1 connections will accept spaces between header names
    /// and the colon that follow them in responses.
    ///
    /// Newline codepoints (`\r` and `\n`) will be transformed to spaces when
    /// parsing.
    ///
    /// You probably don't need this, here is what [RFC 7230 Section 3.2.4.] has
    /// to say about it:
    ///
    /// > No whitespace is allowed between the header field-name and colon. In
    /// > the past, differences in the handling of such whitespace have led to
    /// > security vulnerabilities in request routing and response handling. A
    /// > server MUST reject any received request message that contains
    /// > whitespace between a header field-name and colon with a response code
    /// > of 400 (Bad Request). A proxy MUST remove any such whitespace from a
    /// > response message before forwarding the message downstream.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    ///
    /// [RFC 7230 Section 3.2.4.]: https://tools.ietf.org/html/rfc7230#section-3.2.4
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_allow_spaces_after_header_name_in_responses(&mut self, val: bool) -> &mut Self {
        self.h1_builder
            .allow_spaces_after_header_name_in_responses(val);
        self
    }

    /// Set whether HTTP/1 connections will accept obsolete line folding for
    /// header values.
    ///
    /// You probably don't need this, here is what [RFC 7230 Section 3.2.4.] has
    /// to say about it:
    ///
    /// > A server that receives an obs-fold in a request message that is not
    /// > within a message/http container MUST either reject the message by
    /// > sending a 400 (Bad Request), preferably with a representation
    /// > explaining that obsolete line folding is unacceptable, or replace
    /// > each received obs-fold with one or more SP octets prior to
    /// > interpreting the field value or forwarding the message downstream.
    ///
    /// > A proxy or gateway that receives an obs-fold in a response message
    /// > that is not within a message/http container MUST either discard the
    /// > message and replace it with a 502 (Bad Gateway) response, preferably
    /// > with a representation explaining that unacceptable line folding was
    /// > received, or replace each received obs-fold with one or more SP
    /// > octets prior to interpreting the field value or forwarding the
    /// > message downstream.
    ///
    /// > A user agent that receives an obs-fold in a response message that is
    /// > not within a message/http container MUST replace each received
    /// > obs-fold with one or more SP octets prior to interpreting the field
    /// > value.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    ///
    /// [RFC 7230 Section 3.2.4.]: https://tools.ietf.org/html/rfc7230#section-3.2.4
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_allow_obsolete_multiline_headers_in_responses(&mut self, val: bool) -> &mut Self {
        self.h1_builder
            .allow_obsolete_multiline_headers_in_responses(val);
        self
    }

    /// Sets whether invalid header lines should be silently ignored in HTTP/1 responses.
    ///
    /// This mimics the behaviour of major browsers. You probably don't want this.
    /// You should only want this if you are implementing a proxy whose main
    /// purpose is to sit in front of browsers whose users access arbitrary content
    /// which may be malformed, and they expect everything that works without
    /// the proxy to keep working with the proxy.
    ///
    /// This option will prevent Hyper's client from returning an error encountered
    /// when parsing a header, except if the error was caused by the character NUL
    /// (ASCII code 0), as Chrome specifically always reject those.
    ///
    /// The ignorable errors are:
    /// * empty header names;
    /// * characters that are not allowed in header names, except for `\0` and `\r`;
    /// * when `allow_spaces_after_header_name_in_responses` is not enabled,
    ///   spaces and tabs between the header name and the colon;
    /// * missing colon between header name and colon;
    /// * characters that are not allowed in header values except for `\0` and `\r`.
    ///
    /// If an ignorable error is encountered, the parser tries to find the next
    /// line in the input to resume parsing the rest of the headers. An error
    /// will be emitted nonetheless if it finds `\0` or a lone `\r` while
    /// looking for the next line.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_ignore_invalid_headers_in_responses(&mut self, val: bool) -> &mut Builder {
        self.h1_builder.ignore_invalid_headers_in_responses(val);
        self
    }

    /// Set whether HTTP/1 connections should try to use vectored writes,
    /// or always flatten into a single buffer.
    ///
    /// Note that setting this to false may mean more copies of body data,
    /// but may also improve performance when an IO transport doesn't
    /// support vectored writes well, such as most TLS implementations.
    ///
    /// Setting this to true will force hyper to use queued strategy
    /// which may eliminate unnecessary cloning on some TLS backends
    ///
    /// Default is `auto`. In this mode hyper will try to guess which
    /// mode to use
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_writev(&mut self, enabled: bool) -> &mut Builder {
        self.h1_builder.writev(enabled);
        self
    }

    /// Set whether HTTP/1 connections will write header names as title case at
    /// the socket level.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_title_case_headers(&mut self, val: bool) -> &mut Self {
        self.h1_builder.title_case_headers(val);
        self
    }

    /// Set whether to support preserving original header cases.
    ///
    /// Currently, this will record the original cases received, and store them
    /// in a private extension on the `Response`. It will also look for and use
    /// such an extension in any provided `Request`.
    ///
    /// Since the relevant extension is still private, there is no way to
    /// interact with the original cases. The only effect this can have now is
    /// to forward the cases in a proxy-like fashion.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_preserve_header_case(&mut self, val: bool) -> &mut Self {
        self.h1_builder.preserve_header_case(val);
        self
    }

    /// Set the maximum number of headers.
    ///
    /// When a response is received, the parser will reserve a buffer to store headers for optimal
    /// performance.
    ///
    /// If client receives more headers than the buffer size, the error "message header too large"
    /// is returned.
    ///
    /// The headers is allocated on the stack by default, which has higher performance. After
    /// setting this value, headers will be allocated in heap memory, that is, heap memory
    /// allocation will occur for each response, and there will be a performance drop of about 5%.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is 100.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1_max_headers(&mut self, val: usize) -> &mut Self {
        self.h1_builder.max_headers(val);
        self
    }

    /// Set whether HTTP/0.9 responses should be tolerated.
    ///
    /// Default is false.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http09_responses(&mut self, val: bool) -> &mut Self {
        self.h1_builder.http09_responses(val);
        self
    }

    /// Set whether the connection **must** use HTTP/2.
    ///
    /// The destination must either allow HTTP2 Prior Knowledge, or the
    /// `Connect` should be configured to do use ALPN to upgrade to `h2`
    /// as part of the connection process. This will not make the `Client`
    /// utilize ALPN by itself.
    ///
    /// Note that setting this to true prevents HTTP/1 from being allowed.
    ///
    /// Default is false.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_only(&mut self, val: bool) -> &mut Self {
        self.client_config.ver = if val { Ver::Http2 } else { Ver::Auto };
        self
    }

    /// Configures the maximum number of pending reset streams allowed before a GOAWAY will be sent.
    ///
    /// This will default to the default value set by the [`h2` crate](https://crates.io/crates/h2).
    /// As of v0.4.0, it is 20.
    ///
    /// See <https://github.com/hyperium/hyper/issues/2877> for more information.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_pending_accept_reset_streams(
        &mut self,
        max: impl Into<Option<usize>>,
    ) -> &mut Self {
        self.h2_builder.max_pending_accept_reset_streams(max.into());
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_INITIAL_WINDOW_SIZE
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.initial_stream_window_size(sz.into());
        self
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_initial_connection_window_size(
        &mut self,
        sz: impl Into<Option<u32>>,
    ) -> &mut Self {
        self.h2_builder.initial_connection_window_size(sz.into());
        self
    }

    /// Sets the initial maximum of locally initiated (send) streams.
    ///
    /// This value will be overwritten by the value included in the initial
    /// SETTINGS frame received from the peer as part of a [connection preface].
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    ///
    /// [connection preface]: https://httpwg.org/specs/rfc9113.html#preface
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_initial_max_send_streams(
        &mut self,
        initial: impl Into<Option<usize>>,
    ) -> &mut Self {
        self.h2_builder.initial_max_send_streams(initial);
        self
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in
    /// `http2_initial_stream_window_size` and
    /// `http2_initial_connection_window_size`.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_adaptive_window(&mut self, enabled: bool) -> &mut Self {
        self.h2_builder.adaptive_window(enabled);
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_frame_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.max_frame_size(sz);
        self
    }

    /// Sets the max size of received header frames for HTTP2.
    ///
    /// Default is currently 16KB, but can change.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.h2_builder.max_header_list_size(max);
        self
    }

    /// Sets an interval for HTTP2 Ping frames should be sent to keep a
    /// connection alive.
    ///
    /// Pass `None` to disable HTTP2 keep-alive.
    ///
    /// Default is currently disabled.
    ///
    /// # Cargo Feature
    ///
    /// Requires the `tokio` cargo feature to be enabled.
    #[cfg(feature = "tokio")]
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_keep_alive_interval(
        &mut self,
        interval: impl Into<Option<Duration>>,
    ) -> &mut Self {
        self.h2_builder.keep_alive_interval(interval);
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if `http2_keep_alive_interval` is disabled.
    ///
    /// Default is 20 seconds.
    ///
    /// # Cargo Feature
    ///
    /// Requires the `tokio` cargo feature to be enabled.
    #[cfg(feature = "tokio")]
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_keep_alive_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.h2_builder.keep_alive_timeout(timeout);
        self
    }

    /// Sets whether HTTP2 keep-alive should apply while the connection is idle.
    ///
    /// If disabled, keep-alive pings are only sent while there are open
    /// request/responses streams. If enabled, pings are also sent when no
    /// streams are active. Does nothing if `http2_keep_alive_interval` is
    /// disabled.
    ///
    /// Default is `false`.
    ///
    /// # Cargo Feature
    ///
    /// Requires the `tokio` cargo feature to be enabled.
    #[cfg(feature = "tokio")]
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_keep_alive_while_idle(&mut self, enabled: bool) -> &mut Self {
        self.h2_builder.keep_alive_while_idle(enabled);
        self
    }

    /// Sets the maximum number of HTTP2 concurrent locally reset streams.
    ///
    /// See the documentation of [`h2::client::Builder::max_concurrent_reset_streams`] for more
    /// details.
    ///
    /// The default value is determined by the `h2` crate.
    ///
    /// [`h2::client::Builder::max_concurrent_reset_streams`]: https://docs.rs/h2/client/struct.Builder.html#method.max_concurrent_reset_streams
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.h2_builder.max_concurrent_reset_streams(max);
        self
    }

    /// Provide a timer to be used for h2
    ///
    /// See the documentation of [`h2::client::Builder::timer`] for more
    /// details.
    ///
    /// [`h2::client::Builder::timer`]: https://docs.rs/h2/client/struct.Builder.html#method.timer
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        #[cfg(feature = "http2")]
        self.h2_builder.timer(timer);
        self
    }

    /// Provide a timer to be used for timeouts and intervals in connection pools.
    pub fn pool_timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Clone + Send + Sync + 'static,
    {
        self.pool_timer = Some(timer::Timer::new(timer.clone()));
        self
    }

    /// Set the maximum write buffer size for each HTTP/2 stream.
    ///
    /// Default is currently 1MB, but may change.
    ///
    /// # Panics
    ///
    /// The value must be no larger than `u32::MAX`.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_send_buf_size(&mut self, max: usize) -> &mut Self {
        self.h2_builder.max_send_buf_size(max);
        self
    }

    /// Set whether to retry requests that get disrupted before ever starting
    /// to write.
    ///
    /// This means a request that is queued, and gets given an idle, reused
    /// connection, and then encounters an error immediately as the idle
    /// connection was found to be unusable.
    ///
    /// When this is set to `false`, the related `ResponseFuture` would instead
    /// resolve to an `Error::Cancel`.
    ///
    /// Default is `true`.
    #[inline]
    pub fn retry_canceled_requests(&mut self, val: bool) -> &mut Self {
        self.client_config.retry_canceled_requests = val;
        self
    }

    /// Set whether to automatically add the `Host` header to requests.
    ///
    /// If true, and a request does not include a `Host` header, one will be
    /// added automatically, derived from the authority of the `Uri`.
    ///
    /// Default is `true`.
    #[inline]
    pub fn set_host(&mut self, val: bool) -> &mut Self {
        self.client_config.set_host = val;
        self
    }

    /// Build a client with this configuration and the default `HttpConnector`.
    #[cfg(feature = "tokio")]
    pub fn build_http<B>(&self) -> Client<HttpConnector, B>
    where
        B: Body + Send,
        B::Data: Send,
    {
        let mut connector = HttpConnector::new();
        if self.pool_config.is_enabled() {
            connector.set_keepalive(self.pool_config.idle_timeout);
        }
        self.build(connector)
    }

    /// Combine the configuration of this builder with a connector to create a `Client`.
    pub fn build<C, B>(&self, connector: C) -> Client<C, B>
    where
        C: Connect + Clone,
        B: Body + Send,
        B::Data: Send,
    {
        let exec = self.exec.clone();
        let timer = self.pool_timer.clone();
        Client {
            config: self.client_config,
            exec: exec.clone(),
            #[cfg(feature = "http1")]
            h1_builder: self.h1_builder.clone(),
            #[cfg(feature = "http2")]
            h2_builder: self.h2_builder.clone(),
            connector,
            pool: pool::Pool::new(self.pool_config, exec, timer),
        }
    }
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder")
            .field("client_config", &self.client_config)
            .field("pool_config", &self.pool_config)
            .finish()
    }
}

// ==== impl Error ====

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("hyper_util::client::legacy::Error");
        f.field(&self.kind);
        if let Some(ref cause) = self.source {
            f.field(cause);
        }
        f.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "client error ({:?})", self.kind)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|e| &**e as _)
    }
}

impl Error {
    /// Returns true if this was an error from `Connect`.
    pub fn is_connect(&self) -> bool {
        matches!(self.kind, ErrorKind::Connect)
    }

    /// Returns the info of the client connection on which this error occurred.
    #[cfg(any(feature = "http1", feature = "http2"))]
    pub fn connect_info(&self) -> Option<&Connected> {
        self.connect_info.as_ref()
    }

    #[cfg(any(feature = "http1", feature = "http2"))]
    fn with_connect_info(self, connect_info: Connected) -> Self {
        Self {
            connect_info: Some(connect_info),
            ..self
        }
    }
    fn is_canceled(&self) -> bool {
        matches!(self.kind, ErrorKind::Canceled)
    }

    fn tx(src: hyper::Error) -> Self {
        e!(SendRequest, src)
    }

    fn closed(src: hyper::Error) -> Self {
        e!(ChannelClosed, src)
    }
}
