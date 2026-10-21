//! Terminal reduction for exec-like tool calls.
//!
//! The raw trace records terminal activity as normal tool lifecycle events.
//! Protocol-backed exec events carry `ExecCommand*` payloads with the richest
//! runtime details. Direct tools without protocol observations, such as
//! `write_stdin`, can still form a terminal row from the canonical dispatch
//! invocation/result payloads when those payloads carry the session join key.
//! Runtime process handles are recyclable and thread-local. Each exec creates a
//! distinct reduced session; stdin uses the runtime's resolved exec when recorded,
//! falling back to the latest session for its handle in older traces.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use super::push_unique;
use crate::model::ExecutionStatus;
use crate::model::ExecutionWindow;
use crate::model::TerminalModelObservation;
use crate::model::TerminalObservationSource;
use crate::model::TerminalOperation;
use crate::model::TerminalOperationId;
use crate::model::TerminalOperationKind;
use crate::model::TerminalRequest;
use crate::model::TerminalResult;
use crate::model::TerminalSession;
use crate::model::ToolCallKind;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawEventSeq;
use crate::reducer::TraceReducer;

impl TraceReducer {
    /// Starts a terminal operation from a canonical dispatch invocation payload.
    ///
    /// This is currently needed for direct tools such as write-stdin that do not
    /// emit a richer protocol runtime-begin event with the terminal join key.
    pub(in crate::reducer) fn start_terminal_operation_from_invocation(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        thread_id: &str,
        tool_call_id: &str,
        kind: &ToolCallKind,
        invocation_payload: Option<&RawPayloadRef>,
    ) -> Result<Option<TerminalOperationId>> {
        if !matches!(kind, ToolCallKind::WriteStdin) {
            return Ok(None);
        }
        let operation_kind = TerminalOperationKind::WriteStdin;
        let Some(invocation_payload) = invocation_payload else {
            // Payload writes are best-effort in the live recorder. If the
            // canonical invocation is missing, keep the ToolCall but avoid
            // fabricating a lossy terminal row.
            return Ok(None);
        };

        let payload = self.read_payload_json(invocation_payload)?;
        let Some(request) = parse_dispatch_terminal_request(payload).with_context(|| {
            format!(
                "parse terminal invocation payload {} as dispatch payload",
                invocation_payload.raw_payload_id
            )
        })?
        else {
            return Ok(None);
        };
        self.insert_terminal_operation(TerminalOperationStart {
            seq,
            wall_time_unix_ms,
            thread_id,
            tool_call_id,
            operation_kind,
            raw_payload: invocation_payload,
            request,
        })
    }

    /// Starts a terminal operation from a protocol runtime-begin payload.
    pub(in crate::reducer) fn start_terminal_operation_from_runtime(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        thread_id: &str,
        tool_call_id: &str,
        kind: &ToolCallKind,
        runtime_payload: &RawPayloadRef,
    ) -> Result<Option<TerminalOperationId>> {
        let Some(operation_kind) = terminal_operation_kind(kind) else {
            return Ok(None);
        };

        let payload = self.read_payload_json(runtime_payload)?;
        let payload: ExecCommandBeginPayload =
            serde_json::from_value(payload).with_context(|| {
                format!(
                    "parse terminal runtime start payload {}",
                    runtime_payload.raw_payload_id
                )
            })?;
        let request = parse_protocol_terminal_request(payload, &operation_kind);
        self.insert_terminal_operation(TerminalOperationStart {
            seq,
            wall_time_unix_ms,
            thread_id,
            tool_call_id,
            operation_kind,
            raw_payload: runtime_payload,
            request,
        })
    }

    fn insert_terminal_operation(
        &mut self,
        start: TerminalOperationStart<'_>,
    ) -> Result<Option<TerminalOperationId>> {
        let operation_id = self.next_terminal_operation_id();
        let ParsedTerminalRequest {
            process_id,
            request,
        } = start.request;

        self.rollout.terminal_operations.insert(
            operation_id.clone(),
            TerminalOperation {
                operation_id: operation_id.clone(),
                terminal_id: None,
                tool_call_id: start.tool_call_id.to_string(),
                kind: start.operation_kind,
                execution: ExecutionWindow {
                    started_at_unix_ms: start.wall_time_unix_ms,
                    started_seq: start.seq,
                    ended_at_unix_ms: None,
                    ended_seq: None,
                    status: ExecutionStatus::Running,
                },
                request,
                result: None,
                model_observations: Vec::new(),
                raw_payload_ids: vec![start.raw_payload.raw_payload_id.clone()],
            },
        );

        if let Some(process_id) = process_id {
            self.attach_terminal_session(start.thread_id, &process_id, &operation_id)?;
        }

        Ok(Some(operation_id))
    }

