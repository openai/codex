#![allow(unused_imports)]

use crate::compression::CompressionLevel;
use crate::{
    compression_utils::{AsyncReadBody, BodyIntoStream, DecorateAsyncRead, WrapBody},
    BoxError,
};
#[cfg(feature = "compression-br")]
use async_compression::tokio::bufread::BrotliEncoder;
#[cfg(feature = "compression-gzip")]
use async_compression::tokio::bufread::GzipEncoder;
#[cfg(feature = "compression-deflate")]
use async_compression::tokio::bufread::ZlibEncoder;
#[cfg(feature = "compression-zstd")]
use async_compression::tokio::bufread::ZstdEncoder;

use bytes::{Buf, Bytes};
use http::HeaderMap;
use http_body::Body;
use pin_project_lite::pin_project;
use std::{
    io,
    marker::PhantomData,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio_util::io::StreamReader;

use super::pin_project_cfg::pin_project_cfg;

pin_project! {
    /// Response body of [`Compression`].
    ///
    /// [`Compression`]: super::Compression
    pub struct CompressionBody<B>
    where
        B: Body,
    {
        #[pin]
        pub(crate) inner: BodyInner<B>,
    }
}

impl<B> Default for CompressionBody<B>
where
    B: Body + Default,
{
    fn default() -> Self {
        Self {
            inner: BodyInner::Identity {
                inner: B::default(),
            },
        }
    }
}

impl<B> CompressionBody<B>
where
    B: Body,
{
    pub(crate) fn new(inner: BodyInner<B>) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner body
    pub fn get_ref(&self) -> &B {
        match &self.inner {
            #[cfg(feature = "compression-gzip")]
            BodyInner::Gzip { inner } => inner.read.get_ref().get_ref().get_ref().get_ref(),
            #[cfg(feature = "compression-deflate")]
            BodyInner::Deflate { inner } => inner.read.get_ref().get_ref().get_ref().get_ref(),
            #[cfg(feature = "compression-br")]
            BodyInner::Brotli { inner } => inner.read.get_ref().get_ref().get_ref().get_ref(),
            #[cfg(feature = "compression-zstd")]
            BodyInner::Zstd { inner } => inner.read.get_ref().get_ref().get_ref().get_ref(),
            BodyInner::Identity { inner } => inner,
        }
    }

    /// Get a mutable reference to the inner body
    pub fn get_mut(&mut self) -> &mut B {
        match &mut self.inner {
            #[cfg(feature = "compression-gzip")]
            BodyInner::Gzip { inner } => inner.read.get_mut().get_mut().get_mut().get_mut(),
            #[cfg(feature = "compression-deflate")]
            BodyInner::Deflate { inner } => inner.read.get_mut().get_mut().get_mut().get_mut(),
            #[cfg(feature = "compression-br")]
            BodyInner::Brotli { inner } => inner.read.get_mut().get_mut().get_mut().get_mut(),
            #[cfg(feature = "compression-zstd")]
            BodyInner::Zstd { inner } => inner.read.get_mut().get_mut().get_mut().get_mut(),
            BodyInner::Identity { inner } => inner,
        }
    }

    /// Get a pinned mutable reference to the inner body
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut B> {
        match self.project().inner.project() {
            #[cfg(feature = "compression-gzip")]
            BodyInnerProj::Gzip { inner } => inner
                .project()
                .read
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut(),
            #[cfg(feature = "compression-deflate")]
            BodyInnerProj::Deflate { inner } => inner
                .project()
                .read
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut(),
            #[cfg(feature = "compression-br")]
            BodyInnerProj::Brotli { inner } => inner
                .project()
                .read
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut(),
            #[cfg(feature = "compression-zstd")]
            BodyInnerProj::Zstd { inner } => inner
                .project()
                .read
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut()
                .get_pin_mut(),
            BodyInnerProj::Identity { inner } => inner,
        }
    }

    /// Consume `self`, returning the inner body
    pub fn into_inner(self) -> B {
        match self.inner {
            #[cfg(feature = "compression-gzip")]
            BodyInner::Gzip { inner } => inner
                .read
                .into_inner()
                .into_inner()
                .into_inner()
                .into_inner(),
            #[cfg(feature = "compression-deflate")]
            BodyInner::Deflate { inner } => inner
                .read
                .into_inner()
                .into_inner()
                .into_inner()
                .into_inner(),
            #[cfg(feature = "compression-br")]
            BodyInner::Brotli { inner } => inner
                .read
                .into_inner()
                .into_inner()
                .into_inner()
                .into_inner(),
            #[cfg(feature = "compression-zstd")]
            BodyInner::Zstd { inner } => inner
                .read
                .into_inner()
                .into_inner()
                .into_inner()
                .into_inner(),
            BodyInner::Identity { inner } => inner,
        }
    }
}

#[cfg(feature = "compression-gzip")]
type GzipBody<B> = WrapBody<GzipEncoder<B>>;

#[cfg(feature = "compression-deflate")]
type DeflateBody<B> = WrapBody<ZlibEncoder<B>>;

#[cfg(feature = "compression-br")]
type BrotliBody<B> = WrapBody<BrotliEncoder<B>>;

#[cfg(feature = "compression-zstd")]
type ZstdBody<B> = WrapBody<ZstdEncoder<B>>;

pin_project_cfg! {
    #[project = BodyInnerProj]
    pub(crate) enum BodyInner<B>
    where
        B: Body,
    {
        #[cfg(feature = "compression-gzip")]
        Gzip {
            #[pin]
            inner: GzipBody<B>,
        },
        #[cfg(feature = "compression-deflate")]
        Deflate {
            #[pin]
            inner: DeflateBody<B>,
        },
        #[cfg(feature = "compression-br")]
        Brotli {
            #[pin]
            inner: BrotliBody<B>,
        },
        #[cfg(feature = "compression-zstd")]
        Zstd {
            #[pin]
            inner: ZstdBody<B>,
        },
        Identity {
            #[pin]
            inner: B,
        },
    }
}

impl<B: Body> BodyInner<B> {
    #[cfg(feature = "compression-gzip")]
    pub(crate) fn gzip(inner: WrapBody<GzipEncoder<B>>) -> Self {
        Self::Gzip { inner }
    }

    #[cfg(feature = "compression-deflate")]
    pub(crate) fn deflate(inner: WrapBody<ZlibEncoder<B>>) -> Self {
        Self::Deflate { inner }
    }

    #[cfg(feature = "compression-br")]
    pub(crate) fn brotli(inner: WrapBody<BrotliEncoder<B>>) -> Self {
        Self::Brotli { inner }
    }

    #[cfg(feature = "compression-zstd")]
    pub(crate) fn zstd(inner: WrapBody<ZstdEncoder<B>>) -> Self {
        Self::Zstd { inner }
    }

    pub(crate) fn identity(inner: B) -> Self {
        Self::Identity { inner }
    }
}

impl<B> Body for CompressionBody<B>
where
    B: Body,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match self.project().inner.project() {
            #[cfg(feature = "compression-gzip")]
            BodyInnerProj::Gzip { inner } => inner.poll_frame(cx),
            #[cfg(feature = "compression-deflate")]
            BodyInnerProj::Deflate { inner } => inner.poll_frame(cx),
            #[cfg(feature = "compression-br")]
            BodyInnerProj::Brotli { inner } => inner.poll_frame(cx),
            #[cfg(feature = "compression-zstd")]
            BodyInnerProj::Zstd { inner } => inner.poll_frame(cx),
            BodyInnerProj::Identity { inner } => match ready!(inner.poll_frame(cx)) {
                Some(Ok(frame)) => {
                    let frame = frame.map_data(|mut buf| buf.copy_to_bytes(buf.remaining()));
                    Poll::Ready(Some(Ok(frame)))
                }
                Some(Err(err)) => Poll::Ready(Some(Err(err.into()))),
                None => Poll::Ready(None),
            },
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        if let BodyInner::Identity { inner } = &self.inner {
            inner.size_hint()
        } else {
            http_body::SizeHint::new()
        }
    }

    fn is_end_stream(&self) -> bool {
        if let BodyInner::Identity { inner } = &self.inner {
            inner.is_end_stream()
        } else {
            false
        }
    }
}

