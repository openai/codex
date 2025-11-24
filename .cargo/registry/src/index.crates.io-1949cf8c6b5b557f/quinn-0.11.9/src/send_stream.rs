use std::{
    future::{Future, poll_fn},
    io,
    pin::{Pin, pin},
    task::{Context, Poll},
};

use bytes::Bytes;
use proto::{ClosedStream, ConnectionError, FinishError, StreamId, Written};
use thiserror::Error;

use crate::{
    VarInt,
    connection::{ConnectionRef, State},
};

/// A stream that can only be used to send data
///
/// If dropped, streams that haven't been explicitly [`reset()`] will be implicitly [`finish()`]ed,
/// continuing to (re)transmit previously written data until it has been fully acknowledged or the
/// connection is closed.
///
/// # Cancellation
///
/// A `write` method is said to be *cancel-safe* when dropping its future before the future becomes
/// ready will always result in no data being written to the stream. This is true of methods which
/// succeed immediately when any progress is made, and is not true of methods which might need to
/// perform multiple writes internally before succeeding. Each `write` method documents whether it is
/// cancel-safe.
///
/// [`reset()`]: SendStream::reset
/// [`finish()`]: SendStream::finish
#[derive(Debug)]
pub struct SendStream {
    conn: ConnectionRef,
    stream: StreamId,
    is_0rtt: bool,
}

impl SendStream {
    pub(crate) fn new(conn: ConnectionRef, stream: StreamId, is_0rtt: bool) -> Self {
        Self {
            conn,
            stream,
            is_0rtt,
        }
    }

    /// Write a buffer into this stream, returning how many bytes were written
    ///
    /// Unless this method errors, it waits until some amount of `buf` can be written into this
    /// stream, and then writes as much as it can without waiting again. Due to congestion and flow
    /// control, this may be shorter than `buf.len()`. On success this yields the length of the
    /// prefix that was written.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe. If this does not resolve, no bytes were written.
    pub async fn write(&mut self, buf: &[u8]) -> Result<usize, WriteError> {
        poll_fn(|cx| self.execute_poll(cx, |s| s.write(buf))).await
    }

    /// Write a buffer into this stream in its entirety
    ///
    /// This method repeatedly calls [`write`](Self::write) until all bytes are written, or an
    /// error occurs.
    ///
    /// # Cancel safety
    ///
    /// This method is *not* cancellation safe. Even if this does not resolve, some prefix of `buf`
    /// may have been written when previously polled.
    pub async fn write_all(&mut self, mut buf: &[u8]) -> Result<(), WriteError> {
        while !buf.is_empty() {
            let written = self.write(buf).await?;
            buf = &buf[written..];
        }
        Ok(())
    }

    /// Write a slice of [`Bytes`] into this stream, returning how much was written
    ///
    /// Bytes to try to write are provided to this method as an array of cheaply cloneable chunks.
    /// Unless this method errors, it waits until some amount of those bytes can be written into
    /// this stream, and then writes as much as it can without waiting again. Due to congestion and
    /// flow control, this may be less than the total number of bytes.
    ///
    /// On success, this method both mutates `bufs` and yields an informative [`Written`] struct
    /// indicating how much was written:
    ///
    /// - [`Bytes`] chunks that were fully written are mutated to be [empty](Bytes::is_empty).
    /// - If a [`Bytes`] chunk was partially written, it is [split to](Bytes::split_to) contain
    ///   only the suffix of bytes that were not written.
    /// - The yielded [`Written`] struct indicates how many chunks were fully written as well as
    ///   how many bytes were written.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe. If this does not resolve, no bytes were written.
    pub async fn write_chunks(&mut self, bufs: &mut [Bytes]) -> Result<Written, WriteError> {
        poll_fn(|cx| self.execute_poll(cx, |s| s.write_chunks(bufs))).await
    }

