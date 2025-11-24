use super::{CompressionBody, CompressionLayer, ResponseFuture};
use crate::compression::predicate::{DefaultPredicate, Predicate};
use crate::compression::CompressionLevel;
use crate::{compression_utils::AcceptEncoding, content_encoding::Encoding};
use http::{Request, Response};
use http_body::Body;
use std::task::{Context, Poll};
use tower_service::Service;

/// Compress response bodies of the underlying service.
///
/// This uses the `Accept-Encoding` header to pick an appropriate encoding and adds the
/// `Content-Encoding` header to responses.
///
/// See the [module docs](crate::compression) for more details.
#[derive(Clone, Copy)]
pub struct Compression<S, P = DefaultPredicate> {
    pub(crate) inner: S,
    pub(crate) accept: AcceptEncoding,
    pub(crate) predicate: P,
    pub(crate) quality: CompressionLevel,
}

impl<S> Compression<S, DefaultPredicate> {
    /// Creates a new `Compression` wrapping the `service`.
    pub fn new(service: S) -> Compression<S, DefaultPredicate> {
        Self {
            inner: service,
            accept: AcceptEncoding::default(),
            predicate: DefaultPredicate::default(),
            quality: CompressionLevel::default(),
        }
    }
}

impl<S, P> Compression<S, P> {
    define_inner_service_accessors!();

    /// Returns a new [`Layer`] that wraps services with a `Compression` middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer() -> CompressionLayer {
        CompressionLayer::new()
    }

    /// Sets whether to enable the gzip encoding.
    #[cfg(feature = "compression-gzip")]
    pub fn gzip(mut self, enable: bool) -> Self {
        self.accept.set_gzip(enable);
        self
    }

    /// Sets whether to enable the Deflate encoding.
    #[cfg(feature = "compression-deflate")]
    pub fn deflate(mut self, enable: bool) -> Self {
        self.accept.set_deflate(enable);
        self
    }

    /// Sets whether to enable the Brotli encoding.
    #[cfg(feature = "compression-br")]
    pub fn br(mut self, enable: bool) -> Self {
        self.accept.set_br(enable);
        self
    }

    /// Sets whether to enable the Zstd encoding.
    #[cfg(feature = "compression-zstd")]
    pub fn zstd(mut self, enable: bool) -> Self {
        self.accept.set_zstd(enable);
        self
    }

    /// Sets the compression quality.
    pub fn quality(mut self, quality: CompressionLevel) -> Self {
        self.quality = quality;
        self
    }

    /// Disables the gzip encoding.
    ///
    /// This method is available even if the `gzip` crate feature is disabled.
    pub fn no_gzip(mut self) -> Self {
        self.accept.set_gzip(false);
        self
    }

    /// Disables the Deflate encoding.
    ///
    /// This method is available even if the `deflate` crate feature is disabled.
    pub fn no_deflate(mut self) -> Self {
        self.accept.set_deflate(false);
        self
    }

    /// Disables the Brotli encoding.
    ///
    /// This method is available even if the `br` crate feature is disabled.
    pub fn no_br(mut self) -> Self {
        self.accept.set_br(false);
        self
    }

    /// Disables the Zstd encoding.
    ///
    /// This method is available even if the `zstd` crate feature is disabled.
    pub fn no_zstd(mut self) -> Self {
        self.accept.set_zstd(false);
        self
    }

    /// Replace the current compression predicate.
    ///
    /// Predicates are used to determine whether a response should be compressed or not.
    ///
    /// The default predicate is [`DefaultPredicate`]. See its documentation for more
    /// details on which responses it wont compress.
    ///
    /// # Changing the compression predicate
    ///
    /// ```
    /// use tower_http::compression::{
    ///     Compression,
    ///     predicate::{Predicate, NotForContentType, DefaultPredicate},
    /// };
    /// use tower::util::service_fn;
    ///
    /// // Placeholder service_fn
    /// let service = service_fn(|_: ()| async {
    ///     Ok::<_, std::io::Error>(http::Response::new(()))
    /// });
    ///
    /// // build our custom compression predicate
    /// // its recommended to still include `DefaultPredicate` as part of
    /// // custom predicates
    /// let predicate = DefaultPredicate::new()
    ///     // don't compress responses who's `content-type` starts with `application/json`
    ///     .and(NotForContentType::new("application/json"));
    ///
    /// let service = Compression::new(service).compress_when(predicate);
    /// ```
    ///
    /// See [`predicate`](super::predicate) for more utilities for building compression predicates.
    ///
    /// Responses that are already compressed (ie have a `content-encoding` header) will _never_ be
    /// recompressed, regardless what they predicate says.
    pub fn compress_when<C>(self, predicate: C) -> Compression<S, C>
    where
        C: Predicate,
    {
        Compression {
            inner: self.inner,
            accept: self.accept,
            predicate,
            quality: self.quality,
        }
    }
}

impl<ReqBody, ResBody, S, P> Service<Request<ReqBody>> for Compression<S, P>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: Body,
    P: Predicate,
{
    type Response = Response<CompressionBody<ResBody>>;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future, P>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let encoding = Encoding::from_headers(req.headers(), self.accept);

        ResponseFuture {
            inner: self.inner.call(req),
            encoding,
            predicate: self.predicate.clone(),
            quality: self.quality,
        }
    }
}
