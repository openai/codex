//! Thread events emitted by codex exec.
//!
//! These types are the canonical representation of events streamed to SDK clients.

use mcp_types::ContentBlock as McpContentBlock;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// Top-level JSONL events emitted by codex exec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type")]
pub enum ThreadEvent {
    /// Emitted when a new thread is started as the first event.
    #[serde(rename = "thread.started")]
    ThreadStarted(ThreadStartedEvent),
    /// Emitted when a turn is started by sending a new prompt to the model.
    #[serde(rename = "turn.started")]
    TurnStarted(TurnStartedEvent),
    /// Emitted when a turn is completed.
    #[serde(rename = "turn.completed")]
    TurnCompleted(TurnCompletedEvent),
    /// Indicates that a turn failed with an error.
    #[serde(rename = "turn.failed")]
    TurnFailed(TurnFailedEvent),
    /// Emitted when a new item is added to the thread.
    #[serde(rename = "item.started")]
    ItemStarted(ItemStartedEvent),
    /// Emitted when an item is updated.
    #[serde(rename = "item.updated")]
    ItemUpdated(ItemUpdatedEvent),
    /// Signals that an item has reached a terminal state.
    #[serde(rename = "item.completed")]
    ItemCompleted(ItemCompletedEvent),
    /// Represents an unrecoverable error emitted directly by the event stream.
    #[serde(rename = "error")]
    Error(ThreadErrorEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ThreadStartedEvent {
    /// The identifier of the new thread.
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, Default)]
pub struct TurnStartedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct TurnCompletedEvent {
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct TurnFailedEvent {
    pub error: ThreadErrorEvent,
}

/// Describes the usage of tokens during a turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, Default)]
pub struct Usage {
    /// The number of input tokens used during the turn.
    pub input_tokens: i64,
    /// The number of cached input tokens used during the turn.
    pub cached_input_tokens: i64,
    /// The number of output tokens used during the turn.
    pub output_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ItemStartedEvent {
    pub item: ThreadItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ItemCompletedEvent {
    pub item: ThreadItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ItemUpdatedEvent {
    pub item: ThreadItem,
}

/// Fatal error emitted by the stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ThreadErrorEvent {
    pub message: String,
}

/// Canonical representation of a thread item and its domain-specific payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ThreadItem {
    pub id: String,
    #[serde(flatten)]
    pub details: ThreadItemDetails,
}

/// Typed payloads for each supported thread item type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThreadItemDetails {
    /// Response from the agent.
    AgentMessage(AgentMessageItem),
    /// Agent's reasoning summary.
    Reasoning(ReasoningItem),
    /// A command executed by the agent.
    CommandExecution(CommandExecutionItem),
    /// A set of file changes by the agent.
    FileChange(FileChangeItem),
    /// A call to an MCP tool.
    McpToolCall(McpToolCallItem),
    /// A web search request.
    WebSearch(WebSearchItem),
    /// Agent's to-do list.
    TodoList(TodoListItem),
    /// A non-fatal error surfaced as an item.
    Error(ErrorItem),
}

/// Response from the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct AgentMessageItem {
    pub text: String,
}

/// Agent's reasoning summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ReasoningItem {
    pub text: String,
}

/// The status of a command execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
    Declined,
}

/// A command executed by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct CommandExecutionItem {
    pub command: String,
    pub aggregated_output: String,
    pub exit_code: Option<i32>,
    pub status: CommandExecutionStatus,
}

/// A file update change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: PatchChangeKind,
}

/// The status of a file change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus {
    InProgress,
    Completed,
    Failed,
}

/// A set of file changes by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct FileChangeItem {
    pub changes: Vec<FileUpdateChange>,
    pub status: PatchApplyStatus,
}

/// Indicates the type of the file change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update,
}

/// The status of an MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    #[default]
    InProgress,
    Completed,
    Failed,
}

/// Result payload produced by an MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct McpToolCallItemResult {
    pub content: Vec<McpContentBlock>,
    pub structured_content: Option<JsonValue>,
}

/// Error details reported by a failed MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct McpToolCallItemError {
    pub message: String,
}

/// A call to an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct McpToolCallItem {
    pub server: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: JsonValue,
    pub result: Option<McpToolCallItemResult>,
    pub error: Option<McpToolCallItemError>,
    pub status: McpToolCallStatus,
}

/// A web search request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct WebSearchItem {
    pub query: String,
}

/// An error notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ErrorItem {
    pub message: String,
}

/// An item in agent's to-do list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct TodoItem {
    pub text: String,
    pub completed: bool,
}

/// The to-do list item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct TodoListItem {
    pub items: Vec<TodoItem>,
}
