use crate::auth::AuthProvider;
use crate::common::CompactionInput;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_client::TransportError;
use codex_protocol::models::ResponseItem;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::StatusCode;
use serde::Deserialize;
use serde_json::to_value;
use std::sync::Arc;
use tokio::time::timeout;

pub struct CompactClient<T: HttpTransport, A: AuthProvider> {
    session: EndpointSession<T, A>,
}

impl<T: HttpTransport, A: AuthProvider> CompactClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
        }
    }

    pub fn with_telemetry(self, request: Option<Arc<dyn RequestTelemetry>>) -> Self {
        Self {
            session: self.session.with_request_telemetry(request),
        }
    }

    fn path() -> &'static str {
        "responses/compact"
    }

    fn stream_path() -> &'static str {
        "responses/compact/stream"
    }

    pub async fn compact(
        &self,
        body: serde_json::Value,
        extra_headers: HeaderMap,
    ) -> Result<Vec<ResponseItem>, ApiError> {
        let resp = self
            .session
            .execute(Method::POST, Self::path(), extra_headers, Some(body))
            .await?;
        let parsed: CompactHistoryResponse =
            serde_json::from_slice(&resp.body).map_err(|e| ApiError::Stream(e.to_string()))?;
        Ok(parsed.output)
    }

    pub async fn compact_stream(
        &self,
        body: serde_json::Value,
        extra_headers: HeaderMap,
    ) -> Result<Vec<ResponseItem>, ApiError> {
        let stream_response = self
            .session
            .stream_with(
                Method::POST,
                Self::stream_path(),
                extra_headers,
                Some(body),
                |req| {
                    req.headers.insert(
                        http::header::ACCEPT,
                        HeaderValue::from_static("text/event-stream"),
                    );
                },
            )
            .await?;

        let mut stream = stream_response.bytes.eventsource();
        loop {
            let sse =
                match timeout(self.session.provider().stream_idle_timeout, stream.next()).await {
                    Ok(Some(Ok(sse))) => sse,
                    Ok(Some(Err(error))) => return Err(ApiError::Stream(error.to_string())),
                    Ok(None) => {
                        return Err(ApiError::Stream(
                            "stream closed before compact.completed".to_string(),
                        ));
                    }
                    Err(_) => {
                        return Err(ApiError::Stream(
                            "idle timeout waiting for compact stream".to_string(),
                        ));
                    }
                };

            let event: CompactStreamEvent = match serde_json::from_str(&sse.data) {
                Ok(event) => event,
                Err(_) => continue,
            };

            match event.kind.as_str() {
                "compact.keepalive" => continue,
                "compact.completed" => {
                    if let Some(response) = event.response {
                        return Ok(response.output);
                    }
                    return Err(ApiError::Stream(
                        "compact.completed missing response payload".to_string(),
                    ));
                }
                "compact.failed" => {
                    let status = event
                        .status
                        .and_then(|status| StatusCode::from_u16(status).ok())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                    let message = event
                        .message
                        .unwrap_or_else(|| "compact stream failed".to_string());
                    let mut response_headers = HeaderMap::new();
                    if let Some(model_cap_model) = event.model_cap_model
                        && let Ok(model_cap_header) = HeaderValue::from_str(&model_cap_model)
                    {
                        response_headers.insert(MODEL_CAP_MODEL_HEADER, model_cap_header);
                    }
                    if let Some(model_cap_reset_after_seconds) = event.model_cap_reset_after_seconds
                        && let Ok(reset_after_header) =
                            HeaderValue::from_str(&model_cap_reset_after_seconds.to_string())
                    {
                        response_headers.insert(MODEL_CAP_RESET_AFTER_HEADER, reset_after_header);
                    }
                    let headers = (!response_headers.is_empty()).then_some(response_headers);
                    return Err(ApiError::Transport(TransportError::Http {
                        status,
                        url: None,
                        headers,
                        body: Some(message),
                    }));
                }
                _ => continue,
            }
        }
    }

    pub async fn compact_input(
        &self,
        input: &CompactionInput<'_>,
        extra_headers: HeaderMap,
    ) -> Result<Vec<ResponseItem>, ApiError> {
        let body = to_value(input)
            .map_err(|e| ApiError::Stream(format!("failed to encode compaction input: {e}")))?;
        self.compact(body, extra_headers).await
    }

    pub async fn compact_stream_input(
        &self,
        input: &CompactionInput<'_>,
        extra_headers: HeaderMap,
    ) -> Result<Vec<ResponseItem>, ApiError> {
        let body = to_value(input)
            .map_err(|e| ApiError::Stream(format!("failed to encode compaction input: {e}")))?;
        self.compact_stream(body, extra_headers).await
    }
}

#[derive(Debug, Deserialize)]
struct CompactHistoryResponse {
    output: Vec<ResponseItem>,
}

#[derive(Debug, Deserialize)]
struct CompactStreamEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<CompactHistoryResponse>,
    status: Option<u16>,
    message: Option<String>,
    model_cap_model: Option<String>,
    model_cap_reset_after_seconds: Option<u64>,
}

