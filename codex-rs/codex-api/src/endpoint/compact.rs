use crate::auth::AuthProvider;
use crate::common::CompactionInput;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_client::Response as HttpResponse;
use codex_protocol::models::ResponseItem;
use http::HeaderMap;
use http::Method;
use http::StatusCode;
use serde::Deserialize;
use serde_json::to_value;
use std::sync::Arc;

pub struct CompactClient<T: HttpTransport, A: AuthProvider> {
    session: EndpointSession<T, A>,
}

#[derive(Debug)]
pub enum CompactOperationPollResult {
    Pending { poll_after_ms: Option<u64> },
    Completed(Vec<ResponseItem>),
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

    fn operations_path() -> &'static str {
        "responses/compact/operations"
    }

    fn operation_path(operation_id: &str) -> String {
        format!("responses/compact/operations/{operation_id}")
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

    pub async fn compact_input(
        &self,
        input: &CompactionInput<'_>,
        extra_headers: HeaderMap,
    ) -> Result<Vec<ResponseItem>, ApiError> {
        let body = to_value(input)
            .map_err(|e| ApiError::Stream(format!("failed to encode compaction input: {e}")))?;
        self.compact(body, extra_headers).await
    }

    pub async fn submit_compact_operation(
        &self,
        body: serde_json::Value,
        extra_headers: HeaderMap,
    ) -> Result<String, ApiError> {
        let resp = self
            .session
            .execute(
                Method::POST,
                Self::operations_path(),
                extra_headers,
                Some(body),
            )
            .await?;

        if resp.status != StatusCode::ACCEPTED {
            return Err(response_to_api_error(
                &resp,
                "compact operation submit failed",
            ));
        }

        let parsed: CompactOperationPendingResponse =
            serde_json::from_slice(&resp.body).map_err(|e| ApiError::Stream(e.to_string()))?;
        if parsed.operation_id.is_empty() {
            return Err(ApiError::Stream(
                "compact operation submit returned empty operation_id".to_string(),
            ));
        }
        if parsed.status != "pending" {
            let status = parsed.status;
            return Err(ApiError::Stream(format!(
                "unexpected compact operation status: {status}"
            )));
        }

        Ok(parsed.operation_id)
    }

    pub async fn poll_compact_operation(
        &self,
        operation_id: &str,
        extra_headers: HeaderMap,
    ) -> Result<CompactOperationPollResult, ApiError> {
        let resp = self
            .session
            .execute(
                Method::GET,
                &Self::operation_path(operation_id),
                extra_headers,
                None,
            )
            .await?;

        if resp.status == StatusCode::ACCEPTED {
            let parsed: CompactOperationPendingResponse =
                serde_json::from_slice(&resp.body).map_err(|e| ApiError::Stream(e.to_string()))?;
            if parsed.status == "pending" {
                return Ok(CompactOperationPollResult::Pending {
                    poll_after_ms: parsed.poll_after_ms,
                });
            }
            let status = parsed.status;
            return Err(ApiError::Stream(format!(
                "unexpected compact operation status: {status}"
            )));
        }

        if resp.status != StatusCode::OK {
            return Err(response_to_api_error(
                &resp,
                "compact operation poll failed",
            ));
        }

        let parsed: CompactHistoryResponse =
            serde_json::from_slice(&resp.body).map_err(|e| ApiError::Stream(e.to_string()))?;
        Ok(CompactOperationPollResult::Completed(parsed.output))
    }
}

fn response_to_api_error(resp: &HttpResponse, fallback_message: &str) -> ApiError {
    let message = String::from_utf8_lossy(&resp.body).trim().to_string();
    ApiError::Api {
        status: resp.status,
        message: if message.is_empty() {
            fallback_message.to_string()
        } else {
            message
        },
    }
}

#[derive(Debug, Deserialize)]
struct CompactHistoryResponse {
    output: Vec<ResponseItem>,
}

#[derive(Debug, Deserialize)]
struct CompactOperationPendingResponse {
    operation_id: String,
    status: String,
    poll_after_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RetryConfig;
    use async_trait::async_trait;
    use codex_client::Request;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
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

    #[tokio::test]
    async fn submit_compact_operation_returns_pending_status() {
        let response_body = compact_operation_pending_payload("op_123");
        let client = CompactClient::new(
            StaticResponseTransport {
                response: Response {
                    status: StatusCode::ACCEPTED,
                    headers: HeaderMap::new(),
                    body: bytes::Bytes::from(response_body.to_string()),
                },
            },
            provider("https://example.com/api/codex"),
            DummyAuth,
        );

        let submit_result = client
            .submit_compact_operation(serde_json::json!({}), HeaderMap::new())
            .await
            .expect("submit should succeed");

        assert_eq!(submit_result, "op_123");
    }

    fn compact_operation_pending_payload(operation_id: &str) -> serde_json::Value {
        serde_json::json!({
            "operation_id": operation_id,
            "status": "pending",
            "poll_after_ms": 1000,
        })
    }
}
