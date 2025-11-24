use std::{
    any::Any,
    fmt,
    future::Future,
    io,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker, ready},
};

use bytes::Bytes;
use pin_project_lite::pin_project;
use rustc_hash::FxHashMap;
use thiserror::Error;
use tokio::sync::{Notify, futures::Notified, mpsc, oneshot};
use tracing::{Instrument, Span, debug_span};

use crate::{
    ConnectionEvent, Duration, Instant, VarInt,
    mutex::Mutex,
    recv_stream::RecvStream,
    runtime::{AsyncTimer, AsyncUdpSocket, Runtime, UdpPoller},
    send_stream::SendStream,
    udp_transmit,
};
use proto::{
    ConnectionError, ConnectionHandle, ConnectionStats, Dir, EndpointEvent, Side, StreamEvent,
    StreamId, congestion::Controller,
};

/// In-progress connection attempt future
#[derive(Debug)]
pub struct Connecting {
    conn: Option<ConnectionRef>,
    connected: oneshot::Receiver<bool>,
    handshake_data_ready: Option<oneshot::Receiver<()>>,
}

impl Connecting {
    pub(crate) fn new(
        handle: ConnectionHandle,
        conn: proto::Connection,
        endpoint_events: mpsc::UnboundedSender<(ConnectionHandle, EndpointEvent)>,
        conn_events: mpsc::UnboundedReceiver<ConnectionEvent>,
        socket: Arc<dyn AsyncUdpSocket>,
        runtime: Arc<dyn Runtime>,
    ) -> Self {
        let (on_handshake_data_send, on_handshake_data_recv) = oneshot::channel();
        let (on_connected_send, on_connected_recv) = oneshot::channel();
        let conn = ConnectionRef::new(
            handle,
            conn,
            endpoint_events,
            conn_events,
            on_handshake_data_send,
            on_connected_send,
            socket,
            runtime.clone(),
        );

        let driver = ConnectionDriver(conn.clone());
        runtime.spawn(Box::pin(
            async {
                if let Err(e) = driver.await {
                    tracing::error!("I/O error: {e}");
                }
            }
            .instrument(Span::current()),
        ));

        Self {
            conn: Some(conn),
            connected: on_connected_recv,
            handshake_data_ready: Some(on_handshake_data_recv),
        }
    }

    /// Convert into a 0-RTT or 0.5-RTT connection at the cost of weakened security
    ///
    /// Returns `Ok` immediately if the local endpoint is able to attempt sending 0/0.5-RTT data.
    /// If so, the returned [`Connection`] can be used to send application data without waiting for
    /// the rest of the handshake to complete, at the cost of weakened cryptographic security
    /// guarantees. The returned [`ZeroRttAccepted`] future resolves when the handshake does
    /// complete, at which point subsequently opened streams and written data will have full
    /// cryptographic protection.
    ///
    /// ## Outgoing
    ///
    /// For outgoing connections, the initial attempt to convert to a [`Connection`] which sends
    /// 0-RTT data will proceed if the [`crypto::ClientConfig`][crate::crypto::ClientConfig]
    /// attempts to resume a previous TLS session. However, **the remote endpoint may not actually
    /// _accept_ the 0-RTT data**--yet still accept the connection attempt in general. This
    /// possibility is conveyed through the [`ZeroRttAccepted`] future--when the handshake
    /// completes, it resolves to true if the 0-RTT data was accepted and false if it was rejected.
    /// If it was rejected, the existence of streams opened and other application data sent prior
    /// to the handshake completing will not be conveyed to the remote application, and local
    /// operations on them will return `ZeroRttRejected` errors.
    ///
    /// A server may reject 0-RTT data at its discretion, but accepting 0-RTT data requires the
    /// relevant resumption state to be stored in the server, which servers may limit or lose for
    /// various reasons including not persisting resumption state across server restarts.
    ///
    /// If manually providing a [`crypto::ClientConfig`][crate::crypto::ClientConfig], check your
    /// implementation's docs for 0-RTT pitfalls.
    ///
    /// ## Incoming
    ///
    /// For incoming connections, conversion to 0.5-RTT will always fully succeed. `into_0rtt` will
    /// always return `Ok` and the [`ZeroRttAccepted`] will always resolve to true.
    ///
    /// If manually providing a [`crypto::ServerConfig`][crate::crypto::ServerConfig], check your
    /// implementation's docs for 0-RTT pitfalls.
    ///
    /// ## Security
    ///
    /// On outgoing connections, this enables transmission of 0-RTT data, which is vulnerable to
    /// replay attacks, and should therefore never invoke non-idempotent operations.
    ///
    /// On incoming connections, this enables transmission of 0.5-RTT data, which may be sent
    /// before TLS client authentication has occurred, and should therefore not be used to send
    /// data for which client authentication is being used.
    pub fn into_0rtt(mut self) -> Result<(Connection, ZeroRttAccepted), Self> {
        // This lock borrows `self` and would normally be dropped at the end of this scope, so we'll
        // have to release it explicitly before returning `self` by value.
        let conn = (self.conn.as_mut().unwrap()).state.lock("into_0rtt");

        let is_ok = conn.inner.has_0rtt() || conn.inner.side().is_server();
        drop(conn);

        if is_ok {
            let conn = self.conn.take().unwrap();
            Ok((Connection(conn), ZeroRttAccepted(self.connected)))
        } else {
            Err(self)
        }
    }