    /// Completes the terminal operation associated with a tool call, if one exists.
    ///
    /// Non-terminal tools flow through the same generic tool lifecycle, so callers
    /// may invoke this unconditionally and receive Ok for unrelated tool kinds.
    pub(in crate::reducer) fn end_terminal_operation(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        thread_id: &str,
        operation_id: &str,
        status: ExecutionStatus,
        response_payload: Option<&RawPayloadRef>,
    ) -> Result<()> {
        let Some(operation_kind) = self
            .rollout
            .terminal_operations
            .get(operation_id)
            .map(|operation| operation.kind.clone())
        else {
            bail!("terminal end referenced unknown operation {operation_id}");
        };
        let response = response_payload
            .map(|payload| {
                let value = self.read_payload_json(payload)?;
                let response = parse_terminal_response_payload(
                    value,
                    &operation_kind,
                    &payload.raw_payload_id,
                )?;
                Ok::<_, anyhow::Error>((payload.raw_payload_id.clone(), response))
            })
            .transpose()?;

        let process_id = {
            let Some(operation) = self.rollout.terminal_operations.get_mut(operation_id) else {
                bail!("terminal end referenced unknown operation {operation_id}");
            };
            operation.execution.ended_at_unix_ms = Some(wall_time_unix_ms);
            operation.execution.ended_seq = Some(seq);
            operation.execution.status = status;

            if let Some((raw_payload_id, response)) = response {
                push_unique(&mut operation.raw_payload_ids, &raw_payload_id);
                operation.result = Some(response.result);
                response.process_id
            } else {
                None
            }
        };

        if let Some(process_id) = process_id {
            self.attach_terminal_session(thread_id, &process_id, operation_id)?;
        }

        Ok(())
    }

    fn attach_terminal_session(
        &mut self,
        thread_id: &str,
        process_id: &str,
        operation_id: &str,
    ) -> Result<()> {
        let Some(operation) = self.rollout.terminal_operations.get_mut(operation_id) else {
            bail!("terminal session referenced unknown operation {operation_id}");
        };
        if let Some(terminal_id) = &operation.terminal_id {
            let existing = &self.process_ids_by_terminal[terminal_id];
            if existing != process_id {
                bail!(
                    "terminal operation {operation_id} changed process id from {existing} to {process_id}"
                );
            }
            return Ok(());
        }

        let key = (thread_id.to_string(), process_id.to_string());
        let existing = match operation.kind {
            TerminalOperationKind::ExecCommand => None,
            TerminalOperationKind::WriteStdin => self.terminal_ids_by_process.get(&key).cloned(),
        };
        let terminal_id = existing.unwrap_or_else(|| format!("terminal_session:{operation_id}"));
        let session = self
            .rollout
            .terminal_sessions
            .entry(terminal_id.clone())
            .or_insert_with(|| TerminalSession {
                terminal_id: terminal_id.clone(),
                thread_id: thread_id.to_string(),
                created_by_operation_id: operation_id.to_string(),
                operation_ids: Vec::new(),
                execution: ExecutionWindow {
                    started_at_unix_ms: operation.execution.started_at_unix_ms,
                    started_seq: operation.execution.started_seq,
                    // Raw events do not report session release: buffered output
                    // can still be collected after the process exits.
                    ended_at_unix_ms: None,
                    ended_seq: None,
                    status: ExecutionStatus::Running,
                },
            });
        push_unique(&mut session.operation_ids, operation_id);
        operation.terminal_id = Some(terminal_id.clone());
        self.process_ids_by_terminal
            .insert(terminal_id.clone(), process_id.to_string());
        self.terminal_ids_by_process.insert(key, terminal_id);
        Ok(())
    }

