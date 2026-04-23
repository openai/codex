//! Hot-path helpers for recording canonical tool dispatch boundaries.
//!
//! Core owns tool routing and result conversion. The trace crate owns the raw
//! event schema, payload shape, and no-op behavior, so core only adapts its
//! domain objects into the small request/result structs defined here.

use std::fmt::Display;
use std::sync::Arc;

use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::models::SearchToolCallParams;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use tracing::warn;

use crate::model::AgentThreadId;
use crate::model::CodeModeRuntimeToolId;
use crate::model::CodexTurnId;
use crate::model::ExecutionStatus;
use crate::model::ModelVisibleCallId;
use crate::model::ToolCallId;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::payload::RawPayloadKind;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawToolCallRequester;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;
use crate::writer::TraceWriter;

/// No-op capable trace handle for one resolved tool dispatch.
#[derive(Clone, Debug)]
pub struct ToolDispatchTraceContext {
    state: ToolDispatchTraceContextState,
}

#[derive(Clone, Debug)]
enum ToolDispatchTraceContextState {
    Disabled,
    Enabled(EnabledToolDispatchTraceContext),
}

#[derive(Clone, Debug)]
struct EnabledToolDispatchTraceContext {
    writer: Arc<TraceWriter>,
    thread_id: AgentThreadId,
    codex_turn_id: CodexTurnId,
    tool_call_id: ToolCallId,
}

/// Core-facing request data for the canonical Codex tool boundary.
pub struct ToolDispatchInvocation {
    pub thread_id: AgentThreadId,
    pub codex_turn_id: CodexTurnId,
    pub tool_call_id: ToolCallId,
    pub tool_name: String,
    pub tool_namespace: Option<String>,
    pub requester: ToolDispatchRequester,
    pub payload: ToolDispatchPayload,
}

/// Runtime source that caused a dispatch-level tool call.
pub enum ToolDispatchRequester {
    Model {
        model_visible_call_id: ModelVisibleCallId,
    },
    CodeCell {
        runtime_cell_id: String,
        runtime_tool_call_id: CodeModeRuntimeToolId,
    },
}

/// Tool input observed at the registry boundary.
pub enum ToolDispatchPayload {
    Function {
        arguments: String,
    },
    ToolSearch {
        arguments: SearchToolCallParams,
    },
    Custom {
        input: String,
    },
    LocalShell {
        command: Vec<String>,
        workdir: Option<String>,
        timeout_ms: Option<u64>,
        sandbox_permissions: Option<SandboxPermissions>,
        prefix_rule: Option<Vec<String>>,
        additional_permissions: Option<PermissionProfile>,
        justification: Option<String>,
    },
    Mcp {
        server: String,
        tool: String,
        raw_arguments: String,
    },
}

/// Result data returned from a dispatch-level tool call.
#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ToolDispatchResult {
    DirectResponse { response_item: ResponseInputItem },
    CodeModeResponse { value: JsonValue },
}

/// Raw invocation payload for the canonical Codex tool boundary.
#[derive(Serialize)]
struct DispatchedToolTraceRequest<'a> {
    tool_name: &'a str,
    tool_namespace: Option<&'a str>,
    payload: &'a JsonValue,
}

/// Raw response payload for dispatch-level tool trace events.
#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum DispatchedToolTraceResponse<'a> {
    DirectResponse {
        response_item: &'a ResponseInputItem,
    },
    CodeModeResponse {
        value: &'a JsonValue,
    },
    Error {
        error: String,
    },
}

impl ToolDispatchTraceContext {
    /// Builds a context that accepts trace calls and records nothing.
    pub(crate) fn disabled() -> Self {
        Self {
            state: ToolDispatchTraceContextState::Disabled,
        }
    }

    /// Returns whether caller-side result conversion would be recorded.
    ///
    /// Core uses this to avoid formatting or cloning tool outputs when the
    /// dispatch lifecycle is suppressed or tracing is disabled.
    pub fn is_enabled(&self) -> bool {
        matches!(self.state, ToolDispatchTraceContextState::Enabled(_))
    }

    /// Starts one dispatch-level lifecycle and returns the handle for its result.
    pub(crate) fn start(writer: Arc<TraceWriter>, invocation: ToolDispatchInvocation) -> Self {
        if suppresses_tool_dispatch_trace(&invocation) {
            return Self::disabled();
        }

        let context = EnabledToolDispatchTraceContext {
            writer,
            thread_id: invocation.thread_id.clone(),
            codex_turn_id: invocation.codex_turn_id.clone(),
            tool_call_id: invocation.tool_call_id.clone(),
        };
        record_started(&context, invocation);
        Self {
            state: ToolDispatchTraceContextState::Enabled(context),
        }
    }

