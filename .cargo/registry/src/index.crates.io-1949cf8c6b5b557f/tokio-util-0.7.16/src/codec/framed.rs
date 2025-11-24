use crate::codec::decoder::Decoder;
use crate::codec::encoder::Encoder;
use crate::codec::framed_impl::{FramedImpl, RWFrames, ReadFrame, WriteFrame};

use futures_core::Stream;
use tokio::io::{AsyncRead, AsyncWrite};

use bytes::BytesMut;
use futures_sink::Sink;
use pin_project_lite::pin_project;
use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A unified [`Stream`] and [`Sink`] interface to an underlying I/O object, using
    /// the `Encoder` and `Decoder` traits to encode and decode frames.
    ///
    /// You can create a `Framed` instance by using the [`Decoder::framed`] adapter, or
    /// by using the `new` function seen below.
    ///
    /// # Cancellation safety
    ///
    /// * [`futures_util::sink::SinkExt::send`]: if send is used as the event in a
    /// `tokio::select!` statement and some other branch completes first, then it is
    /// guaranteed that the message was not sent, but the message itself is lost.
    /// * [`tokio_stream::StreamExt::next`]: This method is cancel safe. The returned
    /// future only holds onto a reference to the underlying stream, so dropping it will
    /// never lose a value.
    ///
    /// [`Stream`]: futures_core::Stream
    /// [`Sink`]: futures_sink::Sink
    /// [`AsyncRead`]: tokio::io::AsyncRead
    /// [`Decoder::framed`]: crate::codec::Decoder::framed()
    /// [`futures_util::sink::SinkExt::send`]: futures_util::sink::SinkExt::send
    /// [`tokio_stream::StreamExt::next`]: https://docs.rs/tokio-stream/latest/tokio_stream/trait.StreamExt.html#method.next
    pub struct Framed<T, U> {
        #[pin]
        inner: FramedImpl<T, U, RWFrames>
    }
}

impl<T, U> Framed<T, U>
where
    T: AsyncRead + AsyncWrite,
{
    /// Provides a [`Stream`] and [`Sink`] interface for reading and writing to this
    /// I/O object, using [`Decoder`] and [`Encoder`] to read and write the raw data.
    ///
    /// Raw I/O objects work with byte sequences, but higher-level code usually
    /// wants to batch these into meaningful chunks, called "frames". This
    /// method layers framing on top of an I/O object, by using the codec
    /// traits to handle encoding and decoding of messages frames. Note that
    /// the incoming and outgoing frame types may be distinct.
    ///
    /// This function returns a *single* object that is both [`Stream`] and
    /// [`Sink`]; grouping this into a single object is often useful for layering
    /// things like gzip or TLS, which require both read and write access to the
    /// underlying object.
    ///
    /// If you want to work more directly with the streams and sink, consider
    /// calling [`split`] on the `Framed` returned by this method, which will
    /// break them into separate objects, allowing them to interact more easily.
    ///
    /// Note that, for some byte sources, the stream can be resumed after an EOF
    /// by reading from it, even after it has returned `None`. Repeated attempts
    /// to do so, without new data available, continue to return `None` without
    /// creating more (closing) frames.
    ///
    /// [`Stream`]: futures_core::Stream
    /// [`Sink`]: futures_sink::Sink
    /// [`Decode`]: crate::codec::Decoder
    /// [`Encoder`]: crate::codec::Encoder
    /// [`split`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.split
    pub fn new(inner: T, codec: U) -> Framed<T, U> {
        Framed {
            inner: FramedImpl {
                inner,
                codec,
                state: Default::default(),
            },
        }
    }

    /// Provides a [`Stream`] and [`Sink`] interface for reading and writing to this
    /// I/O object, using [`Decoder`] and [`Encoder`] to read and write the raw data,
    /// with a specific read buffer initial capacity.
    ///
    /// Raw I/O objects work with byte sequences, but higher-level code usually
    /// wants to batch these into meaningful chunks, called "frames". This
    /// method layers framing on top of an I/O object, by using the codec
    /// traits to handle encoding and decoding of messages frames. Note that
    /// the incoming and outgoing frame types may be distinct.
    ///
    /// This function returns a *single* object that is both [`Stream`] and
    /// [`Sink`]; grouping this into a single object is often useful for layering
    /// things like gzip or TLS, which require both read and write access to the
    /// underlying object.
    ///
    /// If you want to work more directly with the streams and sink, consider
    /// calling [`split`] on the `Framed` returned by this method, which will
    /// break them into separate objects, allowing them to interact more easily.
    ///
    /// [`Stream`]: futures_core::Stream
    /// [`Sink`]: futures_sink::Sink
    /// [`Decode`]: crate::codec::Decoder
    /// [`Encoder`]: crate::codec::Encoder
    /// [`split`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.split
    pub fn with_capacity(inner: T, codec: U, capacity: usize) -> Framed<T, U> {
        Framed {
            inner: FramedImpl {
                inner,
                codec,
                state: RWFrames {
                    read: ReadFrame {
                        eof: false,
                        is_readable: false,
                        buffer: BytesMut::with_capacity(capacity),
                        has_errored: false,
                    },
                    write: WriteFrame {
                        buffer: BytesMut::with_capacity(capacity),
                        backpressure_boundary: capacity,
                    },
                },
            },
        }
    }
}