    /// Parameters negotiated during the handshake
    ///
    /// The dynamic type returned is determined by the configured
    /// [`Session`](proto::crypto::Session). For the default `rustls` session, the return value can
    /// be [`downcast`](Box::downcast) to a
    /// [`crypto::rustls::HandshakeData`](crate::crypto::rustls::HandshakeData).
    pub async fn handshake_data(&mut self) -> Result<Box<dyn Any>, ConnectionError> {
        // Taking &mut self allows us to use a single oneshot channel rather than dealing with
        // potentially many tasks waiting on the same event. It's a bit of a hack, but keeps things
        // simple.
        if let Some(x) = self.handshake_data_ready.take() {
            let _ = x.await;
        }
        let conn = self.conn.as_ref().unwrap();
        let inner = conn.state.lock("handshake");
        inner
            .inner
            .crypto_session()
            .handshake_data()
            .ok_or_else(|| {
                inner
                    .error
                    .clone()
                    .expect("spurious handshake data ready notification")
            })
    }

    /// The local IP address which was used when the peer established
    /// the connection
    ///
    /// This can be different from the address the endpoint is bound to, in case
    /// the endpoint is bound to a wildcard address like `0.0.0.0` or `::`.
    ///
    /// This will return `None` for clients, or when the platform does not expose this
    /// information. See [`quinn_udp::RecvMeta::dst_ip`](udp::RecvMeta::dst_ip) for a list of
    /// supported platforms when using [`quinn_udp`](udp) for I/O, which is the default.
    ///
    /// Will panic if called after `poll` has returned `Ready`.
    pub fn local_ip(&self) -> Option<IpAddr> {
        let conn = self.conn.as_ref().unwrap();
        let inner = conn.state.lock("local_ip");

        inner.inner.local_ip()
    }

    /// The peer's UDP address
    ///
    /// Will panic if called after `poll` has returned `Ready`.
    pub fn remote_address(&self) -> SocketAddr {
        let conn_ref: &ConnectionRef = self.conn.as_ref().expect("used after yielding Ready");
        conn_ref.state.lock("remote_address").inner.remote_address()
    }
}

impl Future for Connecting {
    type Output = Result<Connection, ConnectionError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Pin::new(&mut self.connected).poll(cx).map(|_| {
            let conn = self.conn.take().unwrap();
            let inner = conn.state.lock("connecting");
            if inner.connected {
                drop(inner);
                Ok(Connection(conn))
            } else {
                Err(inner
                    .error
                    .clone()
                    .expect("connected signaled without connection success or error"))
            }
        })
    }
}

/// Future that completes when a connection is fully established
///
/// For clients, the resulting value indicates if 0-RTT was accepted. For servers, the resulting
/// value is meaningless.
pub struct ZeroRttAccepted(oneshot::Receiver<bool>);

impl Future for ZeroRttAccepted {
    type Output = bool;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx).map(|x| x.unwrap_or(false))
    }
}

/// A future that drives protocol logic for a connection
///
/// This future handles the protocol logic for a single connection, routing events from the
/// `Connection` API object to the `Endpoint` task and the related stream-related interfaces.
/// It also keeps track of outstanding timeouts for the `Connection`.
///
/// If the connection encounters an error condition, this future will yield an error. It will
/// terminate (yielding `Ok(())`) if the connection was closed without error. Unlike other
/// connection-related futures, this waits for the draining period to complete to ensure that
/// packets still in flight from the peer are handled gracefully.
#[must_use = "connection drivers must be spawned for their connections to function"]
#[derive(Debug)]
struct ConnectionDriver(ConnectionRef);

impl Future for ConnectionDriver {
    type Output = Result<(), io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let conn = &mut *self.0.state.lock("poll");

        let span = debug_span!("drive", id = conn.handle.0);
        let _guard = span.enter();

        if let Err(e) = conn.process_conn_events(&self.0.shared, cx) {
            conn.terminate(e, &self.0.shared);
            return Poll::Ready(Ok(()));
        }
        let mut keep_going = conn.drive_transmit(cx)?;
        // If a timer expires, there might be more to transmit. When we transmit something, we
        // might need to reset a timer. Hence, we must loop until neither happens.
        keep_going |= conn.drive_timer(cx);
        conn.forward_endpoint_events();
        conn.forward_app_events(&self.0.shared);

        if !conn.inner.is_drained() {
            if keep_going {
                // If the connection hasn't processed all tasks, schedule it again
                cx.waker().wake_by_ref();
            } else {
                conn.driver = Some(cx.waker().clone());
            }
            return Poll::Pending;
        }
        if conn.error.is_none() {
            unreachable!("drained connections always have an error");
        }
        Poll::Ready(Ok(()))
    }
}

/// A QUIC connection.
///
/// If all references to a connection (including every clone of the `Connection` handle, streams of
/// incoming streams, and the various stream types) have been dropped, then the connection will be
/// automatically closed with an `error_code` of 0 and an empty `reason`. You can also close the
/// connection explicitly by calling [`Connection::close()`].
///
/// Closing the connection immediately abandons efforts to deliver data to the peer.  Upon
/// receiving CONNECTION_CLOSE the peer *may* drop any stream data not yet delivered to the
/// application. [`Connection::close()`] describes in more detail how to gracefully close a
/// connection without losing application data.
///
/// May be cloned to obtain another handle to the same connection.
///
/// [`Connection::close()`]: Connection::close
#[derive(Debug, Clone)]
pub struct Connection(ConnectionRef);

