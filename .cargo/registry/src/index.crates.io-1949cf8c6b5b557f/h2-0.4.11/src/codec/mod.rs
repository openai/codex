mod error;
mod framed_read;
mod framed_write;

pub use self::error::{SendError, UserError};

use self::framed_read::FramedRead;
use self::framed_write::FramedWrite;

use crate::frame::{self, Data, Frame};
use crate::proto::Error;

use bytes::Buf;
use futures_core::Stream;
use futures_sink::Sink;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::length_delimited;

use std::io;

#[derive(Debug)]
pub struct Codec<T, B> {
    inner: FramedRead<FramedWrite<T, B>>,
}

impl<T, B> Codec<T, B>
where
    T: AsyncRead + AsyncWrite + Unpin,
    B: Buf,
{
    /// Returns a new `Codec` with the default max frame size
    #[inline]
    pub fn new(io: T) -> Self {
        Self::with_max_recv_frame_size(io, frame::DEFAULT_MAX_FRAME_SIZE as usize)
    }

    /// Returns a new `Codec` with the given maximum frame size
    pub fn with_max_recv_frame_size(io: T, max_frame_size: usize) -> Self {
        // Wrap with writer
        let framed_write = FramedWrite::new(io);

        // Delimit the frames
        let delimited = length_delimited::Builder::new()
            .big_endian()
            .length_field_length(3)
            .length_adjustment(9)
            .num_skip(0) // Don't skip the header
            .new_read(framed_write);

        let mut inner = FramedRead::new(delimited);

        // Use FramedRead's method since it checks the value is within range.
        inner.set_max_frame_size(max_frame_size);

        Codec { inner }
    }
}

impl<T, B> Codec<T, B> {
    /// Updates the max received frame size.
    ///
    /// The change takes effect the next time a frame is decoded. In other
    /// words, if a frame is currently in process of being decoded with a frame
    /// size greater than `val` but less than the max frame size in effect
    /// before calling this function, then the frame will be allowed.
    #[inline]
    pub fn set_max_recv_frame_size(&mut self, val: usize) {
        self.inner.set_max_frame_size(val)
    }

    /// Returns the current max received frame size setting.
    ///
    /// This is the largest size this codec will accept from the wire. Larger
    /// frames will be rejected.
    #[cfg(feature = "unstable")]
    #[inline]
    pub fn max_recv_frame_size(&self) -> usize {
        self.inner.max_frame_size()
    }

    /// Returns the max frame size that can be sent to the peer.
    pub fn max_send_frame_size(&self) -> usize {
        self.inner.get_ref().max_frame_size()
    }

    /// Set the peer's max frame size.
    pub fn set_max_send_frame_size(&mut self, val: usize) {
        self.framed_write().set_max_frame_size(val)
    }

    /// Set the peer's header table size size.
    pub fn set_send_header_table_size(&mut self, val: usize) {
        self.framed_write().set_header_table_size(val)
    }

    /// Set the decoder header table size size.
    pub fn set_recv_header_table_size(&mut self, val: usize) {
        self.inner.set_header_table_size(val)
    }

    /// Set the max header list size that can be received.
    pub fn set_max_recv_header_list_size(&mut self, val: usize) {
        self.inner.set_max_header_list_size(val);
    }

    /// Get a reference to the inner stream.
    #[cfg(feature = "unstable")]
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref().get_ref()
    }

    /// Get a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut().get_mut()
    }

    /// Takes the data payload value that was fully written to the socket
    pub(crate) fn take_last_data_frame(&mut self) -> Option<Data<B>> {
        self.framed_write().take_last_data_frame()
    }

    fn framed_write(&mut self) -> &mut FramedWrite<T, B> {
        self.inner.get_mut()
    }
}

impl<T, B> Codec<T, B>
where
    T: AsyncWrite + Unpin,
    B: Buf,
{
    /// Returns `Ready` when the codec can buffer a frame
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().poll_ready(cx)
    }

    /// Buffer a frame.
    ///
    /// `poll_ready` must be called first to ensure that a frame may be
    /// accepted.
    ///
    /// TODO: Rename this to avoid conflicts with Sink::buffer
    pub fn buffer(&mut self, item: Frame<B>) -> Result<(), UserError> {
        self.framed_write().buffer(item)
    }

    /// Flush buffered data to the wire
    pub fn flush(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().flush(cx)
    }

    /// Shutdown the send half
    pub fn shutdown(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        self.framed_write().shutdown(cx)
    }
}

impl<T, B> Stream for Codec<T, B>
where
    T: AsyncRead + Unpin,
{
    type Item = Result<Frame, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl<T, B> Sink<Frame<B>> for Codec<T, B>
where
    T: AsyncWrite + Unpin,
    B: Buf,
{
    type Error = SendError;

    fn start_send(mut self: Pin<&mut Self>, item: Frame<B>) -> Result<(), Self::Error> {
        Codec::buffer(&mut self, item)?;
        Ok(())
    }
    /// Returns `Ready` when the codec can buffer a frame
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.framed_write().poll_ready(cx).map_err(Into::into)
    }

    /// Flush buffered data to the wire
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.framed_write().flush(cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.shutdown(cx))?;
        Poll::Ready(Ok(()))
    }
}

// TODO: remove (or improve) this
impl<T> From<T> for Codec<T, bytes::Bytes>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn from(src: T) -> Self {
        Self::new(src)
    }
}
