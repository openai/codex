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
use crate::requests::responses::Compression;
use crate::sse::spawn_response_stream;
use crate::telemetry::SseTelemetry;
use codex_client::HttpTransport;
use codex_client::RequestCompression;
use codex_client::RequestTelemetry;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use serde_json::Value;
use std::sync::Arc;
use std::sync::OnceLock;
use tracing::instrument;

pub struct ResponsesClient<T: HttpTransport, A: AuthProvider> {
    streaming: StreamingClient<T, A>,
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
    pub compression: Compression,
    pub turn_state: Option<Arc<OnceLock<String>>>,
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
        turn_state: Option<Arc<OnceLock<String>>>,
    ) -> Result<ResponseStream, ApiError> {
        self.stream(
            request.body,
            request.headers,
            request.compression,
            turn_state,
        )
        .await
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn stream_prompt(
        &self,
        model: &str,
        prompt: &ApiPrompt,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let ResponsesOptions {
            reasoning,
            include,
            prompt_cache_key,
            text,
            store_override,
            conversation_id,
            session_source,
            extra_headers,
            compression,
            turn_state,
        } = options;

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
            .compression(compression)
            .build(self.streaming.provider())?;

        self.stream_request(request, turn_state).await
    }

    fn path(&self) -> &'static str {
        match self.streaming.provider().wire {
            WireApi::Responses | WireApi::Compact => "responses",
            WireApi::Chat => "chat/completions",
        }
    }

    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
        compression: Compression,
        turn_state: Option<Arc<OnceLock<String>>>,
    ) -> Result<ResponseStream, ApiError> {
        let compression = match compression {
            Compression::None => RequestCompression::None,
            Compression::Zstd => RequestCompression::Zstd,
        };

        self.streaming
            .stream(
                self.path(),
                body,
                extra_headers,
                compression,
                spawn_response_stream,
                turn_state,
            )
            .await
    }
}
