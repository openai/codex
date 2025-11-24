//! Middleware that decompresses request and response bodies.
//!
//! # Examples
//!
//! #### Request
//!
//! ```rust
//! use bytes::Bytes;
//! use flate2::{write::GzEncoder, Compression};
//! use http::{header, HeaderValue, Request, Response};
//! use http_body_util::{Full, BodyExt};
//! use std::{error::Error, io::Write};
//! use tower::{Service, ServiceBuilder, service_fn, ServiceExt};
//! use tower_http::{BoxError, decompression::{DecompressionBody, RequestDecompressionLayer}};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), BoxError> {
//! // A request encoded with gzip coming from some HTTP client.
//! let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
//! encoder.write_all(b"Hello?")?;
//! let request = Request::builder()
//!     .header(header::CONTENT_ENCODING, "gzip")
//!     .body(Full::from(encoder.finish()?))?;
//!
//! // Our HTTP server
//! let mut server = ServiceBuilder::new()
//!     // Automatically decompress request bodies.
//!     .layer(RequestDecompressionLayer::new())
//!     .service(service_fn(handler));
//!
//! // Send the request, with the gzip encoded body, to our server.
//! let _response = server.ready().await?.call(request).await?;
//!
//! // Handler receives request whose body is decoded when read
//! async fn handler(
//!     mut req: Request<DecompressionBody<Full<Bytes>>>,
//! ) -> Result<Response<Full<Bytes>>, BoxError>{
//!     let data = req.into_body().collect().await?.to_bytes();
//!     assert_eq!(&data[..], b"Hello?");
//!     Ok(Response::new(Full::from("Hello, World!")))
//! }
//! # Ok(())
//! # }
//! ```
//!
//! #### Response
//!
//! ```rust
//! use bytes::Bytes;
//! use http::{Request, Response};
//! use http_body_util::{Full, BodyExt};
//! use std::convert::Infallible;
//! use tower::{Service, ServiceExt, ServiceBuilder, service_fn};
//! use tower_http::{compression::Compression, decompression::DecompressionLayer, BoxError};
//! #
//! # #[tokio::main]
//! # async fn main() -> Result<(), tower_http::BoxError> {
//! # async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//! #     let body = Full::from("Hello, World!");
//! #     Ok(Response::new(body))
//! # }
//!
//! // Some opaque service that applies compression.
//! let service = Compression::new(service_fn(handle));
//!
//! // Our HTTP client.
//! let mut client = ServiceBuilder::new()
//!     // Automatically decompress response bodies.
//!     .layer(DecompressionLayer::new())
//!     .service(service);
//!
//! // Call the service.
//! //
//! // `DecompressionLayer` takes care of setting `Accept-Encoding`.
//! let request = Request::new(Full::<Bytes>::default());
//!
//! let response = client
//!     .ready()
//!     .await?
//!     .call(request)
//!     .await?;
//!
//! // Read the body
//! let body = response.into_body();
//! let bytes = body.collect().await?.to_bytes().to_vec();
//! let body = String::from_utf8(bytes).map_err(Into::<BoxError>::into)?;
//!
//! assert_eq!(body, "Hello, World!");
//! #
//! # Ok(())
//! # }
//! ```

mod request;

mod body;
mod future;
mod layer;
mod service;

pub use self::{
    body::DecompressionBody, future::ResponseFuture, layer::DecompressionLayer,
    service::Decompression,
};

pub use self::request::future::RequestDecompressionFuture;
pub use self::request::layer::RequestDecompressionLayer;
pub use self::request::service::RequestDecompression;

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::io::Write;

    use super::*;
    use crate::test_helpers::Body;
    use crate::{compression::Compression, test_helpers::WithTrailers};
    use flate2::write::GzEncoder;
    use http::Response;
    use http::{HeaderMap, HeaderName, Request};
    use http_body_util::BodyExt;
    use tower::{service_fn, Service, ServiceExt};

    #[tokio::test]
    async fn works() {
        let mut client = Decompression::new(Compression::new(service_fn(handle)));

        let req = Request::builder()
            .header("accept-encoding", "gzip")
            .body(Body::empty())
            .unwrap();
        let res = client.ready().await.unwrap().call(req).await.unwrap();

        // read the body, it will be decompressed automatically
        let body = res.into_body();
        let collected = body.collect().await.unwrap();
        let trailers = collected.trailers().cloned().unwrap();
        let decompressed_data = String::from_utf8(collected.to_bytes().to_vec()).unwrap();

        assert_eq!(decompressed_data, "Hello, World!");

        // maintains trailers
        assert_eq!(trailers["foo"], "bar");
    }

    async fn handle(_req: Request<Body>) -> Result<Response<WithTrailers<Body>>, Infallible> {
        let mut trailers = HeaderMap::new();
        trailers.insert(HeaderName::from_static("foo"), "bar".parse().unwrap());
        let body = Body::from("Hello, World!").with_trailers(trailers);
        Ok(Response::builder().body(body).unwrap())
    }

    #[tokio::test]
    async fn decompress_multi_gz() {
        let mut client = Decompression::new(service_fn(handle_multi_gz));

        let req = Request::builder()
            .header("accept-encoding", "gzip")
            .body(Body::empty())
            .unwrap();
        let res = client.ready().await.unwrap().call(req).await.unwrap();

        // read the body, it will be decompressed automatically
        let body = res.into_body();
        let decompressed_data =
            String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap();

        assert_eq!(decompressed_data, "Hello, World!");
    }

    #[tokio::test]
    async fn decompress_multi_zstd() {
        let mut client = Decompression::new(service_fn(handle_multi_zstd));

        let req = Request::builder()
            .header("accept-encoding", "zstd")
            .body(Body::empty())
            .unwrap();
        let res = client.ready().await.unwrap().call(req).await.unwrap();

        // read the body, it will be decompressed automatically
        let body = res.into_body();
        let decompressed_data =
            String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap();

        assert_eq!(decompressed_data, "Hello, World!");
    }

    async fn handle_multi_gz(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut buf = Vec::new();
        let mut enc1 = GzEncoder::new(&mut buf, Default::default());
        enc1.write_all(b"Hello, ").unwrap();
        enc1.finish().unwrap();

        let mut enc2 = GzEncoder::new(&mut buf, Default::default());
        enc2.write_all(b"World!").unwrap();
        enc2.finish().unwrap();

        let mut res = Response::new(Body::from(buf));
        res.headers_mut()
            .insert("content-encoding", "gzip".parse().unwrap());
        Ok(res)
    }

    async fn handle_multi_zstd(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut buf = Vec::new();
        let mut enc1 = zstd::Encoder::new(&mut buf, Default::default()).unwrap();
        enc1.write_all(b"Hello, ").unwrap();
        enc1.finish().unwrap();

        let mut enc2 = zstd::Encoder::new(&mut buf, Default::default()).unwrap();
        enc2.write_all(b"World!").unwrap();
        enc2.finish().unwrap();

        let mut res = Response::new(Body::from(buf));
        res.headers_mut()
            .insert("content-encoding", "zstd".parse().unwrap());
        Ok(res)
    }

    #[allow(dead_code)]
    async fn is_compatible_with_hyper() {
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();
        let mut client = Decompression::new(client);

        let req = Request::new(Body::empty());

        let _: Response<DecompressionBody<_>> =
            client.ready().await.unwrap().call(req).await.unwrap();
    }
}
