//! HTTP/2 client connections

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::rt::{Read, Write};
use futures_core::ready;
use http::{Request, Response};

use super::super::dispatch::{self, TrySendError};
use crate::body::{Body, Incoming as IncomingBody};
use crate::common::time::Time;
use crate::proto;
use crate::rt::bounds::Http2ClientConnExec;
use crate::rt::Timer;

/// The sender side of an established connection.
pub struct SendRequest<B> {
    dispatch: dispatch::UnboundedSender<Request<B>, Response<IncomingBody>>,
}

impl<B> Clone for SendRequest<B> {
    fn clone(&self) -> SendRequest<B> {
        SendRequest {
            dispatch: self.dispatch.clone(),
        }
    }
}

/// A future that processes all HTTP state for the IO object.
///
/// In most cases, this should just be spawned into an executor, so that it
/// can process incoming and outgoing messages, notice hangups, and the like.
///
/// Instances of this type are typically created via the [`handshake`] function
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B, E>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    E: Http2ClientConnExec<B, T> + Unpin,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    inner: (PhantomData<T>, proto::h2::ClientTask<B, E, T>),
}

/// A builder to configure an HTTP connection.
///
/// After setting options, the builder is used to create a handshake future.
///
/// **Note**: The default values of options are *not considered stable*. They
/// are subject to change at any time.
#[derive(Clone, Debug)]
pub struct Builder<Ex> {
    pub(super) exec: Ex,
    pub(super) timer: Time,
    h2_builder: proto::h2::client::Config,
}

/// Returns a handshake future over some IO.
///
/// This is a shortcut for `Builder::new(exec).handshake(io)`.
/// See [`client::conn`](crate::client::conn) for more.
pub async fn handshake<E, T, B>(
    exec: E,
    io: T,
) -> crate::Result<(SendRequest<B>, Connection<T, B, E>)>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
    E: Http2ClientConnExec<B, T> + Unpin + Clone,
{
    Builder::new(exec).handshake(io).await
}

// ===== impl SendRequest

impl<B> SendRequest<B> {
    /// Polls to determine whether this sender can be used yet for a request.
    ///
    /// If the associated connection is closed, this returns an Error.
    pub fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        if self.is_closed() {
            Poll::Ready(Err(crate::Error::new_closed()))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    /// Waits until the dispatcher is ready
    ///
    /// If the associated connection is closed, this returns an Error.
    pub async fn ready(&mut self) -> crate::Result<()> {
        crate::common::future::poll_fn(|cx| self.poll_ready(cx)).await
    }

    /// Checks if the connection is currently ready to send a request.
    ///
    /// # Note
    ///
    /// This is mostly a hint. Due to inherent latency of networks, it is
    /// possible that even after checking this is ready, sending a request
    /// may still fail because the connection was closed in the meantime.
    pub fn is_ready(&self) -> bool {
        self.dispatch.is_ready()
    }

    /// Checks if the connection side has been closed.
    pub fn is_closed(&self) -> bool {
        self.dispatch.is_closed()
    }
}

impl<B> SendRequest<B>
where
    B: Body + 'static,
{
    /// Sends a `Request` on the associated connection.
    ///
    /// Returns a future that if successful, yields the `Response`.
    ///
    /// `req` must have a `Host` header.
    ///
    /// Absolute-form `Uri`s are not required. If received, they will be serialized
    /// as-is.
    pub fn send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = crate::Result<Response<IncomingBody>>> {
        let sent = self.dispatch.send(req);

        async move {
            match sent {
                Ok(rx) => match rx.await {
                    Ok(Ok(resp)) => Ok(resp),
                    Ok(Err(err)) => Err(err),
                    // this is definite bug if it happens, but it shouldn't happen!
                    Err(_canceled) => panic!("dispatch dropped without returning error"),
                },
                Err(_req) => {
                    debug!("connection was not ready");

                    Err(crate::Error::new_canceled().with("connection was not ready"))
                }
            }
        }
    }

    /// Sends a `Request` on the associated connection.
    ///
    /// Returns a future that if successful, yields the `Response`.
    ///
    /// # Error
    ///
    /// If there was an error before trying to serialize the request to the
    /// connection, the message will be returned as part of this error.
    pub fn try_send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = Result<Response<IncomingBody>, TrySendError<Request<B>>>> {
        let sent = self.dispatch.try_send(req);
        async move {
            match sent {
                Ok(rx) => match rx.await {
                    Ok(Ok(res)) => Ok(res),
                    Ok(Err(err)) => Err(err),
                    // this is definite bug if it happens, but it shouldn't happen!
                    Err(_) => panic!("dispatch dropped without returning error"),
                },
                Err(req) => {
                    debug!("connection was not ready");
                    let error = crate::Error::new_canceled().with("connection was not ready");
                    Err(TrySendError {
                        error,
                        message: Some(req),
                    })
                }
            }
        }
    }
}

impl<B> fmt::Debug for SendRequest<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendRequest").finish()
    }
}

