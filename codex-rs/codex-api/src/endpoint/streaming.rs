use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::provider::RequestCompression;
use crate::telemetry::SseTelemetry;
use crate::telemetry::run_with_request_telemetry;
use bytes::Bytes;
use codex_client::Body;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_client::StreamResponse;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::header::ACCEPT;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_TYPE;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use zstd::stream::encode_all;

pub(crate) struct StreamingClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport, A: AuthProvider> StreamingClient<T, A> {
    pub(crate) fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
            request_telemetry: None,
            sse_telemetry: None,
        }
    }

    pub(crate) fn with_telemetry(
        mut self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        self.request_telemetry = request;
        self.sse_telemetry = sse;
        self
    }

    pub(crate) fn provider(&self) -> &Provider {
        &self.provider
    }

    pub(crate) async fn stream(
        &self,
        path: &str,
        body: Value,
        extra_headers: HeaderMap,
        spawner: fn(StreamResponse, Duration, Option<Arc<dyn SseTelemetry>>) -> ResponseStream,
    ) -> Result<ResponseStream, ApiError> {
        let content_encoding =
            matches!(self.provider.request_compression, RequestCompression::Zstd);
        let encoded_body =
            encode_body(&body, self.provider.request_compression).map_err(ApiError::Stream)?;

        let builder = || {
            let mut req = self.provider.build_request(Method::POST, path);
            req.headers.extend(extra_headers.clone());
            req.headers
                .insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
            req.headers
                .entry(CONTENT_TYPE)
                .or_insert_with(|| HeaderValue::from_static("application/json"));
            if content_encoding {
                req.headers
                    .insert(CONTENT_ENCODING, HeaderValue::from_static("zstd"));
            }
            req.body = Some(encoded_body.clone());
            add_auth_headers(&self.auth, req)
        };

        let stream_response = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            builder,
            |req| self.transport.stream(req),
        )
        .await?;

        Ok(spawner(
            stream_response,
            self.provider.stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }
}

fn encode_body(body: &Value, compression: RequestCompression) -> Result<Body, String> {
    match compression {
        RequestCompression::None => Ok(Body::Json(body.clone())),
        RequestCompression::Zstd => {
            let json = serde_json::to_vec(body)
                .map_err(|err| format!("failed to encode request body as json: {err}"))?;
            let compressed = encode_all(json.as_slice(), 0)
                .map_err(|err| format!("failed to compress request body: {err}"))?;
            Ok(Body::Bytes(Bytes::from(compressed)))
        }
    }
}