impl<T, U> Framed<T, U> {
    /// Provides a [`Stream`] and [`Sink`] interface for reading and writing to this
    /// I/O object, using [`Decoder`] and [`Encoder`] to read and write the raw data.
    ///
    /// Raw I/O objects work with byte sequences, but higher-level code usually
    /// wants to batch these into meaningful chunks, called "frames". This
    /// method layers framing on top of an I/O object, by using the `Codec`
    /// traits to handle encoding and decoding of messages frames. Note that
    /// the incoming and outgoing frame types may be distinct.
    ///
    /// This function returns a *single* object that is both [`Stream`] and
    /// [`Sink`]; grouping this into a single object is often useful for layering
    /// things like gzip or TLS, which require both read and write access to the
    /// underlying object.
    ///
    /// This objects takes a stream and a `readbuffer` and a `writebuffer`. These field
    /// can be obtained from an existing `Framed` with the [`into_parts`] method.
    ///
    /// If you want to work more directly with the streams and sink, consider
    /// calling [`split`] on the `Framed` returned by this method, which will
    /// break them into separate objects, allowing them to interact more easily.
    ///
    /// [`Stream`]: futures_core::Stream
    /// [`Sink`]: futures_sink::Sink
    /// [`Decoder`]: crate::codec::Decoder
    /// [`Encoder`]: crate::codec::Encoder
    /// [`into_parts`]: crate::codec::Framed::into_parts()
    /// [`split`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.split
    pub fn from_parts(parts: FramedParts<T, U>) -> Framed<T, U> {
        Framed {
            inner: FramedImpl {
                inner: parts.io,
                codec: parts.codec,
                state: RWFrames {
                    read: parts.read_buf.into(),
                    write: parts.write_buf.into(),
                },
            },
        }
    }

