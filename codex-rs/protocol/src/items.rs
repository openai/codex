use crate::AgentPath;
use crate::ThreadId;
use crate::dynamic_tools::DynamicToolCallOutputContentItem;
use crate::dynamic_tools::DynamicToolCallRequest;
use crate::mcp::CallToolResult;
use crate::memory_citation::MemoryCitation;
use crate::models::ContentItem;
use crate::models::ImageDetail;
use crate::models::MessagePhase;
use crate::models::ResponseItem;
use crate::models::WebSearchAction;
use crate::openai_models::ReasoningEffort as ReasoningEffortConfig;
use crate::parse_command::ParsedCommand;
use crate::protocol::AgentMessageEvent;
use crate::protocol::AgentReasoningEvent;
use crate::protocol::AgentReasoningRawContentEvent;
use crate::protocol::AgentStatus;
use crate::protocol::CollabAgentInteractionBeginEvent;
use crate::protocol::CollabAgentInteractionEndEvent;
use crate::protocol::CollabAgentRef;
use crate::protocol::CollabAgentSpawnBeginEvent;
use crate::protocol::CollabAgentSpawnEndEvent;
use crate::protocol::CollabAgentStatusEntry;
use crate::protocol::CollabCloseBeginEvent;
use crate::protocol::CollabCloseEndEvent;
use crate::protocol::CollabResumeBeginEvent;
use crate::protocol::CollabResumeEndEvent;
use crate::protocol::CollabWaitingBeginEvent;
use crate::protocol::CollabWaitingEndEvent;
use crate::protocol::ContextCompactedEvent;
use crate::protocol::DynamicToolCallResponseEvent;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::ExecCommandSource;
use crate::protocol::ExecCommandStatus;
use crate::protocol::FileChange;
use crate::protocol::ImageGenerationEndEvent;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
use crate::protocol::PatchApplyStatus;
use crate::protocol::SubAgentActivityEvent;
use crate::protocol::SubAgentActivityKind;
use crate::protocol::UserMessageEvent;
use crate::protocol::ViewImageToolCallEvent;
use crate::protocol::WebSearchEndEvent;
use crate::user_input::ByteRange;
use crate::user_input::TextElement;
use crate::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use quick_xml::de::from_str as from_xml_str;
use quick_xml::se::to_string as to_xml_string;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use ts_rs::TS;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
#[serde(tag = "type")]
#[ts(tag = "type")]
pub enum TurnItem {
    UserMessage(UserMessageItem),
    HookPrompt(HookPromptItem),
    AgentMessage(AgentMessageItem),
    Plan(PlanItem),
    Reasoning(ReasoningItem),
    CommandExecution(CommandExecutionItem),
    DynamicToolCall(DynamicToolCallItem),
    CollabAgentToolCall(CollabAgentToolCallItem),
    SubAgentActivity(SubAgentActivityItem),
    WebSearch(WebSearchItem),
    ImageView(ImageViewItem),
    Sleep(SleepItem),
    ImageGeneration(ImageGenerationItem),
    FileChange(FileChangeItem),
    McpToolCall(McpToolCallItem),
    ContextCompaction(ContextCompactionItem),
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct UserMessageItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub client_id: Option<String>,
    pub content: Vec<UserInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
pub struct HookPromptItem {
    pub id: String,
    pub fragments: Vec<HookPromptFragment>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct HookPromptFragment {
    pub text: String,
    pub hook_run_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "hook_prompt")]
struct HookPromptXml {
    #[serde(rename = "@hook_run_id")]
    hook_run_id: String,
    #[serde(rename = "$text")]
    text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
#[serde(tag = "type")]
#[ts(tag = "type")]
pub enum AgentMessageContent {
    Text { text: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
/// Assistant-authored message payload used in turn-item streams.
///
/// `phase` is optional because not all providers/models emit it. Consumers
/// should use it when present, but retain legacy completion semantics when it
/// is `None`.
pub struct AgentMessageItem {
    pub id: String,
    pub content: Vec<AgentMessageContent>,
    /// Optional phase metadata carried through from `ResponseItem::Message`.
    ///
    /// This is currently used by TUI rendering to distinguish mid-turn
    /// commentary from a final answer and avoid status-indicator jitter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub phase: Option<MessagePhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub memory_citation: Option<MemoryCitation>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct PlanItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct ReasoningItem {
    pub id: String,
    pub summary_text: Vec<String>,
    #[serde(default)]
    pub raw_content: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum CommandExecutionStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

impl From<ExecCommandStatus> for CommandExecutionStatus {
    fn from(value: ExecCommandStatus) -> Self {
        match value {
            ExecCommandStatus::Completed => Self::Completed,
            ExecCommandStatus::Failed => Self::Failed,
            ExecCommandStatus::Declined => Self::Declined,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct CommandExecutionItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub process_id: Option<String>,
    pub command: Vec<String>,
    pub cwd: PathUri,
    pub parsed_cmd: Vec<ParsedCommand>,
    pub source: ExecCommandSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub interaction_input: Option<String>,
    pub status: CommandExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub aggregated_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "string", optional)]
    pub duration: Option<Duration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub formatted_output: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum DynamicToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct DynamicToolCallItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: serde_json::Value,
    pub status: DynamicToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub content_items: Option<Vec<DynamicToolCallOutputContentItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub success: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "string", optional)]
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum CollabAgentTool {
    SpawnAgent,
    SendInput,
    ResumeAgent,
    Wait,
    CloseAgent,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum CollabAgentToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct CollabAgentToolCallItem {
    pub id: String,
    pub tool: CollabAgentTool,
    pub status: CollabAgentToolCallStatus,
    pub sender_thread_id: ThreadId,
    #[serde(default)]
    pub receiver_thread_ids: Vec<ThreadId>,
    #[serde(default)]
    pub receiver_agents: Vec<CollabAgentRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reasoning_effort: Option<ReasoningEffortConfig>,
    #[serde(default)]
    pub agents_states: HashMap<ThreadId, AgentStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct SubAgentActivityItem {
    pub id: String,
    pub kind: SubAgentActivityKind,
    pub agent_thread_id: ThreadId,
    pub agent_path: AgentPath,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct WebSearchItem {
    pub id: String,
    pub query: String,
    pub action: WebSearchAction,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ImageViewItem {
    pub id: String,
    /// Path resolved within the selected execution environment.
    ///
    /// This core protocol type is not exposed directly in the app-server API.
    /// App-server converts the path to `LegacyAppPathString` at its boundary.
    pub path: PathUri,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
pub struct SleepItem {
    pub id: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ImageGenerationItem {
    pub id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub revised_prompt: Option<String>,
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub saved_path: Option<AbsolutePathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct FileChangeItem {
    pub id: String,
    pub changes: HashMap<PathBuf, FileChange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<PatchApplyStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auto_approved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct McpToolCallItem {
    pub id: String,
    pub server: String,
    pub tool: String,
    pub arguments: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mcp_app_resource_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub link_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub app_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plugin_id: Option<String>,
    pub status: McpToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result: Option<CallToolResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<McpToolCallError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "string", optional)]
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum McpToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct McpToolCallError {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct ContextCompactionItem {
    pub id: String,
}

impl ContextCompactionItem {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::ContextCompacted(ContextCompactedEvent {})
    }
}

impl Default for ContextCompactionItem {
    fn default() -> Self {
        Self::new()
    }
}

impl UserMessageItem {
    pub fn new(content: &[UserInput]) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            client_id: None,
            content: content.to_vec(),
        }
    }

    pub fn as_legacy_event(&self) -> EventMsg {
        // Legacy user-message events flatten only text inputs into `message` and
        // rebase text element ranges onto that concatenated text.
        EventMsg::UserMessage(UserMessageEvent {
            client_id: self.client_id.clone(),
            message: self.message(),
            images: Some(self.image_urls()),
            image_details: self.image_details(),
            local_images: self.local_image_paths(),
            local_image_details: self.local_image_details(),
            text_elements: self.text_elements(),
        })
    }

    pub fn message(&self) -> String {
        self.content
            .iter()
            .map(|c| match c {
                UserInput::Text { text, .. } => text.clone(),
                _ => String::new(),
            })
            .collect::<Vec<String>>()
            .join("")
    }

    pub fn text_elements(&self) -> Vec<TextElement> {
        let mut out = Vec::new();
        let mut offset = 0usize;
        for input in &self.content {
            if let UserInput::Text {
                text,
                text_elements,
                ..
            } = input
            {
                // Text element ranges are relative to each text chunk; offset them so they align
                // with the concatenated message returned by `message()`.
                for elem in text_elements {
                    let byte_range = ByteRange {
                        start: offset + elem.byte_range.start,
                        end: offset + elem.byte_range.end,
                    };
                    out.push(TextElement::new(
                        byte_range,
                        elem.placeholder(text).map(str::to_string),
                    ));
                }
                offset += text.len();
            }
        }
        out
    }

    pub fn image_urls(&self) -> Vec<String> {
        self.content
            .iter()
            .filter_map(|c| match c {
                UserInput::Image { image_url, .. } => Some(image_url.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn image_details(&self) -> Vec<Option<ImageDetail>> {
        trim_trailing_default_image_details(
            self.content
                .iter()
                .filter_map(|c| match c {
                    UserInput::Image { detail, .. } => Some(*detail),
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn local_image_paths(&self) -> Vec<std::path::PathBuf> {
        self.content
            .iter()
            .filter_map(|c| match c {
                UserInput::LocalImage { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn local_image_details(&self) -> Vec<Option<ImageDetail>> {
        trim_trailing_default_image_details(
            self.content
                .iter()
                .filter_map(|c| match c {
                    UserInput::LocalImage { detail, .. } => Some(*detail),
                    _ => None,
                })
                .collect(),
        )
    }
}

fn trim_trailing_default_image_details(
    mut details: Vec<Option<ImageDetail>>,
) -> Vec<Option<ImageDetail>> {
    while matches!(details.last(), Some(None)) {
        details.pop();
    }
    details
}

impl HookPromptItem {
    pub fn from_fragments(id: Option<&String>, fragments: Vec<HookPromptFragment>) -> Self {
        Self {
            id: id
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            fragments,
        }
    }
}

impl HookPromptFragment {
    pub fn from_single_hook(text: impl Into<String>, hook_run_id: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            hook_run_id: hook_run_id.into(),
        }
    }
}

pub fn build_hook_prompt_message(fragments: &[HookPromptFragment]) -> Option<ResponseItem> {
    let content = fragments
        .iter()
        .filter(|fragment| !fragment.hook_run_id.trim().is_empty())
        .filter_map(|fragment| {
            serialize_hook_prompt_fragment(&fragment.text, &fragment.hook_run_id)
                .map(|text| ContentItem::InputText { text })
        })
        .collect::<Vec<_>>();

    if content.is_empty() {
        return None;
    }

    Some(ResponseItem::Message {
        id: Some(uuid::Uuid::new_v4().to_string()),
        role: "user".to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
}

pub fn parse_hook_prompt_message(
    id: Option<&String>,
    content: &[ContentItem],
) -> Option<HookPromptItem> {
    let fragments = content
        .iter()
        .map(|content_item| {
            let ContentItem::InputText { text } = content_item else {
                return None;
            };
            parse_hook_prompt_fragment(text)
        })
        .collect::<Option<Vec<_>>>()?;

    if fragments.is_empty() {
        return None;
    }

    Some(HookPromptItem::from_fragments(id, fragments))
}

pub fn parse_hook_prompt_fragment(text: &str) -> Option<HookPromptFragment> {
    let trimmed = text.trim();
    let HookPromptXml { text, hook_run_id } = from_xml_str::<HookPromptXml>(trimmed).ok()?;
    if hook_run_id.trim().is_empty() {
        return None;
    }

    Some(HookPromptFragment { text, hook_run_id })
}

fn serialize_hook_prompt_fragment(text: &str, hook_run_id: &str) -> Option<String> {
    if hook_run_id.trim().is_empty() {
        return None;
    }
    to_xml_string(&HookPromptXml {
        text: text.to_string(),
        hook_run_id: hook_run_id.to_string(),
    })
    .ok()
}

impl AgentMessageItem {
    pub fn new(content: &[AgentMessageContent]) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.to_vec(),
            phase: None,
            memory_citation: None,
        }
    }

    pub fn as_legacy_events(&self) -> Vec<EventMsg> {
        self.content
            .iter()
            .map(|c| match c {
                AgentMessageContent::Text { text } => EventMsg::AgentMessage(AgentMessageEvent {
                    message: text.clone(),
                    phase: self.phase.clone(),
                    memory_citation: self.memory_citation.clone(),
                }),
            })
            .collect()
    }
}

impl ReasoningItem {
    pub fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        let mut events = Vec::new();
        for summary in &self.summary_text {
            events.push(EventMsg::AgentReasoning(AgentReasoningEvent {
                text: summary.clone(),
            }));
        }

        if show_raw_agent_reasoning {
            for entry in &self.raw_content {
                events.push(EventMsg::AgentReasoningRawContent(
                    AgentReasoningRawContentEvent {
                        text: entry.clone(),
                    },
                ));
            }
        }

        events
    }
}

impl CommandExecutionItem {
    pub fn from_exec_command_begin_event(event: ExecCommandBeginEvent) -> Self {
        Self {
            id: event.call_id,
            process_id: event.process_id,
            command: event.command,
            cwd: event.cwd,
            parsed_cmd: event.parsed_cmd,
            source: event.source,
            interaction_input: event.interaction_input,
            status: CommandExecutionStatus::InProgress,
            stdout: None,
            stderr: None,
            aggregated_output: None,
            exit_code: None,
            duration: None,
            formatted_output: None,
        }
    }

    pub fn from_exec_command_end_event(event: ExecCommandEndEvent) -> Self {
        Self {
            id: event.call_id,
            process_id: event.process_id,
            command: event.command,
            cwd: event.cwd,
            parsed_cmd: event.parsed_cmd,
            source: event.source,
            interaction_input: event.interaction_input,
            status: event.status.into(),
            stdout: Some(event.stdout),
            stderr: Some(event.stderr),
            aggregated_output: Some(event.aggregated_output),
            exit_code: Some(event.exit_code),
            duration: Some(event.duration),
            formatted_output: Some(event.formatted_output),
        }
    }

    pub fn as_legacy_begin_event(&self, turn_id: String, started_at_ms: i64) -> EventMsg {
        EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: self.id.clone(),
            process_id: self.process_id.clone(),
            turn_id,
            started_at_ms,
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            parsed_cmd: self.parsed_cmd.clone(),
            source: self.source,
            interaction_input: self.interaction_input.clone(),
        })
    }

    pub fn as_legacy_end_event(&self, turn_id: String, completed_at_ms: i64) -> Option<EventMsg> {
        let status = match self.status {
            CommandExecutionStatus::InProgress => return None,
            CommandExecutionStatus::Completed => ExecCommandStatus::Completed,
            CommandExecutionStatus::Failed => ExecCommandStatus::Failed,
            CommandExecutionStatus::Declined => ExecCommandStatus::Declined,
        };
        Some(EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: self.id.clone(),
            process_id: self.process_id.clone(),
            turn_id,
            completed_at_ms,
            command: self.command.clone(),
            cwd: self.cwd.clone(),
            parsed_cmd: self.parsed_cmd.clone(),
            source: self.source,
            interaction_input: self.interaction_input.clone(),
            stdout: self.stdout.clone().unwrap_or_default(),
            stderr: self.stderr.clone().unwrap_or_default(),
            aggregated_output: self.aggregated_output.clone().unwrap_or_default(),
            exit_code: self.exit_code.unwrap_or_default(),
            duration: self.duration.unwrap_or_default(),
            formatted_output: self.formatted_output.clone().unwrap_or_default(),
            status,
        }))
    }
}

impl DynamicToolCallItem {
    pub fn from_dynamic_tool_call_request(event: DynamicToolCallRequest) -> Self {
        Self {
            id: event.call_id,
            namespace: event.namespace,
            tool: event.tool,
            arguments: event.arguments,
            status: DynamicToolCallStatus::InProgress,
            content_items: None,
            success: None,
            error: None,
            duration: None,
        }
    }

    pub fn from_dynamic_tool_call_response(event: DynamicToolCallResponseEvent) -> Self {
        Self {
            id: event.call_id,
            namespace: event.namespace,
            tool: event.tool,
            arguments: event.arguments,
            status: if event.success {
                DynamicToolCallStatus::Completed
            } else {
                DynamicToolCallStatus::Failed
            },
            content_items: Some(event.content_items),
            success: Some(event.success),
            error: event.error,
            duration: Some(event.duration),
        }
    }

    pub fn as_legacy_request_event(&self, turn_id: String, started_at_ms: i64) -> EventMsg {
        EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
            call_id: self.id.clone(),
            turn_id,
            started_at_ms,
            namespace: self.namespace.clone(),
            tool: self.tool.clone(),
            arguments: self.arguments.clone(),
        })
    }

    pub fn as_legacy_response_event(
        &self,
        turn_id: String,
        completed_at_ms: i64,
    ) -> Option<EventMsg> {
        if matches!(self.status, DynamicToolCallStatus::InProgress) {
            return None;
        }
        Some(EventMsg::DynamicToolCallResponse(
            DynamicToolCallResponseEvent {
                call_id: self.id.clone(),
                turn_id,
                completed_at_ms,
                namespace: self.namespace.clone(),
                tool: self.tool.clone(),
                arguments: self.arguments.clone(),
                content_items: self.content_items.clone().unwrap_or_default(),
                success: self.success.unwrap_or(false),
                error: self.error.clone(),
                duration: self.duration.unwrap_or_default(),
            },
        ))
    }
}

impl CollabAgentToolCallItem {
    pub fn from_collab_agent_spawn_begin_event(event: CollabAgentSpawnBeginEvent) -> Self {
        Self {
            id: event.call_id,
            tool: CollabAgentTool::SpawnAgent,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: Vec::new(),
            receiver_agents: Vec::new(),
            prompt: Some(event.prompt),
            model: Some(event.model),
            reasoning_effort: Some(event.reasoning_effort),
            agents_states: HashMap::new(),
        }
    }

    pub fn from_collab_agent_spawn_end_event(event: CollabAgentSpawnEndEvent) -> Self {
        let receiver_thread_ids = event.new_thread_id.into_iter().collect::<Vec<_>>();
        let receiver_agents = receiver_thread_ids
            .first()
            .copied()
            .map(|thread_id| CollabAgentRef {
                thread_id,
                agent_nickname: event.new_agent_nickname,
                agent_role: event.new_agent_role,
            })
            .into_iter()
            .collect();
        let agents_states = receiver_thread_ids
            .first()
            .copied()
            .map(|thread_id| [(thread_id, event.status.clone())].into_iter().collect())
            .unwrap_or_default();
        Self {
            id: event.call_id,
            tool: CollabAgentTool::SpawnAgent,
            status: collab_tool_call_status(&event.status, !receiver_thread_ids.is_empty()),
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids,
            receiver_agents,
            prompt: Some(event.prompt),
            model: Some(event.model),
            reasoning_effort: Some(event.reasoning_effort),
            agents_states,
        }
    }

    pub fn from_collab_agent_interaction_begin_event(
        event: CollabAgentInteractionBeginEvent,
    ) -> Self {
        Self {
            id: event.call_id,
            tool: CollabAgentTool::SendInput,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: vec![event.receiver_thread_id],
            receiver_agents: Vec::new(),
            prompt: Some(event.prompt),
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        }
    }

    pub fn from_collab_agent_interaction_end_event(event: CollabAgentInteractionEndEvent) -> Self {
        let receiver_agent = CollabAgentRef {
            thread_id: event.receiver_thread_id,
            agent_nickname: event.receiver_agent_nickname,
            agent_role: event.receiver_agent_role,
        };
        Self {
            id: event.call_id,
            tool: CollabAgentTool::SendInput,
            status: collab_tool_call_status(&event.status, true),
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: vec![event.receiver_thread_id],
            receiver_agents: vec![receiver_agent],
            prompt: Some(event.prompt),
            model: None,
            reasoning_effort: None,
            agents_states: [(event.receiver_thread_id, event.status)]
                .into_iter()
                .collect(),
        }
    }

    pub fn from_collab_waiting_begin_event(event: CollabWaitingBeginEvent) -> Self {
        Self {
            id: event.call_id,
            tool: CollabAgentTool::Wait,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: event.receiver_thread_ids,
            receiver_agents: event.receiver_agents,
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        }
    }

    pub fn from_collab_waiting_end_event(event: CollabWaitingEndEvent) -> Self {
        Self {
            id: event.call_id,
            tool: CollabAgentTool::Wait,
            status: if event
                .statuses
                .values()
                .any(|status| matches!(status, AgentStatus::Errored(_) | AgentStatus::NotFound))
            {
                CollabAgentToolCallStatus::Failed
            } else {
                CollabAgentToolCallStatus::Completed
            },
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: event.statuses.keys().copied().collect(),
            receiver_agents: event
                .agent_statuses
                .iter()
                .map(|entry| CollabAgentRef {
                    thread_id: entry.thread_id,
                    agent_nickname: entry.agent_nickname.clone(),
                    agent_role: entry.agent_role.clone(),
                })
                .collect(),
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: event.statuses,
        }
    }

    pub fn from_collab_close_begin_event(event: CollabCloseBeginEvent) -> Self {
        Self {
            id: event.call_id,
            tool: CollabAgentTool::CloseAgent,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: vec![event.receiver_thread_id],
            receiver_agents: Vec::new(),
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        }
    }

    pub fn from_collab_close_end_event(event: CollabCloseEndEvent) -> Self {
        let receiver_agent = CollabAgentRef {
            thread_id: event.receiver_thread_id,
            agent_nickname: event.receiver_agent_nickname,
            agent_role: event.receiver_agent_role,
        };
        Self {
            id: event.call_id,
            tool: CollabAgentTool::CloseAgent,
            status: collab_tool_call_status(&event.status, true),
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: vec![event.receiver_thread_id],
            receiver_agents: vec![receiver_agent],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: [(event.receiver_thread_id, event.status)]
                .into_iter()
                .collect(),
        }
    }

    pub fn from_collab_resume_begin_event(event: CollabResumeBeginEvent) -> Self {
        let receiver_agent = CollabAgentRef {
            thread_id: event.receiver_thread_id,
            agent_nickname: event.receiver_agent_nickname,
            agent_role: event.receiver_agent_role,
        };
        Self {
            id: event.call_id,
            tool: CollabAgentTool::ResumeAgent,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: vec![event.receiver_thread_id],
            receiver_agents: vec![receiver_agent],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        }
    }

    pub fn from_collab_resume_end_event(event: CollabResumeEndEvent) -> Self {
        let receiver_agent = CollabAgentRef {
            thread_id: event.receiver_thread_id,
            agent_nickname: event.receiver_agent_nickname,
            agent_role: event.receiver_agent_role,
        };
        Self {
            id: event.call_id,
            tool: CollabAgentTool::ResumeAgent,
            status: collab_tool_call_status(&event.status, true),
            sender_thread_id: event.sender_thread_id,
            receiver_thread_ids: vec![event.receiver_thread_id],
            receiver_agents: vec![receiver_agent],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: [(event.receiver_thread_id, event.status)]
                .into_iter()
                .collect(),
        }
    }

    pub fn as_legacy_begin_event(&self, started_at_ms: i64) -> Option<EventMsg> {
        let receiver_thread_id = self.receiver_thread_ids.first().copied();
        match self.tool {
            CollabAgentTool::SpawnAgent => Some(EventMsg::CollabAgentSpawnBegin(
                CollabAgentSpawnBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    prompt: self.prompt.clone().unwrap_or_default(),
                    model: self.model.clone().unwrap_or_default(),
                    reasoning_effort: self.reasoning_effort.clone().unwrap_or_default(),
                },
            )),
            CollabAgentTool::SendInput => receiver_thread_id.map(|receiver_thread_id| {
                EventMsg::CollabAgentInteractionBegin(CollabAgentInteractionBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    prompt: self.prompt.clone().unwrap_or_default(),
                })
            }),
            CollabAgentTool::ResumeAgent => receiver_thread_id.map(|receiver_thread_id| {
                let receiver_agent = self.receiver_agent(receiver_thread_id);
                EventMsg::CollabResumeBegin(CollabResumeBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname: receiver_agent
                        .as_ref()
                        .and_then(|agent| agent.agent_nickname.clone()),
                    receiver_agent_role: receiver_agent.and_then(|agent| agent.agent_role),
                })
            }),
            CollabAgentTool::Wait => Some(EventMsg::CollabWaitingBegin(CollabWaitingBeginEvent {
                started_at_ms,
                sender_thread_id: self.sender_thread_id,
                receiver_thread_ids: self.receiver_thread_ids.clone(),
                receiver_agents: self.receiver_agents.clone(),
                call_id: self.id.clone(),
            })),
            CollabAgentTool::CloseAgent => receiver_thread_id.map(|receiver_thread_id| {
                EventMsg::CollabCloseBegin(CollabCloseBeginEvent {
                    call_id: self.id.clone(),
                    started_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                })
            }),
        }
    }

    pub fn as_legacy_end_event(&self, completed_at_ms: i64) -> Option<EventMsg> {
        if matches!(self.status, CollabAgentToolCallStatus::InProgress) {
            return None;
        }
        let receiver_thread_id = self.receiver_thread_ids.first().copied();
        match self.tool {
            CollabAgentTool::SpawnAgent => {
                let receiver_agent = receiver_thread_id.and_then(|id| self.receiver_agent(id));
                Some(EventMsg::CollabAgentSpawnEnd(CollabAgentSpawnEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    new_thread_id: receiver_thread_id,
                    new_agent_nickname: receiver_agent
                        .as_ref()
                        .and_then(|agent| agent.agent_nickname.clone()),
                    new_agent_role: receiver_agent.and_then(|agent| agent.agent_role),
                    prompt: self.prompt.clone().unwrap_or_default(),
                    model: self.model.clone().unwrap_or_default(),
                    reasoning_effort: self.reasoning_effort.clone().unwrap_or_default(),
                    status: receiver_thread_id
                        .map(|thread_id| self.agent_status(thread_id))
                        .unwrap_or(AgentStatus::NotFound),
                }))
            }
            CollabAgentTool::SendInput => receiver_thread_id.map(|receiver_thread_id| {
                let receiver_agent = self.receiver_agent(receiver_thread_id);
                EventMsg::CollabAgentInteractionEnd(CollabAgentInteractionEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname: receiver_agent
                        .as_ref()
                        .and_then(|agent| agent.agent_nickname.clone()),
                    receiver_agent_role: receiver_agent.and_then(|agent| agent.agent_role),
                    prompt: self.prompt.clone().unwrap_or_default(),
                    status: self.agent_status(receiver_thread_id),
                })
            }),
            CollabAgentTool::ResumeAgent => receiver_thread_id.map(|receiver_thread_id| {
                let receiver_agent = self.receiver_agent(receiver_thread_id);
                EventMsg::CollabResumeEnd(CollabResumeEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname: receiver_agent
                        .as_ref()
                        .and_then(|agent| agent.agent_nickname.clone()),
                    receiver_agent_role: receiver_agent.and_then(|agent| agent.agent_role),
                    status: self.agent_status(receiver_thread_id),
                })
            }),
            CollabAgentTool::Wait => Some(EventMsg::CollabWaitingEnd(CollabWaitingEndEvent {
                sender_thread_id: self.sender_thread_id,
                call_id: self.id.clone(),
                completed_at_ms,
                agent_statuses: self
                    .receiver_agents
                    .iter()
                    .map(|agent| CollabAgentStatusEntry {
                        thread_id: agent.thread_id,
                        agent_nickname: agent.agent_nickname.clone(),
                        agent_role: agent.agent_role.clone(),
                        status: self.agent_status(agent.thread_id),
                    })
                    .collect(),
                statuses: self.agents_states.clone(),
            })),
            CollabAgentTool::CloseAgent => receiver_thread_id.map(|receiver_thread_id| {
                let receiver_agent = self.receiver_agent(receiver_thread_id);
                EventMsg::CollabCloseEnd(CollabCloseEndEvent {
                    call_id: self.id.clone(),
                    completed_at_ms,
                    sender_thread_id: self.sender_thread_id,
                    receiver_thread_id,
                    receiver_agent_nickname: receiver_agent
                        .as_ref()
                        .and_then(|agent| agent.agent_nickname.clone()),
                    receiver_agent_role: receiver_agent.and_then(|agent| agent.agent_role),
                    status: self.agent_status(receiver_thread_id),
                })
            }),
        }
    }

    fn receiver_agent(&self, thread_id: ThreadId) -> Option<CollabAgentRef> {
        self.receiver_agents
            .iter()
            .find(|agent| agent.thread_id == thread_id)
            .cloned()
    }

    fn agent_status(&self, thread_id: ThreadId) -> AgentStatus {
        self.agents_states
            .get(&thread_id)
            .cloned()
            .unwrap_or(AgentStatus::NotFound)
    }
}

fn collab_tool_call_status(status: &AgentStatus, has_receiver: bool) -> CollabAgentToolCallStatus {
    match status {
        AgentStatus::Errored(_) | AgentStatus::NotFound => CollabAgentToolCallStatus::Failed,
        _ if has_receiver => CollabAgentToolCallStatus::Completed,
        _ => CollabAgentToolCallStatus::Failed,
    }
}

impl SubAgentActivityItem {
    pub fn from_sub_agent_activity_event(event: SubAgentActivityEvent) -> Self {
        Self {
            id: event.event_id,
            kind: event.kind,
            agent_thread_id: event.agent_thread_id,
            agent_path: event.agent_path,
        }
    }

    pub fn as_legacy_event(&self, occurred_at_ms: i64) -> EventMsg {
        EventMsg::SubAgentActivity(SubAgentActivityEvent {
            event_id: self.id.clone(),
            occurred_at_ms,
            agent_thread_id: self.agent_thread_id,
            agent_path: self.agent_path.clone(),
            kind: self.kind,
        })
    }
}

impl WebSearchItem {
    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::WebSearchEnd(WebSearchEndEvent {
            call_id: self.id.clone(),
            query: self.query.clone(),
            action: self.action.clone(),
        })
    }
}

impl ImageGenerationItem {
    pub fn as_legacy_event(&self) -> EventMsg {
        EventMsg::ImageGenerationEnd(ImageGenerationEndEvent {
            call_id: self.id.clone(),
            status: self.status.clone(),
            revised_prompt: self.revised_prompt.clone(),
            result: self.result.clone(),
            saved_path: self.saved_path.clone(),
        })
    }
}

impl FileChangeItem {
    pub fn as_legacy_begin_event(&self, turn_id: String) -> EventMsg {
        EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: self.id.clone(),
            turn_id,
            auto_approved: self.auto_approved.unwrap_or(false),
            changes: self.changes.clone(),
        })
    }

    pub fn as_legacy_end_event(&self, turn_id: String) -> Option<EventMsg> {
        let status = self.status.clone()?;
        Some(EventMsg::PatchApplyEnd(PatchApplyEndEvent {
            call_id: self.id.clone(),
            turn_id,
            stdout: self.stdout.clone().unwrap_or_default(),
            stderr: self.stderr.clone().unwrap_or_default(),
            success: status == PatchApplyStatus::Completed,
            changes: self.changes.clone(),
            status,
        }))
    }
}

impl McpToolCallItem {
    pub fn as_legacy_begin_event(&self) -> EventMsg {
        EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: self.id.clone(),
            invocation: McpInvocation {
                server: self.server.clone(),
                tool: self.tool.clone(),
                arguments: (!self.arguments.is_null()).then(|| self.arguments.clone()),
            },
            connector_id: self.connector_id.clone(),
            mcp_app_resource_uri: self.mcp_app_resource_uri.clone(),
            link_id: self.link_id.clone(),
            app_name: self.app_name.clone(),
            template_id: self.template_id.clone(),
            action_name: self.action_name.clone(),
            plugin_id: self.plugin_id.clone(),
        })
    }

