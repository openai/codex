use serde::Deserialize;
use serde::Serialize;

/// Top-level events emitted on the Codex Exec conversation stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum ConversationEvent {
    #[serde(rename = "session.created")]
    SessionCreated(SessionCreatedEvent),
    #[serde(rename = "item.completed")]
    ItemCompleted(ItemCompletedEvent),
    #[serde(rename = "error")]
    Error(ConversationErrorEvent),
}

/// Payload describing a newly created conversation item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionCreatedEvent {
    pub session_id: String,
}

/// Payload describing the completion of an existing conversation item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemCompletedEvent {
    pub item: ConversationItem,
}

/// Fatal error emitted by the stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationErrorEvent {
    pub message: String,
}

/// Canonical representation of a conversation item and its domain-specific payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationItem {
    pub id: String,
    #[serde(flatten)]
    pub details: ConversationItemDetails,
}

/// Typed payloads for each supported conversation item type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "item_type", rename_all = "snake_case")]
pub enum ConversationItemDetails {
    AssistantMessage(AssistantMessageItem),
    Reasoning(ReasoningItem),
    CommandExecution(CommandExecutionItem),
    PatchApply(FileUpdateItem),
    McpToolCall(McpToolCallItem),
    WebSearch(WebSearchItem),
    Error(ErrorItem),
}

/// Session conversation metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionItem {
    pub session_id: String,
}

/// Assistant message payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantMessageItem {
    pub text: String,
}

/// Model reasoning summary payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReasoningItem {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
}

/// Local shell command execution payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandExecutionItem {
    pub command: String,
    pub aggregated_output: String,
    pub exit_code: i32,
    pub status: CommandExecutionStatus,
}

/// Single file change summary for a patch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: PatchChangeKind,
}

/// Patch application payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileUpdateItem {
    pub changes: Vec<FileUpdateChange>,
}

/// Known change kinds for a patch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolCallItem {
    pub server: String,
    pub tool: String,
    pub status: McpToolCallStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSearchItem {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorItem {
    pub message: String,
}
