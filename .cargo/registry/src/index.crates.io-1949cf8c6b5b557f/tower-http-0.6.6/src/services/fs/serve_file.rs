//! Service that serves a file.

use super::ServeDir;
use http::{HeaderValue, Request};
use mime::Mime;
use std::{
    path::Path,
    task::{Context, Poll},
};
use tower_service::Service;

/// Service that serves a file.
#[derive(Clone, Debug)]
pub struct ServeFile(ServeDir);

// Note that this is just a special case of ServeDir
impl ServeFile {
    /// Create a new [`ServeFile`].
    ///
    /// The `Content-Type` will be guessed from the file extension.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let guess = mime_guess::from_path(path.as_ref());
        let mime = guess
            .first_raw()
            .map(HeaderValue::from_static)
            .unwrap_or_else(|| {
                HeaderValue::from_str(mime::APPLICATION_OCTET_STREAM.as_ref()).unwrap()
            });

        Self(ServeDir::new_single_file(path, mime))
    }

    /// Create a new [`ServeFile`] with a specific mime type.
    ///
    /// # Panics
    ///
    /// Will panic if the mime type isn't a valid [header value].
    ///
    /// [header value]: https://docs.rs/http/latest/http/header/struct.HeaderValue.html
    pub fn new_with_mime<P: AsRef<Path>>(path: P, mime: &Mime) -> Self {
        let mime = HeaderValue::from_str(mime.as_ref()).expect("mime isn't a valid header value");
        Self(ServeDir::new_single_file(path, mime))
    }

    /// Informs the service that it should also look for a precompressed gzip
    /// version of the file.
    ///
    /// If the client has an `Accept-Encoding` header that allows the gzip encoding,
    /// the file `foo.txt.gz` will be served instead of `foo.txt`.
    /// If the precompressed file is not available, or the client doesn't support it,
    /// the uncompressed version will be served instead.
    /// Both the precompressed version and the uncompressed version are expected
    /// to be present in the same directory. Different precompressed
    /// variants can be combined.
    pub fn precompressed_gzip(self) -> Self {
        Self(self.0.precompressed_gzip())
    }

    /// Informs the service that it should also look for a precompressed brotli
    /// version of the file.
    ///
    /// If the client has an `Accept-Encoding` header that allows the brotli encoding,
    /// the file `foo.txt.br` will be served instead of `foo.txt`.
    /// If the precompressed file is not available, or the client doesn't support it,
    /// the uncompressed version will be served instead.
    /// Both the precompressed version and the uncompressed version are expected
    /// to be present in the same directory. Different precompressed
    /// variants can be combined.
    pub fn precompressed_br(self) -> Self {
        Self(self.0.precompressed_br())
    }

    /// Informs the service that it should also look for a precompressed deflate
    /// version of the file.
    ///
    /// If the client has an `Accept-Encoding` header that allows the deflate encoding,
    /// the file `foo.txt.zz` will be served instead of `foo.txt`.
    /// If the precompressed file is not available, or the client doesn't support it,
    /// the uncompressed version will be served instead.
    /// Both the precompressed version and the uncompressed version are expected
    /// to be present in the same directory. Different precompressed
    /// variants can be combined.
    pub fn precompressed_deflate(self) -> Self {
        Self(self.0.precompressed_deflate())
    }

    /// Informs the service that it should also look for a precompressed zstd
    /// version of the file.
    ///
    /// If the client has an `Accept-Encoding` header that allows the zstd encoding,
    /// the file `foo.txt.zst` will be served instead of `foo.txt`.
    /// If the precompressed file is not available, or the client doesn't support it,
    /// the uncompressed version will be served instead.
    /// Both the precompressed version and the uncompressed version are expected
    /// to be present in the same directory. Different precompressed
    /// variants can be combined.
    pub fn precompressed_zstd(self) -> Self {
        Self(self.0.precompressed_zstd())
    }

    /// Set a specific read buffer chunk size.
    ///
    /// The default capacity is 64kb.
    pub fn with_buf_chunk_size(self, chunk_size: usize) -> Self {
        Self(self.0.with_buf_chunk_size(chunk_size))
    }

    /// Call the service and get a future that contains any `std::io::Error` that might have
    /// happened.
    ///
    /// See [`ServeDir::try_call`] for more details.
    pub fn try_call<ReqBody>(
        &mut self,
        req: Request<ReqBody>,
    ) -> super::serve_dir::future::ResponseFuture<ReqBody>
    where
        ReqBody: Send + 'static,
    {
        self.0.try_call(req)
    }
}

impl<ReqBody> Service<Request<ReqBody>> for ServeFile
where
    ReqBody: Send + 'static,
{
    type Error = <ServeDir as Service<Request<ReqBody>>>::Error;
    type Response = <ServeDir as Service<Request<ReqBody>>>::Response;
    type Future = <ServeDir as Service<Request<ReqBody>>>::Future;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        self.0.call(req)
    }
}

