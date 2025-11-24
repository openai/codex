use crate::codec::UserError;
use crate::frame::Reason;
use crate::proto::{self, WindowSize};

use bytes::{Buf, Bytes};
use http::HeaderMap;

use std::fmt;
#[cfg(feature = "stream")]
use std::pin::Pin;
use std::task::{Context, Poll};

/// Sends the body stream and trailers to the remote peer.
///
/// # Overview
///
/// A `SendStream` is provided by [`SendRequest`] and [`SendResponse`] once the
/// HTTP/2 message header has been sent sent. It is used to stream the message
/// body and send the message trailers. See method level documentation for more
/// details.
///
/// The `SendStream` instance is also used to manage outbound flow control.
///
/// If a `SendStream` is dropped without explicitly closing the send stream, a
/// `RST_STREAM` frame will be sent. This essentially cancels the request /
/// response exchange.
///
/// The ways to explicitly close the send stream are:
///
/// * Set `end_of_stream` to true when calling [`send_request`],
///   [`send_response`], or [`send_data`].
/// * Send trailers with [`send_trailers`].
/// * Explicitly reset the stream with [`send_reset`].
///
/// # Flow control
///
/// In HTTP/2, data cannot be sent to the remote peer unless there is
/// available window capacity on both the stream and the connection. When a data
/// frame is sent, both the stream window and the connection window are
/// decremented. When the stream level window reaches zero, no further data can
/// be sent on that stream. When the connection level window reaches zero, no
/// further data can be sent on any stream for that connection.
///
/// When the remote peer is ready to receive more data, it sends `WINDOW_UPDATE`
/// frames. These frames increment the windows. See the [specification] for more
/// details on the principles of HTTP/2 flow control.
///
/// The implications for sending data are that the caller **should** ensure that
/// both the stream and the connection has available window capacity before
/// loading the data to send into memory. The `SendStream` instance provides the
/// necessary APIs to perform this logic. This, however, is not an obligation.
/// If the caller attempts to send data on a stream when there is no available
/// window capacity, the library will buffer the data until capacity becomes
/// available, at which point the buffer will be flushed to the connection.
///
/// **NOTE**: There is no bound on the amount of data that the library will
/// buffer. If you are sending large amounts of data, you really should hook
/// into the flow control lifecycle. Otherwise, you risk using up significant
/// amounts of memory.
///
/// To hook into the flow control lifecycle, the caller signals to the library
/// that it intends to send data by calling [`reserve_capacity`], specifying the
/// amount of data, in octets, that the caller intends to send. After this,
/// `poll_capacity` is used to be notified when the requested capacity is
/// assigned to the stream. Once [`poll_capacity`] returns `Ready` with the number
/// of octets available to the stream, the caller is able to actually send the
/// data using [`send_data`].
///
/// Because there is also a connection level window that applies to **all**
/// streams on a connection, when capacity is assigned to a stream (indicated by
/// `poll_capacity` returning `Ready`), this capacity is reserved on the
/// connection and will **not** be assigned to any other stream. If data is
/// never written to the stream, that capacity is effectively lost to other
/// streams and this introduces the risk of deadlocking a connection.
///
/// To avoid throttling data on a connection, the caller should not reserve
/// capacity until ready to send data and once any capacity is assigned to the
/// stream, the caller should immediately send data consuming this capacity.
/// There is no guarantee as to when the full capacity requested will become
/// available. For example, if the caller requests 64 KB of data and 512 bytes
/// become available, the caller should immediately send 512 bytes of data.
///
/// See [`reserve_capacity`] documentation for more details.
///
/// [`SendRequest`]: client/struct.SendRequest.html
/// [`SendResponse`]: server/struct.SendResponse.html
/// [specification]: http://httpwg.org/specs/rfc7540.html#FlowControl
/// [`reserve_capacity`]: #method.reserve_capacity
/// [`poll_capacity`]: #method.poll_capacity
/// [`send_data`]: #method.send_data
/// [`send_request`]: client/struct.SendRequest.html#method.send_request
/// [`send_response`]: server/struct.SendResponse.html#method.send_response
/// [`send_data`]: #method.send_data
/// [`send_trailers`]: #method.send_trailers
/// [`send_reset`]: #method.send_reset
#[derive(Debug)]
pub struct SendStream<B> {
    inner: proto::StreamRef<B>,
}