    /// Write a single [`Bytes`] into this stream in its entirety
    ///
    /// Bytes to write are provided to this method as an single cheaply cloneable chunk. This
    /// method repeatedly calls [`write_chunks`](Self::write_chunks) until all bytes are written,
    /// or an error occurs.
    ///
    /// # Cancel safety
    ///
    /// This method is *not* cancellation safe. Even if this does not resolve, some bytes may have
    /// been written when previously polled.
    pub async fn write_chunk(&mut self, buf: Bytes) -> Result<(), WriteError> {
        self.write_all_chunks(&mut [buf]).await?;
        Ok(())
    }

    /// Write a slice of [`Bytes`] into this stream in its entirety
    ///
    /// Bytes to write are provided to this method as an array of cheaply cloneable chunks. This
    /// method repeatedly calls [`write_chunks`](Self::write_chunks) until all bytes are written,
    /// or an error occurs. This method mutates `bufs` by mutating all chunks to be
    /// [empty](Bytes::is_empty).
    ///
    /// # Cancel safety
    ///
    /// This method is *not* cancellation safe. Even if this does not resolve, some bytes may have
    /// been written when previously polled.
    pub async fn write_all_chunks(&mut self, mut bufs: &mut [Bytes]) -> Result<(), WriteError> {
        while !bufs.is_empty() {
            let written = self.write_chunks(bufs).await?;
            bufs = &mut bufs[written.chunks..];
        }
        Ok(())
    }

    fn execute_poll<F, R>(&mut self, cx: &mut Context, write_fn: F) -> Poll<Result<R, WriteError>>
    where
        F: FnOnce(&mut proto::SendStream) -> Result<R, proto::WriteError>,
    {
        use proto::WriteError::*;
        let mut conn = self.conn.state.lock("SendStream::poll_write");
        if self.is_0rtt {
            conn.check_0rtt()
                .map_err(|()| WriteError::ZeroRttRejected)?;
        }
        if let Some(ref x) = conn.error {
            return Poll::Ready(Err(WriteError::ConnectionLost(x.clone())));
        }

        let result = match write_fn(&mut conn.inner.send_stream(self.stream)) {
            Ok(result) => result,
            Err(Blocked) => {
                conn.blocked_writers.insert(self.stream, cx.waker().clone());
                return Poll::Pending;
            }
            Err(Stopped(error_code)) => {
                return Poll::Ready(Err(WriteError::Stopped(error_code)));
            }
            Err(ClosedStream) => {
                return Poll::Ready(Err(WriteError::ClosedStream));
            }
        };

        conn.wake();
        Poll::Ready(Ok(result))
    }

    /// Notify the peer that no more data will ever be written to this stream
    ///
    /// It is an error to write to a [`SendStream`] after `finish()`ing it. [`reset()`](Self::reset)
    /// may still be called after `finish` to abandon transmission of any stream data that might
    /// still be buffered.
    ///
    /// To wait for the peer to receive all buffered stream data, see [`stopped()`](Self::stopped).
    ///
    /// May fail if [`finish()`](Self::finish) or [`reset()`](Self::reset) was previously
    /// called. This error is harmless and serves only to indicate that the caller may have
    /// incorrect assumptions about the stream's state.
    pub fn finish(&mut self) -> Result<(), ClosedStream> {
        let mut conn = self.conn.state.lock("finish");
        match conn.inner.send_stream(self.stream).finish() {
            Ok(()) => {
                conn.wake();
                Ok(())
            }
            Err(FinishError::ClosedStream) => Err(ClosedStream::default()),
            // Harmless. If the application needs to know about stopped streams at this point, it
            // should call `stopped`.
            Err(FinishError::Stopped(_)) => Ok(()),
        }
    }