    /// Replaces the provisional handle-based join with the runtime's actual target.
    pub(in crate::reducer) fn resolve_stdin_target(
        &mut self,
        tool_call_id: &str,
        exec_tool_call_id: &str,
    ) -> Result<()> {
        let stdin = self
            .rollout
            .tool_calls
            .get(tool_call_id)
            .with_context(|| format!("stdin target referenced unknown call {tool_call_id}"))?;
        let exec = self
            .rollout
            .tool_calls
            .get(exec_tool_call_id)
            .with_context(|| format!("stdin target referenced unknown exec {exec_tool_call_id}"))?;
        let (Some(operation_id), Some(exec_operation_id)) =
            (&stdin.terminal_operation_id, &exec.terminal_operation_id)
        else {
            // A failed payload write can leave either call without a terminal projection.
            return Ok(());
        };
        let Some(terminal_id) = self.rollout.terminal_operations[exec_operation_id]
            .terminal_id
            .clone()
        else {
            return Ok(());
        };
        let operation = self
            .rollout
            .terminal_operations
            .get_mut(operation_id)
            .with_context(|| format!("stdin target referenced unknown operation {operation_id}"))?;
        if operation.terminal_id.as_ref() == Some(&terminal_id) {
            return Ok(());
        }
        // Move this operation without rewinding the handle lookup: a newer exec
        // may already own that handle by the time resolution is recorded.
        if let Some(previous) = operation.terminal_id.replace(terminal_id.clone()) {
            let session = self
                .rollout
                .terminal_sessions
                .get_mut(&previous)
                .with_context(|| format!("stdin target referenced unknown session {previous}"))?;
            session.operation_ids.retain(|id| id != operation_id);
            if session.operation_ids.is_empty() {
                self.rollout.terminal_sessions.remove(&previous);
                self.process_ids_by_terminal.remove(&previous);
            }
        }
        self.rollout
            .terminal_sessions
            .get_mut(&terminal_id)
            .with_context(|| format!("stdin target referenced unknown session {terminal_id}"))?
            .operation_ids
            .push(operation_id.clone());
        Ok(())
    }

    /// Mirrors model-visible tool items onto the terminal observation view.
    ///
    /// Runtime terminal rows are useful on their own, but the model-visible call
    /// and output item ids let viewers jump between transcript and terminal timelines.
    pub(in crate::reducer) fn sync_terminal_model_observation(
        &mut self,
        tool_call_id: &str,
    ) -> Result<()> {
        let Some(tool_call) = self.rollout.tool_calls.get(tool_call_id) else {
            bail!("tool call {tool_call_id} disappeared during terminal observation linking");
        };
        let Some(operation_id) = tool_call.terminal_operation_id.clone() else {
            return Ok(());
        };
        let call_item_ids = tool_call.model_visible_call_item_ids.clone();
        let output_item_ids = tool_call.model_visible_output_item_ids.clone();
        if call_item_ids.is_empty() && output_item_ids.is_empty() {
            return Ok(());
        }

        let Some(operation) = self.rollout.terminal_operations.get_mut(&operation_id) else {
            bail!("terminal operation {operation_id} disappeared during observation linking");
        };
        // A terminal result and a model-visible tool output are intentionally
        // separate: the former is what the runtime saw, the latter is what later
        // inference payloads prove was shown back to the model.
        if let Some(observation) = operation
            .model_observations
            .iter_mut()
            .find(|observation| observation.source == TerminalObservationSource::DirectToolCall)
        {
            observation.call_item_ids = call_item_ids;
            observation.output_item_ids = output_item_ids;
        } else {
            operation.model_observations.push(TerminalModelObservation {
                call_item_ids,
                output_item_ids,
                source: TerminalObservationSource::DirectToolCall,
            });
        }
        Ok(())
    }

    fn next_terminal_operation_id(&mut self) -> TerminalOperationId {
        let ordinal = self.next_terminal_operation_ordinal;
        self.next_terminal_operation_ordinal += 1;
        format!("terminal_operation:{ordinal}")
    }
}

