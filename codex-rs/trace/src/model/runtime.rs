use serde::Deserialize;
use serde::Serialize;

use crate::payload::RawPayloadId;
use crate::raw_event::RawEventSeq;

use super::AgentPath;
use super::AgentThreadId;
use super::CodeCellId;
use super::CodeModeRuntimeToolId;
use super::CodexTurnId;
use super::CompactionId;
use super::CompactionRequestId;
use super::ConversationItemId;
use super::EdgeId;
use super::ModelVisibleCallId;
use super::TerminalId;
use super::TerminalOperationId;
use super::ToolCallId;
use super::session::ExecutionWindow;

/// Runtime/debug object for one model-authored `exec` cell.
///
/// The JavaScript source and custom-tool outputs are still conversation items;
/// this object tracks the code-mode runtime boundary and nested runtime work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CodeCell {
    /// Reducer-owned graph id derived from the model-visible `exec` call id.
    /// Runtime cell ids are stored separately because they are only handles for
    /// later waits and nested code-mode tools.
    pub(crate) code_cell_id: CodeCellId,
    pub(crate) model_visible_call_id: ModelVisibleCallId,
    pub(crate) thread_id: AgentThreadId,
    pub(crate) codex_turn_id: CodexTurnId,
    /// Conversation item containing the model-authored JavaScript.
    pub(crate) source_item_id: ConversationItemId,
    pub(crate) output_item_ids: Vec<ConversationItemId>,
    /// Raw code-mode runtime/session id, useful when matching runtime payloads.
    pub(crate) runtime_cell_id: Option<String>,
    /// Full JS-cell runtime window; yielded cells can outlive the initial custom call.
    pub(crate) execution: ExecutionWindow,
    pub(crate) runtime_status: CodeCellRuntimeStatus,
    pub(crate) initial_response_at_unix_ms: Option<i64>,
    pub(crate) initial_response_seq: Option<RawEventSeq>,
    pub(crate) yielded_at_unix_ms: Option<i64>,
    pub(crate) yielded_seq: Option<RawEventSeq>,
    pub(crate) source_js: String,
    pub(crate) nested_tool_call_ids: Vec<ToolCallId>,
    pub(crate) wait_tool_call_ids: Vec<ToolCallId>,
}

/// Code-mode runtime lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CodeCellRuntimeStatus {
    /// The `exec` request has been accepted but the runtime has not yet started user code.
    Starting,
    /// Runtime is executing JavaScript and has not yet yielded or terminated.
    Running,
    /// Initial `exec` returned while JavaScript kept running in the background.
    Yielded,
    /// Runtime reached a normal terminal result.
    Completed,
    /// Runtime reached an error terminal result.
    Failed,
    /// Runtime was explicitly terminated.
    Terminated,
}

/// Installed conversation-history replacement boundary.
///
/// Duration-bearing upstream requests live in `CompactionRequest`. This object
/// is the checkpoint where replacement history became the live thread history.
/// The boundary marker and the model-visible summary are separate conversation
/// items: the marker says where history was replaced, while the summary is part
/// of `replacement_item_ids` when the compact endpoint returned one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Compaction {
    pub(crate) compaction_id: CompactionId,
    pub(crate) thread_id: AgentThreadId,
    pub(crate) codex_turn_id: CodexTurnId,
    pub(crate) installed_at_unix_ms: i64,
    /// Structural conversation item marking where pre-compaction history ended.
    pub(crate) marker_item_id: ConversationItemId,
    /// Upstream compaction request attempts that contributed to this checkpoint.
    pub(crate) request_ids: Vec<CompactionRequestId>,
    /// Logical conversation items present immediately before replacement.
    pub(crate) input_item_ids: Vec<ConversationItemId>,
    /// Replacement conversation items installed by the checkpoint.
    pub(crate) replacement_item_ids: Vec<ConversationItemId>,
}