/// A stream identifier, as described in [Section 5.1.1] of RFC 7540.
///
/// Streams are identified with an unsigned 31-bit integer. Streams
/// initiated by a client MUST use odd-numbered stream identifiers; those
/// initiated by the server MUST use even-numbered stream identifiers.  A
/// stream identifier of zero (0x0) is used for connection control
/// messages; the stream identifier of zero cannot be used to establish a
/// new stream.
///
/// [Section 5.1.1]: https://tools.ietf.org/html/rfc7540#section-5.1.1
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct StreamId(u32);

impl From<StreamId> for u32 {
    fn from(src: StreamId) -> Self {
        src.0
    }
}

/// Receives the body stream and trailers from the remote peer.
///
/// A `RecvStream` is provided by [`client::ResponseFuture`] and
/// [`server::Connection`] with the received HTTP/2 message head (the response
/// and request head respectively).
///
/// A `RecvStream` instance is used to receive the streaming message body and
/// any trailers from the remote peer. It is also used to manage inbound flow
/// control.
///
/// See method level documentation for more details on receiving data. See
/// [`FlowControl`] for more details on inbound flow control.
///
/// [`client::ResponseFuture`]: client/struct.ResponseFuture.html
/// [`server::Connection`]: server/struct.Connection.html
/// [`FlowControl`]: struct.FlowControl.html
/// [`Stream`]: https://docs.rs/futures/0.1/futures/stream/trait.Stream.html
#[must_use = "streams do nothing unless polled"]
pub struct RecvStream {
    inner: FlowControl,
}

/// A handle to release window capacity to a remote stream.
///
/// This type allows the caller to manage inbound data [flow control]. The
/// caller is expected to call [`release_capacity`] after dropping data frames.
///
/// # Overview
///
/// Each stream has a window size. This window size is the maximum amount of
/// inbound data that can be in-flight. In-flight data is defined as data that
/// has been received, but not yet released.
///
/// When a stream is created, the window size is set to the connection's initial
/// window size value. When a data frame is received, the window size is then
/// decremented by size of the data frame before the data is provided to the
/// caller. As the caller finishes using the data, [`release_capacity`] must be
/// called. This will then increment the window size again, allowing the peer to
/// send more data.
///
/// There is also a connection level window as well as the stream level window.
/// Received data counts against the connection level window as well and calls
/// to [`release_capacity`] will also increment the connection level window.
///
/// # Sending `WINDOW_UPDATE` frames
///
/// `WINDOW_UPDATE` frames will not be sent out for **every** call to
/// `release_capacity`, as this would end up slowing down the protocol. Instead,
/// `h2` waits until the window size is increased to a certain threshold and
/// then sends out a single `WINDOW_UPDATE` frame representing all the calls to
/// `release_capacity` since the last `WINDOW_UPDATE` frame.
///
/// This essentially batches window updating.
///
/// # Scenarios
///
/// Following is a basic scenario with an HTTP/2 connection containing a
/// single active stream.
///
/// * A new stream is activated. The receive window is initialized to 1024 (the
///   value of the initial window size for this connection).
/// * A `DATA` frame is received containing a payload of 600 bytes.
/// * The receive window size is reduced to 424 bytes.
/// * [`release_capacity`] is called with 200.
/// * The receive window size is now 624 bytes. The peer may send no more than
///   this.
/// * A `DATA` frame is received with a payload of 624 bytes.
/// * The window size is now 0 bytes. The peer may not send any more data.
/// * [`release_capacity`] is called with 1024.
/// * The receive window size is now 1024 bytes. The peer may now send more
///   data.
///
/// [flow control]: ../index.html#flow-control
/// [`release_capacity`]: struct.FlowControl.html#method.release_capacity
#[derive(Clone, Debug)]
pub struct FlowControl {
    inner: proto::OpaqueStreamRef,
}

/// A handle to send and receive PING frames with the peer.
// NOT Clone on purpose
pub struct PingPong {
    inner: proto::UserPings,
}