#[cfg(feature = "compression-gzip")]
impl<B> DecorateAsyncRead for GzipEncoder<B>
where
    B: Body,
{
    type Input = AsyncReadBody<B>;
    type Output = GzipEncoder<Self::Input>;

    fn apply(input: Self::Input, quality: CompressionLevel) -> Self::Output {
        GzipEncoder::with_quality(input, quality.into_async_compression())
    }

    fn get_pin_mut(pinned: Pin<&mut Self::Output>) -> Pin<&mut Self::Input> {
        pinned.get_pin_mut()
    }
}

#[cfg(feature = "compression-deflate")]
impl<B> DecorateAsyncRead for ZlibEncoder<B>
where
    B: Body,
{
    type Input = AsyncReadBody<B>;
    type Output = ZlibEncoder<Self::Input>;

    fn apply(input: Self::Input, quality: CompressionLevel) -> Self::Output {
        ZlibEncoder::with_quality(input, quality.into_async_compression())
    }

    fn get_pin_mut(pinned: Pin<&mut Self::Output>) -> Pin<&mut Self::Input> {
        pinned.get_pin_mut()
    }
}

#[cfg(feature = "compression-br")]
impl<B> DecorateAsyncRead for BrotliEncoder<B>
where
    B: Body,
{
    type Input = AsyncReadBody<B>;
    type Output = BrotliEncoder<Self::Input>;

    fn apply(input: Self::Input, quality: CompressionLevel) -> Self::Output {
        // The brotli crate used under the hood here has a default compression level of 11,
        // which is the max for brotli. This causes extremely slow compression times, so we
        // manually set a default of 4 here.
        //
        // This is the same default used by NGINX for on-the-fly brotli compression.
        let level = match quality {
            CompressionLevel::Default => async_compression::Level::Precise(4),
            other => other.into_async_compression(),
        };
        BrotliEncoder::with_quality(input, level)
    }

    fn get_pin_mut(pinned: Pin<&mut Self::Output>) -> Pin<&mut Self::Input> {
        pinned.get_pin_mut()
    }
}