impl Connection {
    /// Initiate a new outgoing unidirectional stream.
    ///
    /// Streams are cheap and instantaneous to open unless blocked by flow control. As a
    /// consequence, the peer won't be notified that a stream has been opened until the stream is
    /// actually used.
    pub fn open_uni(&self) -> OpenUni<'_> {
        OpenUni {
            conn: &self.0,
            notify: self.0.shared.stream_budget_available[Dir::Uni as usize].notified(),
        }
    }

    /// Initiate a new outgoing bidirectional stream.
    ///
    /// Streams are cheap and instantaneous to open unless blocked by flow control. As a
    /// consequence, the peer won't be notified that a stream has been opened until the stream is
    /// actually used. Calling [`open_bi()`] then waiting on the [`RecvStream`] without writing
    /// anything to [`SendStream`] will never succeed.
    ///
    /// [`open_bi()`]: crate::Connection::open_bi
    /// [`SendStream`]: crate::SendStream
    /// [`RecvStream`]: crate::RecvStream
    pub fn open_bi(&self) -> OpenBi<'_> {
        OpenBi {
            conn: &self.0,
            notify: self.0.shared.stream_budget_available[Dir::Bi as usize].notified(),
        }
    }

    /// Accept the next incoming uni-directional stream
    pub fn accept_uni(&self) -> AcceptUni<'_> {
        AcceptUni {
            conn: &self.0,
            notify: self.0.shared.stream_incoming[Dir::Uni as usize].notified(),
        }
    }

    /// Accept the next incoming bidirectional stream
    ///
    /// **Important Note**: The `Connection` that calls [`open_bi()`] must write to its [`SendStream`]
    /// before the other `Connection` is able to `accept_bi()`. Calling [`open_bi()`] then
    /// waiting on the [`RecvStream`] without writing anything to [`SendStream`] will never succeed.
    ///
    /// [`accept_bi()`]: crate::Connection::accept_bi
    /// [`open_bi()`]: crate::Connection::open_bi
    /// [`SendStream`]: crate::SendStream
    /// [`RecvStream`]: crate::RecvStream
    pub fn accept_bi(&self) -> AcceptBi<'_> {
        AcceptBi {
            conn: &self.0,
            notify: self.0.shared.stream_incoming[Dir::Bi as usize].notified(),
        }
    }

    /// Receive an application datagram
    pub fn read_datagram(&self) -> ReadDatagram<'_> {
        ReadDatagram {
            conn: &self.0,
            notify: self.0.shared.datagram_received.notified(),
        }
    }

    /// Wait for the connection to be closed for any reason
    ///
    /// Despite the return type's name, closed connections are often not an error condition at the
    /// application layer. Cases that might be routine include [`ConnectionError::LocallyClosed`]
    /// and [`ConnectionError::ApplicationClosed`].
    pub async fn closed(&self) -> ConnectionError {
        {
            let conn = self.0.state.lock("closed");
            if let Some(error) = conn.error.as_ref() {
                return error.clone();
            }
            // Construct the future while the lock is held to ensure we can't miss a wakeup if
            // the `Notify` is signaled immediately after we release the lock. `await` it after
            // the lock guard is out of scope.
            self.0.shared.closed.notified()
        }
        .await;
        self.0
            .state
            .lock("closed")
            .error
            .as_ref()
            .expect("closed without an error")
            .clone()
    }

    /// If the connection is closed, the reason why.
    ///
    /// Returns `None` if the connection is still open.
    pub fn close_reason(&self) -> Option<ConnectionError> {
        self.0.state.lock("close_reason").error.clone()
    }

    /// Close the connection immediately.
    ///
    /// Pending operations will fail immediately with [`ConnectionError::LocallyClosed`]. No
    /// more data is sent to the peer and the peer may drop buffered data upon receiving
    /// the CONNECTION_CLOSE frame.
    ///
    /// `error_code` and `reason` are not interpreted, and are provided directly to the peer.
    ///
    /// `reason` will be truncated to fit in a single packet with overhead; to improve odds that it
    /// is preserved in full, it should be kept under 1KiB.
    ///
    /// # Gracefully closing a connection
    ///
    /// Only the peer last receiving application data can be certain that all data is
    /// delivered. The only reliable action it can then take is to close the connection,
    /// potentially with a custom error code. The delivery of the final CONNECTION_CLOSE
    /// frame is very likely if both endpoints stay online long enough, and
    /// [`Endpoint::wait_idle()`] can be used to provide sufficient time. Otherwise, the
    /// remote peer will time out the connection, provided that the idle timeout is not
    /// disabled.
    ///
    /// The sending side can not guarantee all stream data is delivered to the remote
    /// application. It only knows the data is delivered to the QUIC stack of the remote
    /// endpoint. Once the local side sends a CONNECTION_CLOSE frame in response to calling
    /// [`close()`] the remote endpoint may drop any data it received but is as yet
    /// undelivered to the application, including data that was acknowledged as received to
    /// the local endpoint.
    ///
    /// [`ConnectionError::LocallyClosed`]: crate::ConnectionError::LocallyClosed
    /// [`Endpoint::wait_idle()`]: crate::Endpoint::wait_idle
    /// [`close()`]: Connection::close
    pub fn close(&self, error_code: VarInt, reason: &[u8]) {
        let conn = &mut *self.0.state.lock("close");
        conn.close(error_code, Bytes::copy_from_slice(reason), &self.0.shared);
    }

    /// Transmit `data` as an unreliable, unordered application datagram
    ///
    /// Application datagrams are a low-level primitive. They may be lost or delivered out of order,
    /// and `data` must both fit inside a single QUIC packet and be smaller than the maximum
    /// dictated by the peer.
    ///
    /// Previously queued datagrams which are still unsent may be discarded to make space for this
    /// datagram, in order of oldest to newest.
    pub fn send_datagram(&self, data: Bytes) -> Result<(), SendDatagramError> {
        let conn = &mut *self.0.state.lock("send_datagram");
        if let Some(ref x) = conn.error {
            return Err(SendDatagramError::ConnectionLost(x.clone()));
        }
        use proto::SendDatagramError::*;
        match conn.inner.datagrams().send(data, true) {
            Ok(()) => {
                conn.wake();
                Ok(())
            }
            Err(e) => Err(match e {
                Blocked(..) => unreachable!(),
                UnsupportedByPeer => SendDatagramError::UnsupportedByPeer,
                Disabled => SendDatagramError::Disabled,
                TooLarge => SendDatagramError::TooLarge,
            }),
        }
    }

    /// Transmit `data` as an unreliable, unordered application datagram
    ///
    /// Unlike [`send_datagram()`], this method will wait for buffer space during congestion
    /// conditions, which effectively prioritizes old datagrams over new datagrams.
    ///
    /// See [`send_datagram()`] for details.
    ///
    /// [`send_datagram()`]: Connection::send_datagram
    pub fn send_datagram_wait(&self, data: Bytes) -> SendDatagram<'_> {
        SendDatagram {
            conn: &self.0,
            data: Some(data),
            notify: self.0.shared.datagrams_unblocked.notified(),
        }
    }

    /// Compute the maximum size of datagrams that may be passed to [`send_datagram()`].
    ///
    /// Returns `None` if datagrams are unsupported by the peer or disabled locally.
    ///
    /// This may change over the lifetime of a connection according to variation in the path MTU
    /// estimate. The peer can also enforce an arbitrarily small fixed limit, but if the peer's
    /// limit is large this is guaranteed to be a little over a kilobyte at minimum.
    ///
    /// Not necessarily the maximum size of received datagrams.
    ///
    /// [`send_datagram()`]: Connection::send_datagram
    pub fn max_datagram_size(&self) -> Option<usize> {
        self.0
            .state
            .lock("max_datagram_size")
            .inner
            .datagrams()
            .max_size()
    }

    /// Bytes available in the outgoing datagram buffer
    ///
    /// When greater than zero, calling [`send_datagram()`](Self::send_datagram) with a datagram of
    /// at most this size is guaranteed not to cause older datagrams to be dropped.
    pub fn datagram_send_buffer_space(&self) -> usize {
        self.0
            .state
            .lock("datagram_send_buffer_space")
            .inner
            .datagrams()
            .send_buffer_space()
    }

    /// The side of the connection (client or server)
    pub fn side(&self) -> Side {
        self.0.state.lock("side").inner.side()
    }

    /// The peer's UDP address
    ///
    /// If `ServerConfig::migration` is `true`, clients may change addresses at will, e.g. when
    /// switching to a cellular internet connection.
    pub fn remote_address(&self) -> SocketAddr {
        self.0.state.lock("remote_address").inner.remote_address()
    }

    /// The local IP address which was used when the peer established
    /// the connection
    ///
    /// This can be different from the address the endpoint is bound to, in case
    /// the endpoint is bound to a wildcard address like `0.0.0.0` or `::`.
    ///
    /// This will return `None` for clients, or when the platform does not expose this
    /// information. See [`quinn_udp::RecvMeta::dst_ip`](udp::RecvMeta::dst_ip) for a list of
    /// supported platforms when using [`quinn_udp`](udp) for I/O, which is the default.
    pub fn local_ip(&self) -> Option<IpAddr> {
        self.0.state.lock("local_ip").inner.local_ip()
    }

    /// Current best estimate of this connection's latency (round-trip-time)
    pub fn rtt(&self) -> Duration {
        self.0.state.lock("rtt").inner.rtt()
    }

    /// Returns connection statistics
    pub fn stats(&self) -> ConnectionStats {
        self.0.state.lock("stats").inner.stats()
    }

    /// Current state of the congestion control algorithm, for debugging purposes
    pub fn congestion_state(&self) -> Box<dyn Controller> {
        self.0
            .state
            .lock("congestion_state")
            .inner
            .congestion_state()
            .clone_box()
    }

    /// Parameters negotiated during the handshake
    ///
    /// Guaranteed to return `Some` on fully established connections or after
    /// [`Connecting::handshake_data()`] succeeds. See that method's documentations for details on
    /// the returned value.
    ///
    /// [`Connection::handshake_data()`]: crate::Connecting::handshake_data
    pub fn handshake_data(&self) -> Option<Box<dyn Any>> {
        self.0
            .state
            .lock("handshake_data")
            .inner
            .crypto_session()
            .handshake_data()
    }

    /// Cryptographic identity of the peer
    ///
    /// The dynamic type returned is determined by the configured
    /// [`Session`](proto::crypto::Session). For the default `rustls` session, the return value can
    /// be [`downcast`](Box::downcast) to a <code>Vec<[rustls::pki_types::CertificateDer]></code>
    pub fn peer_identity(&self) -> Option<Box<dyn Any>> {
        self.0
            .state
            .lock("peer_identity")
            .inner
            .crypto_session()
            .peer_identity()
    }

    /// A stable identifier for this connection
    ///
    /// Peer addresses and connection IDs can change, but this value will remain
    /// fixed for the lifetime of the connection.
    pub fn stable_id(&self) -> usize {
        self.0.stable_id()
    }

    /// Update traffic keys spontaneously
    ///
    /// This primarily exists for testing purposes.
    pub fn force_key_update(&self) {
        self.0
            .state
            .lock("force_key_update")
            .inner
            .force_key_update()
    }

    /// Derive keying material from this connection's TLS session secrets.
    ///
    /// When both peers call this method with the same `label` and `context`
    /// arguments and `output` buffers of equal length, they will get the
    /// same sequence of bytes in `output`. These bytes are cryptographically
    /// strong and pseudorandom, and are suitable for use as keying material.
    ///
    /// See [RFC5705](https://tools.ietf.org/html/rfc5705) for more information.
    pub fn export_keying_material(
        &self,
        output: &mut [u8],
        label: &[u8],
        context: &[u8],
    ) -> Result<(), proto::crypto::ExportKeyingMaterialError> {
        self.0
            .state
            .lock("export_keying_material")
            .inner
            .crypto_session()
            .export_keying_material(output, label, context)
    }

    /// Modify the number of remotely initiated unidirectional streams that may be concurrently open
    ///
    /// No streams may be opened by the peer unless fewer than `count` are already open. Large
    /// `count`s increase both minimum and worst-case memory consumption.
    pub fn set_max_concurrent_uni_streams(&self, count: VarInt) {
        let mut conn = self.0.state.lock("set_max_concurrent_uni_streams");
        conn.inner.set_max_concurrent_streams(Dir::Uni, count);
        // May need to send MAX_STREAMS to make progress
        conn.wake();
    }

    /// See [`proto::TransportConfig::send_window()`]
    pub fn set_send_window(&self, send_window: u64) {
        let mut conn = self.0.state.lock("set_send_window");
        conn.inner.set_send_window(send_window);
        conn.wake();
    }

    /// See [`proto::TransportConfig::receive_window()`]
    pub fn set_receive_window(&self, receive_window: VarInt) {
        let mut conn = self.0.state.lock("set_receive_window");
        conn.inner.set_receive_window(receive_window);
        conn.wake();
    }

    /// Modify the number of remotely initiated bidirectional streams that may be concurrently open
    ///
    /// No streams may be opened by the peer unless fewer than `count` are already open. Large
    /// `count`s increase both minimum and worst-case memory consumption.
    pub fn set_max_concurrent_bi_streams(&self, count: VarInt) {
        let mut conn = self.0.state.lock("set_max_concurrent_bi_streams");
        conn.inner.set_max_concurrent_streams(Dir::Bi, count);
        // May need to send MAX_STREAMS to make progress
        conn.wake();
    }
}