/// Sent via [`PingPong`][] to send a PING frame to a peer.
///
/// [`PingPong`]: struct.PingPong.html
pub struct Ping {
    _p: (),
}

/// Received via [`PingPong`][] when a peer acknowledges a [`Ping`][].
///
/// [`PingPong`]: struct.PingPong.html
/// [`Ping`]: struct.Ping.html
pub struct Pong {
    _p: (),
}

// ===== impl SendStream =====

impl<B: Buf> SendStream<B> {
    pub(crate) fn new(inner: proto::StreamRef<B>) -> Self {
        SendStream { inner }
    }

    /// Requests capacity to send data.
    ///
    /// This function is used to express intent to send data. This requests
    /// connection level capacity. Once the capacity is available, it is
    /// assigned to the stream and not reused by other streams.
    ///
    /// This function may be called repeatedly. The `capacity` argument is the
    /// **total** amount of requested capacity. Sequential calls to
    /// `reserve_capacity` are *not* additive. Given the following:
    ///
    /// ```rust
    /// # use h2::*;
    /// # fn doc(mut send_stream: SendStream<&'static [u8]>) {
    /// send_stream.reserve_capacity(100);
    /// send_stream.reserve_capacity(200);
    /// # }
    /// ```
    ///
    /// After the second call to `reserve_capacity`, the *total* requested
    /// capacity will be 200.
    ///
    /// `reserve_capacity` is also used to cancel previous capacity requests.
    /// Given the following:
    ///
    /// ```rust
    /// # use h2::*;
    /// # fn doc(mut send_stream: SendStream<&'static [u8]>) {
    /// send_stream.reserve_capacity(100);
    /// send_stream.reserve_capacity(0);
    /// # }
    /// ```
    ///
    /// After the second call to `reserve_capacity`, the *total* requested
    /// capacity will be 0, i.e. there is no requested capacity for the stream.
    ///
    /// If `reserve_capacity` is called with a lower value than the amount of
    /// capacity **currently** assigned to the stream, this capacity will be
    /// returned to the connection to be re-assigned to other streams.
    ///
    /// Also, the amount of capacity that is reserved gets decremented as data
    /// is sent. For example:
    ///
    /// ```rust
    /// # use h2::*;
    /// # async fn doc(mut send_stream: SendStream<&'static [u8]>) {
    /// send_stream.reserve_capacity(100);
    ///
    /// send_stream.send_data(b"hello", false).unwrap();
    /// // At this point, the total amount of requested capacity is 95 bytes.
    ///
    /// // Calling `reserve_capacity` with `100` again essentially requests an
    /// // additional 5 bytes.
    /// send_stream.reserve_capacity(100);
    /// # }
    /// ```
    ///
    /// See [Flow control](struct.SendStream.html#flow-control) for an overview
    /// of how send flow control works.
    pub fn reserve_capacity(&mut self, capacity: usize) {
        // TODO: Check for overflow
        self.inner.reserve_capacity(capacity as WindowSize)
    }

    /// Returns the stream's current send capacity.
    ///
    /// This allows the caller to check the current amount of available capacity
    /// before sending data.
    pub fn capacity(&self) -> usize {
        self.inner.capacity() as usize
    }

    /// Requests to be notified when the stream's capacity increases.
    ///
    /// Before calling this, capacity should be requested with
    /// `reserve_capacity`. Once capacity is requested, the connection will
    /// assign capacity to the stream **as it becomes available**. There is no
    /// guarantee as to when and in what increments capacity gets assigned to
    /// the stream.
    ///
    /// To get notified when the available capacity increases, the caller calls
    /// `poll_capacity`, which returns `Ready(Some(n))` when `n` has been
    /// increased by the connection. Note that `n` here represents the **total**
    /// amount of assigned capacity at that point in time. It is also possible
    /// that `n` is lower than the previous call if, since then, the caller has
    /// sent data.
    pub fn poll_capacity(&mut self, cx: &mut Context) -> Poll<Option<Result<usize, crate::Error>>> {
        self.inner
            .poll_capacity(cx)
            .map_ok(|w| w as usize)
            .map_err(Into::into)
    }

