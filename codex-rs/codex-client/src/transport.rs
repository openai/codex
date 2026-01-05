use crate::default_client::CodexHttpClient;
use crate::default_client::CodexRequestBuilder;
use crate::error::TransportError;
use crate::request::Request;
use crate::request::Response;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use futures::stream::BoxStream;
use http::HeaderMap;
use http::Method;
use http::StatusCode;
use tracing::Level;
use tracing::enabled;
use tracing::trace;

pub type ByteStream = BoxStream<'static, Result<Bytes, TransportError>>;

pub struct StreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub bytes: ByteStream,
}

#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn execute(&self, req: Request) -> Result<Response, TransportError>;
    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError>;
}

#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: CodexHttpClient,
    enable_request_compression: bool,
}

impl ReqwestTransport {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client: CodexHttpClient::new(client),
            enable_request_compression: false,
        }
    }

    pub fn with_request_compression(mut self, enabled: bool) -> Self {
        self.enable_request_compression = enabled;
        self
    }

    fn should_compress_request(&self, method: &Method, url: &str, headers: &HeaderMap) -> bool {
        if !self.enable_request_compression {
            return false;
        }

        if *method != Method::POST {
            return false;
        }

        if headers.contains_key(http::header::CONTENT_ENCODING) {
            return false;
        }

        let Ok(parsed) = reqwest::Url::parse(url) else {
            return false;
        };

        let path = parsed.path().to_ascii_lowercase();
        path.contains("/backend-api/codex") || path.contains("/api/codex")
    }

    fn build(&self, req: Request) -> Result<CodexRequestBuilder, TransportError> {
        let Request {
            method,
            url,
            mut headers,
            body,
            timeout,
        } = req;

        let mut builder = self.client.request(
            Method::from_bytes(method.as_str().as_bytes()).unwrap_or(Method::GET),
            &url,
        );

        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(body) = body {
            if self.should_compress_request(&method, &url, &headers) {
                let json = serde_json::to_vec(&body)
                    .map_err(|err| TransportError::Build(err.to_string()))?;
                let pre_compression_bytes = json.len();
                let compression_start = std::time::Instant::now();
                let compressed = zstd::stream::encode_all(std::io::Cursor::new(json), 3)
                    .map_err(|err| TransportError::Build(err.to_string()))?;
                let post_compression_bytes = compressed.len();
                let compression_duration = compression_start.elapsed();

                // Ensure the server knows to unpack the request body.
                headers.insert(
                    http::header::CONTENT_ENCODING,
                    http::HeaderValue::from_static("zstd"),
                );
                if !headers.contains_key(http::header::CONTENT_TYPE) {
                    headers.insert(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static("application/json"),
                    );
                }

                tracing::info!(
                    pre_compression_bytes,
                    post_compression_bytes,
                    compression_duration_ms = compression_duration.as_millis(),
                    "Compressed request body with zstd"
                );

                builder = builder.headers(headers).body(compressed);
            } else {
                builder = builder.headers(headers).json(&body);
            }
        } else {
            builder = builder.headers(headers);
        }
        Ok(builder)
    }

    fn map_error(err: reqwest::Error) -> TransportError {
        if err.is_timeout() {
            TransportError::Timeout
        } else {
            TransportError::Network(err.to_string())
        }
    }
}

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn execute(&self, req: Request) -> Result<Response, TransportError> {
        if enabled!(Level::TRACE) {
            trace!(
                "{} to {}: {}",
                req.method,
                req.url,
                req.body.as_ref().unwrap_or_default()
            );
        }

        let builder = self.build(req)?;
        let resp = builder.send().await.map_err(Self::map_error)?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.bytes().await.map_err(Self::map_error)?;
        if !status.is_success() {
            let body = String::from_utf8(bytes.to_vec()).ok();
            return Err(TransportError::Http {
                status,
                headers: Some(headers),
                body,
            });
        }
        Ok(Response {
            status,
            headers,
            body: bytes,
        })
    }

    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
        if enabled!(Level::TRACE) {
            trace!(
                "{} to {}: {}",
                req.method,
                req.url,
                req.body.as_ref().unwrap_or_default()
            );
        }

        let builder = self.build(req)?;
        let resp = builder.send().await.map_err(Self::map_error)?;
        let status = resp.status();
        let headers = resp.headers().clone();
        if !status.is_success() {
            let body = resp.text().await.ok();
            return Err(TransportError::Http {
                status,
                headers: Some(headers),
                body,
            });
        }
        let stream = resp
            .bytes_stream()
            .map(|result| result.map_err(Self::map_error));
        Ok(StreamResponse {
            status,
            headers,
            bytes: Box::pin(stream),
        })
    }
}