    /// Records the caller-facing successful or failed tool result.
    pub fn record_completed(&self, status: ExecutionStatus, result: ToolDispatchResult) {
        let ToolDispatchTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        let response = match &result {
            ToolDispatchResult::DirectResponse { response_item } => {
                DispatchedToolTraceResponse::DirectResponse { response_item }
            }
            ToolDispatchResult::CodeModeResponse { value } => {
                DispatchedToolTraceResponse::CodeModeResponse { value }
            }
        };
        append_tool_call_ended(context, status, &response);
    }

    /// Records a dispatch failure before the tool produced a normal result payload.
    pub fn record_failed(&self, error: impl Display) {
        let ToolDispatchTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        append_tool_call_ended(
            context,
            ExecutionStatus::Failed,
            &DispatchedToolTraceResponse::Error {
                error: error.to_string(),
            },
        );
    }
}

fn suppresses_tool_dispatch_trace(invocation: &ToolDispatchInvocation) -> bool {
    matches!(invocation.payload, ToolDispatchPayload::Custom { .. })
        && invocation.tool_namespace.is_none()
        && invocation.tool_name == codex_code_mode::PUBLIC_TOOL_NAME
}

fn record_started(context: &EnabledToolDispatchTraceContext, invocation: ToolDispatchInvocation) {
    let tool_name = invocation.tool_name;
    let tool_namespace = invocation.tool_namespace;
    let kind = dispatched_tool_kind(&tool_name, &invocation.payload);
    let label = dispatched_tool_label(&tool_name, tool_namespace.as_deref(), &invocation.payload);
    let input_preview = Some(invocation.payload.log_payload_preview());
    let payload = invocation.payload.into_json_payload();
    let request = DispatchedToolTraceRequest {
        tool_name: tool_name.as_str(),
        tool_namespace: tool_namespace.as_deref(),
        payload: &payload,
    };
    let request_payload =
        write_json_payload_best_effort(&context.writer, RawPayloadKind::ToolInvocation, &request);
    let (model_visible_call_id, code_mode_runtime_tool_id, requester) =
        requester_fields(invocation.requester);

    append_with_context_best_effort(
        context,
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: context.tool_call_id.clone(),
            model_visible_call_id,
            code_mode_runtime_tool_id,
            requester,
            kind,
            summary: ToolCallSummary::Generic {
                label,
                input_preview,
                output_preview: None,
            },
            invocation_payload: request_payload,
        },
    );
}

fn requester_fields(
    requester: ToolDispatchRequester,
) -> (
    Option<ModelVisibleCallId>,
    Option<CodeModeRuntimeToolId>,
    RawToolCallRequester,
) {
    match requester {
        ToolDispatchRequester::Model {
            model_visible_call_id,
        } => (
            Some(model_visible_call_id),
            None,
            RawToolCallRequester::Model,
        ),
        ToolDispatchRequester::CodeCell {
            runtime_cell_id,
            runtime_tool_call_id,
        } => (
            None,
            Some(runtime_tool_call_id),
            RawToolCallRequester::CodeCell { runtime_cell_id },
        ),
    }
}

fn dispatched_tool_kind(tool_name: &str, payload: &ToolDispatchPayload) -> ToolCallKind {
    if let ToolDispatchPayload::Mcp { server, tool, .. } = payload {
        return ToolCallKind::Mcp {
            server: server.clone(),
            tool: tool.clone(),
        };
    }

    match tool_name {
        "exec_command" | "local_shell" | "shell" | "shell_command" => ToolCallKind::ExecCommand,
        "write_stdin" => ToolCallKind::WriteStdin,
        "apply_patch" => ToolCallKind::ApplyPatch,
        "web_search" | "web_search_preview" => ToolCallKind::Web,
        "image_generation" | "image_query" => ToolCallKind::ImageGeneration,
        "spawn_agent" => ToolCallKind::SpawnAgent,
        "send_message" => ToolCallKind::SendMessage,
        "followup_task" => ToolCallKind::AssignAgentTask,
        "wait_agent" => ToolCallKind::WaitAgent,
        "close_agent" => ToolCallKind::CloseAgent,
        other => ToolCallKind::Other {
            name: other.to_string(),
        },
    }
}

fn dispatched_tool_label(
    tool_name: &str,
    tool_namespace: Option<&str>,
    payload: &ToolDispatchPayload,
) -> String {
    if let ToolDispatchPayload::Mcp { server, tool, .. } = payload {
        return format!("mcp:{server}:{tool}");
    }

    match tool_namespace {
        Some(namespace) => format!("{namespace}.{tool_name}"),
        None => tool_name.to_string(),
    }
}