    /// Close the send stream immediately.
    ///
    /// No new data can be written after calling this method. Locally buffered data is dropped, and
    /// previously transmitted data will no longer be retransmitted if lost. If an attempt has
    /// already been made to finish the stream, the peer may still receive all written data.
    ///
    /// May fail if [`finish()`](Self::finish) or [`reset()`](Self::reset) was previously
    /// called. This error is harmless and serves only to indicate that the caller may have
    /// incorrect assumptions about the stream's state.
    pub fn reset(&mut self, error_code: VarInt) -> Result<(), ClosedStream> {
        let mut conn = self.conn.state.lock("SendStream::reset");
        if self.is_0rtt && conn.check_0rtt().is_err() {
            return Ok(());
        }
        conn.inner.send_stream(self.stream).reset(error_code)?;
        conn.wake();
        Ok(())
    }

    /// Set the priority of the send stream
    ///
    /// Every send stream has an initial priority of 0. Locally buffered data from streams with
    /// higher priority will be transmitted before data from streams with lower priority. Changing
    /// the priority of a stream with pending data may only take effect after that data has been
    /// transmitted. Using many different priority levels per connection may have a negative
    /// impact on performance.
    pub fn set_priority(&self, priority: i32) -> Result<(), ClosedStream> {
        let mut conn = self.conn.state.lock("SendStream::set_priority");
        conn.inner.send_stream(self.stream).set_priority(priority)?;
        Ok(())
    }

    /// Get the priority of the send stream
    pub fn priority(&self) -> Result<i32, ClosedStream> {
        let mut conn = self.conn.state.lock("SendStream::priority");
        conn.inner.send_stream(self.stream).priority()
    }

    /// Completes when the peer stops the stream or reads the stream to completion
    ///
    /// Yields `Some` with the stop error code if the peer stops the stream. Yields `None` if the
    /// local side [`finish()`](Self::finish)es the stream and then the peer acknowledges receipt
    /// of all stream data (although not necessarily the processing of it), after which the peer
    /// closing the stream is no longer meaningful.
    ///
    /// For a variety of reasons, the peer may not send acknowledgements immediately upon receiving
    /// data. As such, relying on `stopped` to know when the peer has read a stream to completion
    /// may introduce more latency than using an application-level response of some sort.
    pub fn stopped(
        &self,
    ) -> impl Future<Output = Result<Option<VarInt>, StoppedError>> + Send + Sync + 'static {
        let conn = self.conn.clone();
        let stream = self.stream;
        let is_0rtt = self.is_0rtt;
        async move {
            loop {
                // The `Notify::notified` future needs to be created while the lock is being held,
                // otherwise a wakeup could be missed if triggered inbetween releasing the lock
                // and creating the future.
                // The lock may only be held in a block without `await`s, otherwise the future
                // becomes `!Send`. `Notify::notified` is lifetime-bound to `Notify`, therefore
                // we need to declare `notify` outside of the block, and initialize it inside.
                let notify;
                {
                    let mut conn = conn.state.lock("SendStream::stopped");
                    if let Some(output) = send_stream_stopped(&mut conn, stream, is_0rtt) {
                        return output;
                    }

                    notify = conn.stopped.entry(stream).or_default().clone();
                    notify.notified()
                }
                .await
            }
        }
    }

    /// Get the identity of this stream
    pub fn id(&self) -> StreamId {
        self.stream
    }

    /// Attempt to write bytes from buf into the stream.
    ///
    /// On success, returns Poll::Ready(Ok(num_bytes_written)).
    ///
    /// If the stream is not ready for writing, the method returns Poll::Pending and arranges
    /// for the current task (via cx.waker().wake_by_ref()) to receive a notification when the
    /// stream becomes writable or is closed.
    pub fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, WriteError>> {
        pin!(self.get_mut().write(buf)).as_mut().poll(cx)
    }
}

/// Check if a send stream is stopped.
///
/// Returns `Some` if the stream is stopped or the connection is closed.
/// Returns `None` if the stream is not stopped.
fn send_stream_stopped(
    conn: &mut State,
    stream: StreamId,
    is_0rtt: bool,
) -> Option<Result<Option<VarInt>, StoppedError>> {
    if is_0rtt && conn.check_0rtt().is_err() {
        return Some(Err(StoppedError::ZeroRttRejected));
    }
    match conn.inner.send_stream(stream).stopped() {
        Err(ClosedStream { .. }) => Some(Ok(None)),
        Ok(Some(error_code)) => Some(Ok(Some(error_code))),
        Ok(None) => conn.error.clone().map(|error| Err(error.into())),
    }
}