fn terminal_operation_kind(kind: &ToolCallKind) -> Option<TerminalOperationKind> {
    match kind {
        ToolCallKind::ExecCommand => Some(TerminalOperationKind::ExecCommand),
        ToolCallKind::WriteStdin => Some(TerminalOperationKind::WriteStdin),
        ToolCallKind::ApplyPatch
        | ToolCallKind::Mcp { .. }
        | ToolCallKind::Web
        | ToolCallKind::ImageGeneration
        | ToolCallKind::SpawnAgent
        | ToolCallKind::AssignAgentTask
        | ToolCallKind::SendMessage
        | ToolCallKind::WaitAgent
        | ToolCallKind::CloseAgent
        | ToolCallKind::Other { .. } => None,
    }
}

struct TerminalOperationStart<'a> {
    seq: RawEventSeq,
    wall_time_unix_ms: i64,
    thread_id: &'a str,
    tool_call_id: &'a str,
    operation_kind: TerminalOperationKind,
    raw_payload: &'a RawPayloadRef,
    request: ParsedTerminalRequest,
}

struct ParsedTerminalRequest {
    process_id: Option<String>,
    request: TerminalRequest,
}

struct ParsedTerminalResponse {
    process_id: Option<String>,
    result: TerminalResult,
}

fn parse_protocol_terminal_request(
    payload: ExecCommandBeginPayload,
    operation_kind: &TerminalOperationKind,
) -> ParsedTerminalRequest {
    // Startup/poll paths usually include a process id at begin time, but plain
    // exec starts may only learn it in the matching end event.
    let process_id = payload.process_id.clone();
    let request = match operation_kind {
        TerminalOperationKind::ExecCommand => TerminalRequest::ExecCommand {
            display_command: payload.command.join(" "),
            command: payload.command,
            cwd: payload.cwd,
            yield_time_ms: None,
            max_output_tokens: None,
        },
        TerminalOperationKind::WriteStdin => TerminalRequest::WriteStdin {
            stdin: payload.interaction_input.unwrap_or_default(),
            yield_time_ms: None,
            max_output_tokens: None,
        },
    };
    ParsedTerminalRequest {
        process_id,
        request,
    }
}

fn parse_dispatch_terminal_request(value: JsonValue) -> Result<Option<ParsedTerminalRequest>> {
    let payload: DispatchedToolTraceRequestPayload = serde_json::from_value(value)?;
    // Recorded kinds are name-based. Only the built-in write_stdin owns terminal
    // sessions; core records its default namespace as None. Namespaced aliases
    // must wait for runtime classification, even if their arguments look native.
    if payload.tool_namespace.is_some() {
        return Ok(None);
    }
    if payload.tool_name != "write_stdin" {
        bail!(
            "dispatch terminal request is for {}, not write_stdin",
            payload.tool_name
        );
    }
    if payload.payload.kind != "function" {
        bail!(
            "write_stdin dispatch payload used unsupported {} payload",
            payload.payload.kind
        );
    }
    let arguments = payload
        .payload
        .arguments
        .context("write_stdin dispatch payload omitted function arguments")?;
    let args: DispatchedWriteStdinArgs = serde_json::from_str(&arguments)
        .context("parse write_stdin dispatch function arguments")?;
    let process_id = process_id_from_json(&args.session_id)
        .context("write_stdin dispatch payload omitted session_id")?;

    Ok(Some(ParsedTerminalRequest {
        process_id: Some(process_id),
        request: TerminalRequest::WriteStdin {
            stdin: args.chars,
            yield_time_ms: args.yield_time_ms,
            max_output_tokens: args.max_output_tokens,
        },
    }))
}

fn parse_terminal_response_payload(
    value: JsonValue,
    operation_kind: &TerminalOperationKind,
    raw_payload_id: &str,
) -> Result<ParsedTerminalResponse> {
    match operation_kind {
        TerminalOperationKind::ExecCommand => {
            let payload = serde_json::from_value::<ExecCommandEndPayload>(value)
                .with_context(|| format!("parse exec terminal response {raw_payload_id}"))?;
            Ok(parse_protocol_terminal_response(payload))
        }
        TerminalOperationKind::WriteStdin => {
            match serde_json::from_value::<ExecCommandEndPayload>(value.clone()) {
                Ok(payload) => Ok(parse_protocol_terminal_response(payload)),
                Err(protocol_err) => parse_dispatch_terminal_response(value).with_context(|| {
                    format!(
                        "parse write_stdin terminal response {raw_payload_id} as protocol payload \
                         ({protocol_err}) or dispatch payload"
                    )
                }),
            }
        }
    }
}

