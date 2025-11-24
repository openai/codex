//! Server implementation of the HTTP/2 protocol.
//!
//! # Getting started
//!
//! Running an HTTP/2 server requires the caller to manage accepting the
//! connections as well as getting the connections to a state that is ready to
//! begin the HTTP/2 handshake. See [here](../index.html#handshake) for more
//! details.
//!
//! This could be as basic as using Tokio's [`TcpListener`] to accept
//! connections, but usually it means using either ALPN or HTTP/1.1 protocol
//! upgrades.
//!
//! Once a connection is obtained, it is passed to [`handshake`],
//! which will begin the [HTTP/2 handshake]. This returns a future that
//! completes once the handshake process is performed and HTTP/2 streams may
//! be received.
//!
//! [`handshake`] uses default configuration values. There are a number of
//! settings that can be changed by using [`Builder`] instead.
//!
//! # Inbound streams
//!
//! The [`Connection`] instance is used to accept inbound HTTP/2 streams. It
//! does this by implementing [`futures::Stream`]. When a new stream is
//! received, a call to [`Connection::accept`] will return `(request, response)`.
//! The `request` handle (of type [`http::Request<RecvStream>`]) contains the
//! HTTP request head as well as provides a way to receive the inbound data
//! stream and the trailers. The `response` handle (of type [`SendResponse`])
//! allows responding to the request, stream the response payload, send
//! trailers, and send push promises.
//!
//! The send ([`SendStream`]) and receive ([`RecvStream`]) halves of the stream
//! can be operated independently.
//!
//! # Managing the connection
//!
//! The [`Connection`] instance is used to manage connection state. The caller
//! is required to call either [`Connection::accept`] or
//! [`Connection::poll_close`] in order to advance the connection state. Simply
//! operating on [`SendStream`] or [`RecvStream`] will have no effect unless the
//! connection state is advanced.
//!
//! It is not required to call **both** [`Connection::accept`] and
//! [`Connection::poll_close`]. If the caller is ready to accept a new stream,
//! then only [`Connection::accept`] should be called. When the caller **does
//! not** want to accept a new stream, [`Connection::poll_close`] should be
//! called.
//!
//! The [`Connection`] instance should only be dropped once
//! [`Connection::poll_close`] returns `Ready`. Once [`Connection::accept`]
//! returns `Ready(None)`, there will no longer be any more inbound streams. At
//! this point, only [`Connection::poll_close`] should be called.
//!
//! # Shutting down the server
//!
//! Graceful shutdown of the server is [not yet
//! implemented](https://github.com/hyperium/h2/issues/69).
//!
//! # Example
//!
//! A basic HTTP/2 server example that runs over TCP and assumes [prior
//! knowledge], i.e. both the client and the server assume that the TCP socket
//! will use the HTTP/2 protocol without prior negotiation.
//!
//! ```no_run
//! use h2::server;
//! use http::{Response, StatusCode};
//! use tokio::net::TcpListener;
//!
//! #[tokio::main]
//! pub async fn main() {
//!     let mut listener = TcpListener::bind("127.0.0.1:5928").await.unwrap();
//!
//!     // Accept all incoming TCP connections.
//!     loop {
//!         if let Ok((socket, _peer_addr)) = listener.accept().await {
//!             // Spawn a new task to process each connection.
//!             tokio::spawn(async {
//!                 // Start the HTTP/2 connection handshake
//!                 let mut h2 = server::handshake(socket).await.unwrap();
//!                 // Accept all inbound HTTP/2 streams sent over the
//!                 // connection.
//!                 while let Some(request) = h2.accept().await {
//!                     let (request, mut respond) = request.unwrap();
//!                     println!("Received request: {:?}", request);
//!
//!                     // Build a response with no body
//!                     let response = Response::builder()
//!                         .status(StatusCode::OK)
//!                         .body(())
//!                         .unwrap();
//!
//!                     // Send the response back to the client
//!                     respond.send_response(response, true)
//!                         .unwrap();
//!                 }
//!
//!             });
//!         }
//!     }
//! }
//! ```
//!
//! [prior knowledge]: http://httpwg.org/specs/rfc7540.html#known-http
//! [`handshake`]: fn.handshake.html
//! [HTTP/2 handshake]: http://httpwg.org/specs/rfc7540.html#ConnectionHeader
//! [`Builder`]: struct.Builder.html
//! [`Connection`]: struct.Connection.html
//! [`Connection::poll`]: struct.Connection.html#method.poll
//! [`Connection::poll_close`]: struct.Connection.html#method.poll_close
//! [`futures::Stream`]: https://docs.rs/futures/0.1/futures/stream/trait.Stream.html
//! [`http::Request<RecvStream>`]: ../struct.RecvStream.html
//! [`RecvStream`]: ../struct.RecvStream.html
//! [`SendStream`]: ../struct.SendStream.html
//! [`TcpListener`]: https://docs.rs/tokio-core/0.1/tokio_core/net/struct.TcpListener.html

use crate::codec::{Codec, UserError};
use crate::frame::{self, Pseudo, PushPromiseHeaderError, Reason, Settings, StreamId};
use crate::proto::{self, Config, Error, Prioritized};
use crate::{FlowControl, PingPong, RecvStream, SendStream};

