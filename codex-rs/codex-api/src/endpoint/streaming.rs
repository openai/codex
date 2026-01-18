use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::telemetry::SseTelemetry;
use crate::telemetry::run_with_request_telemetry;
use codex_client::HttpTransport;
use codex_client::RequestCompression;
use codex_client::RequestTelemetry;
use codex_client::StreamResponse;
use http::HeaderMap;
use http::Method;
use serde_json::Value;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::debug;

pub(crate) struct StreamingClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

type StreamSpawner = fn(
    StreamResponse,
    Duration,
    Option<Arc<dyn SseTelemetry>>,
    Option<Arc<OnceLock<String>>>,
) -> ResponseStream;

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
        compression: RequestCompression,
        spawner: StreamSpawner,
        turn_state: Option<Arc<OnceLock<String>>>,
    ) -> Result<ResponseStream, ApiError> {
        // Bedrock uses non-streaming invoke endpoint with JSON response
        let is_bedrock = self.provider.is_claude_provider();

        // Log request details for debugging
        if is_bedrock {
            debug!("=== Bedrock Request ===");
            debug!("Path: {}", path);
            debug!("max_tokens: {:?}", body.get("max_tokens"));
            debug!("tool_choice: {:?}", body.get("tool_choice"));
            let tools = body.get("tools").and_then(|t| t.as_array());
            debug!("tools count: {:?}", tools.map(std::vec::Vec::len));
            debug!("=== End Bedrock Request ===");
        }

        let builder = || {
            let mut req = self.provider.build_request(Method::POST, path);
            req.headers.extend(extra_headers.clone());

            // Bedrock streaming uses AWS Event Stream format
            let accept = if is_bedrock {
                "application/vnd.amazon.eventstream"
            } else {
                "text/event-stream"
            };
            req.headers
                .insert(http::header::ACCEPT, http::HeaderValue::from_static(accept));
            req.body = Some(body.clone());
            req.compression = compression;
            add_auth_headers(&self.auth, req)
        };

        let stream_response = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            builder,
            |req| self.transport.stream(req),
        )
        .await?;

        // For Bedrock, use the streaming event stream handler
        if is_bedrock {
            return Ok(crate::sse::bedrock_stream::spawn_bedrock_stream(
                stream_response,
            ));
        }

        Ok(spawner(
            stream_response,
            self.provider.stream_idle_timeout,
            self.sse_telemetry.clone(),
            turn_state,
        ))
    }
}