/// One upstream remote request made while computing a compaction checkpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CompactionRequest {
    pub(crate) compaction_request_id: CompactionRequestId,
    pub(crate) compaction_id: CompactionId,
    pub(crate) thread_id: AgentThreadId,
    pub(crate) codex_turn_id: CodexTurnId,
    pub(crate) execution: ExecutionWindow,
    pub(crate) model: String,
    pub(crate) provider_name: String,
    pub(crate) raw_request_payload_id: RawPayloadId,
    /// Full compaction response payload. `None` while running or after pre-response failures.
    pub(crate) raw_response_payload_id: Option<RawPayloadId>,
}

/// Runtime operation requested by the model, a JS code cell, or Codex itself.
///
/// A `ToolCall` is not a chat transcript row. Model-visible call/output items
/// link to it through `model_visible_*_item_ids`; runtime-only tools can have
/// empty model-visible lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ToolCall {
    pub(crate) tool_call_id: ToolCallId,
    /// Model-visible protocol call ID, if the model directly requested this tool.
    pub(crate) model_visible_call_id: Option<ModelVisibleCallId>,
    /// Code-mode runtime's internal tool invocation ID, if this call came from JS.
    pub(crate) code_mode_runtime_tool_id: Option<CodeModeRuntimeToolId>,
    pub(crate) thread_id: AgentThreadId,
    /// Runtime activation that started the tool. Background work may outlive this turn.
    pub(crate) started_by_codex_turn_id: Option<CodexTurnId>,
    pub(crate) execution: ExecutionWindow,
    pub(crate) requester: ToolCallRequester,
    pub(crate) kind: ToolCallKind,
    pub(crate) model_visible_call_item_ids: Vec<ConversationItemId>,
    pub(crate) model_visible_output_item_ids: Vec<ConversationItemId>,
    /// Terminal operation started by this tool, when the tool touched a terminal.
    pub(crate) terminal_operation_id: Option<TerminalOperationId>,
    pub(crate) summary: ToolCallSummary,
    /// Original invocation at the Codex tool boundary.
    ///
    /// Direct model tools store the model's function/custom call payload here.
    /// Code-mode nested tools store the JSON call made by model-authored JS.
    /// Runtime protocol events are deliberately kept separate below because
    /// they describe how Codex executed the request, not what the caller sent.
    pub(crate) raw_invocation_payload_id: Option<RawPayloadId>,
    /// Result returned to the immediate requester.
    ///
    /// For direct tools this is the tool output item returned to the model; for
    /// code-mode nested tools this is the value returned to JavaScript.
    pub(crate) raw_result_payload_id: Option<RawPayloadId>,
    /// Runtime/protocol payloads observed while executing the tool.
    ///
    /// Examples include exec begin/end, patch begin/end, and MCP begin/end
    /// events. Reducers can use these to build richer runtime objects such as
    /// terminal operations without overwriting the canonical invocation/result.
    pub(crate) raw_runtime_payload_ids: Vec<RawPayloadId>,
}

/// Requester of a runtime tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub(crate) enum ToolCallRequester {
    Model,
    /// Model-authored JavaScript requested the tool through code-mode.
    CodeCell {
        code_cell_id: CodeCellId,
    },
}

/// Runtime tool category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub(crate) enum ToolCallKind {
    ExecCommand,
    WriteStdin,
    ApplyPatch,
    Mcp {
        server: String,
        tool: String,
    },
    Web,
    ImageGeneration,
    SpawnAgent,
    AssignAgentTask,
    SendMessage,
    /// Multi-agent wait operation. Code-mode wait is modeled separately.
    WaitAgent,
    CloseAgent,
    Other {
        name: String,
    },
}

/// Bounded card/list summary for a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub(crate) enum ToolCallSummary {
    /// Tool is summarized by its terminal operation.
    Terminal { operation_id: TerminalOperationId },
    Agent {
        target_agent_path: AgentPath,
        /// Task name/path segment when the operation creates or targets a task.
        task_name: Option<String>,
        message_preview: String,
    },
    WaitAgent {
        /// Wait target, when narrower than "any child".
        target_agent_path: Option<AgentPath>,
        timeout_ms: Option<u64>,
    },
    Generic {
        label: String,
        input_preview: Option<String>,
        output_preview: Option<String>,
    },
}