use bytes::{Buf, Bytes};
use http::{HeaderMap, Method, Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{fmt, io};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tracing::instrument::{Instrument, Instrumented};

/// In progress HTTP/2 connection handshake future.
///
/// This type implements `Future`, yielding a `Connection` instance once the
/// handshake has completed.
///
/// The handshake is completed once the connection preface is fully received
/// from the client **and** the initial settings frame is sent to the client.
///
/// The handshake future does not wait for the initial settings frame from the
/// client.
///
/// See [module] level docs for more details.
///
/// [module]: index.html
#[must_use = "futures do nothing unless polled"]
pub struct Handshake<T, B: Buf = Bytes> {
    /// The config to pass to Connection::new after handshake succeeds.
    builder: Builder,
    /// The current state of the handshake.
    state: Handshaking<T, B>,
    /// Span tracking the handshake
    span: tracing::Span,
}

/// Accepts inbound HTTP/2 streams on a connection.
///
/// A `Connection` is backed by an I/O resource (usually a TCP socket) and
/// implements the HTTP/2 server logic for that connection. It is responsible
/// for receiving inbound streams initiated by the client as well as driving the
/// internal state forward.
///
/// `Connection` values are created by calling [`handshake`]. Once a
/// `Connection` value is obtained, the caller must call [`poll`] or
/// [`poll_close`] in order to drive the internal connection state forward.
///
/// See [module level] documentation for more details
///
/// [module level]: index.html
/// [`handshake`]: struct.Connection.html#method.handshake
/// [`poll`]: struct.Connection.html#method.poll
/// [`poll_close`]: struct.Connection.html#method.poll_close
///
/// # Examples
///
/// ```
/// # use tokio::io::{AsyncRead, AsyncWrite};
/// # use h2::server;
/// # use h2::server::*;
/// #
/// # async fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T) {
/// let mut server = server::handshake(my_io).await.unwrap();
/// while let Some(request) = server.accept().await {
///     tokio::spawn(async move {
///         let (request, respond) = request.unwrap();
///         // Process the request and send the response back to the client
///         // using `respond`.
///     });
/// }
/// # }
/// #
/// # pub fn main() {}
/// ```
#[must_use = "streams do nothing unless polled"]
pub struct Connection<T, B: Buf> {
    connection: proto::Connection<T, Peer, B>,
}

/// Builds server connections with custom configuration values.
///
/// Methods can be chained in order to set the configuration values.
///
/// The server is constructed by calling [`handshake`] and passing the I/O
/// handle that will back the HTTP/2 server.
///
/// New instances of `Builder` are obtained via [`Builder::new`].
///
/// See function level documentation for details on the various server
/// configuration settings.
///
/// [`Builder::new`]: struct.Builder.html#method.new
/// [`handshake`]: struct.Builder.html#method.handshake
///
/// # Examples
///
/// ```
/// # use tokio::io::{AsyncRead, AsyncWrite};
/// # use h2::server::*;
/// #
/// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
/// # -> Handshake<T>
/// # {
/// // `server_fut` is a future representing the completion of the HTTP/2
/// // handshake.
/// let server_fut = Builder::new()
///     .initial_window_size(1_000_000)
///     .max_concurrent_streams(1000)
///     .handshake(my_io);
/// # server_fut
/// # }
/// #
/// # pub fn main() {}
/// ```
#[derive(Clone, Debug)]
pub struct Builder {
    /// Time to keep locally reset streams around before reaping.
    reset_stream_duration: Duration,

    /// Maximum number of locally reset streams to keep at a time.
    reset_stream_max: usize,

    /// Maximum number of remotely reset streams to allow in the pending
    /// accept queue.
    pending_accept_reset_stream_max: usize,

    /// Initial `Settings` frame to send as part of the handshake.
    settings: Settings,

    /// Initial target window size for new connections.
    initial_target_connection_window_size: Option<u32>,

    /// Maximum amount of bytes to "buffer" for writing per stream.
    max_send_buffer_size: usize,

    /// Maximum number of locally reset streams due to protocol error across
    /// the lifetime of the connection.
    ///
    /// When this gets exceeded, we issue GOAWAYs.
    local_max_error_reset_streams: Option<usize>,
}

/// Send a response back to the client
///
/// A `SendResponse` instance is provided when receiving a request and is used
/// to send the associated response back to the client. It is also used to
/// explicitly reset the stream with a custom reason.
///
/// It will also be used to initiate push promises linked with the associated
/// stream.
///
/// If the `SendResponse` instance is dropped without sending a response, then
/// the HTTP/2 stream will be reset.
///
/// See [module] level docs for more details.
///
/// [module]: index.html
#[derive(Debug)]
pub struct SendResponse<B: Buf> {
    inner: proto::StreamRef<B>,
}

/// Send a response to a promised request
///
/// A `SendPushedResponse` instance is provided when promising a request and is used
/// to send the associated response to the client. It is also used to
/// explicitly reset the stream with a custom reason.
///
/// It can not be used to initiate push promises.
///
/// If the `SendPushedResponse` instance is dropped without sending a response, then
/// the HTTP/2 stream will be reset.
///
/// See [module] level docs for more details.
///
/// [module]: index.html
pub struct SendPushedResponse<B: Buf> {
    inner: SendResponse<B>,
}

// Manual implementation necessary because of rust-lang/rust#26925
impl<B: Buf + fmt::Debug> fmt::Debug for SendPushedResponse<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SendPushedResponse {{ {:?} }}", self.inner)
    }
}

/// Stages of an in-progress handshake.
enum Handshaking<T, B: Buf> {
    /// State 1. Connection is flushing pending SETTINGS frame.
    Flushing(Instrumented<Flush<T, Prioritized<B>>>),
    /// State 2. Connection is waiting for the client preface.
    ReadingPreface(Instrumented<ReadPreface<T, Prioritized<B>>>),
    /// State 3. Handshake is done, polling again would panic.
    Done,
}

/// Flush a Sink
struct Flush<T, B> {
    codec: Option<Codec<T, B>>,
}

/// Read the client connection preface
struct ReadPreface<T, B> {
    codec: Option<Codec<T, B>>,
    pos: usize,
}

#[derive(Debug)]
pub(crate) struct Peer;