    pub fn as_legacy_end_event(&self) -> Option<EventMsg> {
        let result = match (&self.result, &self.error) {
            (Some(result), _) => Ok(result.clone()),
            (None, Some(error)) => Err(error.message.clone()),
            (None, None) => return None,
        };

        Some(EventMsg::McpToolCallEnd(McpToolCallEndEvent {
            call_id: self.id.clone(),
            invocation: McpInvocation {
                server: self.server.clone(),
                tool: self.tool.clone(),
                arguments: (!self.arguments.is_null()).then(|| self.arguments.clone()),
            },
            mcp_app_resource_uri: self.mcp_app_resource_uri.clone(),
            connector_id: self.connector_id.clone(),
            link_id: self.link_id.clone(),
            app_name: self.app_name.clone(),
            template_id: self.template_id.clone(),
            action_name: self.action_name.clone(),
            plugin_id: self.plugin_id.clone(),
            duration: self.duration?,
            result,
        }))
    }
}

impl TurnItem {
    pub fn id(&self) -> String {
        match self {
            TurnItem::UserMessage(item) => item.id.clone(),
            TurnItem::HookPrompt(item) => item.id.clone(),
            TurnItem::AgentMessage(item) => item.id.clone(),
            TurnItem::Plan(item) => item.id.clone(),
            TurnItem::Reasoning(item) => item.id.clone(),
            TurnItem::CommandExecution(item) => item.id.clone(),
            TurnItem::DynamicToolCall(item) => item.id.clone(),
            TurnItem::CollabAgentToolCall(item) => item.id.clone(),
            TurnItem::SubAgentActivity(item) => item.id.clone(),
            TurnItem::WebSearch(item) => item.id.clone(),
            TurnItem::ImageView(item) => item.id.clone(),
            TurnItem::Sleep(item) => item.id.clone(),
            TurnItem::ImageGeneration(item) => item.id.clone(),
            TurnItem::FileChange(item) => item.id.clone(),
            TurnItem::McpToolCall(item) => item.id.clone(),
            TurnItem::ContextCompaction(item) => item.id.clone(),
        }
    }