    /// Sends a single data frame to the remote peer.
    ///
    /// This function may be called repeatedly as long as `end_of_stream` is set
    /// to `false`. Setting `end_of_stream` to `true` sets the end stream flag
    /// on the data frame. Any further calls to `send_data` or `send_trailers`
    /// will return an [`Error`].
    ///
    /// `send_data` can be called without reserving capacity. In this case, the
    /// data is buffered and the capacity is implicitly requested. Once the
    /// capacity becomes available, the data is flushed to the connection.
    /// However, this buffering is unbounded. As such, sending large amounts of
    /// data without reserving capacity before hand could result in large
    /// amounts of data being buffered in memory.
    ///
    /// [`Error`]: struct.Error.html
    pub fn send_data(&mut self, data: B, end_of_stream: bool) -> Result<(), crate::Error> {
        self.inner
            .send_data(data, end_of_stream)
            .map_err(Into::into)
    }

    /// Sends trailers to the remote peer.
    ///
    /// Sending trailers implicitly closes the send stream. Once the send stream
    /// is closed, no more data can be sent.
    pub fn send_trailers(&mut self, trailers: HeaderMap) -> Result<(), crate::Error> {
        self.inner.send_trailers(trailers).map_err(Into::into)
    }

    /// Resets the stream.
    ///
    /// This cancels the request / response exchange. If the response has not
    /// yet been received, the associated `ResponseFuture` will return an
    /// [`Error`] to reflect the canceled exchange.
    ///
    /// [`Error`]: struct.Error.html
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
    /// If connection sees an error, this returns that error instead of a
    /// `Reason`.
    pub fn poll_reset(&mut self, cx: &mut Context) -> Poll<Result<Reason, crate::Error>> {
        self.inner.poll_reset(cx, proto::PollReset::Streaming)
    }

    /// Returns the stream ID of this `SendStream`.
    ///
    /// # Panics
    ///
    /// If the lock on the stream store has been poisoned.
    pub fn stream_id(&self) -> StreamId {
        StreamId::from_internal(self.inner.stream_id())
    }
}

// ===== impl StreamId =====

impl StreamId {
    pub(crate) fn from_internal(id: crate::frame::StreamId) -> Self {
        StreamId(id.into())
    }

    /// Returns the `u32` corresponding to this `StreamId`
    ///
    /// # Note
    ///
    /// This is the same as the `From<StreamId>` implementation, but
    /// included as an inherent method because that implementation doesn't
    /// appear in rustdocs, as well as a way to force the type instead of
    /// relying on inference.
    pub fn as_u32(&self) -> u32 {
        (*self).into()
    }
}
// ===== impl RecvStream =====

impl RecvStream {
    pub(crate) fn new(inner: FlowControl) -> Self {
        RecvStream { inner }
    }

    /// Get the next data frame.
    pub async fn data(&mut self) -> Option<Result<Bytes, crate::Error>> {
        crate::poll_fn(move |cx| self.poll_data(cx)).await
    }

    /// Get optional trailers for this stream.
    pub async fn trailers(&mut self) -> Result<Option<HeaderMap>, crate::Error> {
        crate::poll_fn(move |cx| self.poll_trailers(cx)).await
    }

    /// Poll for the next data frame.
    pub fn poll_data(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Bytes, crate::Error>>> {
        self.inner.inner.poll_data(cx).map_err(Into::into)
    }

    #[doc(hidden)]
    pub fn poll_trailers(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Result<Option<HeaderMap>, crate::Error>> {
        match ready!(self.inner.inner.poll_trailers(cx)) {
            Some(Ok(map)) => Poll::Ready(Ok(Some(map))),
            Some(Err(e)) => Poll::Ready(Err(e.into())),
            None => Poll::Ready(Ok(None)),
        }
    }

    /// Returns true if the receive half has reached the end of stream.
    ///
    /// A return value of `true` means that calls to `poll` and `poll_trailers`
    /// will both return `None`.
    pub fn is_end_stream(&self) -> bool {
        self.inner.inner.is_end_stream()
    }

    /// Get a mutable reference to this stream's `FlowControl`.
    ///
    /// It can be used immediately, or cloned to be used later.
    pub fn flow_control(&mut self) -> &mut FlowControl {
        &mut self.inner
    }

    /// Returns the stream ID of this stream.
    ///
    /// # Panics
    ///
    /// If the lock on the stream store has been poisoned.
    pub fn stream_id(&self) -> StreamId {
        self.inner.stream_id()
    }
}

#[cfg(feature = "stream")]
impl futures_core::Stream for RecvStream {
    type Item = Result<Bytes, crate::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_data(cx)
    }
}

impl fmt::Debug for RecvStream {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("RecvStream")
            .field("inner", &self.inner)
            .finish()
    }
}