const PREFACE: [u8; 24] = *b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Creates a new configured HTTP/2 server with default configuration
/// values backed by `io`.
///
/// It is expected that `io` already be in an appropriate state to commence
/// the [HTTP/2 handshake]. See [Handshake] for more details.
///
/// Returns a future which resolves to the [`Connection`] instance once the
/// HTTP/2 handshake has been completed. The returned [`Connection`]
/// instance will be using default configuration values. Use [`Builder`] to
/// customize the configuration values used by a [`Connection`] instance.
///
/// [HTTP/2 handshake]: http://httpwg.org/specs/rfc7540.html#ConnectionHeader
/// [Handshake]: ../index.html#handshake
/// [`Connection`]: struct.Connection.html
///
/// # Examples
///
/// ```
/// # use tokio::io::{AsyncRead, AsyncWrite};
/// # use h2::server;
/// # use h2::server::*;
/// #
/// # async fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
/// # {
/// let connection = server::handshake(my_io).await.unwrap();
/// // The HTTP/2 handshake has completed, now use `connection` to
/// // accept inbound HTTP/2 streams.
/// # }
/// #
/// # pub fn main() {}
/// ```
pub fn handshake<T>(io: T) -> Handshake<T, Bytes>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    Builder::new().handshake(io)
}

// ===== impl Connection =====

impl<T, B> Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    fn handshake2(io: T, builder: Builder) -> Handshake<T, B> {
        let span = tracing::trace_span!("server_handshake");
        let entered = span.enter();

        // Create the codec.
        let mut codec = Codec::new(io);

        if let Some(max) = builder.settings.max_frame_size() {
            codec.set_max_recv_frame_size(max as usize);
        }

        if let Some(max) = builder.settings.max_header_list_size() {
            codec.set_max_recv_header_list_size(max as usize);
        }

        // Send initial settings frame.
        codec
            .buffer(builder.settings.clone().into())
            .expect("invalid SETTINGS frame");

        // Create the handshake future.
        let state =
            Handshaking::Flushing(Flush::new(codec).instrument(tracing::trace_span!("flush")));

        drop(entered);

        Handshake {
            builder,
            state,
            span,
        }
    }

    /// Accept the next incoming request on this connection.
    pub async fn accept(
        &mut self,
    ) -> Option<Result<(Request<RecvStream>, SendResponse<B>), crate::Error>> {
        crate::poll_fn(move |cx| self.poll_accept(cx)).await
    }

    #[doc(hidden)]
    pub fn poll_accept(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(Request<RecvStream>, SendResponse<B>), crate::Error>>> {
        // Always try to advance the internal state. Getting Pending also is
        // needed to allow this function to return Pending.
        if self.poll_closed(cx)?.is_ready() {
            // If the socket is closed, don't return anything
            // TODO: drop any pending streams
            return Poll::Ready(None);
        }

        if let Some(inner) = self.connection.next_incoming() {
            tracing::trace!("received incoming");
            let (head, _) = inner.take_request().into_parts();
            let body = RecvStream::new(FlowControl::new(inner.clone_to_opaque()));

            let request = Request::from_parts(head, body);
            let respond = SendResponse { inner };

            return Poll::Ready(Some(Ok((request, respond))));
        }

        Poll::Pending
    }

    /// Sets the target window size for the whole connection.
    ///
    /// If `size` is greater than the current value, then a `WINDOW_UPDATE`
    /// frame will be immediately sent to the remote, increasing the connection
    /// level window by `size - current_value`.
    ///
    /// If `size` is less than the current value, nothing will happen
    /// immediately. However, as window capacity is released by
    /// [`FlowControl`] instances, no `WINDOW_UPDATE` frames will be sent
    /// out until the number of "in flight" bytes drops below `size`.
    ///
    /// The default value is 65,535.
    ///
    /// See [`FlowControl`] documentation for more details.
    ///
    /// [`FlowControl`]: ../struct.FlowControl.html
    /// [library level]: ../index.html#flow-control
    pub fn set_target_window_size(&mut self, size: u32) {
        assert!(size <= proto::MAX_WINDOW_SIZE);
        self.connection.set_target_window_size(size);
    }

    /// Set a new `INITIAL_WINDOW_SIZE` setting (in octets) for stream-level
    /// flow control for received data.
    ///
    /// The `SETTINGS` will be sent to the remote, and only applied once the
    /// remote acknowledges the change.
    ///
    /// This can be used to increase or decrease the window size for existing
    /// streams.
    ///
    /// # Errors
    ///
    /// Returns an error if a previous call is still pending acknowledgement
    /// from the remote endpoint.
    pub fn set_initial_window_size(&mut self, size: u32) -> Result<(), crate::Error> {
        assert!(size <= proto::MAX_WINDOW_SIZE);
        self.connection.set_initial_window_size(size)?;
        Ok(())
    }

    /// Enables the [extended CONNECT protocol].
    ///
    /// [extended CONNECT protocol]: https://datatracker.ietf.org/doc/html/rfc8441#section-4
    ///
    /// # Errors
    ///
    /// Returns an error if a previous call is still pending acknowledgement
    /// from the remote endpoint.
    pub fn enable_connect_protocol(&mut self) -> Result<(), crate::Error> {
        self.connection.set_enable_connect_protocol()?;
        Ok(())
    }

    /// Returns `Ready` when the underlying connection has closed.
    ///
    /// If any new inbound streams are received during a call to `poll_closed`,
    /// they will be queued and returned on the next call to [`poll_accept`].
    ///
    /// This function will advance the internal connection state, driving
    /// progress on all the other handles (e.g. [`RecvStream`] and [`SendStream`]).
    ///
    /// See [here](index.html#managing-the-connection) for more details.
    ///
    /// [`poll_accept`]: struct.Connection.html#method.poll_accept
    /// [`RecvStream`]: ../struct.RecvStream.html
    /// [`SendStream`]: ../struct.SendStream.html
    pub fn poll_closed(&mut self, cx: &mut Context) -> Poll<Result<(), crate::Error>> {
        self.connection.poll(cx).map_err(Into::into)
    }

    /// Sets the connection to a GOAWAY state.
    ///
    /// Does not terminate the connection. Must continue being polled to close
    /// connection.
    ///
    /// After flushing the GOAWAY frame, the connection is closed. Any
    /// outstanding streams do not prevent the connection from closing. This
    /// should usually be reserved for shutting down when something bad
    /// external to `h2` has happened, and open streams cannot be properly
    /// handled.
    ///
    /// For graceful shutdowns, see [`graceful_shutdown`](Connection::graceful_shutdown).
    pub fn abrupt_shutdown(&mut self, reason: Reason) {
        self.connection.go_away_from_user(reason);
    }

    /// Starts a [graceful shutdown][1] process.
    ///
    /// Must continue being polled to close connection.
    ///
    /// It's possible to receive more requests after calling this method, since
    /// they might have been in-flight from the client already. After about
    /// 1 RTT, no new requests should be accepted. Once all active streams
    /// have completed, the connection is closed.
    ///
    /// [1]: http://httpwg.org/specs/rfc7540.html#GOAWAY
    pub fn graceful_shutdown(&mut self) {
        self.connection.go_away_gracefully();
    }

    /// Takes a `PingPong` instance from the connection.
    ///
    /// # Note
    ///
    /// This may only be called once. Calling multiple times will return `None`.
    pub fn ping_pong(&mut self) -> Option<PingPong> {
        self.connection.take_user_pings().map(PingPong::new)
    }

    /// Checks if there are any streams
    pub fn has_streams(&self) -> bool {
        self.connection.has_streams()
    }

    /// Returns the maximum number of concurrent streams that may be initiated
    /// by the server on this connection.
    ///
    /// This limit is configured by the client peer by sending the
    /// [`SETTINGS_MAX_CONCURRENT_STREAMS` parameter][1] in a `SETTINGS` frame.
    /// This method returns the currently acknowledged value received from the
    /// remote.
    ///
    /// [1]: https://tools.ietf.org/html/rfc7540#section-5.1.2
    pub fn max_concurrent_send_streams(&self) -> usize {
        self.connection.max_send_streams()
    }

    /// Returns the maximum number of concurrent streams that may be initiated
    /// by the client on this connection.
    ///
    /// This returns the value of the [`SETTINGS_MAX_CONCURRENT_STREAMS`
    /// parameter][1] sent in a `SETTINGS` frame that has been
    /// acknowledged by the remote peer. The value to be sent is configured by
    /// the [`Builder::max_concurrent_streams`][2] method before handshaking
    /// with the remote peer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc7540#section-5.1.2
    /// [2]: ../struct.Builder.html#method.max_concurrent_streams
    pub fn max_concurrent_recv_streams(&self) -> usize {
        self.connection.max_recv_streams()
    }

    // Could disappear at anytime.
    #[doc(hidden)]
    #[cfg(feature = "unstable")]
    pub fn num_wired_streams(&self) -> usize {
        self.connection.num_wired_streams()
    }
}

