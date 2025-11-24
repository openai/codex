//! Types used by compression and decompression middleware.

use crate::{content_encoding::SupportedEncodings, BoxError};
use bytes::{Buf, Bytes, BytesMut};
use futures_core::Stream;
use http::HeaderValue;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use std::{
    io,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;

#[derive(Debug, Clone, Copy)]
pub(crate) struct AcceptEncoding {
    pub(crate) gzip: bool,
    pub(crate) deflate: bool,
    pub(crate) br: bool,
    pub(crate) zstd: bool,
}

impl AcceptEncoding {
    #[allow(dead_code)]
    pub(crate) fn to_header_value(self) -> Option<HeaderValue> {
        let accept = match (self.gzip(), self.deflate(), self.br(), self.zstd()) {
            (true, true, true, false) => "gzip,deflate,br",
            (true, true, false, false) => "gzip,deflate",
            (true, false, true, false) => "gzip,br",
            (true, false, false, false) => "gzip",
            (false, true, true, false) => "deflate,br",
            (false, true, false, false) => "deflate",
            (false, false, true, false) => "br",
            (true, true, true, true) => "zstd,gzip,deflate,br",
            (true, true, false, true) => "zstd,gzip,deflate",
            (true, false, true, true) => "zstd,gzip,br",
            (true, false, false, true) => "zstd,gzip",
            (false, true, true, true) => "zstd,deflate,br",
            (false, true, false, true) => "zstd,deflate",
            (false, false, true, true) => "zstd,br",
            (false, false, false, true) => "zstd",
            (false, false, false, false) => return None,
        };
        Some(HeaderValue::from_static(accept))
    }

    #[allow(dead_code)]
    pub(crate) fn set_gzip(&mut self, enable: bool) {
        self.gzip = enable;
    }

    #[allow(dead_code)]
    pub(crate) fn set_deflate(&mut self, enable: bool) {
        self.deflate = enable;
    }

    #[allow(dead_code)]
    pub(crate) fn set_br(&mut self, enable: bool) {
        self.br = enable;
    }

    #[allow(dead_code)]
    pub(crate) fn set_zstd(&mut self, enable: bool) {
        self.zstd = enable;
    }
}

impl SupportedEncodings for AcceptEncoding {
    #[allow(dead_code)]
    fn gzip(&self) -> bool {
        #[cfg(any(feature = "decompression-gzip", feature = "compression-gzip"))]
        return self.gzip;

        #[cfg(not(any(feature = "decompression-gzip", feature = "compression-gzip")))]
        return false;
    }

    #[allow(dead_code)]
    fn deflate(&self) -> bool {
        #[cfg(any(feature = "decompression-deflate", feature = "compression-deflate"))]
        return self.deflate;

        #[cfg(not(any(feature = "decompression-deflate", feature = "compression-deflate")))]
        return false;
    }

    #[allow(dead_code)]
    fn br(&self) -> bool {
        #[cfg(any(feature = "decompression-br", feature = "compression-br"))]
        return self.br;

        #[cfg(not(any(feature = "decompression-br", feature = "compression-br")))]
        return false;
    }

    #[allow(dead_code)]
    fn zstd(&self) -> bool {
        #[cfg(any(feature = "decompression-zstd", feature = "compression-zstd"))]
        return self.zstd;

        #[cfg(not(any(feature = "decompression-zstd", feature = "compression-zstd")))]
        return false;
    }
}

impl Default for AcceptEncoding {
    fn default() -> Self {
        AcceptEncoding {
            gzip: true,
            deflate: true,
            br: true,
            zstd: true,
        }
    }
}

/// A `Body` that has been converted into an `AsyncRead`.
pub(crate) type AsyncReadBody<B> =
    StreamReader<StreamErrorIntoIoError<BodyIntoStream<B>, <B as Body>::Error>, <B as Body>::Data>;

/// Trait for applying some decorator to an `AsyncRead`
pub(crate) trait DecorateAsyncRead {
    type Input: AsyncRead;
    type Output: AsyncRead;

    /// Apply the decorator
    fn apply(input: Self::Input, quality: CompressionLevel) -> Self::Output;

    /// Get a pinned mutable reference to the original input.
    ///
    /// This is necessary to implement `Body::poll_trailers`.
    fn get_pin_mut(pinned: Pin<&mut Self::Output>) -> Pin<&mut Self::Input>;
}

pin_project! {
    /// `Body` that has been decorated by an `AsyncRead`
    pub(crate) struct WrapBody<M: DecorateAsyncRead> {
        #[pin]
        // rust-analyer thinks this field is private if its `pub(crate)` but works fine when its
        // `pub`
        pub read: M::Output,
        // A buffer to temporarily store the data read from the underlying body.
        // Reused as much as possible to optimize allocations.
        buf: BytesMut,
        read_all_data: bool,
    }
}

impl<M: DecorateAsyncRead> WrapBody<M> {
    const INTERNAL_BUF_CAPACITY: usize = 4096;
}

impl<M: DecorateAsyncRead> WrapBody<M> {
    #[allow(dead_code)]
    pub(crate) fn new<B>(body: B, quality: CompressionLevel) -> Self
    where
        B: Body,
        M: DecorateAsyncRead<Input = AsyncReadBody<B>>,
    {
        // convert `Body` into a `Stream`
        let stream = BodyIntoStream::new(body);

        // an adapter that converts the error type into `io::Error` while storing the actual error
        // `StreamReader` requires the error type is `io::Error`
        let stream = StreamErrorIntoIoError::<_, B::Error>::new(stream);

        // convert `Stream` into an `AsyncRead`
        let read = StreamReader::new(stream);

        // apply decorator to `AsyncRead` yielding another `AsyncRead`
        let read = M::apply(read, quality);

        Self {
            read,
            buf: BytesMut::with_capacity(Self::INTERNAL_BUF_CAPACITY),
            read_all_data: false,
        }
    }
}

impl<B, M> Body for WrapBody<M>
where
    B: Body,
    B::Error: Into<BoxError>,
    M: DecorateAsyncRead<Input = AsyncReadBody<B>>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();

        if !*this.read_all_data {
            if this.buf.capacity() == 0 {
                this.buf.reserve(Self::INTERNAL_BUF_CAPACITY);
            }

            let result = tokio_util::io::poll_read_buf(this.read.as_mut(), cx, &mut this.buf);

            match ready!(result) {
                Ok(0) => {
                    *this.read_all_data = true;
                }
                Ok(_) => {
                    let chunk = this.buf.split().freeze();
                    return Poll::Ready(Some(Ok(Frame::data(chunk))));
                }
                Err(err) => {
                    let body_error: Option<B::Error> = M::get_pin_mut(this.read)
                        .get_pin_mut()
                        .project()
                        .error
                        .take();

                    if let Some(body_error) = body_error {
                        return Poll::Ready(Some(Err(body_error.into())));
                    } else if err.raw_os_error() == Some(SENTINEL_ERROR_CODE) {
                        // SENTINEL_ERROR_CODE only gets used when storing
                        // an underlying body error
                        unreachable!()
                    } else {
                        return Poll::Ready(Some(Err(err.into())));
                    }
                }
            }
        }

        // poll any remaining frames, such as trailers
        let body = M::get_pin_mut(this.read).get_pin_mut().get_pin_mut();
        body.poll_frame(cx).map(|option| {
            option.map(|result| {
                result
                    .map(|frame| frame.map_data(|mut data| data.copy_to_bytes(data.remaining())))
                    .map_err(|err| err.into())
            })
        })
    }
}