    pub fn as_legacy_events(&self, show_raw_agent_reasoning: bool) -> Vec<EventMsg> {
        match self {
            TurnItem::UserMessage(item) => vec![item.as_legacy_event()],
            TurnItem::HookPrompt(_) => Vec::new(),
            TurnItem::AgentMessage(item) => item.as_legacy_events(),
            TurnItem::Plan(_) => Vec::new(),
            TurnItem::CommandExecution(_)
            | TurnItem::DynamicToolCall(_)
            | TurnItem::CollabAgentToolCall(_) => Vec::new(),
            TurnItem::SubAgentActivity(_) => Vec::new(),
            TurnItem::WebSearch(item) => vec![item.as_legacy_event()],
            TurnItem::ImageView(item) => {
                vec![EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
                    call_id: item.id.clone(),
                    path: item.path.clone(),
                })]
            }
            TurnItem::Sleep(_) => Vec::new(),
            TurnItem::ImageGeneration(item) => vec![item.as_legacy_event()],
            TurnItem::FileChange(item) => item
                .as_legacy_end_event(String::new())
                .into_iter()
                .collect(),
            TurnItem::McpToolCall(item) => item.as_legacy_end_event().into_iter().collect(),
            TurnItem::Reasoning(item) => item.as_legacy_events(show_raw_agent_reasoning),
            TurnItem::ContextCompaction(item) => vec![item.as_legacy_event()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn hook_prompt_roundtrips_multiple_fragments() {
        let original = vec![
            HookPromptFragment::from_single_hook("Retry with care & joy.", "hook-run-1"),
            HookPromptFragment::from_single_hook("Then summarize cleanly.", "hook-run-2"),
        ];
        let message = build_hook_prompt_message(&original).expect("hook prompt");

        let ResponseItem::Message { content, .. } = message else {
            panic!("expected hook prompt message");
        };

        let parsed = parse_hook_prompt_message(/*id*/ None, &content).expect("parsed hook prompt");
        assert_eq!(parsed.fragments, original);
    }

    #[test]
    fn hook_prompt_parses_legacy_single_hook_run_id() {
        let parsed = parse_hook_prompt_fragment(
            r#"<hook_prompt hook_run_id="hook-run-1">Retry with tests.</hook_prompt>"#,
        )
        .expect("legacy hook prompt");

        assert_eq!(
            parsed,
            HookPromptFragment {
                text: "Retry with tests.".to_string(),
                hook_run_id: "hook-run-1".to_string(),
            }
        );
    }
}
