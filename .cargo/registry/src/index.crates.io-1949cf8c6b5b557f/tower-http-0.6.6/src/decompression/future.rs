#![allow(unused_imports)]

use super::{body::BodyInner, DecompressionBody};
use crate::compression_utils::{AcceptEncoding, CompressionLevel, WrapBody};
use crate::content_encoding::SupportedEncodings;
use http::{header, Response};
use http_body::Body;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

pin_project! {
    /// Response future of [`Decompression`].
    ///
    /// [`Decompression`]: super::Decompression
    #[derive(Debug)]
    pub struct ResponseFuture<F> {
        #[pin]
        pub(crate) inner: F,
        pub(crate) accept: AcceptEncoding,
    }
}

impl<F, B, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body,
{
    type Output = Result<Response<DecompressionBody<B>>, E>;

    #[allow(unreachable_code, unused_mut, unused_variables)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.as_mut().project().inner.poll(cx)?);
        let (mut parts, body) = res.into_parts();

        let res =
            if let header::Entry::Occupied(entry) = parts.headers.entry(header::CONTENT_ENCODING) {
                let body = match entry.get().as_bytes() {
                    #[cfg(feature = "decompression-gzip")]
                    b"gzip" if self.accept.gzip() => DecompressionBody::new(BodyInner::gzip(
                        WrapBody::new(body, CompressionLevel::default()),
                    )),

                    #[cfg(feature = "decompression-deflate")]
                    b"deflate" if self.accept.deflate() => DecompressionBody::new(
                        BodyInner::deflate(WrapBody::new(body, CompressionLevel::default())),
                    ),

                    #[cfg(feature = "decompression-br")]
                    b"br" if self.accept.br() => DecompressionBody::new(BodyInner::brotli(
                        WrapBody::new(body, CompressionLevel::default()),
                    )),

                    #[cfg(feature = "decompression-zstd")]
                    b"zstd" if self.accept.zstd() => DecompressionBody::new(BodyInner::zstd(
                        WrapBody::new(body, CompressionLevel::default()),
                    )),

                    _ => {
                        return Poll::Ready(Ok(Response::from_parts(
                            parts,
                            DecompressionBody::new(BodyInner::identity(body)),
                        )))
                    }
                };

                entry.remove();
                parts.headers.remove(header::CONTENT_LENGTH);

                Response::from_parts(parts, body)
            } else {
                Response::from_parts(parts, DecompressionBody::new(BodyInner::identity(body)))
            };

        Poll::Ready(Ok(res))
    }
}