#[cfg(feature = "stream")]
impl<T, B> futures_core::Stream for Connection<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    type Item = Result<(Request<RecvStream>, SendResponse<B>), crate::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_accept(cx)
    }
}

impl<T, B> fmt::Debug for Connection<T, B>
where
    T: fmt::Debug,
    B: fmt::Debug + Buf,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Connection")
            .field("connection", &self.connection)
            .finish()
    }
}

// ===== impl Builder =====

impl Builder {
    /// Returns a new server builder instance initialized with default
    /// configuration values.
    ///
    /// Configuration methods can be chained on the return value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .initial_window_size(1_000_000)
    ///     .max_concurrent_streams(1000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn new() -> Builder {
        Builder {
            reset_stream_duration: Duration::from_secs(proto::DEFAULT_RESET_STREAM_SECS),
            reset_stream_max: proto::DEFAULT_RESET_STREAM_MAX,
            pending_accept_reset_stream_max: proto::DEFAULT_REMOTE_RESET_STREAM_MAX,
            settings: Settings::default(),
            initial_target_connection_window_size: None,
            max_send_buffer_size: proto::DEFAULT_MAX_SEND_BUFFER_SIZE,

            local_max_error_reset_streams: Some(proto::DEFAULT_LOCAL_RESET_COUNT_MAX),
        }
    }

    /// Indicates the initial window size (in octets) for stream-level
    /// flow control for received data.
    ///
    /// The initial window of a stream is used as part of flow control. For more
    /// details, see [`FlowControl`].
    ///
    /// The default value is 65,535.
    ///
    /// [`FlowControl`]: ../struct.FlowControl.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .initial_window_size(1_000_000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn initial_window_size(&mut self, size: u32) -> &mut Self {
        self.settings.set_initial_window_size(Some(size));
        self
    }

    /// Indicates the initial window size (in octets) for connection-level flow control
    /// for received data.
    ///
    /// The initial window of a connection is used as part of flow control. For more details,
    /// see [`FlowControl`].
    ///
    /// The default value is 65,535.
    ///
    /// [`FlowControl`]: ../struct.FlowControl.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .initial_connection_window_size(1_000_000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn initial_connection_window_size(&mut self, size: u32) -> &mut Self {
        self.initial_target_connection_window_size = Some(size);
        self
    }

    /// Indicates the size (in octets) of the largest HTTP/2 frame payload that the
    /// configured server is able to accept.
    ///
    /// The sender may send data frames that are **smaller** than this value,
    /// but any data larger than `max` will be broken up into multiple `DATA`
    /// frames.
    ///
    /// The value **must** be between 16,384 and 16,777,215. The default value is 16,384.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_frame_size(1_000_000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if `max` is not within the legal range specified
    /// above.
    pub fn max_frame_size(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_frame_size(Some(max));
        self
    }

    /// Sets the max size of received header frames.
    ///
    /// This advisory setting informs a peer of the maximum size of header list
    /// that the sender is prepared to accept, in octets. The value is based on
    /// the uncompressed size of header fields, including the length of the name
    /// and value in octets plus an overhead of 32 octets for each header field.
    ///
    /// This setting is also used to limit the maximum amount of data that is
    /// buffered to decode HEADERS frames.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_header_list_size(16 * 1024)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_header_list_size(Some(max));
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_concurrent_streams(1000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_concurrent_streams(&mut self, max: u32) -> &mut Self {
        self.settings.set_max_concurrent_streams(Some(max));
        self
    }

    /// Sets the maximum number of concurrent locally reset streams.
    ///
    /// When a stream is explicitly reset by either calling
    /// [`SendResponse::send_reset`] or by dropping a [`SendResponse`] instance
    /// before completing the stream, the HTTP/2 specification requires that
    /// any further frames received for that stream must be ignored for "some
    /// time".
    ///
    /// In order to satisfy the specification, internal state must be maintained
    /// to implement the behavior. This state grows linearly with the number of
    /// streams that are locally reset.
    ///
    /// The `max_concurrent_reset_streams` setting configures sets an upper
    /// bound on the amount of state that is maintained. When this max value is
    /// reached, the oldest reset stream is purged from memory.
    ///
    /// Once the stream has been fully purged from memory, any additional frames
    /// received for that stream will result in a connection level protocol
    /// error, forcing the connection to terminate.
    ///
    /// The default value is 10.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_concurrent_reset_streams(1000)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_concurrent_reset_streams(&mut self, max: usize) -> &mut Self {
        self.reset_stream_max = max;
        self
    }

    /// Sets the maximum number of local resets due to protocol errors made by the remote end.
    ///
    /// Invalid frames and many other protocol errors will lead to resets being generated for those streams.
    /// Too many of these often indicate a malicious client, and there are attacks which can abuse this to DOS servers.
    /// This limit protects against these DOS attacks by limiting the amount of resets we can be forced to generate.
    ///
    /// When the number of local resets exceeds this threshold, the server will issue GOAWAYs with an error code of
    /// `ENHANCE_YOUR_CALM` to the client.
    ///
    /// If you really want to disable this, supply [`Option::None`] here.
    /// Disabling this is not recommended and may expose you to DOS attacks.
    ///
    /// The default value is currently 1024, but could change.
    pub fn max_local_error_reset_streams(&mut self, max: Option<usize>) -> &mut Self {
        self.local_max_error_reset_streams = max;
        self
    }

    /// Sets the maximum number of pending-accept remotely-reset streams.
    ///
    /// Streams that have been received by the peer, but not accepted by the
    /// user, can also receive a RST_STREAM. This is a legitimate pattern: one
    /// could send a request and then shortly after, realize it is not needed,
    /// sending a CANCEL.
    ///
    /// However, since those streams are now "closed", they don't count towards
    /// the max concurrent streams. So, they will sit in the accept queue,
    /// using memory.
    ///
    /// When the number of remotely-reset streams sitting in the pending-accept
    /// queue reaches this maximum value, a connection error with the code of
    /// `ENHANCE_YOUR_CALM` will be sent to the peer, and returned by the
    /// `Future`.
    ///
    /// The default value is currently 20, but could change.
    ///
    /// # Examples
    ///
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .max_pending_accept_reset_streams(100)
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn max_pending_accept_reset_streams(&mut self, max: usize) -> &mut Self {
        self.pending_accept_reset_stream_max = max;
        self
    }

    /// Sets the maximum send buffer size per stream.
    ///
    /// Once a stream has buffered up to (or over) the maximum, the stream's
    /// flow control will not "poll" additional capacity. Once bytes for the
    /// stream have been written to the connection, the send buffer capacity
    /// will be freed up again.
    ///
    /// The default is currently ~400KB, but may change.
    ///
    /// # Panics
    ///
    /// This function panics if `max` is larger than `u32::MAX`.
    pub fn max_send_buffer_size(&mut self, max: usize) -> &mut Self {
        assert!(max <= u32::MAX as usize);
        self.max_send_buffer_size = max;
        self
    }

    /// Sets the maximum number of concurrent locally reset streams.
    ///
    /// When a stream is explicitly reset by either calling
    /// [`SendResponse::send_reset`] or by dropping a [`SendResponse`] instance
    /// before completing the stream, the HTTP/2 specification requires that
    /// any further frames received for that stream must be ignored for "some
    /// time".
    ///
    /// In order to satisfy the specification, internal state must be maintained
    /// to implement the behavior. This state grows linearly with the number of
    /// streams that are locally reset.
    ///
    /// The `reset_stream_duration` setting configures the max amount of time
    /// this state will be maintained in memory. Once the duration elapses, the
    /// stream state is purged from memory.
    ///
    /// Once the stream has been fully purged from memory, any additional frames
    /// received for that stream will result in a connection level protocol
    /// error, forcing the connection to terminate.
    ///
    /// The default value is 30 seconds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// # use std::time::Duration;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .reset_stream_duration(Duration::from_secs(10))
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn reset_stream_duration(&mut self, dur: Duration) -> &mut Self {
        self.reset_stream_duration = dur;
        self
    }

    /// Enables the [extended CONNECT protocol].
    ///
    /// [extended CONNECT protocol]: https://datatracker.ietf.org/doc/html/rfc8441#section-4
    pub fn enable_connect_protocol(&mut self) -> &mut Self {
        self.settings.set_enable_connect_protocol(Some(1));
        self
    }

    /// Creates a new configured HTTP/2 server backed by `io`.
    ///
    /// It is expected that `io` already be in an appropriate state to commence
    /// the [HTTP/2 handshake]. See [Handshake] for more details.
    ///
    /// Returns a future which resolves to the [`Connection`] instance once the
    /// HTTP/2 handshake has been completed.
    ///
    /// This function also allows the caller to configure the send payload data
    /// type. See [Outbound data type] for more details.
    ///
    /// [HTTP/2 handshake]: http://httpwg.org/specs/rfc7540.html#ConnectionHeader
    /// [Handshake]: ../index.html#handshake
    /// [`Connection`]: struct.Connection.html
    /// [Outbound data type]: ../index.html#outbound-data-type.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut = Builder::new()
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    ///
    /// Configures the send-payload data type. In this case, the outbound data
    /// type will be `&'static [u8]`.
    ///
    /// ```
    /// # use tokio::io::{AsyncRead, AsyncWrite};
    /// # use h2::server::*;
    /// #
    /// # fn doc<T: AsyncRead + AsyncWrite + Unpin>(my_io: T)
    /// # -> Handshake<T, &'static [u8]>
    /// # {
    /// // `server_fut` is a future representing the completion of the HTTP/2
    /// // handshake.
    /// let server_fut: Handshake<_, &'static [u8]> = Builder::new()
    ///     .handshake(my_io);
    /// # server_fut
    /// # }
    /// #
    /// # pub fn main() {}
    /// ```
    pub fn handshake<T, B>(&self, io: T) -> Handshake<T, B>
    where
        T: AsyncRead + AsyncWrite + Unpin,
        B: Buf,
    {
        Connection::handshake2(io, self.clone())
    }
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

