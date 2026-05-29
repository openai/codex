use bytes::Bytes;
use http::Method;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestCompression {
    #[default]
    None,
    Zstd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestBody {
    Json(Value),
    Raw(Bytes),
}

impl RequestBody {
    pub fn json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Raw(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequestBody {
    pub headers: HeaderMap,
    pub body: Option<Bytes>,
}

impl PreparedRequestBody {
    pub fn body_bytes(&self) -> Bytes {
        self.body.clone().unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: HeaderMap,
    pub body: Option<RequestBody>,
    pub compression: RequestCompression,
    pub timeout: Option<Duration>,
}

impl Request {
    pub fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
            compression: RequestCompression::None,
            timeout: None,
        }
    }

    pub fn with_json<T: Serialize>(mut self, body: &T) -> Self {
        self.body = serde_json::to_value(body).ok().map(RequestBody::Json);
        self
    }

    pub fn with_raw_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(RequestBody::Raw(body.into()));
        self
    }

    pub fn with_compression(mut self, compression: RequestCompression) -> Self {
        self.compression = compression;
        self
    }

    /// Convert the request body into the exact bytes that will be sent.
    ///
    /// Auth schemes such as AWS SigV4 need to sign the final body bytes, including
    /// compression and content headers. Calling this method does not mutate the
    /// request.
    pub fn prepare_body_for_send(&self) -> Result<PreparedRequestBody, String> {
        let mut headers = self.headers.clone();
        match self.body.as_ref() {
            Some(RequestBody::Raw(raw_body)) => {
                if self.compression != RequestCompression::None {
                    return Err("request compression cannot be used with raw bodies".to_string());
                }
                Ok(PreparedRequestBody {
                    headers,
                    body: Some(raw_body.clone()),
                })
            }
            Some(RequestBody::Json(body)) => {
                let json = serde_json::to_vec(&body).map_err(|err| err.to_string())?;
                let bytes = if self.compression != RequestCompression::None {
                    if headers.contains_key(http::header::CONTENT_ENCODING) {
                        return Err(
                            "request compression was requested but content-encoding is already set"
                                .to_string(),
                        );
                    }

                    let pre_compression_bytes = json.len();
                    let compression_start = std::time::Instant::now();
                    let (compressed, content_encoding) = match self.compression {
                        RequestCompression::None => unreachable!("guarded by compression != None"),
                        RequestCompression::Zstd => (
                            zstd::stream::encode_all(std::io::Cursor::new(json), 3)
                                .map_err(|err| err.to_string())?,
                            HeaderValue::from_static("zstd"),
                        ),
                    };
                    let post_compression_bytes = compressed.len();
                    let compression_duration = compression_start.elapsed();

                    headers.insert(http::header::CONTENT_ENCODING, content_encoding);

                    tracing::debug!(
                        pre_compression_bytes,
                        post_compression_bytes,
                        compression_duration_ms = compression_duration.as_millis(),
                        "Compressed request body with zstd"
                    );

                    compressed
                } else {
                    json
                };

                if !headers.contains_key(http::header::CONTENT_TYPE) {
                    headers.insert(
                        http::header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json"),
                    );
                }

                Ok(PreparedRequestBody {
                    headers,
                    body: Some(Bytes::from(bytes)),
                })
            }
            None => Ok(PreparedRequestBody {
                headers,
                body: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn prepare_body_for_send_serializes_json_and_sets_content_type() {
        let request = Request::new(Method::POST, "https://example.com/v1/responses".to_string())
            .with_json(&json!({"model": "test-model"}));

        let prepared = request
            .prepare_body_for_send()
            .expect("body should prepare");

        assert_eq!(
            prepared.body,
            Some(Bytes::from_static(br#"{"model":"test-model"}"#))
        );
        assert_eq!(
            prepared
                .headers
                .get(http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            request.body,
            Some(RequestBody::Json(json!({"model": "test-model"})))
        );
        assert_eq!(request.compression, RequestCompression::None);
    }

    #[test]
    fn prepare_body_for_send_rejects_existing_content_encoding_when_compressing() {
        let mut request =
            Request::new(Method::POST, "https://example.com/v1/responses".to_string())
                .with_json(&json!({"model": "test-model"}))
                .with_compression(RequestCompression::Zstd);
        request.headers.insert(
            http::header::CONTENT_ENCODING,
            HeaderValue::from_static("gzip"),
        );

        let err = request
            .prepare_body_for_send()
            .expect_err("conflicting content-encoding should fail");

        assert_eq!(
            err,
            "request compression was requested but content-encoding is already set"
        );
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: http::StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}