// ===== impl Connection

impl<T, B, E> Connection<T, B, E>
where
    T: Read + Write + Unpin + 'static,
    B: Body + Unpin + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
    E: Http2ClientConnExec<B, T> + Unpin,
{
    /// Returns whether the [extended CONNECT protocol][1] is enabled or not.
    ///
    /// This setting is configured by the server peer by sending the
    /// [`SETTINGS_ENABLE_CONNECT_PROTOCOL` parameter][2] in a `SETTINGS` frame.
    /// This method returns the currently acknowledged value received from the
    /// remote.
    ///
    /// [1]: https://datatracker.ietf.org/doc/html/rfc8441#section-4
    /// [2]: https://datatracker.ietf.org/doc/html/rfc8441#section-3
    pub fn is_extended_connect_protocol_enabled(&self) -> bool {
        self.inner.1.is_extended_connect_protocol_enabled()
    }
}

impl<T, B, E> fmt::Debug for Connection<T, B, E>
where
    T: Read + Write + fmt::Debug + 'static + Unpin,
    B: Body + 'static,
    E: Http2ClientConnExec<B, T> + Unpin,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

impl<T, B, E> Future for Connection<T, B, E>
where
    T: Read + Write + Unpin + 'static,
    B: Body + 'static + Unpin,
    B::Data: Send,
    E: Unpin,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
    E: Http2ClientConnExec<B, T> + Unpin,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.inner.1).poll(cx))? {
            proto::Dispatched::Shutdown => Poll::Ready(Ok(())),
            #[cfg(feature = "http1")]
            proto::Dispatched::Upgrade(_pending) => unreachable!("http2 cannot upgrade"),
        }
    }
}

// ===== impl Builder