// ===== impl SendResponse =====

impl<B: Buf> SendResponse<B> {
    /// Send a response to a client request.
    ///
    /// On success, a [`SendStream`] instance is returned. This instance can be
    /// used to stream the response body and send trailers.
    ///
    /// If a body or trailers will be sent on the returned [`SendStream`]
    /// instance, then `end_of_stream` must be set to `false` when calling this
    /// function.
    ///
    /// The [`SendResponse`] instance is already associated with a received
    /// request.  This function may only be called once per instance and only if
    /// [`send_reset`] has not been previously called.
    ///
    /// [`SendResponse`]: #
    /// [`SendStream`]: ../struct.SendStream.html
    /// [`send_reset`]: #method.send_reset
    pub fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<B>, crate::Error> {
        self.inner
            .send_response(response, end_of_stream)
            .map(|_| SendStream::new(self.inner.clone()))
            .map_err(Into::into)
    }

    /// Push a request and response to the client
    ///
    /// On success, a [`SendResponse`] instance is returned.
    ///
    /// [`SendResponse`]: #
    pub fn push_request(
        &mut self,
        request: Request<()>,
    ) -> Result<SendPushedResponse<B>, crate::Error> {
        self.inner
            .send_push_promise(request)
            .map(|inner| SendPushedResponse {
                inner: SendResponse { inner },
            })
            .map_err(Into::into)
    }

