use super::service::RequestDecompression;
use crate::compression_utils::AcceptEncoding;
use tower_layer::Layer;

/// Decompresses request bodies and calls its underlying service.
///
/// Transparently decompresses request bodies based on the `Content-Encoding` header.
/// When the encoding in the `Content-Encoding` header is not accepted an `Unsupported Media Type`
/// status code will be returned with the accepted encodings in the `Accept-Encoding` header.
///
/// Enabling pass-through of unaccepted encodings will not return an `Unsupported Media Type`. But
/// will call the underlying service with the unmodified request if the encoding is not supported.
/// This is disabled by default.
///
/// See the [module docs](crate::decompression) for more details.
#[derive(Debug, Default, Clone)]
pub struct RequestDecompressionLayer {
    accept: AcceptEncoding,
    pass_through_unaccepted: bool,
}

impl<S> Layer<S> for RequestDecompressionLayer {
    type Service = RequestDecompression<S>;

    fn layer(&self, service: S) -> Self::Service {
        RequestDecompression {
            inner: service,
            accept: self.accept,
            pass_through_unaccepted: self.pass_through_unaccepted,
        }
    }
}

impl RequestDecompressionLayer {
    /// Creates a new `RequestDecompressionLayer`.
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets whether to support gzip encoding.
    #[cfg(feature = "decompression-gzip")]
    pub fn gzip(mut self, enable: bool) -> Self {
        self.accept.set_gzip(enable);
        self
    }

    /// Sets whether to support Deflate encoding.
    #[cfg(feature = "decompression-deflate")]
    pub fn deflate(mut self, enable: bool) -> Self {
        self.accept.set_deflate(enable);
        self
    }

    /// Sets whether to support Brotli encoding.
    #[cfg(feature = "decompression-br")]
    pub fn br(mut self, enable: bool) -> Self {
        self.accept.set_br(enable);
        self
    }

    /// Sets whether to support Zstd encoding.
    #[cfg(feature = "decompression-zstd")]
    pub fn zstd(mut self, enable: bool) -> Self {
        self.accept.set_zstd(enable);
        self
    }

    /// Disables support for gzip encoding.
    ///
    /// This method is available even if the `gzip` crate feature is disabled.
    pub fn no_gzip(mut self) -> Self {
        self.accept.set_gzip(false);
        self
    }

    /// Disables support for Deflate encoding.
    ///
    /// This method is available even if the `deflate` crate feature is disabled.
    pub fn no_deflate(mut self) -> Self {
        self.accept.set_deflate(false);
        self
    }

    /// Disables support for Brotli encoding.
    ///
    /// This method is available even if the `br` crate feature is disabled.
    pub fn no_br(mut self) -> Self {
        self.accept.set_br(false);
        self
    }

    /// Disables support for Zstd encoding.
    ///
    /// This method is available even if the `zstd` crate feature is disabled.
    pub fn no_zstd(mut self) -> Self {
        self.accept.set_zstd(false);
        self
    }

    /// Sets whether to pass through the request even when the encoding is not supported.
    pub fn pass_through_unaccepted(mut self, enable: bool) -> Self {
        self.pass_through_unaccepted = enable;
        self
    }
}