pin_project! {
    /// Future produced by [`Connection::open_uni`]
    pub struct OpenUni<'a> {
        conn: &'a ConnectionRef,
        #[pin]
        notify: Notified<'a>,
    }
}

impl Future for OpenUni<'_> {
    type Output = Result<SendStream, ConnectionError>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let (conn, id, is_0rtt) = ready!(poll_open(ctx, this.conn, this.notify, Dir::Uni))?;
        Poll::Ready(Ok(SendStream::new(conn, id, is_0rtt)))
    }
}

pin_project! {
    /// Future produced by [`Connection::open_bi`]
    pub struct OpenBi<'a> {
        conn: &'a ConnectionRef,
        #[pin]
        notify: Notified<'a>,
    }
}

impl Future for OpenBi<'_> {
    type Output = Result<(SendStream, RecvStream), ConnectionError>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let (conn, id, is_0rtt) = ready!(poll_open(ctx, this.conn, this.notify, Dir::Bi))?;

        Poll::Ready(Ok((
            SendStream::new(conn.clone(), id, is_0rtt),
            RecvStream::new(conn, id, is_0rtt),
        )))
    }
}

fn poll_open<'a>(
    ctx: &mut Context<'_>,
    conn: &'a ConnectionRef,
    mut notify: Pin<&mut Notified<'a>>,
    dir: Dir,
) -> Poll<Result<(ConnectionRef, StreamId, bool), ConnectionError>> {
    let mut state = conn.state.lock("poll_open");
    if let Some(ref e) = state.error {
        return Poll::Ready(Err(e.clone()));
    } else if let Some(id) = state.inner.streams().open(dir) {
        let is_0rtt = state.inner.side().is_client() && state.inner.is_handshaking();
        drop(state); // Release the lock so clone can take it
        return Poll::Ready(Ok((conn.clone(), id, is_0rtt)));
    }
    loop {
        match notify.as_mut().poll(ctx) {
            // `state` lock ensures we didn't race with readiness
            Poll::Pending => return Poll::Pending,
            // Spurious wakeup, get a new future
            Poll::Ready(()) => {
                notify.set(conn.shared.stream_budget_available[dir as usize].notified())
            }
        }
    }
}

