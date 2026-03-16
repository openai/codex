use crate::auth::AuthProvider;
use crate::common::MemorySummarizeInput;
use crate::common::MemorySummarizeOutput;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use http::HeaderMap;
use http::Method;
use serde::Deserialize;
use serde_json::to_value;
use std::sync::Arc;

pub struct MemoriesClient<T: HttpTransport, A: AuthProvider> {
    session: EndpointSession<T, A>,
}

impl<T: HttpTransport, A: AuthProvider> MemoriesClient<T, A> {
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
        "memories/trace_summarize"
    }

    pub async fn summarize(
        &self,
        body: serde_json::Value,
        extra_headers: HeaderMap,
    ) -> Result<Vec<MemorySummarizeOutput>, ApiError> {
        let resp = self
            .session
            .execute(Method::POST, Self::path(), extra_headers, Some(body))
            .await?;
        let parsed: SummarizeResponse =
            serde_json::from_slice(&resp.body).map_err(|e| ApiError::Stream(e.to_string()))?;
        Ok(parsed.output)
    }

    pub async fn summarize_input(
        &self,
        input: &MemorySummarizeInput,
        extra_headers: HeaderMap,
    ) -> Result<Vec<MemorySummarizeOutput>, ApiError> {
        let body = to_value(input).map_err(|e| {
            ApiError::Stream(format!("failed to encode memory summarize input: {e}"))
        })?;
        self.summarize(body, extra_headers).await
    }
}

#[derive(Debug, Deserialize)]
struct SummarizeResponse {
    output: Vec<MemorySummarizeOutput>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::RawMemory;
    use crate::common::RawMemoryMetadata;
    use crate::provider::RetryConfig;
    use async_trait::async_trait;
    use codex_client::Request;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
    use http::HeaderMap;
    use http::Method;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct DummyTransport;

    #[async_trait]
    impl HttpTransport for DummyTransport {
        async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
            Err(TransportError::Build("execute should not run".to_string()))
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
        }
    }

    #[derive(Clone, Default)]
    struct DummyAuth;

    impl AuthProvider for DummyAuth {
        fn bearer_token(&self) -> Option<String> {
            None
        }
    }

    #[derive(Clone)]
    struct CapturingTransport {
        last_request: Arc<Mutex<Option<Request>>>,
        response_body: Arc<Vec<u8>>,
    }

    impl CapturingTransport {
        fn new(response_body: Vec<u8>) -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_body: Arc::new(response_body),
            }
        }
    }

    #[async_trait]
    impl HttpTransport for CapturingTransport {
        async fn execute(&self, req: Request) -> Result<Response, TransportError> {
            *self.last_request.lock().expect("lock request store") = Some(req);
            Ok(Response {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: self.response_body.as_ref().clone().into(),
            })
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
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

    #[test]
    fn path_is_memories_trace_summarize_for_wire_compatibility() {
        assert_eq!(
            MemoriesClient::<DummyTransport, DummyAuth>::path(),
            "memories/trace_summarize"
        );
    }

    #[tokio::test]
    async fn summarize_input_posts_expected_payload_and_parses_output() {
        let transport = CapturingTransport::new(
            serde_json::to_vec(&json!({
                "output": [
                    {
                        "trace_summary": "raw summary",
                        "memory_summary": "memory summary"
                    }
                ]
            }))
            .expect("serialize response"),
        );
        let client = MemoriesClient::new(
            transport.clone(),
            provider("https://example.com/api/codex"),
            DummyAuth,
        );

        let input = MemorySummarizeInput {
            model: "gpt-test".to_string(),
            raw_memories: vec![RawMemory {
                id: "trace-1".to_string(),
                metadata: RawMemoryMetadata {
                    source_path: "/tmp/trace.json".to_string(),
                },
                items: vec![json!({"type": "message", "role": "user", "content": []})],
            }],
            reasoning: None,
        };

        let output = client
            .summarize_input(&input, HeaderMap::new())
            .await
            .expect("summarize input request should succeed");
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].raw_memory, "raw summary");
        assert_eq!(output[0].memory_summary, "memory summary");

        let request = transport
            .last_request
            .lock()
            .expect("lock request store")
            .clone()
            .expect("request should be captured");
        assert_eq!(request.method, Method::POST);
        assert_eq!(
            request.url,
            "https://example.com/api/codex/memories/trace_summarize"
        );
        let body = request.body.expect("request body should be present");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["traces"][0]["id"], "trace-1");
        assert_eq!(
            body["traces"][0]["metadata"]["source_path"],
            "/tmp/trace.json"
        );
    }
}