    /// Send a stream reset to the peer.
    ///
    /// This essentially cancels the stream, including any inbound or outbound
    /// data streams.
    ///
    /// If this function is called before [`send_response`], a call to
    /// [`send_response`] will result in an error.
    ///
    /// If this function is called while a [`SendStream`] instance is active,
    /// any further use of the instance will result in an error.
    ///
    /// This function should only be called once.
    ///
    /// [`send_response`]: #method.send_response
    /// [`SendStream`]: ../struct.SendStream.html
    pub fn send_reset(&mut self, reason: Reason) {
        self.inner.send_reset(reason)
    }

    /// Polls to be notified when the client resets this stream.
    ///
    /// If stream is still open, this returns `Poll::Pending`, and
    /// registers the task to be notified if a `RST_STREAM` is received.
    ///
    /// If a `RST_STREAM` frame is received for this stream, calling this
    /// method will yield the `Reason` for the reset.
    ///
    /// # Error
    ///
    /// Calling this method after having called `send_response` will return
    /// a user error.
    pub fn poll_reset(&mut self, cx: &mut Context) -> Poll<Result<Reason, crate::Error>> {
        self.inner.poll_reset(cx, proto::PollReset::AwaitingHeaders)
    }

    /// Returns the stream ID of the response stream.
    ///
    /// # Panics
    ///
    /// If the lock on the stream store has been poisoned.
    pub fn stream_id(&self) -> crate::StreamId {
        crate::StreamId::from_internal(self.inner.stream_id())
    }
}

// ===== impl SendPushedResponse =====

impl<B: Buf> SendPushedResponse<B> {
    /// Send a response to a promised request.
    ///
    /// On success, a [`SendStream`] instance is returned. This instance can be
    /// used to stream the response body and send trailers.
    ///
    /// If a body or trailers will be sent on the returned [`SendStream`]
    /// instance, then `end_of_stream` must be set to `false` when calling this
    /// function.
    ///
    /// The [`SendPushedResponse`] instance is associated with a promised
    /// request.  This function may only be called once per instance and only if
    /// [`send_reset`] has not been previously called.
    ///
    /// [`SendPushedResponse`]: #
    /// [`SendStream`]: ../struct.SendStream.html
    /// [`send_reset`]: #method.send_reset
    pub fn send_response(
        &mut self,
        response: Response<()>,
        end_of_stream: bool,
    ) -> Result<SendStream<B>, crate::Error> {
        self.inner.send_response(response, end_of_stream)
    }