pin_project! {
    pub(crate) struct BodyIntoStream<B>
    where
        B: Body,
    {
        #[pin]
        body: B,
        yielded_all_data: bool,
        non_data_frame: Option<Frame<B::Data>>,
    }
}

#[allow(dead_code)]
impl<B> BodyIntoStream<B>
where
    B: Body,
{
    pub(crate) fn new(body: B) -> Self {
        Self {
            body,
            yielded_all_data: false,
            non_data_frame: None,
        }
    }

    /// Get a reference to the inner body
    pub(crate) fn get_ref(&self) -> &B {
        &self.body
    }

    /// Get a mutable reference to the inner body
    pub(crate) fn get_mut(&mut self) -> &mut B {
        &mut self.body
    }

    /// Get a pinned mutable reference to the inner body
    pub(crate) fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut B> {
        self.project().body
    }

    /// Consume `self`, returning the inner body
    pub(crate) fn into_inner(self) -> B {
        self.body
    }
}

impl<B> Stream for BodyIntoStream<B>
where
    B: Body,
{
    type Item = Result<B::Data, B::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let this = self.as_mut().project();

            if *this.yielded_all_data {
                return Poll::Ready(None);
            }

            match std::task::ready!(this.body.poll_frame(cx)) {
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(data) => return Poll::Ready(Some(Ok(data))),
                    Err(frame) => {
                        *this.yielded_all_data = true;
                        *this.non_data_frame = Some(frame);
                    }
                },
                Some(Err(err)) => return Poll::Ready(Some(Err(err))),
                None => {
                    *this.yielded_all_data = true;
                }
            }
        }
    }
}