impl Drop for RecvStream {
    fn drop(&mut self) {
        // Eagerly clear any received DATA frames now, since its no longer
        // possible to retrieve them. However, this will be called
        // again once *all* stream refs have been dropped, since
        // this won't send a RST_STREAM frame, in case the user wishes to
        // still *send* DATA.
        self.inner.inner.clear_recv_buffer();
    }
}

// ===== impl FlowControl =====

impl FlowControl {
    pub(crate) fn new(inner: proto::OpaqueStreamRef) -> Self {
        FlowControl { inner }
    }

    /// Returns the stream ID of the stream whose capacity will
    /// be released by this `FlowControl`.
    pub fn stream_id(&self) -> StreamId {
        StreamId::from_internal(self.inner.stream_id())
    }

    /// Get the current available capacity of data this stream *could* receive.
    pub fn available_capacity(&self) -> isize {
        self.inner.available_recv_capacity()
    }

    /// Get the currently *used* capacity for this stream.
    ///
    /// This is the amount of bytes that can be released back to the remote.
    pub fn used_capacity(&self) -> usize {
        self.inner.used_recv_capacity() as usize
    }

    /// Release window capacity back to remote stream.
    ///
    /// This releases capacity back to the stream level and the connection level
    /// windows. Both window sizes will be increased by `sz`.
    ///
    /// See [struct level] documentation for more details.
    ///
    /// # Errors
    ///
    /// This function errors if increasing the receive window size by `sz` would
    /// result in a window size greater than the target window size. In other
    /// words, the caller cannot release more capacity than data has been
    /// received. If 1024 bytes of data have been received, at most 1024 bytes
    /// can be released.
    ///
    /// [struct level]: #
    pub fn release_capacity(&mut self, sz: usize) -> Result<(), crate::Error> {
        if sz > proto::MAX_WINDOW_SIZE as usize {
            return Err(UserError::ReleaseCapacityTooBig.into());
        }
        self.inner
            .release_capacity(sz as proto::WindowSize)
            .map_err(Into::into)
    }
}

// ===== impl PingPong =====

impl PingPong {
    pub(crate) fn new(inner: proto::UserPings) -> Self {
        PingPong { inner }
    }

    /// Send a PING frame and wait for the peer to send the pong.
    pub async fn ping(&mut self, ping: Ping) -> Result<Pong, crate::Error> {
        self.send_ping(ping)?;
        crate::poll_fn(|cx| self.poll_pong(cx)).await
    }

    #[doc(hidden)]
    pub fn send_ping(&mut self, ping: Ping) -> Result<(), crate::Error> {
        // Passing a `Ping` here is just to be forwards-compatible with
        // eventually allowing choosing a ping payload. For now, we can
        // just ignore it.
        let _ = ping;

        self.inner.send_ping().map_err(|err| match err {
            Some(err) => err.into(),
            None => UserError::SendPingWhilePending.into(),
        })
    }

    #[doc(hidden)]
    pub fn poll_pong(&mut self, cx: &mut Context) -> Poll<Result<Pong, crate::Error>> {
        ready!(self.inner.poll_pong(cx))?;
        Poll::Ready(Ok(Pong { _p: () }))
    }
}

impl fmt::Debug for PingPong {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("PingPong").finish()
    }
}

// ===== impl Ping =====

impl Ping {
    /// Creates a new opaque `Ping` to be sent via a [`PingPong`][].
    ///
    /// The payload is "opaque", such that it shouldn't be depended on.
    ///
    /// [`PingPong`]: struct.PingPong.html
    pub fn opaque() -> Ping {
        Ping { _p: () }
    }
}

impl fmt::Debug for Ping {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Ping").finish()
    }
}

// ===== impl Pong =====

impl fmt::Debug for Pong {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Pong").finish()
    }
}