impl<Ex> Builder<Ex>
where
    Ex: Clone,
{
    /// Creates a new connection builder.
    #[inline]
    pub fn new(exec: Ex) -> Builder<Ex> {
        Builder {
            exec,
            timer: Time::Empty,
            h2_builder: Default::default(),
        }
    }

    /// Provide a timer to execute background HTTP2 tasks.
    pub fn timer<M>(&mut self, timer: M) -> &mut Builder<Ex>
    where
        M: Timer + Send + Sync + 'static,
    {
        self.timer = Time::Timer(Arc::new(timer));
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    ///
    /// [spec]: https://httpwg.org/specs/rfc9113.html#SETTINGS_INITIAL_WINDOW_SIZE
    pub fn initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.adaptive_window = false;
            self.h2_builder.initial_stream_window_size = sz;
        }
        self
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn initial_connection_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        if let Some(sz) = sz.into() {
            self.h2_builder.adaptive_window = false;
            self.h2_builder.initial_conn_window_size = sz;
        }
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
    pub fn initial_max_send_streams(&mut self, initial: impl Into<Option<usize>>) -> &mut Self {
        if let Some(initial) = initial.into() {
            self.h2_builder.initial_max_send_streams = initial;
        }
        self
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in
    /// `initial_stream_window_size` and
    /// `initial_connection_window_size`.
    pub fn adaptive_window(&mut self, enabled: bool) -> &mut Self {
        use proto::h2::SPEC_WINDOW_SIZE;

        self.h2_builder.adaptive_window = enabled;
        if enabled {
            self.h2_builder.initial_conn_window_size = SPEC_WINDOW_SIZE;
            self.h2_builder.initial_stream_window_size = SPEC_WINDOW_SIZE;
        }
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Default is currently 16KB, but can change.
    pub fn max_frame_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.max_frame_size = sz.into();
        self
    }

    /// Sets the max size of received header frames.
    ///
    /// Default is currently 16KB, but can change.
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.h2_builder.max_header_list_size = max;
        self
    }

    /// Sets the header table size.
    ///
    /// This setting informs the peer of the maximum size of the header compression
    /// table used to encode header blocks, in octets. The encoder may select any value
    /// equal to or less than the header table size specified by the sender.
    ///
    /// The default value of crate `h2` is 4,096.
    pub fn header_table_size(&mut self, size: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.header_table_size = size.into();
        self
    }

    /// Sets the maximum number of concurrent streams.
    ///
    /// The maximum concurrent streams setting only controls the maximum number
    /// of streams that can be initiated by the remote peer. In other words,
    /// when this setting is set to 100, this does not limit the number of
    /// concurrent streams that can be created by the caller.
    ///
    /// It is recommended that this value be no smaller than 100, so as to not
    /// unnecessarily limit parallelism. However, any value is legal, including
    /// 0. If `max` is set to 0, then the remote will not be permitted to
    /// initiate streams.
    ///
    /// Note that streams in the reserved state, i.e., push promises that have
    /// been reserved but the stream has not started, do not count against this
    /// setting.
    ///
    /// Also note that if the remote *does* exceed the value set here, it is not
    /// a protocol level error. Instead, the `h2` library will immediately reset
    /// the stream.
    ///
    /// See [Section 5.1.2] in the HTTP/2 spec for more details.
    ///
    /// [Section 5.1.2]: https://http2.github.io/http2-spec/#rfc.section.5.1.2
    pub fn max_concurrent_streams(&mut self, max: impl Into<Option<u32>>) -> &mut Self {
        self.h2_builder.max_concurrent_streams = max.into();
        self
    }

    /// Sets an interval for HTTP2 Ping frames should be sent to keep a
    /// connection alive.
    ///
    /// Pass `None` to disable HTTP2 keep-alive.
    ///
    /// Default is currently disabled.
    pub fn keep_alive_interval(&mut self, interval: impl Into<Option<Duration>>) -> &mut Self {
        self.h2_builder.keep_alive_interval = interval.into();
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if `keep_alive_interval` is disabled.
    ///
    /// Default is 20 seconds.
    pub fn keep_alive_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.h2_builder.keep_alive_timeout = timeout;
        self
    }

    /// Sets whether HTTP2 keep-alive should apply while the connection is idle.
    ///
    /// If disabled, keep-alive pings are only sent while there are open
    /// request/responses streams. If enabled, pings are also sent when no
    /// streams are active. Does nothing if `keep_alive_interval` is
    /// disabled.
    ///
    /// Default is `false`.
    pub fn keep_alive_while_idle(&mut self, enabled: bool) -> &mut Self {
        self.h2_builder.keep_alive_while_idle = enabled;
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
    pub fn max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.h2_builder.max_concurrent_reset_streams = Some(max);
        self
    }

    /// Set the maximum write buffer size for each HTTP/2 stream.
    ///
    /// Default is currently 1MB, but may change.
    ///
    /// # Panics
    ///
    /// The value must be no larger than `u32::MAX`.
    pub fn max_send_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(max <= u32::MAX as usize);
        self.h2_builder.max_send_buffer_size = max;
        self
    }

    /// Configures the maximum number of pending reset streams allowed before a GOAWAY will be sent.
    ///
    /// This will default to the default value set by the [`h2` crate](https://crates.io/crates/h2).
    /// As of v0.4.0, it is 20.
    ///
    /// See <https://github.com/hyperium/hyper/issues/2877> for more information.
    pub fn max_pending_accept_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.h2_builder.max_pending_accept_reset_streams = max.into();
        self
    }

    /// Constructs a connection with the configured options and IO.
    /// See [`client::conn`](crate::client::conn) for more.
    ///
    /// Note, if [`Connection`] is not `await`-ed, [`SendRequest`] will
    /// do nothing.
    pub fn handshake<T, B>(
        &self,
        io: T,
    ) -> impl Future<Output = crate::Result<(SendRequest<B>, Connection<T, B, Ex>)>>
    where
        T: Read + Write + Unpin,
        B: Body + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        Ex: Http2ClientConnExec<B, T> + Unpin,
    {
        let opts = self.clone();

        async move {
            trace!("client handshake HTTP/2");

            let (tx, rx) = dispatch::channel();
            let h2 = proto::h2::client::handshake(io, rx, &opts.h2_builder, opts.exec, opts.timer)
                .await?;
            Ok((
                SendRequest {
                    dispatch: tx.unbound(),
                },
                Connection {
                    inner: (PhantomData, h2),
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    #[ignore] // only compilation is checked
    async fn send_sync_executor_of_non_send_futures() {
        #[derive(Clone)]
        struct LocalTokioExecutor;

        impl<F> crate::rt::Executor<F> for LocalTokioExecutor
        where
            F: std::future::Future + 'static, // not requiring `Send`
        {
            fn execute(&self, fut: F) {
                // This will spawn into the currently running `LocalSet`.
                tokio::task::spawn_local(fut);
            }
        }

        #[allow(unused)]
        async fn run(io: impl crate::rt::Read + crate::rt::Write + Unpin + 'static) {
            let (_sender, conn) = crate::client::conn::http2::handshake::<
                _,
                _,
                http_body_util::Empty<bytes::Bytes>,
            >(LocalTokioExecutor, io)
            .await
            .unwrap();

            tokio::task::spawn_local(async move {
                conn.await.unwrap();
            });
        }
    }

    #[tokio::test]
    #[ignore] // only compilation is checked
    async fn not_send_not_sync_executor_of_not_send_futures() {
        #[derive(Clone)]
        struct LocalTokioExecutor {
            _x: std::marker::PhantomData<std::rc::Rc<()>>,
        }

        impl<F> crate::rt::Executor<F> for LocalTokioExecutor
        where
            F: std::future::Future + 'static, // not requiring `Send`
        {
            fn execute(&self, fut: F) {
                // This will spawn into the currently running `LocalSet`.
                tokio::task::spawn_local(fut);
            }
        }

        #[allow(unused)]
        async fn run(io: impl crate::rt::Read + crate::rt::Write + Unpin + 'static) {
            let (_sender, conn) =
                crate::client::conn::http2::handshake::<_, _, http_body_util::Empty<bytes::Bytes>>(
                    LocalTokioExecutor {
                        _x: Default::default(),
                    },
                    io,
                )
                .await
                .unwrap();

            tokio::task::spawn_local(async move {
                conn.await.unwrap();
            });
        }
    }

    #[tokio::test]
    #[ignore] // only compilation is checked
    async fn send_not_sync_executor_of_not_send_futures() {
        #[derive(Clone)]
        struct LocalTokioExecutor {
            _x: std::marker::PhantomData<std::cell::Cell<()>>,
        }

        impl<F> crate::rt::Executor<F> for LocalTokioExecutor
        where
            F: std::future::Future + 'static, // not requiring `Send`
        {
            fn execute(&self, fut: F) {
                // This will spawn into the currently running `LocalSet`.
                tokio::task::spawn_local(fut);
            }
        }

        #[allow(unused)]
        async fn run(io: impl crate::rt::Read + crate::rt::Write + Unpin + 'static) {
            let (_sender, conn) =
                crate::client::conn::http2::handshake::<_, _, http_body_util::Empty<bytes::Bytes>>(
                    LocalTokioExecutor {
                        _x: Default::default(),
                    },
                    io,
                )
                .await
                .unwrap();

            tokio::task::spawn_local(async move {
                conn.await.unwrap();
            });
        }
    }

    #[tokio::test]
    #[ignore] // only compilation is checked
    async fn send_sync_executor_of_send_futures() {
        #[derive(Clone)]
        struct TokioExecutor;

        impl<F> crate::rt::Executor<F> for TokioExecutor
        where
            F: std::future::Future + 'static + Send,
            F::Output: Send + 'static,
        {
            fn execute(&self, fut: F) {
                tokio::task::spawn(fut);
            }
        }

        #[allow(unused)]
        async fn run(io: impl crate::rt::Read + crate::rt::Write + Send + Unpin + 'static) {
            let (_sender, conn) = crate::client::conn::http2::handshake::<
                _,
                _,
                http_body_util::Empty<bytes::Bytes>,
            >(TokioExecutor, io)
            .await
            .unwrap();

            tokio::task::spawn(async move {
                conn.await.unwrap();
            });
        }
    }

    #[tokio::test]
    #[ignore] // only compilation is checked
    async fn not_send_not_sync_executor_of_send_futures() {
        #[derive(Clone)]
        struct TokioExecutor {
            // !Send, !Sync
            _x: std::marker::PhantomData<std::rc::Rc<()>>,
        }

        impl<F> crate::rt::Executor<F> for TokioExecutor
        where
            F: std::future::Future + 'static + Send,
            F::Output: Send + 'static,
        {
            fn execute(&self, fut: F) {
                tokio::task::spawn(fut);
            }
        }

        #[allow(unused)]
        async fn run(io: impl crate::rt::Read + crate::rt::Write + Send + Unpin + 'static) {
            let (_sender, conn) =
                crate::client::conn::http2::handshake::<_, _, http_body_util::Empty<bytes::Bytes>>(
                    TokioExecutor {
                        _x: Default::default(),
                    },
                    io,
                )
                .await
                .unwrap();

            tokio::task::spawn_local(async move {
                // can't use spawn here because when executor is !Send
                conn.await.unwrap();
            });
        }
    }

    #[tokio::test]
    #[ignore] // only compilation is checked
    async fn send_not_sync_executor_of_send_futures() {
        #[derive(Clone)]
        struct TokioExecutor {
            // !Sync
            _x: std::marker::PhantomData<std::cell::Cell<()>>,
        }

        impl<F> crate::rt::Executor<F> for TokioExecutor
        where
            F: std::future::Future + 'static + Send,
            F::Output: Send + 'static,
        {
            fn execute(&self, fut: F) {
                tokio::task::spawn(fut);
            }
        }

        #[allow(unused)]
        async fn run(io: impl crate::rt::Read + crate::rt::Write + Send + Unpin + 'static) {
            let (_sender, conn) =
                crate::client::conn::http2::handshake::<_, _, http_body_util::Empty<bytes::Bytes>>(
                    TokioExecutor {
                        _x: Default::default(),
                    },
                    io,
                )
                .await
                .unwrap();

            tokio::task::spawn_local(async move {
                // can't use spawn here because when executor is !Send
                conn.await.unwrap();
            });
        }
    }
}
