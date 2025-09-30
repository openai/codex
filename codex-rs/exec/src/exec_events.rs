use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Top-level events emitted on the Codex Exec thread stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(tag = "type")]
pub enum ThreadEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted(ThreadStartedEvent),
    #[serde(rename = "turn.started")]
    TurnStarted(TurnStartedEvent),
    #[serde(rename = "turn.completed")]
    TurnCompleted(TurnCompletedEvent),
    #[serde(rename = "turn.failed")]
    TurnFailed(TurnFailedEvent),
    #[serde(rename = "item.started")]
    ItemStarted(ItemStartedEvent),
    #[serde(rename = "item.updated")]
    ItemUpdated(ItemUpdatedEvent),
    #[serde(rename = "item.completed")]
    ItemCompleted(ItemCompletedEvent),
    #[serde(rename = "error")]
    Error(ThreadErrorEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ThreadStartedEvent {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, Default)]
pub struct TurnStartedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct TurnCompletedEvent {
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct TurnFailedEvent {
    pub error: ThreadErrorEvent,
}

/// Minimal usage summary for a turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS, Default)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ItemStartedEvent {
    pub item: ThreadItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ItemCompletedEvent {
    pub item: ThreadItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ItemUpdatedEvent {
    pub item: ThreadItem,
}

/// Fatal error emitted by the stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ThreadErrorEvent {
    pub message: String,
}

/// Canonical representation of a thread item and its domain-specific payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ThreadItem {
    pub id: String,
    #[serde(flatten)]
    pub details: ThreadItemDetails,
}

/// Typed payloads for each supported thread item type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(tag = "item_type", rename_all = "snake_case")]
pub enum ThreadItemDetails {
    AssistantMessage(AssistantMessageItem),
    Reasoning(ReasoningItem),
    CommandExecution(CommandExecutionItem),
    FileChange(FileChangeItem),
    McpToolCall(McpToolCallItem),
    WebSearch(WebSearchItem),
    TodoList(TodoListItem),
    Error(ErrorItem),
}

/// Session metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SessionItem {
    pub session_id: String,
}

/// Assistant message payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct AssistantMessageItem {
    pub text: String,
}

/// Model reasoning summary payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ReasoningItem {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
}

/// Local shell command execution payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct CommandExecutionItem {
    pub command: String,
    pub aggregated_output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub status: CommandExecutionStatus,
}

/// Single file change summary for a patch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: PatchChangeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus {
    Completed,
    Failed,
}

/// Patch application payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct FileChangeItem {
    pub changes: Vec<FileUpdateChange>,
    pub status: PatchApplyStatus,
}

/// Known change kinds for a patch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, TS)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct McpToolCallItem {
    pub server: String,
    pub tool: String,
    pub status: McpToolCallStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct WebSearchItem {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct ErrorItem {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct TodoItem {
    pub text: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct TodoListItem {
    pub items: Vec<TodoItem>,
}