    /// Returns a reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_ref(&self) -> &T {
        &self.inner.inner
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner.inner
    }

    /// Returns a pinned mutable reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner.project().inner
    }

    /// Returns a reference to the underlying codec wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying codec
    /// as it may corrupt the stream of frames otherwise being worked with.
    pub fn codec(&self) -> &U {
        &self.inner.codec
    }

    /// Returns a mutable reference to the underlying codec wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying codec
    /// as it may corrupt the stream of frames otherwise being worked with.
    pub fn codec_mut(&mut self) -> &mut U {
        &mut self.inner.codec
    }

    /// Maps the codec `U` to `C`, preserving the read and write buffers
    /// wrapped by `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying codec
    /// as it may corrupt the stream of frames otherwise being worked with.
    pub fn map_codec<C, F>(self, map: F) -> Framed<T, C>
    where
        F: FnOnce(U) -> C,
    {
        // This could be potentially simplified once rust-lang/rust#86555 hits stable
        let parts = self.into_parts();
        Framed::from_parts(FramedParts {
            io: parts.io,
            codec: map(parts.codec),
            read_buf: parts.read_buf,
            write_buf: parts.write_buf,
            _priv: (),
        })
    }

    /// Returns a mutable reference to the underlying codec wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying codec
    /// as it may corrupt the stream of frames otherwise being worked with.
    pub fn codec_pin_mut(self: Pin<&mut Self>) -> &mut U {
        self.project().inner.project().codec
    }

    /// Returns a reference to the read buffer.
    pub fn read_buffer(&self) -> &BytesMut {
        &self.inner.state.read.buffer
    }

    /// Returns a mutable reference to the read buffer.
    pub fn read_buffer_mut(&mut self) -> &mut BytesMut {
        &mut self.inner.state.read.buffer
    }

    /// Returns a reference to the write buffer.
    pub fn write_buffer(&self) -> &BytesMut {
        &self.inner.state.write.buffer
    }

    /// Returns a mutable reference to the write buffer.
    pub fn write_buffer_mut(&mut self) -> &mut BytesMut {
        &mut self.inner.state.write.buffer
    }

    /// Returns backpressure boundary
    pub fn backpressure_boundary(&self) -> usize {
        self.inner.state.write.backpressure_boundary
    }

    /// Updates backpressure boundary
    pub fn set_backpressure_boundary(&mut self, boundary: usize) {
        self.inner.state.write.backpressure_boundary = boundary;
    }

    /// Consumes the `Framed`, returning its underlying I/O stream.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn into_inner(self) -> T {
        self.inner.inner
    }

    /// Consumes the `Framed`, returning its underlying I/O stream, the buffer
    /// with unprocessed data, and the codec.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn into_parts(self) -> FramedParts<T, U> {
        FramedParts {
            io: self.inner.inner,
            codec: self.inner.codec,
            read_buf: self.inner.state.read.buffer,
            write_buf: self.inner.state.write.buffer,
            _priv: (),
        }
    }
}

// This impl just defers to the underlying FramedImpl
impl<T, U> Stream for Framed<T, U>
where
    T: AsyncRead,
    U: Decoder,
{
    type Item = Result<U::Item, U::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

// This impl just defers to the underlying FramedImpl
impl<T, I, U> Sink<I> for Framed<T, U>
where
    T: AsyncWrite,
    U: Encoder<I>,
    U::Error: From<io::Error>,
{
    type Error = U::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: I) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

impl<T, U> fmt::Debug for Framed<T, U>
where
    T: fmt::Debug,
    U: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Framed")
            .field("io", self.get_ref())
            .field("codec", self.codec())
            .finish()
    }
}

/// `FramedParts` contains an export of the data of a Framed transport.
/// It can be used to construct a new [`Framed`] with a different codec.
/// It contains all current buffers and the inner transport.
///
/// [`Framed`]: crate::codec::Framed
#[derive(Debug)]
#[allow(clippy::manual_non_exhaustive)]
pub struct FramedParts<T, U> {
    /// The inner transport used to read bytes to and write bytes to
    pub io: T,

    /// The codec
    pub codec: U,

    /// The buffer with read but unprocessed data.
    pub read_buf: BytesMut,

    /// A buffer with unprocessed data which are not written yet.
    pub write_buf: BytesMut,

    /// This private field allows us to add additional fields in the future in a
    /// backwards compatible way.
    _priv: (),
}

impl<T, U> FramedParts<T, U> {
    /// Create a new, default, `FramedParts`
    pub fn new<I>(io: T, codec: U) -> FramedParts<T, U>
    where
        U: Encoder<I>,
    {
        FramedParts {
            io,
            codec,
            read_buf: BytesMut::new(),
            write_buf: BytesMut::new(),
            _priv: (),
        }
    }
}