/// Reusable terminal process/session returned by the runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TerminalSession {
    pub(crate) terminal_id: TerminalId,
    pub(crate) thread_id: AgentThreadId,
    pub(crate) created_by_operation_id: TerminalOperationId,
    pub(crate) operation_ids: Vec<TerminalOperationId>,
    /// Terminal lifetime. This can outlive the operation that created it.
    pub(crate) execution: ExecutionWindow,
}

/// One command/write/poll operation against a terminal session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TerminalOperation {
    pub(crate) operation_id: TerminalOperationId,
    /// Runtime terminal/process ID. `None` is legal only while the operation that creates it is starting.
    pub(crate) terminal_id: Option<TerminalId>,
    pub(crate) tool_call_id: ToolCallId,
    pub(crate) kind: TerminalOperationKind,
    /// Operation execution window. This is not necessarily the terminal session lifetime.
    pub(crate) execution: ExecutionWindow,
    pub(crate) request: TerminalRequest,
    /// Runtime-observed terminal result. Model-visible output links through observations.
    pub(crate) result: Option<TerminalResult>,
    pub(crate) model_observations: Vec<TerminalModelObservation>,
    pub(crate) raw_payload_ids: Vec<RawPayloadId>,
}

/// Terminal operation category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminalOperationKind {
    ExecCommand,
    WriteStdin,
}

/// Terminal request summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub(crate) enum TerminalRequest {
    ExecCommand {
        command: Vec<String>,
        display_command: String,
        cwd: String,
        yield_time_ms: Option<u64>,
        max_output_tokens: Option<usize>,
    },
    /// Request to interact with an existing terminal.
    WriteStdin {
        /// Bytes/text sent to stdin. Empty string means poll/read without writing bytes.
        stdin: String,
        yield_time_ms: Option<u64>,
        max_output_tokens: Option<usize>,
    },
}

/// Terminal result observed by the runtime.
///
/// This is debugger/runtime output. It is not proof that the model saw the same
/// bytes; link model-visible call/output items through `TerminalModelObservation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TerminalResult {
    /// Process exit code. `None` if the process is still running or no exit status was produced.
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    /// Tool runtime's formatted caller-facing output, when present.
    pub(crate) formatted_output: Option<String>,
    /// Token count before truncation, when the tool runtime reported it.
    pub(crate) original_token_count: Option<usize>,
    /// Streaming chunk ID, when this result was assembled from chunked terminal output.
    pub(crate) chunk_id: Option<String>,
}

/// Conversation items that observed a terminal operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TerminalModelObservation {
    pub(crate) call_item_ids: Vec<ConversationItemId>,
    pub(crate) output_item_ids: Vec<ConversationItemId>,
    pub(crate) source: TerminalObservationSource,
}

/// Source of model-visible terminal observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminalObservationSource {
    DirectToolCall,
    CodeCellOutput,
}

/// Directed information-flow relationship between trace objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InteractionEdge {
    pub(crate) edge_id: EdgeId,
    pub(crate) kind: InteractionEdgeKind,
    pub(crate) source: TraceAnchor,
    pub(crate) target: TraceAnchor,
    pub(crate) started_at_unix_ms: i64,
    pub(crate) ended_at_unix_ms: Option<i64>,
    pub(crate) carried_item_ids: Vec<ConversationItemId>,
    pub(crate) carried_raw_payload_ids: Vec<RawPayloadId>,
}

/// Information-flow edge category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InteractionEdgeKind {
    SpawnAgent,
    AssignAgentTask,
    SendMessage,
    AgentResult,
    CloseAgent,
}

/// Typed pointer to one stable reduced-trace object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub(crate) enum TraceAnchor {
    ConversationItem { item_id: ConversationItemId },
    ToolCall { tool_call_id: ToolCallId },
    Thread { thread_id: AgentThreadId },
}