const MODEL_CAP_MODEL_HEADER: &str = "x-codex-model-cap-model";
const MODEL_CAP_RESET_AFTER_HEADER: &str = "x-codex-model-cap-reset-after-seconds";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthProvider;
    use crate::provider::RetryConfig;
    use async_trait::async_trait;
    use bytes::Bytes;
    use codex_client::Request;
    use codex_client::RequestCompression;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use futures::stream;
    use http::HeaderMap;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct DummyAuth;

    impl AuthProvider for DummyAuth {
        fn bearer_token(&self) -> Option<String> {
            None
        }
    }

    fn provider(base_url: &str) -> Provider {
        Provider {
            name: "test".to_string(),
            base_url: base_url.to_string(),
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(1),
        }
    }

    #[derive(Clone)]
    struct StaticResponseTransport {
        response: Response,
    }

    #[async_trait]
    impl HttpTransport for StaticResponseTransport {
        async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
            Ok(self.response.clone())
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
        }
    }

    #[derive(Clone)]
    struct StaticStreamTransport {
        stream_body: Bytes,
    }

    #[async_trait]
    impl HttpTransport for StaticStreamTransport {
        async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
            Err(TransportError::Build("execute should not run".to_string()))
        }

        async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
            assert_eq!(req.compression, RequestCompression::None);
            let bytes = self.stream_body.clone();
            let stream = stream::iter(vec![Ok(bytes)]);
            Ok(StreamResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                bytes: Box::pin(stream),
            })
        }
    }

    #[tokio::test]
    async fn compact_stream_returns_output_from_completed_event() {
        let stream_body = concat!(
            "event: compact.keepalive\n",
            "data: {\"type\":\"compact.keepalive\"}\n\n",
            "event: compact.completed\n",
            "data: {\"type\":\"compact.completed\",\"response\":{\"output\":[{\"type\":\"compaction_summary\",\"encrypted_content\":\"abc\"}]}}\n\n"
        )
        .to_string();
        let client = CompactClient::new(
            StaticStreamTransport {
                stream_body: Bytes::from(stream_body),
            },
            provider("https://example.com/api/codex"),
            DummyAuth,
        );

        let output = client
            .compact_stream(serde_json::json!({}), HeaderMap::new())
            .await
            .expect("stream compact should succeed");

        assert_eq!(
            output,
            vec![ResponseItem::Compaction {
                encrypted_content: "abc".to_string()
            }]
        );
    }

    #[tokio::test]
    async fn compact_stream_returns_api_error_from_failed_event() {
        let stream_body = concat!(
            "event: compact.failed\n",
            "data: {\"type\":\"compact.failed\",\"status\":429,\"message\":\"slow down\"}\n\n"
        );
        let client = CompactClient::new(
            StaticStreamTransport {
                stream_body: Bytes::from(stream_body),
            },
            provider("https://example.com/api/codex"),
            DummyAuth,
        );

        let error = client
            .compact_stream(serde_json::json!({}), HeaderMap::new())
            .await
            .expect_err("stream compact should fail");

        match error {
            ApiError::Transport(TransportError::Http {
                status,
                headers,
                body,
                ..
            }) => {
                assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(body, Some("slow down".to_string()));
                assert_eq!(headers, None);
            }
            other => panic!("expected ApiError::Transport(Http), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn compact_stream_failed_event_carries_model_cap_headers() {
        let stream_body = concat!(
            "event: compact.failed\n",
            "data: {\"type\":\"compact.failed\",\"status\":429,\"message\":\"slow down\",\"model_cap_model\":\"gpt-5-codex\",\"model_cap_reset_after_seconds\":30}\n\n"
        );
        let client = CompactClient::new(
            StaticStreamTransport {
                stream_body: Bytes::from(stream_body),
            },
            provider("https://example.com/api/codex"),
            DummyAuth,
        );

        let error = client
            .compact_stream(serde_json::json!({}), HeaderMap::new())
            .await
            .expect_err("stream compact should fail");

        match error {
            ApiError::Transport(TransportError::Http {
                status,
                headers,
                body,
                ..
            }) => {
                assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(body, Some("slow down".to_string()));
                let headers = headers.expect("headers should be present");
                assert_eq!(
                    headers
                        .get(MODEL_CAP_MODEL_HEADER)
                        .expect("model cap header should be present"),
                    "gpt-5-codex"
                );
                assert_eq!(
                    headers
                        .get(MODEL_CAP_RESET_AFTER_HEADER)
                        .expect("reset-after header should be present"),
                    "30"
                );
            }
            other => panic!("expected ApiError::Transport(Http), got {other:?}"),
        }
    }

    #[test]
    fn path_is_responses_compact() {
        assert_eq!(
            CompactClient::<StaticResponseTransport, DummyAuth>::path(),
            "responses/compact"
        );
    }
}
