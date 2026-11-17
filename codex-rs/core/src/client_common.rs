pub use codex_api::ResponseEvent;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_tools::ToolSpec;
use futures::Stream;
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

/// Core inputs to provider request formatting for a single model turn.
/// The rollout trace serializes this shape before transport-specific rewrites.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Prompt {
    /// Conversation context input items.
    #[serde(serialize_with = "codex_rollout_trace::serialize_trace_items")]
    pub input: Vec<ResponseItem>,

    /// Tool definitions to inject into this request, empty when supplied by history.
    pub(crate) tools: Arc<[ToolSpec]>,

    /// Whether parallel tool calls are permitted for this prompt.
    pub(crate) parallel_tool_calls: bool,

    pub base_instructions: BaseInstructions,

    /// Optional the output schema for the model's response.
    pub output_schema: Option<Value>,

    /// Whether the Responses API should strictly validate `output_schema`.
    pub output_schema_strict: bool,

    pub(crate) prompt_cache_ttl: Option<crate::PromptCacheTtl>,

    pub(crate) cyber_access_program: Option<codex_protocol::turn_input::CyberAccessProgram>,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            tools: Arc::default(),
            parallel_tool_calls: false,
            base_instructions: BaseInstructions::default(),
            output_schema: None,
            output_schema_strict: true,
            prompt_cache_ttl: None,
            cyber_access_program: None,
        }
    }
}

impl Prompt {
    pub(crate) fn get_formatted_input_for_request(
        &self,
        model_info: &ModelInfo,
    ) -> Vec<ResponseItem> {
        let mut input = self.input.clone();
        normalize_image_details(&mut input, model_info);
        input
    }
}

fn normalize_image_details(items: &mut [ResponseItem], model_info: &ModelInfo) {
    for item in items {
        match item {
            ResponseItem::Message { content, .. } => {
                for content_item in content {
                    if let ContentItem::InputImage { detail, .. } = content_item {
                        normalize_image_detail(detail, model_info);
                    }
                }
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content) = output.content_items_mut() {
                    for content_item in content {
                        if let FunctionCallOutputContentItem::InputImage { detail, .. } =
                            content_item
                        {
                            normalize_image_detail(detail, model_info);
                        }
                    }
                }
            }
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::ConfigurationUpdate { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
    }
}

fn normalize_image_detail(detail: &mut Option<ImageDetail>, model_info: &ModelInfo) {
    if model_info.use_responses_lite {
        *detail = None;
    } else if *detail == Some(ImageDetail::Original) && !model_info.supports_image_detail_original {
        *detail = Some(DEFAULT_IMAGE_DETAIL);
    }
}

/// Whether the producer has posted a response's bill and deferred a turn error.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum ChargePosting {
    #[default]
    Unposted,
    Posted,
    PostedTurnExhausted,
}

/// Carries the hook price and the response producer's billing state.
#[derive(Debug, Default)]
pub(crate) struct SamplingCharge {
    pub(crate) hooks: codex_hooks::ModelRequestHookOutcome,
    pub(crate) posting: ChargePosting,
}

/// Core consumes a terminal event and its hook output together. Other readers
/// of the public response stream still see the provider's unmodified event.
#[derive(Debug)]
pub(crate) struct SamplingEvent {
    pub(crate) event: Result<ResponseEvent>,
    pub(crate) hook_output: SamplingCharge,
}

/// The provider's usage and the hook's contribution describe the same
/// response. Keeping them separate preserves the provider's original numbers.
#[derive(Debug)]
pub(crate) struct SamplingUsage {
    pub(crate) provider: Option<codex_protocol::protocol::TokenUsage>,
    pub(crate) charge: SamplingCharge,
}

#[cfg(test)]
impl From<codex_protocol::protocol::TokenUsage> for SamplingUsage {
    fn from(provider: codex_protocol::protocol::TokenUsage) -> Self {
        Self {
            provider: Some(provider),
            charge: Default::default(),
        }
    }
}

impl From<Result<ResponseEvent>> for SamplingEvent {
    fn from(event: Result<ResponseEvent>) -> Self {
        Self {
            event,
            hook_output: Default::default(),
        }
    }
}

/// A sampling consumer receives the hook outcome with the provider event it settles.
pub(crate) struct SamplingStream {
    pub(crate) routing_affinity: Option<codex_http_client::ChatGptRoutingAffinity>,
    pub(crate) rx_event: mpsc::Receiver<SamplingEvent>,
    pub(crate) interrupt: Option<oneshot::Sender<codex_api::ResponseInterrupt>>,
    /// Signals the mapper task that the consumer stopped polling before the
    /// provider stream reached its own terminal event.
    pub(crate) consumer_dropped: CancellationToken,
    /// Once settlement begins, a steer waits for its terminal accounting decision.
    pub(crate) settlement_started: Arc<std::sync::atomic::AtomicBool>,
}

impl SamplingStream {
    /// A cancelled turn discards any queued event, including an incomplete response's usage.
    pub(crate) fn accept_for_turn(
        event: Option<SamplingEvent>,
        cancellation: &CancellationToken,
    ) -> Option<SamplingEvent> {
        if cancellation.is_cancelled() {
            Some(Err(CodexErr::TurnAborted).into())
        } else {
            event
        }
    }

    pub(crate) async fn next_for_turn(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Option<SamplingEvent> {
        let event = tokio::select! {
            biased;
            event = self.rx_event.recv() => event,
            () = cancellation.cancelled() => return Some(Err(CodexErr::TurnAborted).into()),
        };
        Self::accept_for_turn(event, cancellation)
    }
}

impl Stream for SamplingStream {
    type Item = SamplingEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

impl Drop for SamplingStream {
    fn drop(&mut self) {
        self.consumer_dropped.cancel();
    }
}

/// Response events for low-level client requests that do not run sampling hooks.
pub struct ResponseStream(pub(crate) SamplingStream);

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.0)
            .poll_next(cx)
            .map(|event| event.map(|event| event.event))
    }
}

#[cfg(test)]
#[path = "client_common_tests.rs"]
mod tests;