impl<B> Body for BodyIntoStream<B>
where
    B: Body,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // First drive the stream impl. This consumes all data frames and buffer at most one
        // trailers frame.
        if let Some(frame) = std::task::ready!(self.as_mut().poll_next(cx)) {
            return Poll::Ready(Some(frame.map(Frame::data)));
        }

        let this = self.project();

        // Yield the trailers frame `poll_next` hit.
        if let Some(frame) = this.non_data_frame.take() {
            return Poll::Ready(Some(Ok(frame)));
        }

        // Yield any remaining frames in the body. There shouldn't be any after the trailers but
        // you never know.
        this.body.poll_frame(cx)
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.body.size_hint()
    }
}

pin_project! {
    pub(crate) struct StreamErrorIntoIoError<S, E> {
        #[pin]
        inner: S,
        error: Option<E>,
    }
}

impl<S, E> StreamErrorIntoIoError<S, E> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner, error: None }
    }

    /// Get a reference to the inner body
    pub(crate) fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Get a mutable reference to the inner inner
    pub(crate) fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Get a pinned mutable reference to the inner inner
    pub(crate) fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().inner
    }

    /// Consume `self`, returning the inner inner
    pub(crate) fn into_inner(self) -> S {
        self.inner
    }
}

impl<S, T, E> Stream for StreamErrorIntoIoError<S, E>
where
    S: Stream<Item = Result<T, E>>,
{
    type Item = Result<T, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match ready!(this.inner.poll_next(cx)) {
            None => Poll::Ready(None),
            Some(Ok(value)) => Poll::Ready(Some(Ok(value))),
            Some(Err(err)) => {
                *this.error = Some(err);
                Poll::Ready(Some(Err(io::Error::from_raw_os_error(SENTINEL_ERROR_CODE))))
            }
        }
    }
}

pub(crate) const SENTINEL_ERROR_CODE: i32 = -837459418;

/// Level of compression data should be compressed with.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum CompressionLevel {
    /// Fastest quality of compression, usually produces bigger size.
    Fastest,
    /// Best quality of compression, usually produces the smallest size.
    Best,
    /// Default quality of compression defined by the selected compression
    /// algorithm.
    #[default]
    Default,
    /// Precise quality based on the underlying compression algorithms'
    /// qualities.
    ///
    /// The interpretation of this depends on the algorithm chosen and the
    /// specific implementation backing it.
    ///
    /// Qualities are implicitly clamped to the algorithm's maximum.
    Precise(i32),
}

#[cfg(any(
    feature = "compression-br",
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-zstd"
))]
use async_compression::Level as AsyncCompressionLevel;

#[cfg(any(
    feature = "compression-br",
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-zstd"
))]
impl CompressionLevel {
    pub(crate) fn into_async_compression(self) -> AsyncCompressionLevel {
        match self {
            CompressionLevel::Fastest => AsyncCompressionLevel::Fastest,
            CompressionLevel::Best => AsyncCompressionLevel::Best,
            CompressionLevel::Default => AsyncCompressionLevel::Default,
            CompressionLevel::Precise(quality) => AsyncCompressionLevel::Precise(quality),
        }
    }
}
