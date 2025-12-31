use crate::auth::AuthProvider;
use crate::common::Prompt as ApiPrompt;
use crate::common::Reasoning;
use crate::common::ResponseStream;
use crate::common::TextControls;
use crate::endpoint::streaming::StreamingClient;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::provider::WireApi;
use crate::requests::ResponsesRequest;
use crate::requests::ResponsesRequestBuilder;
use crate::sse::spawn_response_stream;
use crate::telemetry::SseTelemetry;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use serde_json::Value;
use std::sync::Arc;
use tracing::instrument;

pub struct ResponsesClient<T: HttpTransport, A: AuthProvider> {
    pub(crate) streaming: StreamingClient<T, A>,
}

#[derive(Default)]
pub struct ResponsesOptions {
    pub reasoning: Option<Reasoning>,
    pub include: Vec<String>,
    pub prompt_cache_key: Option<String>,
    pub text: Option<TextControls>,
    pub store_override: Option<bool>,
    pub conversation_id: Option<String>,
    pub session_source: Option<SessionSource>,
    pub extra_headers: HeaderMap,
}

impl<T: HttpTransport, A: AuthProvider> ResponsesClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            streaming: StreamingClient::new(transport, provider, auth),
        }
    }

    pub fn with_telemetry(
        self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        Self {
            streaming: self.streaming.with_telemetry(request, sse),
        }
    }

    pub async fn stream_request(
        &self,
        request: ResponsesRequest,
    ) -> Result<ResponseStream, ApiError> {
        self.stream(request.body, request.headers, None).await
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn stream_prompt(
        &self,
        model: &str,
        prompt: &ApiPrompt,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        // Try adapter routing for non-OpenAI providers (ext)
        if let Some(stream) = self.try_adapter(model, prompt).await? {
            return Ok(stream);
        }

        // Built-in OpenAI format
        let ResponsesOptions {
            reasoning,
            include,
            prompt_cache_key,
            text,
            store_override,
            conversation_id,
            session_source,
            extra_headers,
        } = options;

        // Build interceptor context for this request
        let ctx = self
            .streaming
            .build_interceptor_context(Some(model), conversation_id.as_deref());

        let provider = self.streaming.provider();
        let request = ResponsesRequestBuilder::new(model, &prompt.instructions, &prompt.input)
            .tools(&prompt.tools)
            .parallel_tool_calls(prompt.parallel_tool_calls)
            .reasoning(reasoning)
            .include(include)
            .prompt_cache_key(prompt_cache_key)
            .text(text)
            .conversation(conversation_id)
            .session_source(session_source)
            .store_override(store_override)
            .extra_headers(extra_headers)
            .model_parameters(provider.model_parameters.clone())
            .stream(provider.streaming)
            .build(self.streaming.provider())?;

        self.stream(request.body, request.headers, Some(&ctx)).await
    }

    pub(crate) fn path(&self) -> &'static str {
        match self.streaming.provider().wire {
            WireApi::Responses | WireApi::Compact => "responses",
            WireApi::Chat => "chat/completions",
        }
    }

    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
        ctx: Option<&crate::interceptors::InterceptorContext>,
    ) -> Result<ResponseStream, ApiError> {
        self.streaming
            .stream(self.path(), body, extra_headers, ctx, spawn_response_stream)
            .await
    }
}
