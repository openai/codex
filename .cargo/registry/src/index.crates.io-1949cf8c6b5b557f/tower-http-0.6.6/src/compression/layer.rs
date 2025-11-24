use super::{Compression, Predicate};
use crate::compression::predicate::DefaultPredicate;
use crate::compression::CompressionLevel;
use crate::compression_utils::AcceptEncoding;
use tower_layer::Layer;

/// Compress response bodies of the underlying service.
///
/// This uses the `Accept-Encoding` header to pick an appropriate encoding and adds the
/// `Content-Encoding` header to responses.
///
/// See the [module docs](crate::compression) for more details.
#[derive(Clone, Debug, Default)]
pub struct CompressionLayer<P = DefaultPredicate> {
    accept: AcceptEncoding,
    predicate: P,
    quality: CompressionLevel,
}

impl<S, P> Layer<S> for CompressionLayer<P>
where
    P: Predicate,
{
    type Service = Compression<S, P>;

    fn layer(&self, inner: S) -> Self::Service {
        Compression {
            inner,
            accept: self.accept,
            predicate: self.predicate.clone(),
            quality: self.quality,
        }
    }
}

impl CompressionLayer {
    /// Creates a new [`CompressionLayer`].
    pub fn new() -> Self {
        Self::default()
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
    /// See [`Compression::compress_when`] for more details.
    pub fn compress_when<C>(self, predicate: C) -> CompressionLayer<C>
    where
        C: Predicate,
    {
        CompressionLayer {
            accept: self.accept,
            predicate,
            quality: self.quality,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::Body;
    use http::{header::ACCEPT_ENCODING, Request, Response};
    use http_body_util::BodyExt;
    use std::convert::Infallible;
    use tokio::fs::File;
    use tokio_util::io::ReaderStream;
    use tower::{Service, ServiceBuilder, ServiceExt};

    async fn handle(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        // Open the file.
        let file = File::open("Cargo.toml").await.expect("file missing");
        // Convert the file into a `Stream`.
        let stream = ReaderStream::new(file);
        // Convert the `Stream` into a `Body`.
        let body = Body::from_stream(stream);
        // Create response.
        Ok(Response::new(body))
    }

    #[tokio::test]
    async fn accept_encoding_configuration_works() -> Result<(), crate::BoxError> {
        let deflate_only_layer = CompressionLayer::new()
            .quality(CompressionLevel::Best)
            .no_br()
            .no_gzip();

        let mut service = ServiceBuilder::new()
            // Compress responses based on the `Accept-Encoding` header.
            .layer(deflate_only_layer)
            .service_fn(handle);

        // Call the service with the deflate only layer
        let request = Request::builder()
            .header(ACCEPT_ENCODING, "gzip, deflate, br")
            .body(Body::empty())?;

        let response = service.ready().await?.call(request).await?;

        assert_eq!(response.headers()["content-encoding"], "deflate");

        // Read the body
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();

        let deflate_bytes_len = bytes.len();

        let br_only_layer = CompressionLayer::new()
            .quality(CompressionLevel::Best)
            .no_gzip()
            .no_deflate();

        let mut service = ServiceBuilder::new()
            // Compress responses based on the `Accept-Encoding` header.
            .layer(br_only_layer)
            .service_fn(handle);

        // Call the service with the br only layer
        let request = Request::builder()
            .header(ACCEPT_ENCODING, "gzip, deflate, br")
            .body(Body::empty())?;

        let response = service.ready().await?.call(request).await?;

        assert_eq!(response.headers()["content-encoding"], "br");

        // Read the body
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();

        let br_byte_length = bytes.len();

        // check the corresponding algorithms are actually used
        // br should compresses better than deflate
        assert!(br_byte_length < deflate_bytes_len * 9 / 10);

        Ok(())
    }

    /// Test ensuring that zstd compression will not exceed an 8MiB window size; browsers do not
    /// accept responses using 16MiB+ window sizes.
    #[tokio::test]
    async fn zstd_is_web_safe() -> Result<(), crate::BoxError> {
        async fn zeroes(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
            Ok(Response::new(Body::from(vec![0u8; 18_874_368])))
        }
        // zstd will (I believe) lower its window size if a larger one isn't beneficial and
        // it knows the size of the input; use an 18MiB body to ensure it would want a
        // >=16MiB window (though it might not be able to see the input size here).

        let zstd_layer = CompressionLayer::new()
            .quality(CompressionLevel::Best)
            .no_br()
            .no_deflate()
            .no_gzip();

        let mut service = ServiceBuilder::new().layer(zstd_layer).service_fn(zeroes);

        let request = Request::builder()
            .header(ACCEPT_ENCODING, "zstd")
            .body(Body::empty())?;

        let response = service.ready().await?.call(request).await?;

        assert_eq!(response.headers()["content-encoding"], "zstd");

        let body = response.into_body();
        let bytes = body.collect().await?.to_bytes();
        let mut dec = zstd::Decoder::new(&*bytes)?;
        dec.window_log_max(23)?; // Limit window size accepted by decoder to 2 ^ 23 bytes (8MiB)

        std::io::copy(&mut dec, &mut std::io::sink())?;

        Ok(())
    }
}