    /// Send a stream reset to the peer.
    ///
    /// This essentially cancels the stream, including any inbound or outbound
    /// data streams.
    ///
    /// If this function is called before [`send_response`], a call to
    /// [`send_response`] will result in an error.
    ///
    /// If this function is called while a [`SendStream`] instance is active,
    /// any further use of the instance will result in an error.
    ///
    /// This function should only be called once.
    ///
    /// [`send_response`]: #method.send_response
    /// [`SendStream`]: ../struct.SendStream.html
    pub fn send_reset(&mut self, reason: Reason) {
        self.inner.send_reset(reason)
    }

    /// Polls to be notified when the client resets this stream.
    ///
    /// If stream is still open, this returns `Poll::Pending`, and
    /// registers the task to be notified if a `RST_STREAM` is received.
    ///
    /// If a `RST_STREAM` frame is received for this stream, calling this
    /// method will yield the `Reason` for the reset.
    ///
    /// # Error
    ///
    /// Calling this method after having called `send_response` will return
    /// a user error.
    pub fn poll_reset(&mut self, cx: &mut Context) -> Poll<Result<Reason, crate::Error>> {
        self.inner.poll_reset(cx)
    }

    /// Returns the stream ID of the response stream.
    ///
    /// # Panics
    ///
    /// If the lock on the stream store has been poisoned.
    pub fn stream_id(&self) -> crate::StreamId {
        self.inner.stream_id()
    }
}

// ===== impl Flush =====

impl<T, B: Buf> Flush<T, B> {
    fn new(codec: Codec<T, B>) -> Self {
        Flush { codec: Some(codec) }
    }
}

impl<T, B> Future for Flush<T, B>
where
    T: AsyncWrite + Unpin,
    B: Buf,
{
    type Output = Result<Codec<T, B>, crate::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Flush the codec
        ready!(self.codec.as_mut().unwrap().flush(cx)).map_err(crate::Error::from_io)?;

        // Return the codec
        Poll::Ready(Ok(self.codec.take().unwrap()))
    }
}

impl<T, B: Buf> ReadPreface<T, B> {
    fn new(codec: Codec<T, B>) -> Self {
        ReadPreface {
            codec: Some(codec),
            pos: 0,
        }
    }

    fn inner_mut(&mut self) -> &mut T {
        self.codec.as_mut().unwrap().get_mut()
    }
}

impl<T, B> Future for ReadPreface<T, B>
where
    T: AsyncRead + Unpin,
    B: Buf,
{
    type Output = Result<Codec<T, B>, crate::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut buf = [0; 24];
        let mut rem = PREFACE.len() - self.pos;

        while rem > 0 {
            let mut buf = ReadBuf::new(&mut buf[..rem]);
            ready!(Pin::new(self.inner_mut()).poll_read(cx, &mut buf))
                .map_err(crate::Error::from_io)?;
            let n = buf.filled().len();
            if n == 0 {
                return Poll::Ready(Err(crate::Error::from_io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed before reading preface",
                ))));
            }

            if &PREFACE[self.pos..self.pos + n] != buf.filled() {
                proto_err!(conn: "read_preface: invalid preface");
                // TODO: Should this just write the GO_AWAY frame directly?
                return Poll::Ready(Err(Error::library_go_away(Reason::PROTOCOL_ERROR).into()));
            }

            self.pos += n;
            rem -= n; // TODO test
        }

        Poll::Ready(Ok(self.codec.take().unwrap()))
    }
}

// ===== impl Handshake =====

impl<T, B: Buf> Future for Handshake<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    type Output = Result<Connection<T, B>, crate::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let span = self.span.clone(); // XXX(eliza): T_T
        let _e = span.enter();
        tracing::trace!(state = ?self.state);

        loop {
            match &mut self.state {
                Handshaking::Flushing(flush) => {
                    // We're currently flushing a pending SETTINGS frame. Poll the
                    // flush future, and, if it's completed, advance our state to wait
                    // for the client preface.
                    let codec = match Pin::new(flush).poll(cx)? {
                        Poll::Pending => {
                            tracing::trace!(flush.poll = %"Pending");
                            return Poll::Pending;
                        }
                        Poll::Ready(flushed) => {
                            tracing::trace!(flush.poll = %"Ready");
                            flushed
                        }
                    };
                    self.state = Handshaking::ReadingPreface(
                        ReadPreface::new(codec).instrument(tracing::trace_span!("read_preface")),
                    );
                }
                Handshaking::ReadingPreface(read) => {
                    let codec = ready!(Pin::new(read).poll(cx)?);

                    self.state = Handshaking::Done;

                    let connection = proto::Connection::new(
                        codec,
                        Config {
                            next_stream_id: 2.into(),
                            // Server does not need to locally initiate any streams
                            initial_max_send_streams: 0,
                            max_send_buffer_size: self.builder.max_send_buffer_size,
                            reset_stream_duration: self.builder.reset_stream_duration,
                            reset_stream_max: self.builder.reset_stream_max,
                            remote_reset_stream_max: self.builder.pending_accept_reset_stream_max,
                            local_error_reset_streams_max: self
                                .builder
                                .local_max_error_reset_streams,
                            settings: self.builder.settings.clone(),
                        },
                    );

                    tracing::trace!("connection established!");
                    let mut c = Connection { connection };
                    if let Some(sz) = self.builder.initial_target_connection_window_size {
                        c.set_target_window_size(sz);
                    }

                    return Poll::Ready(Ok(c));
                }
                Handshaking::Done => {
                    panic!("Handshaking::poll() called again after handshaking was complete")
                }
            }
        }
    }
}

impl<T, B> fmt::Debug for Handshake<T, B>
where
    T: AsyncRead + AsyncWrite + fmt::Debug,
    B: fmt::Debug + Buf,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "server::Handshake")
    }
}