fn parse_protocol_terminal_response(payload: ExecCommandEndPayload) -> ParsedTerminalResponse {
    ParsedTerminalResponse {
        process_id: payload.process_id,
        result: TerminalResult {
            exit_code: Some(payload.exit_code),
            stdout: payload.aggregated_output,
            stderr: String::new(),
            formatted_output: None,
            original_token_count: None,
            chunk_id: None,
        },
    }
}

fn parse_dispatch_terminal_response(value: JsonValue) -> Result<ParsedTerminalResponse> {
    let payload: DispatchedToolTraceResponsePayload = serde_json::from_value(value)?;
    let result = match payload {
        DispatchedToolTraceResponsePayload::DirectResponse { response_item } => {
            let output = response_item
                .get("output")
                .and_then(json_text_content)
                .unwrap_or_else(|| response_item.to_string());
            TerminalResult {
                exit_code: None,
                stdout: output.clone(),
                stderr: String::new(),
                formatted_output: Some(output),
                original_token_count: None,
                chunk_id: None,
            }
        }
        DispatchedToolTraceResponsePayload::CodeModeResponse { value } => {
            // Code-mode returns the JavaScript-facing tool value, not the text
            // shown to the model. For write_stdin that value is the structured
            // unified-exec result, so keep ToolCall.raw_result_payload_id as the
            // raw boundary while projecting terminal-specific fields here.
            parse_code_mode_exec_result(value)
        }
        DispatchedToolTraceResponsePayload::Error { error } => TerminalResult {
            exit_code: None,
            stdout: String::new(),
            stderr: error.clone(),
            formatted_output: Some(error),
            original_token_count: None,
            chunk_id: None,
        },
    };
    Ok(ParsedTerminalResponse {
        process_id: None,
        result,
    })
}

fn parse_code_mode_exec_result(value: JsonValue) -> TerminalResult {
    match serde_json::from_value::<CodeModeExecResult>(value.clone()) {
        Ok(result) => TerminalResult {
            exit_code: result.exit_code,
            stdout: result.output.clone(),
            stderr: String::new(),
            formatted_output: Some(result.output),
            original_token_count: result.original_token_count,
            chunk_id: result.chunk_id,
        },
        Err(_) => {
            let output = json_text_content(&value).unwrap_or_else(|| value.to_string());
            TerminalResult {
                exit_code: None,
                stdout: output.clone(),
                stderr: String::new(),
                formatted_output: Some(output),
                original_token_count: None,
                chunk_id: None,
            }
        }
    }
}

fn json_text_content(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(text) => Some(text.clone()),
        JsonValue::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| item.get("text").and_then(JsonValue::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        JsonValue::Null => None,
        other => Some(other.to_string()),
    }
}

fn process_id_from_json(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) if !value.is_empty() => Some(value.clone()),
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[derive(Deserialize)]
struct ExecCommandBeginPayload {
    process_id: Option<String>,
    command: Vec<String>,
    cwd: String,
    interaction_input: Option<String>,
}

#[derive(Deserialize)]
struct ExecCommandEndPayload {
    process_id: Option<String>,
    #[serde(default)]
    aggregated_output: String,
    exit_code: i32,
}

#[derive(Deserialize)]
struct DispatchedToolTraceRequestPayload {
    tool_name: String,
    tool_namespace: Option<String>,
    payload: DispatchedToolPayload,
}

#[derive(Deserialize)]
struct DispatchedToolPayload {
    #[serde(rename = "type")]
    kind: String,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct DispatchedWriteStdinArgs {
    session_id: JsonValue,
    #[serde(default)]
    chars: String,
    yield_time_ms: Option<u64>,
    max_output_tokens: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum DispatchedToolTraceResponsePayload {
    DirectResponse { response_item: JsonValue },
    CodeModeResponse { value: JsonValue },
    Error { error: String },
}

#[derive(Deserialize)]
struct CodeModeExecResult {
    chunk_id: Option<String>,
    exit_code: Option<i32>,
    original_token_count: Option<usize>,
    output: String,
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
