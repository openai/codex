use bytes::Bytes;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::time::Duration;

/// A JSON request body serialized once into reference-counted bytes.
///
/// Clones share the encoded allocation. Internally, the body can also hold the
/// final compressed wire bytes while retaining the original JSON only when
/// request-body trace logging is enabled.
#[derive(Clone, PartialEq, Eq)]
pub struct EncodedJsonBody {
    bytes: Bytes,
    trace_bytes: Option<Bytes>,
    prepared: bool,
    private: bool,
}

impl fmt::Debug for EncodedJsonBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut body = f.debug_struct("EncodedJsonBody");
        if self.private {
            return body
                .field("byte_len", &self.bytes.len())
                .finish_non_exhaustive();
        }
        body.field("bytes", &self.bytes)
            .field("trace_bytes", &self.trace_bytes)
            .field("prepared", &self.prepared)
            .finish()
    }
}

impl EncodedJsonBody {
    /// Serializes `value` into a reusable JSON body.
    pub fn encode<T: Serialize + ?Sized>(value: &T) -> Result<Self, serde_json::Error> {
        serde_json::to_vec(value).map(|bytes| Self {
            bytes: Bytes::from(bytes),
            trace_bytes: None,
            prepared: false,
            private: false,
        })
    }

    /// Returns the encoded bytes currently stored by this body.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Observers can clone this handle to retain the transport JSON without copying it.
    pub fn shared_bytes(&self) -> &Bytes {
        &self.bytes
    }

    /// Send richer JSON only to the configured origin and use the original body in traces.
    pub fn with_private_json(&self, json: String) -> Self {
        Self {
            bytes: Bytes::from(json),
            trace_bytes: Some(self.bytes.clone()),
            prepared: false,
            private: true,
        }
    }