pin_project! {
    /// Future produced by [`Connection::accept_uni`]
    pub struct AcceptUni<'a> {
        conn: &'a ConnectionRef,
        #[pin]
        notify: Notified<'a>,
    }
}

impl Future for AcceptUni<'_> {
    type Output = Result<RecvStream, ConnectionError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let (conn, id, is_0rtt) = ready!(poll_accept(ctx, this.conn, this.notify, Dir::Uni))?;
        Poll::Ready(Ok(RecvStream::new(conn, id, is_0rtt)))
    }
}

pin_project! {
    /// Future produced by [`Connection::accept_bi`]
    pub struct AcceptBi<'a> {
        conn: &'a ConnectionRef,
        #[pin]
        notify: Notified<'a>,
    }
}

impl Future for AcceptBi<'_> {
    type Output = Result<(SendStream, RecvStream), ConnectionError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let (conn, id, is_0rtt) = ready!(poll_accept(ctx, this.conn, this.notify, Dir::Bi))?;
        Poll::Ready(Ok((
            SendStream::new(conn.clone(), id, is_0rtt),
            RecvStream::new(conn, id, is_0rtt),
        )))
    }
}

fn poll_accept<'a>(
    ctx: &mut Context<'_>,
    conn: &'a ConnectionRef,
    mut notify: Pin<&mut Notified<'a>>,
    dir: Dir,
) -> Poll<Result<(ConnectionRef, StreamId, bool), ConnectionError>> {
    let mut state = conn.state.lock("poll_accept");
    // Check for incoming streams before checking `state.error` so that already-received streams,
    // which are necessarily finite, can be drained from a closed connection.
    if let Some(id) = state.inner.streams().accept(dir) {
        let is_0rtt = state.inner.is_handshaking();
        state.wake(); // To send additional stream ID credit
        drop(state); // Release the lock so clone can take it
        return Poll::Ready(Ok((conn.clone(), id, is_0rtt)));
    } else if let Some(ref e) = state.error {
        return Poll::Ready(Err(e.clone()));
    }
    loop {
        match notify.as_mut().poll(ctx) {
            // `state` lock ensures we didn't race with readiness
            Poll::Pending => return Poll::Pending,
            // Spurious wakeup, get a new future
            Poll::Ready(()) => notify.set(conn.shared.stream_incoming[dir as usize].notified()),
        }
    }
}