#[cfg(feature = "futures-io")]
impl futures_io::AsyncWrite for SendStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.poll_write(cx, buf).map_err(Into::into)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(self.get_mut().finish().map_err(Into::into))
    }
}

impl tokio::io::AsyncWrite for SendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_write(cx, buf).map_err(Into::into)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(self.get_mut().finish().map_err(Into::into))
    }
}

impl Drop for SendStream {
    fn drop(&mut self) {
        let mut conn = self.conn.state.lock("SendStream::drop");

        // clean up any previously registered wakers
        conn.blocked_writers.remove(&self.stream);

        if conn.error.is_some() || (self.is_0rtt && conn.check_0rtt().is_err()) {
            return;
        }
        match conn.inner.send_stream(self.stream).finish() {
            Ok(()) => conn.wake(),
            Err(FinishError::Stopped(reason)) => {
                if conn.inner.send_stream(self.stream).reset(reason).is_ok() {
                    conn.wake();
                }
            }
            // Already finished or reset, which is fine.
            Err(FinishError::ClosedStream) => {}
        }
    }
}

/// Errors that arise from writing to a stream
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The peer is no longer accepting data on this stream
    ///
    /// Carries an application-defined error code.
    #[error("sending stopped by peer: error {0}")]
    Stopped(VarInt),
    /// The connection was lost
    #[error("connection lost")]
    ConnectionLost(#[from] ConnectionError),
    /// The stream has already been finished or reset
    #[error("closed stream")]
    ClosedStream,
    /// This was a 0-RTT stream and the server rejected it
    ///
    /// Can only occur on clients for 0-RTT streams, which can be opened using
    /// [`Connecting::into_0rtt()`].
    ///
    /// [`Connecting::into_0rtt()`]: crate::Connecting::into_0rtt()
    #[error("0-RTT rejected")]
    ZeroRttRejected,
}

impl From<ClosedStream> for WriteError {
    #[inline]
    fn from(_: ClosedStream) -> Self {
        Self::ClosedStream
    }
}

impl From<StoppedError> for WriteError {
    fn from(x: StoppedError) -> Self {
        match x {
            StoppedError::ConnectionLost(e) => Self::ConnectionLost(e),
            StoppedError::ZeroRttRejected => Self::ZeroRttRejected,
        }
    }
}

impl From<WriteError> for io::Error {
    fn from(x: WriteError) -> Self {
        use WriteError::*;
        let kind = match x {
            Stopped(_) | ZeroRttRejected => io::ErrorKind::ConnectionReset,
            ConnectionLost(_) | ClosedStream => io::ErrorKind::NotConnected,
        };
        Self::new(kind, x)
    }
}

/// Errors that arise while monitoring for a send stream stop from the peer
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StoppedError {
    /// The connection was lost
    #[error("connection lost")]
    ConnectionLost(#[from] ConnectionError),
    /// This was a 0-RTT stream and the server rejected it
    ///
    /// Can only occur on clients for 0-RTT streams, which can be opened using
    /// [`Connecting::into_0rtt()`].
    ///
    /// [`Connecting::into_0rtt()`]: crate::Connecting::into_0rtt()
    #[error("0-RTT rejected")]
    ZeroRttRejected,
}

impl From<StoppedError> for io::Error {
    fn from(x: StoppedError) -> Self {
        use StoppedError::*;
        let kind = match x {
            ZeroRttRejected => io::ErrorKind::ConnectionReset,
            ConnectionLost(_) => io::ErrorKind::NotConnected,
        };
        Self::new(kind, x)
    }
}