impl ToolDispatchPayload {
    fn log_payload_preview(&self) -> String {
        match self {
            ToolDispatchPayload::Function { arguments } => truncate_preview(arguments),
            ToolDispatchPayload::ToolSearch { arguments } => truncate_preview(&arguments.query),
            ToolDispatchPayload::Custom { input } => truncate_preview(input),
            ToolDispatchPayload::LocalShell { command, .. } => truncate_preview(&command.join(" ")),
            ToolDispatchPayload::Mcp { raw_arguments, .. } => truncate_preview(raw_arguments),
        }
    }

    fn into_json_payload(self) -> JsonValue {
        match self {
            ToolDispatchPayload::Function { arguments } => json!({
                "type": "function",
                "arguments": arguments,
            }),
            ToolDispatchPayload::ToolSearch { arguments } => json!({
                "type": "tool_search",
                "arguments": arguments,
            }),
            ToolDispatchPayload::Custom { input } => json!({
                "type": "custom",
                "input": input,
            }),
            ToolDispatchPayload::LocalShell {
                command,
                workdir,
                timeout_ms,
                sandbox_permissions,
                prefix_rule,
                additional_permissions,
                justification,
            } => json!({
                "type": "local_shell",
                "command": command,
                "workdir": workdir,
                "timeout_ms": timeout_ms,
                "sandbox_permissions": sandbox_permissions,
                "prefix_rule": prefix_rule,
                "additional_permissions": additional_permissions,
                "justification": justification,
            }),
            ToolDispatchPayload::Mcp {
                server,
                tool,
                raw_arguments,
            } => json!({
                "type": "mcp",
                "server": server,
                "tool": tool,
                "raw_arguments": raw_arguments,
            }),
        }
    }
}

fn truncate_preview(value: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 160;
    let mut chars = value.chars();
    let mut preview = chars.by_ref().take(MAX_PREVIEW_CHARS).collect::<String>();
    if chars.next().is_some() {
        preview.push_str("...");
    }
    preview
}

fn append_tool_call_ended(
    context: &EnabledToolDispatchTraceContext,
    status: ExecutionStatus,
    response: &DispatchedToolTraceResponse<'_>,
) {
    let response_payload =
        write_json_payload_best_effort(&context.writer, RawPayloadKind::ToolResult, response);
    append_with_context_best_effort(
        context,
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: context.tool_call_id.clone(),
            status,
            result_payload: response_payload,
        },
    );
}

fn write_json_payload_best_effort(
    writer: &TraceWriter,
    kind: RawPayloadKind,
    payload: &impl Serialize,
) -> Option<RawPayloadRef> {
    match writer.write_json_payload(kind, payload) {
        Ok(payload_ref) => Some(payload_ref),
        Err(err) => {
            warn!("failed to write rollout trace payload: {err:#}");
            None
        }
    }
}

fn append_with_context_best_effort(
    context: &EnabledToolDispatchTraceContext,
    payload: RawTraceEventPayload,
) {
    let event_context = RawTraceEventContext {
        thread_id: Some(context.thread_id.clone()),
        codex_turn_id: Some(context.codex_turn_id.clone()),
    };
    if let Err(err) = context.writer.append_with_context(event_context, payload) {
        warn!("failed to append rollout trace event: {err:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_only_noncanonical_dispatch_boundaries() {
        assert!(suppresses_tool_dispatch_trace(&invocation(
            codex_code_mode::PUBLIC_TOOL_NAME,
            /*tool_namespace*/ None,
            ToolDispatchRequester::Model {
                model_visible_call_id: "call-exec".to_string(),
            },
            ToolDispatchPayload::Custom {
                input: "1 + 1".to_string(),
            },
        )));
        assert!(!suppresses_tool_dispatch_trace(&invocation(
            "custom_tool",
            /*tool_namespace*/ None,
            ToolDispatchRequester::Model {
                model_visible_call_id: "call-custom".to_string(),
            },
            ToolDispatchPayload::Custom {
                input: "payload".to_string(),
            },
        )));
        assert!(!suppresses_tool_dispatch_trace(&invocation(
            codex_code_mode::PUBLIC_TOOL_NAME,
            Some("mcp__server".to_string()),
            ToolDispatchRequester::Model {
                model_visible_call_id: "call-namespaced".to_string(),
            },
            ToolDispatchPayload::Custom {
                input: "payload".to_string(),
            },
        )));
    }

    fn invocation(
        tool_name: &str,
        tool_namespace: Option<String>,
        requester: ToolDispatchRequester,
        payload: ToolDispatchPayload,
    ) -> ToolDispatchInvocation {
        ToolDispatchInvocation {
            thread_id: "thread-1".to_string(),
            codex_turn_id: "turn-1".to_string(),
            tool_call_id: "tool-call-1".to_string(),
            tool_name: tool_name.to_string(),
            tool_namespace,
            requester,
            payload,
        }
    }
}
