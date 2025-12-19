use crate::error::ApiError;
use crate::provider::RequestCompression;
use bytes::Bytes;
use codex_client::Body;
use http::HeaderMap;
use http::HeaderValue;
use http::header::CONTENT_ENCODING;
use serde_json::Value;
use std::time::Instant;
use tracing::info;
use zstd::stream::encode_all;

pub(crate) fn encode_body(body: &Value, compression: RequestCompression) -> Result<Body, ApiError> {
    match compression {
        RequestCompression::None => Ok(Body::Json(body.clone())),
        RequestCompression::Zstd => {
            let json = serde_json::to_vec(body).map_err(|err| {
                ApiError::Stream(format!("failed to encode request body as json: {err}"))
            })?;
            let started_at = Instant::now();
            let compressed = encode_all(json.as_slice(), 0).map_err(|err| {
                ApiError::Stream(format!("failed to compress request body: {err}"))
            })?;
            let elapsed = started_at.elapsed();
            info!(
                input_bytes = json.len(),
                output_bytes = compressed.len(),
                elapsed_ms = elapsed.as_millis(),
                "compressed request body"
            );
            Ok(Body::Bytes(Bytes::from(compressed)))
        }
    }
}

pub(crate) fn insert_compression_headers(headers: &mut HeaderMap, compression: RequestCompression) {
    if matches!(compression, RequestCompression::Zstd) {
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("zstd"));
    }
}