impl Peer {
    pub fn convert_send_message(
        id: StreamId,
        response: Response<()>,
        end_of_stream: bool,
    ) -> frame::Headers {
        use http::response::Parts;

        // Extract the components of the HTTP request
        let (
            Parts {
                status, headers, ..
            },
            _,
        ) = response.into_parts();

        // Build the set pseudo header set. All requests will include `method`
        // and `path`.
        let pseudo = Pseudo::response(status);

        // Create the HEADERS frame
        let mut frame = frame::Headers::new(id, pseudo, headers);

        if end_of_stream {
            frame.set_end_stream()
        }

        frame
    }

    pub fn convert_push_message(
        stream_id: StreamId,
        promised_id: StreamId,
        request: Request<()>,
    ) -> Result<frame::PushPromise, UserError> {
        use http::request::Parts;

        if let Err(e) = frame::PushPromise::validate_request(&request) {
            use PushPromiseHeaderError::*;
            match e {
                NotSafeAndCacheable => tracing::debug!(
                    ?promised_id,
                    "convert_push_message: method {} is not safe and cacheable",
                    request.method(),
                ),
                InvalidContentLength(e) => tracing::debug!(
                    ?promised_id,
                    "convert_push_message; promised request has invalid content-length {:?}",
                    e,
                ),
            }
            return Err(UserError::MalformedHeaders);
        }

        // Extract the components of the HTTP request
        let (
            Parts {
                method,
                uri,
                headers,
                ..
            },
            _,
        ) = request.into_parts();

        let pseudo = Pseudo::request(method, uri, None);

        Ok(frame::PushPromise::new(
            stream_id,
            promised_id,
            pseudo,
            headers,
        ))
    }
}

impl proto::Peer for Peer {
    type Poll = Request<()>;

    const NAME: &'static str = "Server";

    /*
    fn is_server() -> bool {
        true
    }
    */

    fn r#dyn() -> proto::DynPeer {
        proto::DynPeer::Server
    }

    fn convert_poll_message(
        pseudo: Pseudo,
        fields: HeaderMap,
        stream_id: StreamId,
    ) -> Result<Self::Poll, Error> {
        use http::{uri, Version};

        let mut b = Request::builder();

        macro_rules! malformed {
            ($($arg:tt)*) => {{
                tracing::debug!($($arg)*);
                return Err(Error::library_reset(stream_id, Reason::PROTOCOL_ERROR));
            }}
        }

        b = b.version(Version::HTTP_2);

        let is_connect;
        if let Some(method) = pseudo.method {
            is_connect = method == Method::CONNECT;
            b = b.method(method);
        } else {
            malformed!("malformed headers: missing method");
        }

        let has_protocol = pseudo.protocol.is_some();
        if has_protocol {
            if is_connect {
                // Assert that we have the right type.
                b = b.extension::<crate::ext::Protocol>(pseudo.protocol.unwrap());
            } else {
                malformed!("malformed headers: :protocol on non-CONNECT request");
            }
        }

        if pseudo.status.is_some() {
            malformed!("malformed headers: :status field on request");
        }

        // Convert the URI
        let mut parts = uri::Parts::default();

        // A request translated from HTTP/1 must not include the :authority
        // header
        if let Some(authority) = pseudo.authority {
            let maybe_authority = uri::Authority::from_maybe_shared(authority.clone().into_inner());
            parts.authority = Some(maybe_authority.or_else(|why| {
                malformed!(
                    "malformed headers: malformed authority ({:?}): {}",
                    authority,
                    why,
                )
            })?);
        }

        // A :scheme is required, except CONNECT.
        if let Some(scheme) = pseudo.scheme {
            if is_connect && !has_protocol {
                malformed!("malformed headers: :scheme in CONNECT");
            }
            let maybe_scheme = scheme.parse();
            let scheme = maybe_scheme.or_else(|why| {
                malformed!(
                    "malformed headers: malformed scheme ({:?}): {}",
                    scheme,
                    why,
                )
            })?;

            // It's not possible to build an `Uri` from a scheme and path. So,
            // after validating is was a valid scheme, we just have to drop it
            // if there isn't an :authority.
            if parts.authority.is_some() {
                parts.scheme = Some(scheme);
            }
        } else if !is_connect || has_protocol {
            malformed!("malformed headers: missing scheme");
        }

        if let Some(path) = pseudo.path {
            if is_connect && !has_protocol {
                malformed!("malformed headers: :path in CONNECT");
            }

            // This cannot be empty
            if path.is_empty() {
                malformed!("malformed headers: missing path");
            }

            let maybe_path = uri::PathAndQuery::from_maybe_shared(path.clone().into_inner());
            parts.path_and_query = Some(maybe_path.or_else(|why| {
                malformed!("malformed headers: malformed path ({:?}): {}", path, why,)
            })?);
        } else if is_connect && has_protocol {
            malformed!("malformed headers: missing path in extended CONNECT");
        }

        b = b.uri(parts);

        let mut request = match b.body(()) {
            Ok(request) => request,
            Err(e) => {
                // TODO: Should there be more specialized handling for different
                // kinds of errors
                proto_err!(stream: "error building request: {}; stream={:?}", e, stream_id);
                return Err(Error::library_reset(stream_id, Reason::PROTOCOL_ERROR));
            }
        };

        *request.headers_mut() = fields;

        Ok(request)
    }
}

// ===== impl Handshaking =====

impl<T, B> fmt::Debug for Handshaking<T, B>
where
    B: Buf,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Handshaking::Flushing(_) => f.write_str("Flushing(_)"),
            Handshaking::ReadingPreface(_) => f.write_str("ReadingPreface(_)"),
            Handshaking::Done => f.write_str("Done"),
        }
    }
}
