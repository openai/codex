use crate::byte_len;
use crate::events::AppServerRpcTransport;
use crate::events::CodexRuntimeMetadata;
use crate::events::GuardianReviewEventParams;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone)]
pub struct TrackEventsContext {
    pub model_slug: String,
    pub thread_id: String,
    pub turn_id: String,
}

pub fn build_track_events_context(
    model_slug: String,
    thread_id: String,
    turn_id: String,
) -> TrackEventsContext {
    TrackEventsContext {
        model_slug,
        thread_id,
        turn_id,
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSubmissionType {
    Default,
    Queued,
}

#[derive(Clone)]
pub struct TurnResolvedConfigFact {
    pub turn_id: String,
    pub thread_id: String,
    pub num_input_images: usize,
    pub submission_type: Option<TurnSubmissionType>,
    pub ephemeral: bool,
    pub session_source: SessionSource,
    pub model: String,
    pub model_provider: String,
    pub sandbox_policy: SandboxPolicy,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub service_tier: Option<ServiceTier>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox_network_access: bool,
    pub collaboration_mode: ModeKind,
    pub personality: Option<Personality>,
    pub is_first_turn: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadInitializationMode {
    New,
    Forked,
    Resumed,
}

#[derive(Clone)]
pub struct TurnTokenUsageFact {
    pub turn_id: String,
    pub thread_id: String,
    pub token_usage: TokenUsage,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSteerResult {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSteerRejectionReason {
    NoActiveTurn,
    ExpectedTurnMismatch,
    NonSteerableReview,
    NonSteerableCompact,
    EmptyInput,
    InputTooLarge,
}

#[derive(Clone)]
pub struct CodexTurnSteerEvent {
    pub expected_turn_id: Option<String>,
    pub accepted_turn_id: Option<String>,
    pub num_input_images: usize,
    pub result: TurnSteerResult,
    pub rejection_reason: Option<TurnSteerRejectionReason>,
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum AnalyticsJsonRpcError {
    TurnSteer(TurnSteerRequestError),
    Input(InputError),
}

#[derive(Clone, Copy, Debug)]
pub enum TurnSteerRequestError {
    NoActiveTurn,
    ExpectedTurnMismatch,
    NonSteerableReview,
    NonSteerableCompact,
}

#[derive(Clone, Copy, Debug)]
pub enum InputError {
    Empty,
    TooLarge,
}

impl From<TurnSteerRequestError> for TurnSteerRejectionReason {
    fn from(error: TurnSteerRequestError) -> Self {
        match error {
            TurnSteerRequestError::NoActiveTurn => Self::NoActiveTurn,
            TurnSteerRequestError::ExpectedTurnMismatch => Self::ExpectedTurnMismatch,
            TurnSteerRequestError::NonSteerableReview => Self::NonSteerableReview,
            TurnSteerRequestError::NonSteerableCompact => Self::NonSteerableCompact,
        }
    }
}

impl From<InputError> for TurnSteerRejectionReason {
    fn from(error: InputError) -> Self {
        match error {
            InputError::Empty => Self::EmptyInput,
            InputError::TooLarge => Self::InputTooLarge,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkillInvocation {
    pub skill_name: String,
    pub skill_scope: SkillScope,
    pub skill_path: PathBuf,
    pub invocation_type: InvocationType,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvocationType {
    Explicit,
    Implicit,
}

pub struct AppInvocation {
    pub connector_id: Option<String>,
    pub app_name: Option<String>,
    pub invocation_type: Option<InvocationType>,
}

#[derive(Clone)]
pub struct SubAgentThreadStartedInput {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub product_client_id: String,
    pub client_name: String,
    pub client_version: String,
    pub model: String,
    pub ephemeral: bool,
    pub subagent_source: SubAgentSource,
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    Manual,
    Auto,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionReason {
    UserRequested,
    ContextLimit,
    ModelDownshift,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionImplementation {
    Responses,
    ResponsesCompact,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionPhase {
    StandaloneTurn,
    PreTurn,
    MidTurn,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    Memento,
    PrefixCompaction,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone)]
pub struct CodexCompactionEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub trigger: CompactionTrigger,
    pub reason: CompactionReason,
    pub implementation: CompactionImplementation,
    pub phase: CompactionPhase,
    pub strategy: CompactionStrategy,
    pub status: CompactionStatus,
    pub error: Option<String>,
    pub active_context_tokens_before: i64,
    pub active_context_tokens_after: i64,
    pub started_at: u64,
    pub completed_at: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexResponsesApiCallStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodexResponsesApiItemPhase {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexResponseItemType {
    Message,
    Reasoning,
    LocalShellCall,
    FunctionCall,
    FunctionCallOutput,
    CustomToolCall,
    CustomToolCallOutput,
    ToolSearchCall,
    ToolSearchOutput,
    WebSearchCall,
    ImageGenerationCall,
    GhostSnapshot,
    Compaction,
    Other,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CodexResponsesApiItemMetadata {
    pub item_phase: CodexResponsesApiItemPhase,
    pub item_index: usize,
    pub response_item_type: CodexResponseItemType,
    pub role: Option<String>,
    pub status: Option<String>,
    pub message_phase: Option<MessagePhase>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub payload_bytes: Option<i64>,
    pub text_part_count: Option<usize>,
    pub image_part_count: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct CodexResponsesApiCallFact {
    pub thread_id: String,
    pub turn_id: String,
    pub responses_id: Option<String>,
    pub turn_responses_call_index: u64,
    pub status: CodexResponsesApiCallStatus,
    pub error: Option<String>,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub input_item_count: usize,
    pub output_item_count: usize,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub items: Vec<CodexResponsesApiItemMetadata>,
}

pub fn response_items_metadata(
    phase: CodexResponsesApiItemPhase,
    items: &[ResponseItem],
) -> Vec<CodexResponsesApiItemMetadata> {
    items
        .iter()
        .enumerate()
        .map(|(item_index, item)| response_item_metadata(phase, item_index, item))
        .collect()
}

fn response_item_metadata(
    item_phase: CodexResponsesApiItemPhase,
    item_index: usize,
    item: &ResponseItem,
) -> CodexResponsesApiItemMetadata {
    let mut metadata =
        CodexResponsesApiItemMetadata::new(item_phase, item_index, response_item_type(item));

    match item {
        ResponseItem::Message {
            role,
            content,
            phase,
            ..
        } => {
            metadata.role = Some(role.clone());
            metadata.message_phase = phase.clone();
            metadata.payload_bytes = nonzero_i64(message_content_text_bytes(content));
            let (text_part_count, image_part_count) = message_content_part_counts(content);
            metadata.text_part_count = Some(text_part_count);
            metadata.image_part_count = Some(image_part_count);
        }
        ResponseItem::Reasoning {
            summary,
            content,
            encrypted_content,
            ..
        } => {
            metadata.payload_bytes = encrypted_content
                .as_ref()
                .map(|value| byte_len(value))
                .or_else(|| nonzero_i64(reasoning_content_bytes(summary, content)));
            metadata.text_part_count =
                Some(summary.len() + content.as_ref().map(std::vec::Vec::len).unwrap_or_default());
            metadata.image_part_count = Some(0);
        }
        ResponseItem::LocalShellCall {
            call_id,
            status,
            action,
            ..
        } => {
            metadata.call_id = call_id.clone();
            metadata.tool_name = Some("local_shell".to_string());
            metadata.status = serialized_string(status);
            metadata.payload_bytes = serialized_bytes(action);
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => {
            metadata.call_id = Some(call_id.clone());
            metadata.tool_name = Some(name.clone());
            metadata.payload_bytes = Some(byte_len(arguments));
        }
        ResponseItem::ToolSearchCall {
            call_id,
            status,
            arguments,
            ..
        } => {
            metadata.call_id = call_id.clone();
            metadata.tool_name = Some("tool_search".to_string());
            metadata.status = status.clone();
            metadata.payload_bytes = serialized_bytes(arguments);
        }
        ResponseItem::FunctionCallOutput { call_id, output } => {
            metadata.call_id = Some(call_id.clone());
            metadata.payload_bytes = function_call_output_bytes(output);
            let (text_part_count, image_part_count) = function_call_output_part_counts(output);
            metadata.text_part_count = text_part_count;
            metadata.image_part_count = image_part_count;
        }
        ResponseItem::CustomToolCall {
            status,
            call_id,
            name,
            input,
            ..
        } => {
            metadata.call_id = Some(call_id.clone());
            metadata.tool_name = Some(name.clone());
            metadata.status = status.clone();
            metadata.payload_bytes = Some(byte_len(input));
        }
        ResponseItem::CustomToolCallOutput {
            call_id,
            name,
            output,
        } => {
            metadata.call_id = Some(call_id.clone());
            metadata.tool_name = name.clone();
            metadata.payload_bytes = function_call_output_bytes(output);
            let (text_part_count, image_part_count) = function_call_output_part_counts(output);
            metadata.text_part_count = text_part_count;
            metadata.image_part_count = image_part_count;
        }
        ResponseItem::ToolSearchOutput {
            call_id,
            status,
            tools,
            ..
        } => {
            metadata.call_id = call_id.clone();
            metadata.tool_name = Some("tool_search".to_string());
            metadata.status = Some(status.clone());
            metadata.payload_bytes = serialized_bytes(tools);
        }
        ResponseItem::WebSearchCall { status, action, .. } => {
            metadata.tool_name = Some("web_search".to_string());
            metadata.status = status.clone();
            metadata.payload_bytes = action.as_ref().and_then(serialized_bytes);
        }
        ResponseItem::ImageGenerationCall {
            id,
            status,
            revised_prompt,
            result,
        } => {
            metadata.call_id = Some(id.clone());
            metadata.tool_name = Some("image_generation".to_string());
            metadata.status = Some(status.clone());
            metadata.payload_bytes = nonzero_i64(byte_len(result))
                .or_else(|| revised_prompt.as_ref().map(|value| byte_len(value)));
        }
        ResponseItem::Compaction { encrypted_content } => {
            metadata.payload_bytes = Some(byte_len(encrypted_content));
        }
        ResponseItem::GhostSnapshot { .. } | ResponseItem::Other => {}
    }

    metadata
}

impl CodexResponsesApiItemMetadata {
    fn new(
        item_phase: CodexResponsesApiItemPhase,
        item_index: usize,
        response_item_type: CodexResponseItemType,
    ) -> Self {
        Self {
            item_phase,
            item_index,
            response_item_type,
            role: None,
            status: None,
            message_phase: None,
            call_id: None,
            tool_name: None,
            payload_bytes: None,
            text_part_count: None,
            image_part_count: None,
        }
    }
}

fn response_item_type(item: &ResponseItem) -> CodexResponseItemType {
    match item {
        ResponseItem::Message { .. } => CodexResponseItemType::Message,
        ResponseItem::Reasoning { .. } => CodexResponseItemType::Reasoning,
        ResponseItem::LocalShellCall { .. } => CodexResponseItemType::LocalShellCall,
        ResponseItem::FunctionCall { .. } => CodexResponseItemType::FunctionCall,
        ResponseItem::ToolSearchCall { .. } => CodexResponseItemType::ToolSearchCall,
        ResponseItem::FunctionCallOutput { .. } => CodexResponseItemType::FunctionCallOutput,
        ResponseItem::CustomToolCall { .. } => CodexResponseItemType::CustomToolCall,
        ResponseItem::CustomToolCallOutput { .. } => CodexResponseItemType::CustomToolCallOutput,
        ResponseItem::ToolSearchOutput { .. } => CodexResponseItemType::ToolSearchOutput,
        ResponseItem::WebSearchCall { .. } => CodexResponseItemType::WebSearchCall,
        ResponseItem::ImageGenerationCall { .. } => CodexResponseItemType::ImageGenerationCall,
        ResponseItem::GhostSnapshot { .. } => CodexResponseItemType::GhostSnapshot,
        ResponseItem::Compaction { .. } => CodexResponseItemType::Compaction,
        ResponseItem::Other => CodexResponseItemType::Other,
    }
}

fn message_content_text_bytes(content: &[ContentItem]) -> i64 {
    content
        .iter()
        .map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => byte_len(text),
            ContentItem::InputImage { .. } => 0,
        })
        .sum()
}

fn message_content_part_counts(content: &[ContentItem]) -> (usize, usize) {
    let mut text_part_count = 0;
    let mut image_part_count = 0;
    for item in content {
        match item {
            ContentItem::InputText { .. } | ContentItem::OutputText { .. } => {
                text_part_count += 1;
            }
            ContentItem::InputImage { .. } => {
                image_part_count += 1;
            }
        }
    }
    (text_part_count, image_part_count)
}

fn reasoning_content_bytes(
    summary: &[ReasoningItemReasoningSummary],
    content: &Option<Vec<ReasoningItemContent>>,
) -> i64 {
    let summary_bytes = summary
        .iter()
        .map(|summary| match summary {
            ReasoningItemReasoningSummary::SummaryText { text } => byte_len(text),
        })
        .sum::<i64>();
    let content_bytes = content
        .as_ref()
        .map(|content| {
            content
                .iter()
                .map(|content| match content {
                    ReasoningItemContent::ReasoningText { text }
                    | ReasoningItemContent::Text { text } => byte_len(text),
                })
                .sum::<i64>()
        })
        .unwrap_or_default();
    summary_bytes + content_bytes
}

fn function_call_output_bytes(output: &FunctionCallOutputPayload) -> Option<i64> {
    match &output.body {
        FunctionCallOutputBody::Text(text) => Some(byte_len(text)),
        FunctionCallOutputBody::ContentItems(items) => serialized_bytes(items),
    }
}

fn function_call_output_part_counts(
    output: &FunctionCallOutputPayload,
) -> (Option<usize>, Option<usize>) {
    let Some(content_items) = output.content_items() else {
        return (None, None);
    };
    let mut text_part_count = 0;
    let mut image_part_count = 0;
    for item in content_items {
        match item {
            FunctionCallOutputContentItem::InputText { .. } => {
                text_part_count += 1;
            }
            FunctionCallOutputContentItem::InputImage { .. } => {
                image_part_count += 1;
            }
        }
    }
    (Some(text_part_count), Some(image_part_count))
}

#[allow(dead_code)]
pub(crate) enum AnalyticsFact {
    Initialize {
        connection_id: u64,
        params: InitializeParams,
        product_client_id: String,
        runtime: CodexRuntimeMetadata,
        rpc_transport: AppServerRpcTransport,
    },
    Request {
        connection_id: u64,
        request_id: RequestId,
        request: Box<ClientRequest>,
    },
    Response {
        connection_id: u64,
        response: Box<ClientResponse>,
    },
    ErrorResponse {
        connection_id: u64,
        request_id: RequestId,
        error: JSONRPCErrorError,
        error_type: Option<AnalyticsJsonRpcError>,
    },
    Notification(Box<ServerNotification>),
    // Facts that do not naturally exist on the app-server protocol surface, or
    // would require non-trivial protocol reshaping on this branch.
    Custom(CustomAnalyticsFact),
}

pub(crate) enum CustomAnalyticsFact {
    SubAgentThreadStarted(SubAgentThreadStartedInput),
    Compaction(Box<CodexCompactionEvent>),
    GuardianReview(Box<GuardianReviewEventParams>),
    TurnResolvedConfig(Box<TurnResolvedConfigFact>),
    TurnTokenUsage(Box<TurnTokenUsageFact>),
    SkillInvoked(SkillInvokedInput),
    AppMentioned(AppMentionedInput),
    AppUsed(AppUsedInput),
    HookRun(HookRunInput),
    PluginUsed(PluginUsedInput),
    PluginStateChanged(PluginStateChangedInput),
}

pub(crate) struct SkillInvokedInput {
    pub tracking: TrackEventsContext,
    pub invocations: Vec<SkillInvocation>,
}

pub(crate) struct AppMentionedInput {
    pub tracking: TrackEventsContext,
    pub mentions: Vec<AppInvocation>,
}

pub(crate) struct AppUsedInput {
    pub tracking: TrackEventsContext,
    pub app: AppInvocation,
}

pub(crate) struct HookRunInput {
    pub tracking: TrackEventsContext,
    pub hook: HookRunFact,
}

pub struct HookRunFact {
    pub event_name: HookEventName,
    pub hook_source: HookSource,
    pub status: HookRunStatus,
}

pub(crate) struct PluginUsedInput {
    pub tracking: TrackEventsContext,
    pub plugin: PluginTelemetryMetadata,
}

pub(crate) struct PluginStateChangedInput {
    pub plugin: PluginTelemetryMetadata,
    pub state: PluginState,
}

#[derive(Clone, Copy)]
pub(crate) enum PluginState {
    Installed,
    Uninstalled,
    Enabled,
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ImageDetail;

    #[test]
    fn maps_message_metadata() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![
                ContentItem::OutputText {
                    text: "hello".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,abc".to_string(),
                    detail: None,
                },
            ],
            end_turn: None,
            phase: Some(MessagePhase::FinalAnswer),
        }];

        let metadata = response_items_metadata(CodexResponsesApiItemPhase::Output, &items);

        assert_eq!(metadata[0].item_phase, CodexResponsesApiItemPhase::Output);
        assert_eq!(
            metadata[0].response_item_type,
            CodexResponseItemType::Message
        );
        assert_eq!(metadata[0].role.as_deref(), Some("assistant"));
        assert_eq!(metadata[0].message_phase, Some(MessagePhase::FinalAnswer));
        assert_eq!(metadata[0].payload_bytes, Some(5));
        assert_eq!(metadata[0].text_part_count, Some(1));
        assert_eq!(metadata[0].image_part_count, Some(1));
    }

    #[test]
    fn maps_tool_call_output_metadata() {
        let items = vec![ResponseItem::CustomToolCallOutput {
            call_id: "call_1".to_string(),
            name: Some("custom_tool".to_string()),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "result".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "https://example.test/image.png".to_string(),
                    detail: Some(ImageDetail::High),
                },
            ]),
        }];

        let metadata = response_items_metadata(CodexResponsesApiItemPhase::Output, &items);

        assert_eq!(
            metadata[0].response_item_type,
            CodexResponseItemType::CustomToolCallOutput
        );
        assert_eq!(metadata[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(metadata[0].tool_name.as_deref(), Some("custom_tool"));
        assert!(metadata[0].payload_bytes.unwrap_or_default() > 0);
        assert_eq!(metadata[0].text_part_count, Some(1));
        assert_eq!(metadata[0].image_part_count, Some(1));
    }
}