#[cfg(feature = "compression-zstd")]
impl<B> DecorateAsyncRead for ZstdEncoder<B>
where
    B: Body,
{
    type Input = AsyncReadBody<B>;
    type Output = ZstdEncoder<Self::Input>;

    fn apply(input: Self::Input, quality: CompressionLevel) -> Self::Output {
        // See https://issues.chromium.org/issues/41493659:
        //  "For memory usage reasons, Chromium limits the window size to 8MB"
        // See https://datatracker.ietf.org/doc/html/rfc8878#name-window-descriptor
        //  "For improved interoperability, it's recommended for decoders to support values
        //  of Window_Size up to 8 MB and for encoders not to generate frames requiring a
        //  Window_Size larger than 8 MB."
        // Level 17 in zstd (as of v1.5.6) is the first level with a window size of 8 MB (2^23):
        // https://github.com/facebook/zstd/blob/v1.5.6/lib/compress/clevels.h#L25-L51
        // Set the parameter for all levels >= 17. This will either have no effect (but reduce
        // the risk of future changes in zstd) or limit the window log to 8MB.
        let needs_window_limit = match quality {
            CompressionLevel::Best => true, // level 20
            CompressionLevel::Precise(level) => level >= 17,
            _ => false,
        };
        // The parameter is not set for levels below 17 as it will increase the window size
        // for those levels.
        if needs_window_limit {
            let params = [async_compression::zstd::CParameter::window_log(23)];
            ZstdEncoder::with_quality_and_params(input, quality.into_async_compression(), &params)
        } else {
            ZstdEncoder::with_quality(input, quality.into_async_compression())
        }
    }

    fn get_pin_mut(pinned: Pin<&mut Self::Output>) -> Pin<&mut Self::Input> {
        pinned.get_pin_mut()
    }
}