pin_project! {
    /// Future produced by [`Connection::read_datagram`]
    pub struct ReadDatagram<'a> {
        conn: &'a ConnectionRef,
        #[pin]
        notify: Notified<'a>,
    }
}

impl Future for ReadDatagram<'_> {
    type Output = Result<Bytes, ConnectionError>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut state = this.conn.state.lock("ReadDatagram::poll");
        // Check for buffered datagrams before checking `state.error` so that already-received
        // datagrams, which are necessarily finite, can be drained from a closed connection.
        if let Some(x) = state.inner.datagrams().recv() {
            return Poll::Ready(Ok(x));
        } else if let Some(ref e) = state.error {
            return Poll::Ready(Err(e.clone()));
        }
        loop {
            match this.notify.as_mut().poll(ctx) {
                // `state` lock ensures we didn't race with readiness
                Poll::Pending => return Poll::Pending,
                // Spurious wakeup, get a new future
                Poll::Ready(()) => this
                    .notify
                    .set(this.conn.shared.datagram_received.notified()),
            }
        }
    }
}

pin_project! {
    /// Future produced by [`Connection::send_datagram_wait`]
    pub struct SendDatagram<'a> {
        conn: &'a ConnectionRef,
        data: Option<Bytes>,
        #[pin]
        notify: Notified<'a>,
    }
}