    pub(crate) fn trace_bytes(&self) -> &[u8] {
        self.trace_bytes.as_ref().unwrap_or(&self.bytes)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestCompression {
    #[default]
    None,
    Zstd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestBody {
    Json(Value),
    EncodedJson(EncodedJsonBody),
    Raw(Bytes),
}

impl RequestBody {
    pub fn json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::EncodedJson(_) | Self::Raw(_) => None,
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

// Wire bytes can contain private connector results. Deliberately omit Debug.
pub(crate) struct WireBody {
    pub(crate) headers: HeaderMap,
    pub(crate) body: Option<Bytes>,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: HeaderMap,
    pub body: Option<RequestBody>,
    pub compression: RequestCompression,
    pub timeout: Option<Duration>,
    /// Maximum accepted response-body bytes, including streaming and error bodies.
    /// `None` leaves the response unbounded.
    pub response_body_limit_bytes: Option<usize>,
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
            response_body_limit_bytes: None,
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

    /// Prepares the body once and stores the exact bytes that will be sent.
    ///
    /// Cloning the returned request shares the body bytes, so retry attempts do
    /// not repeat JSON serialization or compression. Request-signing auth also
    /// sees the same final headers and bytes that the transport will send.
    pub fn into_prepared(mut self) -> Result<Self, String> {
        let is_json = matches!(
            self.body,
            Some(RequestBody::Json(_) | RequestBody::EncodedJson(_))
        );
        let private_trace = match &self.body {
            Some(RequestBody::EncodedJson(body)) if body.private => body.trace_bytes.clone(),
            _ => None,
        };
        let private = private_trace.is_some();
        let trace_bytes = if private {
            private_trace
        } else if self.compression != RequestCompression::None
            && tracing::enabled!(target: "codex_http_client::transport", tracing::Level::TRACE)
        {
            match self.body.as_ref() {
                Some(RequestBody::Json(body)) => Some(Bytes::from(
                    serde_json::to_vec(body).map_err(|err| err.to_string())?,
                )),
                Some(RequestBody::EncodedJson(body)) => Some(body.bytes.clone()),
                Some(RequestBody::Raw(_)) | None => None,
            }
        } else {
            None
        };
        let prepared = self.prepare_wire_body()?;
        self.headers = prepared.headers;
        self.body = match (is_json, prepared.body) {
            (true, Some(bytes)) => Some(RequestBody::EncodedJson(EncodedJsonBody {
                bytes,
                trace_bytes,
                prepared: true,
                private,
            })),
            (false, Some(body)) => Some(RequestBody::Raw(body)),
            (_, None) => None,
        };
        self.compression = RequestCompression::None;
        Ok(self)
    }

    /// Convert the request body into the exact bytes that will be sent.
    ///
    /// Auth schemes such as AWS SigV4 need to sign the final body bytes, including
    /// compression and content headers. Calling this method does not mutate the
    /// request. Use [`Self::into_prepared`] for private JSON to retain its redacted
    /// debug representation.
    pub fn prepare_body_for_send(&self) -> Result<PreparedRequestBody, String> {
        if self.has_private_body() {
            return Err("private JSON requires Request::into_prepared".to_string());
        }
        let wire = self.prepare_wire_body()?;
        Ok(PreparedRequestBody {
            headers: wire.headers,
            body: wire.body,
        })
    }

    pub(crate) fn has_private_body(&self) -> bool {
        matches!(&self.body, Some(RequestBody::EncodedJson(body)) if body.private)
    }

    pub(crate) fn prepare_wire_body(&self) -> Result<WireBody, String> {
        let headers = self.headers.clone();
        match self.body.as_ref() {
            Some(RequestBody::Raw(raw_body)) => {
                if self.compression != RequestCompression::None {
                    return Err("request compression cannot be used with raw bodies".to_string());
                }
                Ok(WireBody {
                    headers,
                    body: Some(raw_body.clone()),
                })
            }
            Some(RequestBody::Json(body)) => {
                let body = EncodedJsonBody::encode(body).map_err(|err| err.to_string())?;
                self.prepare_encoded_json(headers, &body)
            }
            Some(RequestBody::EncodedJson(body)) => self.prepare_encoded_json(headers, body),
            None => Ok(WireBody {
                headers,
                body: None,
            }),
        }
    }

    fn prepare_encoded_json(
        &self,
        mut headers: HeaderMap,
        body: &EncodedJsonBody,
    ) -> Result<WireBody, String> {
        if body.prepared {
            return Ok(WireBody {
                headers,
                body: Some(body.bytes.clone()),
            });
        }

        let bytes = if self.compression != RequestCompression::None {
            if headers.contains_key(http::header::CONTENT_ENCODING) {
                return Err(
                    "request compression was requested but content-encoding is already set"
                        .to_string(),
                );
            }

            let pre_compression_bytes = body.bytes.len();
            let compression_start = std::time::Instant::now();
            let (compressed, content_encoding) = match self.compression {
                RequestCompression::None => unreachable!("guarded by compression != None"),
                RequestCompression::Zstd => (
                    zstd::stream::encode_all(std::io::Cursor::new(body.as_bytes()), 3)
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

            Bytes::from(compressed)
        } else {
            body.bytes.clone()
        };

        if !headers.contains_key(http::header::CONTENT_TYPE) {
            headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }

        Ok(WireBody {
            headers,
            body: Some(bytes),
        })
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

    #[test]
    fn into_prepared_stores_compressed_body_for_reuse() {
        let body =
            EncodedJsonBody::encode(&json!({"model": "test-model"})).expect("JSON should encode");
        let mut request =
            Request::new(Method::POST, "https://example.com/v1/responses".to_string())
                .with_compression(RequestCompression::Zstd);
        request.body = Some(RequestBody::EncodedJson(body));
        let request = request.into_prepared().expect("body should prepare");
        let Some(RequestBody::EncodedJson(body)) = request.body.as_ref() else {
            panic!("expected an encoded JSON body");
        };
        let decompressed = zstd::stream::decode_all(std::io::Cursor::new(body.as_bytes()))
            .expect("body should decompress");

        assert_eq!(decompressed, br#"{"model":"test-model"}"#);
        assert_eq!(request.compression, RequestCompression::None);
        assert_eq!(
            request.headers.get(http::header::CONTENT_ENCODING),
            Some(&HeaderValue::from_static("zstd"))
        );
        assert_eq!(
            request.headers.get(http::header::CONTENT_TYPE),
            Some(&HeaderValue::from_static("application/json"))
        );
    }

    #[test]
    fn ordinary_request_bodies_keep_debug_fields() {
        let canonical = EncodedJsonBody::encode(&json!({"model": "test"})).expect("valid JSON");
        let json = &canonical.bytes;
        assert_eq!(
            format!("{canonical:?}"),
            format!("EncodedJsonBody {{ bytes: {json:?}, trace_bytes: None, prepared: false }}")
        );

        for compression in [RequestCompression::None, RequestCompression::Zstd] {
            let mut request = Request::new(Method::POST, "https://example.com".to_string())
                .with_compression(compression);
            request
                .headers
                .insert("x-test", HeaderValue::from_static("header-marker"));
            request.body = Some(RequestBody::EncodedJson(canonical.clone()));
            let subscriber = tracing_subscriber::registry();
            let prepared =
                tracing::subscriber::with_default(subscriber, || request.into_prepared())
                    .expect("body should prepare");
            let wire = prepared.prepare_body_for_send().expect("prepared body");
            let Some(RequestBody::EncodedJson(encoded)) = prepared.body.as_ref() else {
                panic!("expected encoded JSON");
            };
            let PreparedRequestBody { headers, body } = &wire;
            let bytes = body.as_ref().expect("wire bytes");
            let trace_bytes = (compression == RequestCompression::Zstd).then_some(json);
            assert_eq!(
                format!("{encoded:?}"),
                format!(
                    "EncodedJsonBody {{ bytes: {bytes:?}, trace_bytes: {trace_bytes:?}, prepared: true }}"
                )
            );
            assert_eq!(
                format!("{wire:?}"),
                format!("PreparedRequestBody {{ headers: {headers:?}, body: Some({bytes:?}) }}")
            );
        }
    }

    #[test]
    fn private_json_keeps_canonical_trace_body() {
        let mut call = json!({
            "arguments": "argument-marker",
            "tool_result_metadata": "metadata-marker"
        });
        let canonical = EncodedJsonBody::encode(&call).expect("valid JSON");
        call["tool_result"] = json!("private-result-marker");
        let wire = call.to_string();
        let enriched = canonical.with_private_json(wire.clone());
        for compression in [RequestCompression::None, RequestCompression::Zstd] {
            let mut request = Request::new(Method::POST, "https://example.com".to_string())
                .with_compression(compression);
            request.body = Some(RequestBody::EncodedJson(enriched.clone()));
            let prepared = request
                .clone()
                .into_prepared()
                .expect("body should prepare");
            for request in [&request, &prepared] {
                assert_eq!(
                    request.prepare_body_for_send().unwrap_err(),
                    "private JSON requires Request::into_prepared"
                );
            }
            let Some(RequestBody::EncodedJson(body)) = prepared.body.as_ref() else {
                panic!("expected encoded JSON");
            };
            for encoded in [&enriched, body] {
                let len = encoded.as_bytes().len();
                assert_eq!(
                    format!("{encoded:?}"),
                    format!("EncodedJsonBody {{ byte_len: {len}, .. }}")
                );
            }
            assert_eq!(body.trace_bytes(), canonical.as_bytes());
            let wire_body = prepared.prepare_wire_body().expect("wire body");
            let bytes = wire_body.body.as_deref().expect("wire bytes");
            assert_eq!(bytes, body.as_bytes());
            let bytes = match compression {
                RequestCompression::None => bytes.to_vec(),
                RequestCompression::Zstd => zstd::stream::decode_all(bytes).expect("valid zstd"),
            };
            assert_eq!(bytes, wire.as_bytes());
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: http::StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}