#[cfg(test)]
mod tests {
    use crate::services::ServeFile;
    use crate::test_helpers::Body;
    use async_compression::tokio::bufread::ZstdDecoder;
    use brotli::BrotliDecompress;
    use flate2::bufread::DeflateDecoder;
    use flate2::bufread::GzDecoder;
    use http::header;
    use http::Method;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use mime::Mime;
    use std::io::Read;
    use std::str::FromStr;
    use tokio::io::AsyncReadExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn basic() {
        let svc = ServeFile::new("../README.md");

        let res = svc.oneshot(Request::new(Body::empty())).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/markdown");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.starts_with("# Tower HTTP"));
    }

    #[tokio::test]
    async fn basic_with_mime() {
        let svc = ServeFile::new_with_mime("../README.md", &Mime::from_str("image/jpg").unwrap());

        let res = svc.oneshot(Request::new(Body::empty())).await.unwrap();

        assert_eq!(res.headers()["content-type"], "image/jpg");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.starts_with("# Tower HTTP"));
    }

    #[tokio::test]
    async fn head_request() {
        let svc = ServeFile::new("../test-files/precompressed.txt");

        let mut request = Request::new(Body::empty());
        *request.method_mut() = Method::HEAD;
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-length"], "23");

        assert!(res.into_body().frame().await.is_none());
    }

    #[tokio::test]
    async fn precompresed_head_request() {
        let svc = ServeFile::new("../test-files/precompressed.txt").precompressed_gzip();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip")
            .method(Method::HEAD)
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "gzip");
        assert_eq!(res.headers()["content-length"], "59");

        assert!(res.into_body().frame().await.is_none());
    }

    #[tokio::test]
    async fn precompressed_gzip() {
        let svc = ServeFile::new("../test-files/precompressed.txt").precompressed_gzip();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "gzip");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decoder = GzDecoder::new(&body[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert!(decompressed.starts_with("\"This is a test file!\""));
    }

    #[tokio::test]
    async fn unsupported_precompression_alogrithm_fallbacks_to_uncompressed() {
        let svc = ServeFile::new("../test-files/precompressed.txt").precompressed_gzip();

        let request = Request::builder()
            .header("Accept-Encoding", "br")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert!(res.headers().get("content-encoding").is_none());

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.starts_with("\"This is a test file!\""));
    }

    #[tokio::test]
    async fn missing_precompressed_variant_fallbacks_to_uncompressed() {
        let svc = ServeFile::new("../test-files/missing_precompressed.txt").precompressed_gzip();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        // Uncompressed file is served because compressed version is missing
        assert!(res.headers().get("content-encoding").is_none());

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.starts_with("Test file!"));
    }

    #[tokio::test]
    async fn missing_precompressed_variant_fallbacks_to_uncompressed_head_request() {
        let svc = ServeFile::new("../test-files/missing_precompressed.txt").precompressed_gzip();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip")
            .method(Method::HEAD)
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-length"], "11");
        // Uncompressed file is served because compressed version is missing
        assert!(res.headers().get("content-encoding").is_none());

        assert!(res.into_body().frame().await.is_none());
    }

    #[tokio::test]
    async fn only_precompressed_variant_existing() {
        let svc = ServeFile::new("../test-files/only_gzipped.txt").precompressed_gzip();

        let request = Request::builder().body(Body::empty()).unwrap();
        let res = svc.clone().oneshot(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        // Should reply with gzipped file if client supports it
        let request = Request::builder()
            .header("Accept-Encoding", "gzip")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "gzip");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decoder = GzDecoder::new(&body[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert!(decompressed.starts_with("\"This is a test file\""));
    }

    #[tokio::test]
    async fn precompressed_br() {
        let svc = ServeFile::new("../test-files/precompressed.txt").precompressed_br();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip,br")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "br");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decompressed = Vec::new();
        BrotliDecompress(&mut &body[..], &mut decompressed).unwrap();
        let decompressed = String::from_utf8(decompressed.to_vec()).unwrap();
        assert!(decompressed.starts_with("\"This is a test file!\""));
    }

    #[tokio::test]
    async fn precompressed_deflate() {
        let svc = ServeFile::new("../test-files/precompressed.txt").precompressed_deflate();
        let request = Request::builder()
            .header("Accept-Encoding", "deflate,br")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "deflate");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decoder = DeflateDecoder::new(&body[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert!(decompressed.starts_with("\"This is a test file!\""));
    }

    #[tokio::test]
    async fn precompressed_zstd() {
        let svc = ServeFile::new("../test-files/precompressed.txt").precompressed_zstd();
        let request = Request::builder()
            .header("Accept-Encoding", "zstd,br")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "zstd");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decoder = ZstdDecoder::new(&body[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).await.unwrap();
        assert!(decompressed.starts_with("\"This is a test file!\""));
    }

    #[tokio::test]
    async fn multi_precompressed() {
        let svc = ServeFile::new("../test-files/precompressed.txt")
            .precompressed_gzip()
            .precompressed_br();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip")
            .body(Body::empty())
            .unwrap();
        let res = svc.clone().oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "gzip");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decoder = GzDecoder::new(&body[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert!(decompressed.starts_with("\"This is a test file!\""));

        let request = Request::builder()
            .header("Accept-Encoding", "br")
            .body(Body::empty())
            .unwrap();
        let res = svc.clone().oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "br");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decompressed = Vec::new();
        BrotliDecompress(&mut &body[..], &mut decompressed).unwrap();
        let decompressed = String::from_utf8(decompressed.to_vec()).unwrap();
        assert!(decompressed.starts_with("\"This is a test file!\""));
    }

    #[tokio::test]
    async fn with_custom_chunk_size() {
        let svc = ServeFile::new("../README.md").with_buf_chunk_size(1024 * 32);

        let res = svc.oneshot(Request::new(Body::empty())).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/markdown");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.starts_with("# Tower HTTP"));
    }

    #[tokio::test]
    async fn fallbacks_to_different_precompressed_variant_if_not_found() {
        let svc = ServeFile::new("../test-files/precompressed_br.txt")
            .precompressed_gzip()
            .precompressed_deflate()
            .precompressed_br();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip,deflate,br")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-encoding"], "br");

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut decompressed = Vec::new();
        BrotliDecompress(&mut &body[..], &mut decompressed).unwrap();
        let decompressed = String::from_utf8(decompressed.to_vec()).unwrap();
        assert!(decompressed.starts_with("Test file"));
    }

    #[tokio::test]
    async fn fallbacks_to_different_precompressed_variant_if_not_found_head_request() {
        let svc = ServeFile::new("../test-files/precompressed_br.txt")
            .precompressed_gzip()
            .precompressed_deflate()
            .precompressed_br();

        let request = Request::builder()
            .header("Accept-Encoding", "gzip,deflate,br")
            .method(Method::HEAD)
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.headers()["content-length"], "15");
        assert_eq!(res.headers()["content-encoding"], "br");

        assert!(res.into_body().frame().await.is_none());
    }

    #[tokio::test]
    async fn returns_404_if_file_doesnt_exist() {
        let svc = ServeFile::new("../this-doesnt-exist.md");

        let res = svc.oneshot(Request::new(Body::empty())).await.unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert!(res.headers().get(header::CONTENT_TYPE).is_none());
    }

    #[tokio::test]
    async fn returns_404_if_file_doesnt_exist_when_precompression_is_used() {
        let svc = ServeFile::new("../this-doesnt-exist.md").precompressed_deflate();

        let request = Request::builder()
            .header("Accept-Encoding", "deflate")
            .body(Body::empty())
            .unwrap();
        let res = svc.oneshot(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert!(res.headers().get(header::CONTENT_TYPE).is_none());
    }

    #[tokio::test]
    async fn last_modified() {
        let svc = ServeFile::new("../README.md");

        let req = Request::builder().body(Body::empty()).unwrap();
        let res = svc.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);

        let last_modified = res
            .headers()
            .get(header::LAST_MODIFIED)
            .expect("Missing last modified header!");

        // -- If-Modified-Since

        let svc = ServeFile::new("../README.md");
        let req = Request::builder()
            .header(header::IF_MODIFIED_SINCE, last_modified)
            .body(Body::empty())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
        assert!(res.into_body().frame().await.is_none());

        let svc = ServeFile::new("../README.md");
        let req = Request::builder()
            .header(header::IF_MODIFIED_SINCE, "Fri, 09 Aug 1996 14:21:40 GMT")
            .body(Body::empty())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let readme_bytes = include_bytes!("../../../../README.md");
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), readme_bytes);

        // -- If-Unmodified-Since

        let svc = ServeFile::new("../README.md");
        let req = Request::builder()
            .header(header::IF_UNMODIFIED_SINCE, last_modified)
            .body(Body::empty())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), readme_bytes);

        let svc = ServeFile::new("../README.md");
        let req = Request::builder()
            .header(header::IF_UNMODIFIED_SINCE, "Fri, 09 Aug 1996 14:21:40 GMT")
            .body(Body::empty())
            .unwrap();

        let res = svc.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::PRECONDITION_FAILED);
        assert!(res.into_body().frame().await.is_none());
    }
}