impl Future for SendDatagram<'_> {
    type Output = Result<(), SendDatagramError>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut state = this.conn.state.lock("SendDatagram::poll");
        if let Some(ref e) = state.error {
            return Poll::Ready(Err(SendDatagramError::ConnectionLost(e.clone())));
        }
        use proto::SendDatagramError::*;
        match state
            .inner
            .datagrams()
            .send(this.data.take().unwrap(), false)
        {
            Ok(()) => {
                state.wake();
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(match e {
                Blocked(data) => {
                    this.data.replace(data);
                    loop {
                        match this.notify.as_mut().poll(ctx) {
                            Poll::Pending => return Poll::Pending,
                            // Spurious wakeup, get a new future
                            Poll::Ready(()) => this
                                .notify
                                .set(this.conn.shared.datagrams_unblocked.notified()),
                        }
                    }
                }
                UnsupportedByPeer => SendDatagramError::UnsupportedByPeer,
                Disabled => SendDatagramError::Disabled,
                TooLarge => SendDatagramError::TooLarge,
            })),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConnectionRef(Arc<ConnectionInner>);

impl ConnectionRef {
    #[allow(clippy::too_many_arguments)]
    fn new(
        handle: ConnectionHandle,
        conn: proto::Connection,
        endpoint_events: mpsc::UnboundedSender<(ConnectionHandle, EndpointEvent)>,
        conn_events: mpsc::UnboundedReceiver<ConnectionEvent>,
        on_handshake_data: oneshot::Sender<()>,
        on_connected: oneshot::Sender<bool>,
        socket: Arc<dyn AsyncUdpSocket>,
        runtime: Arc<dyn Runtime>,
    ) -> Self {
        Self(Arc::new(ConnectionInner {
            state: Mutex::new(State {
                inner: conn,
                driver: None,
                handle,
                on_handshake_data: Some(on_handshake_data),
                on_connected: Some(on_connected),
                connected: false,
                timer: None,
                timer_deadline: None,
                conn_events,
                endpoint_events,
                blocked_writers: FxHashMap::default(),
                blocked_readers: FxHashMap::default(),
                stopped: FxHashMap::default(),
                error: None,
                ref_count: 0,
                io_poller: socket.clone().create_io_poller(),
                socket,
                runtime,
                send_buffer: Vec::new(),
                buffered_transmit: None,
            }),
            shared: Shared::default(),
        }))
    }

    fn stable_id(&self) -> usize {
        &*self.0 as *const _ as usize
    }
}

impl Clone for ConnectionRef {
    fn clone(&self) -> Self {
        self.state.lock("clone").ref_count += 1;
        Self(self.0.clone())
    }
}

impl Drop for ConnectionRef {
    fn drop(&mut self) {
        let conn = &mut *self.state.lock("drop");
        if let Some(x) = conn.ref_count.checked_sub(1) {
            conn.ref_count = x;
            if x == 0 && !conn.inner.is_closed() {
                // If the driver is alive, it's just it and us, so we'd better shut it down. If it's
                // not, we can't do any harm. If there were any streams being opened, then either
                // the connection will be closed for an unrelated reason or a fresh reference will
                // be constructed for the newly opened stream.
                conn.implicit_close(&self.shared);
            }
        }
    }
}

impl std::ops::Deref for ConnectionRef {
    type Target = ConnectionInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct ConnectionInner {
    pub(crate) state: Mutex<State>,
    pub(crate) shared: Shared,
}

#[derive(Debug, Default)]
pub(crate) struct Shared {
    /// Notified when new streams may be locally initiated due to an increase in stream ID flow
    /// control budget
    stream_budget_available: [Notify; 2],
    /// Notified when the peer has initiated a new stream
    stream_incoming: [Notify; 2],
    datagram_received: Notify,
    datagrams_unblocked: Notify,
    closed: Notify,
}

pub(crate) struct State {
    pub(crate) inner: proto::Connection,
    driver: Option<Waker>,
    handle: ConnectionHandle,
    on_handshake_data: Option<oneshot::Sender<()>>,
    on_connected: Option<oneshot::Sender<bool>>,
    connected: bool,
    timer: Option<Pin<Box<dyn AsyncTimer>>>,
    timer_deadline: Option<Instant>,
    conn_events: mpsc::UnboundedReceiver<ConnectionEvent>,
    endpoint_events: mpsc::UnboundedSender<(ConnectionHandle, EndpointEvent)>,
    pub(crate) blocked_writers: FxHashMap<StreamId, Waker>,
    pub(crate) blocked_readers: FxHashMap<StreamId, Waker>,
    pub(crate) stopped: FxHashMap<StreamId, Arc<Notify>>,
    /// Always set to Some before the connection becomes drained
    pub(crate) error: Option<ConnectionError>,
    /// Number of live handles that can be used to initiate or handle I/O; excludes the driver
    ref_count: usize,
    socket: Arc<dyn AsyncUdpSocket>,
    io_poller: Pin<Box<dyn UdpPoller>>,
    runtime: Arc<dyn Runtime>,
    send_buffer: Vec<u8>,
    /// We buffer a transmit when the underlying I/O would block
    buffered_transmit: Option<proto::Transmit>,
}

impl State {
    fn drive_transmit(&mut self, cx: &mut Context) -> io::Result<bool> {
        let now = self.runtime.now();
        let mut transmits = 0;

        let max_datagrams = self
            .socket
            .max_transmit_segments()
            .min(MAX_TRANSMIT_SEGMENTS);

        loop {
            // Retry the last transmit, or get a new one.
            let t = match self.buffered_transmit.take() {
                Some(t) => t,
                None => {
                    self.send_buffer.clear();
                    self.send_buffer.reserve(self.inner.current_mtu() as usize);
                    match self
                        .inner
                        .poll_transmit(now, max_datagrams, &mut self.send_buffer)
                    {
                        Some(t) => {
                            transmits += match t.segment_size {
                                None => 1,
                                Some(s) => t.size.div_ceil(s), // round up
                            };
                            t
                        }
                        None => break,
                    }
                }
            };

            if self.io_poller.as_mut().poll_writable(cx)?.is_pending() {
                // Retry after a future wakeup
                self.buffered_transmit = Some(t);
                return Ok(false);
            }

            let len = t.size;
            let retry = match self
                .socket
                .try_send(&udp_transmit(&t, &self.send_buffer[..len]))
            {
                Ok(()) => false,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => true,
                Err(e) => return Err(e),
            };
            if retry {
                // We thought the socket was writable, but it wasn't. Retry so that either another
                // `poll_writable` call determines that the socket is indeed not writable and
                // registers us for a wakeup, or the send succeeds if this really was just a
                // transient failure.
                self.buffered_transmit = Some(t);
                continue;
            }

            if transmits >= MAX_TRANSMIT_DATAGRAMS {
                // TODO: What isn't ideal here yet is that if we don't poll all
                // datagrams that could be sent we don't go into the `app_limited`
                // state and CWND continues to grow until we get here the next time.
                // See https://github.com/quinn-rs/quinn/issues/1126
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn forward_endpoint_events(&mut self) {
        while let Some(event) = self.inner.poll_endpoint_events() {
            // If the endpoint driver is gone, noop.
            let _ = self.endpoint_events.send((self.handle, event));
        }
    }

    /// If this returns `Err`, the endpoint is dead, so the driver should exit immediately.
    fn process_conn_events(
        &mut self,
        shared: &Shared,
        cx: &mut Context,
    ) -> Result<(), ConnectionError> {
        loop {
            match self.conn_events.poll_recv(cx) {
                Poll::Ready(Some(ConnectionEvent::Rebind(socket))) => {
                    self.socket = socket;
                    self.io_poller = self.socket.clone().create_io_poller();
                    self.inner.local_address_changed();
                }
                Poll::Ready(Some(ConnectionEvent::Proto(event))) => {
                    self.inner.handle_event(event);
                }
                Poll::Ready(Some(ConnectionEvent::Close { reason, error_code })) => {
                    self.close(error_code, reason, shared);
                }
                Poll::Ready(None) => {
                    return Err(ConnectionError::TransportError(proto::TransportError {
                        code: proto::TransportErrorCode::INTERNAL_ERROR,
                        frame: None,
                        reason: "endpoint driver future was dropped".to_string(),
                    }));
                }
                Poll::Pending => {
                    return Ok(());
                }
            }
        }
    }

    fn forward_app_events(&mut self, shared: &Shared) {
        while let Some(event) = self.inner.poll() {
            use proto::Event::*;
            match event {
                HandshakeDataReady => {
                    if let Some(x) = self.on_handshake_data.take() {
                        let _ = x.send(());
                    }
                }
                Connected => {
                    self.connected = true;
                    if let Some(x) = self.on_connected.take() {
                        // We don't care if the on-connected future was dropped
                        let _ = x.send(self.inner.accepted_0rtt());
                    }
                    if self.inner.side().is_client() && !self.inner.accepted_0rtt() {
                        // Wake up rejected 0-RTT streams so they can fail immediately with
                        // `ZeroRttRejected` errors.
                        wake_all(&mut self.blocked_writers);
                        wake_all(&mut self.blocked_readers);
                        wake_all_notify(&mut self.stopped);
                    }
                }
                ConnectionLost { reason } => {
                    self.terminate(reason, shared);
                }
                Stream(StreamEvent::Writable { id }) => wake_stream(id, &mut self.blocked_writers),
                Stream(StreamEvent::Opened { dir: Dir::Uni }) => {
                    shared.stream_incoming[Dir::Uni as usize].notify_waiters();
                }
                Stream(StreamEvent::Opened { dir: Dir::Bi }) => {
                    shared.stream_incoming[Dir::Bi as usize].notify_waiters();
                }
                DatagramReceived => {
                    shared.datagram_received.notify_waiters();
                }
                DatagramsUnblocked => {
                    shared.datagrams_unblocked.notify_waiters();
                }
                Stream(StreamEvent::Readable { id }) => wake_stream(id, &mut self.blocked_readers),
                Stream(StreamEvent::Available { dir }) => {
                    // Might mean any number of streams are ready, so we wake up everyone
                    shared.stream_budget_available[dir as usize].notify_waiters();
                }
                Stream(StreamEvent::Finished { id }) => wake_stream_notify(id, &mut self.stopped),
                Stream(StreamEvent::Stopped { id, .. }) => {
                    wake_stream_notify(id, &mut self.stopped);
                    wake_stream(id, &mut self.blocked_writers);
                }
            }
        }
    }

    fn drive_timer(&mut self, cx: &mut Context) -> bool {
        // Check whether we need to (re)set the timer. If so, we must poll again to ensure the
        // timer is registered with the runtime (and check whether it's already
        // expired).
        match self.inner.poll_timeout() {
            Some(deadline) => {
                if let Some(delay) = &mut self.timer {
                    // There is no need to reset the tokio timer if the deadline
                    // did not change
                    if self
                        .timer_deadline
                        .map(|current_deadline| current_deadline != deadline)
                        .unwrap_or(true)
                    {
                        delay.as_mut().reset(deadline);
                    }
                } else {
                    self.timer = Some(self.runtime.new_timer(deadline));
                }
                // Store the actual expiration time of the timer
                self.timer_deadline = Some(deadline);
            }
            None => {
                self.timer_deadline = None;
                return false;
            }
        }

        if self.timer_deadline.is_none() {
            return false;
        }

        let delay = self
            .timer
            .as_mut()
            .expect("timer must exist in this state")
            .as_mut();
        if delay.poll(cx).is_pending() {
            // Since there wasn't a timeout event, there is nothing new
            // for the connection to do
            return false;
        }

        // A timer expired, so the caller needs to check for
        // new transmits, which might cause new timers to be set.
        self.inner.handle_timeout(self.runtime.now());
        self.timer_deadline = None;
        true
    }

    /// Wake up a blocked `Driver` task to process I/O
    pub(crate) fn wake(&mut self) {
        if let Some(x) = self.driver.take() {
            x.wake();
        }
    }

    /// Used to wake up all blocked futures when the connection becomes closed for any reason
    fn terminate(&mut self, reason: ConnectionError, shared: &Shared) {
        self.error = Some(reason.clone());
        if let Some(x) = self.on_handshake_data.take() {
            let _ = x.send(());
        }
        wake_all(&mut self.blocked_writers);
        wake_all(&mut self.blocked_readers);
        shared.stream_budget_available[Dir::Uni as usize].notify_waiters();
        shared.stream_budget_available[Dir::Bi as usize].notify_waiters();
        shared.stream_incoming[Dir::Uni as usize].notify_waiters();
        shared.stream_incoming[Dir::Bi as usize].notify_waiters();
        shared.datagram_received.notify_waiters();
        shared.datagrams_unblocked.notify_waiters();
        if let Some(x) = self.on_connected.take() {
            let _ = x.send(false);
        }
        wake_all_notify(&mut self.stopped);
        shared.closed.notify_waiters();
    }

    fn close(&mut self, error_code: VarInt, reason: Bytes, shared: &Shared) {
        self.inner.close(self.runtime.now(), error_code, reason);
        self.terminate(ConnectionError::LocallyClosed, shared);
        self.wake();
    }

    /// Close for a reason other than the application's explicit request
    pub(crate) fn implicit_close(&mut self, shared: &Shared) {
        self.close(0u32.into(), Bytes::new(), shared);
    }

    pub(crate) fn check_0rtt(&self) -> Result<(), ()> {
        if self.inner.is_handshaking()
            || self.inner.accepted_0rtt()
            || self.inner.side().is_server()
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if !self.inner.is_drained() {
            // Ensure the endpoint can tidy up
            let _ = self
                .endpoint_events
                .send((self.handle, proto::EndpointEvent::drained()));
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State").field("inner", &self.inner).finish()
    }
}

fn wake_stream(stream_id: StreamId, wakers: &mut FxHashMap<StreamId, Waker>) {
    if let Some(waker) = wakers.remove(&stream_id) {
        waker.wake();
    }
}

fn wake_all(wakers: &mut FxHashMap<StreamId, Waker>) {
    wakers.drain().for_each(|(_, waker)| waker.wake())
}

fn wake_stream_notify(stream_id: StreamId, wakers: &mut FxHashMap<StreamId, Arc<Notify>>) {
    if let Some(notify) = wakers.remove(&stream_id) {
        notify.notify_waiters()
    }
}

fn wake_all_notify(wakers: &mut FxHashMap<StreamId, Arc<Notify>>) {
    wakers
        .drain()
        .for_each(|(_, notify)| notify.notify_waiters())
}

/// Errors that can arise when sending a datagram
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum SendDatagramError {
    /// The peer does not support receiving datagram frames
    #[error("datagrams not supported by peer")]
    UnsupportedByPeer,
    /// Datagram support is disabled locally
    #[error("datagram support disabled")]
    Disabled,
    /// The datagram is larger than the connection can currently accommodate
    ///
    /// Indicates that the path MTU minus overhead or the limit advertised by the peer has been
    /// exceeded.
    #[error("datagram too large")]
    TooLarge,
    /// The connection was lost
    #[error("connection lost")]
    ConnectionLost(#[from] ConnectionError),
}

/// The maximum amount of datagrams which will be produced in a single `drive_transmit` call
///
/// This limits the amount of CPU resources consumed by datagram generation,
/// and allows other tasks (like receiving ACKs) to run in between.
const MAX_TRANSMIT_DATAGRAMS: usize = 20;

/// The maximum amount of datagrams that are sent in a single transmit
///
/// This can be lower than the maximum platform capabilities, to avoid excessive
/// memory allocations when calling `poll_transmit()`. Benchmarks have shown
/// that numbers around 10 are a good compromise.
const MAX_TRANSMIT_SEGMENTS: usize = 10;
