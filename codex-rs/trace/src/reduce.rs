use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

use crate::model::*;
use crate::payload::RawPayloadKind;
use crate::payload::RawPayloadRef as ModelRawPayloadRef;

pub const REDUCED_STATE_FILE_NAME: &str = "state.json";

#[derive(Debug)]
struct CapturedEvent {
    seq: u64,
    wall_time_unix_ms: i64,
    fields: Map<String, Value>,
}

struct Reducer<'a> {
    bundle_dir: &'a Path,
    trace: RolloutTrace,
    code_cell_ids_by_runtime: BTreeMap<(String, String), String>,
    agent_result_observations: BTreeMap<String, AgentResultObservation>,
    next_conversation_item_ordinal: u64,
    next_terminal_operation_ordinal: u64,
}

#[derive(Debug, Clone)]
struct AgentResultObservation {
    child_thread_id: String,
    child_turn_id: String,
    parent_thread_id: String,
    message: String,
    observed_at_unix_ms: i64,
}

pub fn reduce_bundle_to_path(bundle_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let trace = reduce_bundle(bundle_dir.as_ref())?;
    let file = File::create(output.as_ref())
        .with_context(|| format!("create {}", output.as_ref().display()))?;
    serde_json::to_writer_pretty(file, &trace)
        .with_context(|| format!("write {}", output.as_ref().display()))
}

fn reduce_bundle(bundle_dir: &Path) -> Result<RolloutTrace> {
    let manifest = read_json(bundle_dir.join("manifest.json")).unwrap_or_else(|_| json!({}));
    let trace_id = manifest
        .get("trace_id")
        .and_then(Value::as_str)
        .unwrap_or("trace")
        .to_string();
    let started_at_unix_ms = manifest
        .get("started_at_unix_ms")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut reducer = Reducer {
        bundle_dir,
        trace: RolloutTrace::new(
            1,
            trace_id.clone(),
            trace_id,
            String::new(),
            started_at_unix_ms,
        ),
        code_cell_ids_by_runtime: BTreeMap::new(),
        agent_result_observations: BTreeMap::new(),
        next_conversation_item_ordinal: 1,
        next_terminal_operation_ordinal: 1,
    };

    let event_log_path = bundle_dir.join("events.jsonl");
    let event_log = File::open(&event_log_path)
        .with_context(|| format!("open trace event log {}", event_log_path.display()))?;
    for (line_index, line) in BufReader::new(event_log).lines().enumerate() {
        let line = line.with_context(|| format!("read trace event line {}", line_index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let event = parse_captured_event(&line, line_index + 1)?;
        reducer.apply_event(event)?;
    }
    reducer.link_tools_to_conversation_items();
    reducer.link_code_cells_to_conversation_items();
    reducer.drop_redundant_code_cell_tool_calls();
    reducer.sync_terminal_model_observations();
    reducer.resolve_agent_result_edges();
    reducer.attach_tool_payloads_to_interaction_edges();
    if reducer.trace.started_at_unix_ms == 0 {
        reducer.trace.started_at_unix_ms = reducer
            .trace
            .threads
            .values()
            .map(|thread| thread.execution.started_at_unix_ms)
            .min()
            .unwrap_or(0);
    }
    reducer.trace.ended_at_unix_ms = reducer
        .trace
        .threads
        .values()
        .filter_map(|thread| thread.execution.ended_at_unix_ms)
        .max();
    if reducer.trace.ended_at_unix_ms.is_some() {
        reducer.trace.status = RolloutStatus::Completed;
    }
    Ok(reducer.trace)
}

impl Reducer<'_> {
    fn apply_event(&mut self, event: CapturedEvent) -> Result<()> {
        self.insert_payload_ref(&event, "request");
        self.insert_payload_ref(&event, "response");
        self.insert_payload_ref(&event, "invocation");
        self.insert_payload_ref(&event, "result");
        self.insert_payload_ref(&event, "agent_result");

        match event_name(&event).unwrap_or_default() {
            "codex.thread.started" | "thread_started" => self.thread_started(&event),
            "codex.turn.started" | "turn_started" => self.turn_started(&event),
            "codex.turn.ended" | "turn_ended" => self.turn_ended(&event),
            "codex.inference.started" | "inference_started" => self.inference_started(&event)?,
            "codex.inference.completed" | "inference_completed" => {
                self.inference_completed(&event)?;
            }
            "codex.inference.failed" | "inference_failed" => self.inference_failed(&event),
            "codex.tool.started" | "tool_started" => self.tool_started(&event)?,
            "codex.tool.ended" | "tool_ended" => self.tool_ended(&event),
            "codex.code_cell.started" => self.code_cell_started(&event),
            "codex.code_cell.ended" => self.code_cell_ended(&event),
            "codex.collab.spawn.started" => self.tool_to_thread_edge_started(&event, "spawn_agent"),
            "codex.collab.spawn.ended" => self.tool_to_thread_edge_ended(&event, "spawn_agent"),
            "codex.collab.message.started" => {
                self.tool_to_thread_edge_started(&event, "send_message");
            }
            "codex.collab.message.ended" => self.tool_to_thread_edge_ended(&event, "send_message"),
            "codex.collab.agent_result.observed" => self.agent_result_observed(&event),
            "codex.collab.close.started" => self.tool_to_thread_edge_started(&event, "close_agent"),
            "codex.collab.close.ended" => self.tool_to_thread_edge_ended(&event, "close_agent"),
            _ => {}
        }
        Ok(())
    }

    fn insert_payload_ref(&mut self, event: &CapturedEvent, prefix: &str) {
        let dotted_prefix = format!("raw_payload.{prefix}");
        let legacy_prefix = format!("raw_{prefix}");
        let id = field_str(event, &format!("{dotted_prefix}.id"))
            .or_else(|| field_str(event, &format!("{legacy_prefix}_payload_id")));
        let path = field_str(event, &format!("{dotted_prefix}.path"))
            .or_else(|| field_str(event, &format!("{legacy_prefix}_payload_path")));
        let kind = field_str(event, &format!("{dotted_prefix}.kind"))
            .or_else(|| field_str(event, &format!("{legacy_prefix}_payload_kind")));
        let (Some(id), Some(path), Some(kind)) = (id, path, kind) else {
            return;
        };
        if id.is_empty() || path.is_empty() {
            return;
        }
        self.trace.raw_payloads.insert(
            id.to_string(),
            ModelRawPayloadRef {
                raw_payload_id: id.to_string(),
                kind: raw_payload_kind(kind),
                path: path.to_string(),
            },
        );
    }

    fn ensure_thread(&mut self, thread_id: &str, now: i64) {
        if self.trace.root_thread_id.is_empty() {
            self.trace.root_thread_id = thread_id.to_string();
        }
        self.trace
            .threads
            .entry(thread_id.to_string())
            .or_insert_with(|| AgentThread {
                thread_id: thread_id.to_string(),
                agent_path: "/root".to_string(),
                nickname: None,
                origin: AgentOrigin::Root,
                execution: execution_window(now, None, ExecutionStatus::Running, 0, None),
                default_model: None,
                conversation_item_ids: Vec::new(),
            });
    }

    fn thread_started(&mut self, event: &CapturedEvent) {
        let Some(thread_id) = trace_field_str(event, "thread", "id") else {
            return;
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        if let Some(thread) = self.trace.threads.get_mut(thread_id) {
            let agent_path = trace_field_str(event, "agent", "path")
                .unwrap_or("/root")
                .to_string();
            thread.agent_path = agent_path.clone();
            thread.default_model = field_str(event, "default_model")
                .or_else(|| field_str(event, "model"))
                .filter(|model| !model.is_empty())
                .map(str::to_string);
            let parent_thread_id = trace_field_str(event, "parent_thread", "id").unwrap_or("");
            thread.origin = if parent_thread_id.is_empty() {
                AgentOrigin::Root
            } else {
                AgentOrigin::Spawned {
                    parent_thread_id: parent_thread_id.to_string(),
                    spawn_edge_id: String::new(),
                    task_name: task_name_from_agent_path(&agent_path),
                    agent_role: String::new(),
                }
            };
            set_execution_start(&mut thread.execution, event.wall_time_unix_ms, event.seq);
        }
    }

    fn turn_started(&mut self, event: &CapturedEvent) {
        let (Some(thread_id), Some(turn_id)) = (
            trace_field_str(event, "thread", "id"),
            trace_field_str(event, "turn", "id"),
        ) else {
            return;
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        self.trace.codex_turns.insert(
            turn_id.to_string(),
            CodexTurn {
                codex_turn_id: turn_id.to_string(),
                thread_id: thread_id.to_string(),
                execution: execution_window(
                    event.wall_time_unix_ms,
                    None,
                    ExecutionStatus::Running,
                    event.seq,
                    None,
                ),
                input_item_ids: Vec::new(),
            },
        );
    }

    fn turn_ended(&mut self, event: &CapturedEvent) {
        let Some(turn_id) = trace_field_str(event, "turn", "id") else {
            return;
        };
        if let Some(turn) = self.trace.codex_turns.get_mut(turn_id) {
            set_execution_end(
                &mut turn.execution,
                event.wall_time_unix_ms,
                event.seq,
                execution_status(field_str(event, "status").unwrap_or("completed")),
            );
        }
    }

    fn inference_started(&mut self, event: &CapturedEvent) -> Result<()> {
        let (Some(inference_id), Some(thread_id), Some(turn_id)) = (
            trace_field_str(event, "inference", "id"),
            trace_field_str(event, "thread", "id"),
            trace_field_str(event, "turn", "id"),
        ) else {
            return Ok(());
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        let request_payload_id = raw_payload_field_str(event, "request", "id").unwrap_or("");
        let request_item_ids = self.add_request_items(thread_id, event, request_payload_id)?;
        self.trace.inference_calls.insert(
            inference_id.to_string(),
            InferenceCall {
                inference_call_id: inference_id.to_string(),
                thread_id: thread_id.to_string(),
                codex_turn_id: turn_id.to_string(),
                execution: execution_window(
                    event.wall_time_unix_ms,
                    None,
                    ExecutionStatus::Running,
                    event.seq,
                    None,
                ),
                model: field_str(event, "model").unwrap_or("").to_string(),
                provider_name: trace_field_str(event, "provider", "name")
                    .unwrap_or("")
                    .to_string(),
                upstream_request_id: None,
                request_item_ids,
                response_item_ids: Vec::new(),
                tool_call_ids_started_by_response: Vec::new(),
                usage: None,
                raw_request_payload_id: request_payload_id.to_string(),
                raw_response_payload_id: None,
            },
        );
        Ok(())
    }

    fn inference_completed(&mut self, event: &CapturedEvent) -> Result<()> {
        let Some(inference_id) = trace_field_str(event, "inference", "id") else {
            return Ok(());
        };
        let response_payload_id = raw_payload_field_str(event, "response", "id").unwrap_or("");
        let (thread_id, response_item_ids, usage) =
            self.add_response_items(inference_id, event, response_payload_id)?;
        if let Some(inference) = self.trace.inference_calls.get_mut(inference_id) {
            set_execution_end(
                &mut inference.execution,
                event.wall_time_unix_ms,
                event.seq,
                ExecutionStatus::Completed,
            );
            inference.raw_response_payload_id = non_empty_string(response_payload_id);
            inference.response_item_ids = response_item_ids.clone();
            inference.usage = usage.and_then(token_usage_from_value);
        }
        if let Some(thread_id) = thread_id
            && let Some(thread) = self.trace.threads.get_mut(&thread_id)
        {
            extend_unique(&mut thread.conversation_item_ids, &response_item_ids);
        }
        Ok(())
    }

    fn inference_failed(&mut self, event: &CapturedEvent) {
        let Some(inference_id) = trace_field_str(event, "inference", "id") else {
            return;
        };
        if let Some(inference) = self.trace.inference_calls.get_mut(inference_id) {
            set_execution_end(
                &mut inference.execution,
                event.wall_time_unix_ms,
                event.seq,
                ExecutionStatus::Failed,
            );
        }
    }

    fn tool_started(&mut self, event: &CapturedEvent) -> Result<()> {
        let (Some(tool_call_id), Some(thread_id), Some(turn_id)) = (
            trace_field_str(event, "tool", "call_id"),
            trace_field_str(event, "thread", "id"),
            trace_field_str(event, "turn", "id"),
        ) else {
            return Ok(());
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        let tool_name = trace_field_str(event, "tool", "name").unwrap_or("tool");
        let raw_invocation_payload_id =
            raw_payload_field_str(event, "invocation", "id").unwrap_or("");
        let model_visible_call_id = trace_field_str(event, "model_visible_call", "id")
            .filter(|call_id| !call_id.is_empty());
        let code_mode_runtime_tool_id =
            field_str(event, "code_mode_runtime_tool.id").filter(|tool_id| !tool_id.is_empty());
        let requester = self.tool_requester(event, thread_id);
        let terminal_operation_id = self.start_terminal_operation_for_tool(
            event,
            thread_id,
            tool_call_id,
            tool_name,
            raw_invocation_payload_id,
        )?;
        self.trace.tool_calls.insert(
            tool_call_id.to_string(),
            ToolCall {
                tool_call_id: tool_call_id.to_string(),
                model_visible_call_id: model_visible_call_id.map(str::to_string),
                code_mode_runtime_tool_id: code_mode_runtime_tool_id.map(str::to_string),
                thread_id: thread_id.to_string(),
                started_by_codex_turn_id: Some(turn_id.to_string()),
                execution: execution_window(
                    event.wall_time_unix_ms,
                    None,
                    ExecutionStatus::Running,
                    event.seq,
                    None,
                ),
                requester,
                kind: tool_call_kind(tool_name),
                model_visible_call_item_ids: Vec::new(),
                model_visible_output_item_ids: Vec::new(),
                terminal_operation_id: terminal_operation_id.clone(),
                summary: terminal_operation_id.map_or_else(
                    || ToolCallSummary::Generic {
                        label: tool_name.to_string(),
                        input_preview: Some(String::new()),
                        output_preview: Some(String::new()),
                    },
                    |operation_id| ToolCallSummary::Terminal { operation_id },
                ),
                raw_invocation_payload_id: non_empty_string(raw_invocation_payload_id),
                raw_result_payload_id: None,
                raw_runtime_payload_ids: Vec::new(),
            },
        );
        self.link_tool_call_to_code_cell(tool_call_id);
        Ok(())
    }

    fn tool_ended(&mut self, event: &CapturedEvent) {
        let Some(tool_call_id) = trace_field_str(event, "tool", "call_id") else {
            return;
        };
        let raw_result_payload_id = raw_payload_field_str(event, "result", "id").unwrap_or("");
        let mut terminal_operation_id = None;
        let mut thread_id = None;
        if let Some(tool) = self.trace.tool_calls.get_mut(tool_call_id) {
            let status = field_str(event, "status").unwrap_or("completed");
            set_execution_end(
                &mut tool.execution,
                event.wall_time_unix_ms,
                event.seq,
                execution_status(status),
            );
            if let ToolCallSummary::Generic { output_preview, .. } = &mut tool.summary {
                *output_preview =
                    Some(field_str(event, "output_preview").unwrap_or("").to_string());
            }
            tool.raw_result_payload_id = non_empty_string(raw_result_payload_id);
            terminal_operation_id = tool.terminal_operation_id.clone();
            thread_id = Some(tool.thread_id.clone());
        }
        if let Some(operation_id) = terminal_operation_id {
            self.end_terminal_operation(
                &operation_id,
                thread_id.as_deref().unwrap_or(""),
                event,
                raw_result_payload_id,
            );
        }
    }

    fn code_cell_started(&mut self, event: &CapturedEvent) {
        let (Some(thread_id), Some(turn_id), Some(runtime_cell_id), Some(model_visible_call_id)) = (
            trace_field_str(event, "thread", "id"),
            trace_field_str(event, "turn", "id"),
            field_str(event, "code_cell.runtime_id"),
            trace_field_str(event, "model_visible_call", "id"),
        ) else {
            return;
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        let code_cell_id = reduced_code_cell_id(model_visible_call_id);
        self.code_cell_ids_by_runtime.insert(
            (thread_id.to_string(), runtime_cell_id.to_string()),
            code_cell_id.clone(),
        );
        self.trace
            .code_cells
            .entry(code_cell_id.clone())
            .or_insert_with(|| CodeCell {
                code_cell_id: code_cell_id.clone(),
                model_visible_call_id: model_visible_call_id.to_string(),
                thread_id: thread_id.to_string(),
                codex_turn_id: turn_id.to_string(),
                source_item_id: String::new(),
                output_item_ids: Vec::new(),
                runtime_cell_id: Some(runtime_cell_id.to_string()),
                execution: execution_window(
                    event.wall_time_unix_ms,
                    None,
                    ExecutionStatus::Running,
                    event.seq,
                    None,
                ),
                runtime_status: CodeCellRuntimeStatus::Running,
                initial_response_at_unix_ms: None,
                initial_response_seq: None,
                yielded_at_unix_ms: None,
                yielded_seq: None,
                source_js: field_str(event, "code_cell.source_js")
                    .unwrap_or("")
                    .to_string(),
                nested_tool_call_ids: Vec::new(),
                wait_tool_call_ids: Vec::new(),
            });
    }

    fn code_cell_ended(&mut self, event: &CapturedEvent) {
        let (Some(thread_id), Some(runtime_cell_id)) = (
            trace_field_str(event, "thread", "id"),
            field_str(event, "code_cell.runtime_id"),
        ) else {
            return;
        };
        let Some(code_cell_id) = self
            .code_cell_ids_by_runtime
            .get(&(thread_id.to_string(), runtime_cell_id.to_string()))
            .cloned()
        else {
            return;
        };
        if let Some(cell) = self.trace.code_cells.get_mut(&code_cell_id) {
            let runtime_status = field_str(event, "status").unwrap_or("completed");
            cell.runtime_status = code_cell_runtime_status(runtime_status);
            if matches!(cell.runtime_status, CodeCellRuntimeStatus::Yielded) {
                // Yielding is a partial result: the model-visible custom
                // `exec` call has returned a cell id, but the runtime cell is
                // still alive and may later complete through `wait`.
                cell.initial_response_at_unix_ms = Some(event.wall_time_unix_ms);
                cell.initial_response_seq = Some(event.seq);
                cell.yielded_at_unix_ms = Some(event.wall_time_unix_ms);
                cell.yielded_seq = Some(event.seq);
            } else {
                set_execution_end(
                    &mut cell.execution,
                    event.wall_time_unix_ms,
                    event.seq,
                    code_cell_execution_status(runtime_status),
                );
            }
            if let Some(wait_call_id) = trace_field_str(event, "model_visible_wait_call", "id")
                .filter(|call_id| !call_id.is_empty())
            {
                let tool_call_id = normalize_tool_call_id(wait_call_id);
                push_unique(&mut cell.wait_tool_call_ids, &tool_call_id);
            }
        }
    }

    fn tool_to_thread_edge_started(&mut self, event: &CapturedEvent, kind: &str) {
        let Some(tool_call_id) = normalized_tool_call_id(event) else {
            return;
        };
        let target_thread_id = trace_field_str(event, "target.thread", "id").unwrap_or("");
        let edge_id = interaction_edge_id(kind, &tool_call_id);
        self.upsert_interaction_edge(
            edge_id,
            interaction_edge_kind(kind),
            TraceAnchor::ToolCall { tool_call_id },
            thread_anchor(target_thread_id),
            event,
        );
    }

    fn tool_to_thread_edge_ended(&mut self, event: &CapturedEvent, kind: &str) {
        self.tool_to_thread_edge_started(event, kind);
        let mut completed_spawn_edge_id = None;
        if let Some(tool_call_id) = normalized_tool_call_id(event) {
            let edge_id = interaction_edge_id(kind, &tool_call_id);
            if let Some(edge) = self.trace.interaction_edges.get_mut(&edge_id) {
                edge.ended_at_unix_ms = Some(event.wall_time_unix_ms);
            }
            if kind == "spawn_agent" {
                completed_spawn_edge_id = Some(edge_id);
            }
        }
        self.apply_target_agent_metadata(event, completed_spawn_edge_id.as_deref());
    }

    fn agent_result_observed(&mut self, event: &CapturedEvent) {
        let (Some(child_thread_id), Some(child_turn_id), Some(parent_thread_id)) = (
            trace_field_str(event, "child.thread", "id").filter(|id| !id.is_empty()),
            trace_field_str(event, "child.turn", "id").filter(|id| !id.is_empty()),
            trace_field_str(event, "parent.thread", "id").filter(|id| !id.is_empty()),
        ) else {
            return;
        };
        let edge_id =
            agent_result_observed_edge_id(child_thread_id, child_turn_id, parent_thread_id);
        self.upsert_interaction_edge(
            edge_id.clone(),
            InteractionEdgeKind::AgentResult,
            TraceAnchor::Thread {
                thread_id: child_thread_id.to_string(),
            },
            Some(TraceAnchor::Thread {
                thread_id: parent_thread_id.to_string(),
            }),
            event,
        );
        if let Some(edge) = self.trace.interaction_edges.get_mut(&edge_id) {
            // This event is emitted at the exact control-plane handoff, so the
            // edge is an observed instant unless later instrumentation gives us
            // a wider lifecycle.
            edge.ended_at_unix_ms = Some(event.wall_time_unix_ms);
            if let Some(raw_payload_id) = raw_payload_field_str(event, "agent_result", "id") {
                push_unique(&mut edge.carried_raw_payload_ids, raw_payload_id);
            }
        }
        self.agent_result_observations.insert(
            edge_id,
            AgentResultObservation {
                child_thread_id: child_thread_id.to_string(),
                child_turn_id: child_turn_id.to_string(),
                parent_thread_id: parent_thread_id.to_string(),
                message: field_str(event, "message").unwrap_or("").to_string(),
                observed_at_unix_ms: event.wall_time_unix_ms,
            },
        );
    }

    fn upsert_interaction_edge(
        &mut self,
        edge_id: String,
        kind: InteractionEdgeKind,
        source: TraceAnchor,
        target: Option<TraceAnchor>,
        event: &CapturedEvent,
    ) {
        let edge = self
            .trace
            .interaction_edges
            .entry(edge_id.clone())
            .or_insert_with(|| InteractionEdge {
                edge_id,
                kind,
                source: source.clone(),
                target: target.clone().unwrap_or_else(|| source.clone()),
                started_at_unix_ms: event.wall_time_unix_ms,
                ended_at_unix_ms: None,
                carried_item_ids: Vec::new(),
                carried_raw_payload_ids: Vec::new(),
            });
        edge.source = source;
        if let Some(target) = target {
            edge.target = target;
        }
    }

    fn apply_target_agent_metadata(&mut self, event: &CapturedEvent, spawn_edge_id: Option<&str>) {
        let Some(target_thread_id) =
            trace_field_str(event, "target.thread", "id").filter(|thread_id| !thread_id.is_empty())
        else {
            return;
        };
        let Some(thread) = self.trace.threads.get_mut(target_thread_id) else {
            return;
        };
        if let Some(nickname) = trace_field_str(event, "target.agent", "nickname")
            .filter(|nickname| !nickname.is_empty())
        {
            thread.nickname = Some(nickname.to_string());
        }
        let fallback_task_name = task_name_from_agent_path(&thread.agent_path);
        if let AgentOrigin::Spawned {
            spawn_edge_id: existing_spawn_edge_id,
            task_name,
            agent_role,
            ..
        } = &mut thread.origin
        {
            if let Some(spawn_edge_id) = spawn_edge_id {
                *existing_spawn_edge_id = spawn_edge_id.to_string();
            }
            if task_name.is_empty() {
                *task_name = fallback_task_name;
            }
            if let Some(observed_role) =
                trace_field_str(event, "target.agent", "role").filter(|role| !role.is_empty())
            {
                *agent_role = observed_role.to_string();
            }
        }
    }

    fn add_request_items(
        &mut self,
        thread_id: &str,
        event: &CapturedEvent,
        raw_payload_id: &str,
    ) -> Result<Vec<String>> {
        let Some(payload) = self.payload_by_id(raw_payload_id)? else {
            return Ok(Vec::new());
        };

        let mut ids = Vec::new();
        let items = payload
            .get("input")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let thread_item_ids = self
            .trace
            .threads
            .get(thread_id)
            .map(|thread| thread.conversation_item_ids.clone())
            .unwrap_or_default();

        // Requests carry a full snapshot of model-visible input. Most items
        // should already exist from previous responses; request snapshots are
        // where we learn that those canonical items were consumed again. Match
        // by reduced content instead of raw JSON because reasoning can be
        // replayed later with only `encrypted_content` after the response
        // payload carried readable text.
        let mut used_existing_item_ids: Vec<String> = Vec::new();
        for item in items {
            let normalized = normalize_conversation_item(
                "",
                thread_id,
                event.wall_time_unix_ms,
                &item,
                Vec::new(),
                raw_payload_id,
            );
            let existing_item_id = thread_item_ids.iter().find(|item_id| {
                !used_existing_item_ids.contains(*item_id)
                    && self
                        .trace
                        .conversation_items
                        .get(*item_id)
                        .is_some_and(|item| conversation_item_matches(item, &normalized))
            });
            if let Some(item_id) = existing_item_id {
                push_unique(&mut ids, item_id);
                used_existing_item_ids.push(item_id.clone());
                continue;
            }

            let item_id = self.add_conversation_item(
                thread_id,
                event.wall_time_unix_ms,
                item,
                None,
                trace_field_str(event, "turn", "id"),
                raw_payload_id,
            );
            push_unique(&mut ids, &item_id);
            used_existing_item_ids.push(item_id);
        }
        if let Some(thread) = self.trace.threads.get_mut(thread_id) {
            extend_unique(&mut thread.conversation_item_ids, &ids);
        }
        Ok(ids)
    }

    fn add_response_items(
        &mut self,
        inference_id: &str,
        event: &CapturedEvent,
        raw_payload_id: &str,
    ) -> Result<(Option<String>, Vec<String>, Option<Value>)> {
        let Some(inference) = self.trace.inference_calls.get(inference_id) else {
            return Ok((None, Vec::new(), None));
        };
        let thread_id = Some(inference.thread_id.clone());
        let codex_turn_id = inference.codex_turn_id.clone();
        let Some(payload) = self.payload_by_id(raw_payload_id)? else {
            return Ok((thread_id, Vec::new(), None));
        };
        let items = payload
            .get("output_items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let producer = Some(ProducerRef::Inference {
            inference_call_id: inference_id.to_string(),
        });
        let ids = thread_id.as_deref().map_or_else(Vec::new, |thread_id| {
            self.add_conversation_items(
                thread_id,
                event.wall_time_unix_ms,
                items,
                producer,
                Some(&codex_turn_id),
                raw_payload_id,
            )
        });
        Ok((thread_id, ids, payload.get("token_usage").cloned()))
    }

    fn add_conversation_items(
        &mut self,
        thread_id: &str,
        first_seen_at_unix_ms: i64,
        items: Vec<Value>,
        producer: Option<ProducerRef>,
        codex_turn_id: Option<&str>,
        raw_payload_id: &str,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        for item in items {
            let item_id = self.add_conversation_item(
                thread_id,
                first_seen_at_unix_ms,
                item,
                producer.clone(),
                codex_turn_id,
                raw_payload_id,
            );
            ids.push(item_id);
        }
        ids
    }

    fn add_conversation_item(
        &mut self,
        thread_id: &str,
        first_seen_at_unix_ms: i64,
        item: Value,
        producer: Option<ProducerRef>,
        codex_turn_id: Option<&str>,
        raw_payload_id: &str,
    ) -> String {
        let item_id = format!("item:{}", self.next_conversation_item_ordinal);
        self.next_conversation_item_ordinal += 1;
        let produced_by = producer.into_iter().collect::<Vec<_>>();
        let mut normalized = normalize_conversation_item(
            &item_id,
            thread_id,
            first_seen_at_unix_ms,
            &item,
            produced_by,
            raw_payload_id,
        );
        normalized.codex_turn_id = codex_turn_id.map(str::to_string);
        self.trace
            .conversation_items
            .insert(item_id.clone(), normalized);
        item_id
    }

    fn tool_requester(&self, event: &CapturedEvent, thread_id: &str) -> ToolCallRequester {
        if field_str(event, "tool.requester.type") != Some("code_cell") {
            return ToolCallRequester::Model;
        }

        // Code-mode nested tools are not visible to the model as ordinary
        // request/response items. The runtime cell id is the stable bridge
        // from the tool event back to the model-visible `exec` code cell.
        let Some(runtime_cell_id) = field_str(event, "code_cell.runtime_id") else {
            return ToolCallRequester::CodeCell {
                code_cell_id: String::new(),
            };
        };
        let code_cell_id = self
            .code_cell_ids_by_runtime
            .get(&(thread_id.to_string(), runtime_cell_id.to_string()))
            .cloned()
            .unwrap_or_default();
        ToolCallRequester::CodeCell { code_cell_id }
    }

    fn link_tool_call_to_code_cell(&mut self, tool_call_id: &str) {
        let Some(code_cell_id) =
            self.trace
                .tool_calls
                .get(tool_call_id)
                .and_then(|tool| match &tool.requester {
                    ToolCallRequester::CodeCell { code_cell_id } => Some(code_cell_id.clone()),
                    ToolCallRequester::Model => None,
                })
        else {
            return;
        };
        if let Some(cell) = self.trace.code_cells.get_mut(&code_cell_id) {
            push_unique(&mut cell.nested_tool_call_ids, tool_call_id);
        }
    }

    fn start_terminal_operation_for_tool(
        &mut self,
        event: &CapturedEvent,
        thread_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        raw_invocation_payload_id: &str,
    ) -> Result<Option<TerminalOperationId>> {
        if !is_terminal_tool(tool_name) {
            return Ok(None);
        }

        // Terminal operations are a runtime view over shell-like tools. They
        // deliberately hang off the reduced tool call so direct model tools and
        // nested code-cell tools share the same terminal model.
        let operation_id = format!(
            "terminal_operation:{}",
            self.next_terminal_operation_ordinal
        );
        self.next_terminal_operation_ordinal += 1;
        let payload = self.payload_by_id(raw_invocation_payload_id)?;
        let terminal_id = payload
            .as_ref()
            .and_then(terminal_id_from_invocation_payload);
        let request = payload
            .as_ref()
            .map(terminal_request_from_invocation)
            .unwrap_or_else(|| TerminalRequest::ExecCommand {
                command: Vec::new(),
                display_command: String::new(),
                cwd: String::new(),
                yield_time_ms: None,
                max_output_tokens: None,
            });
        self.trace.terminal_operations.insert(
            operation_id.clone(),
            TerminalOperation {
                operation_id: operation_id.clone(),
                terminal_id: terminal_id.clone(),
                tool_call_id: tool_call_id.to_string(),
                kind: terminal_operation_kind(tool_name),
                execution: execution_window(
                    event.wall_time_unix_ms,
                    None,
                    ExecutionStatus::Running,
                    event.seq,
                    None,
                ),
                request,
                result: None,
                model_observations: Vec::new(),
                raw_payload_ids: non_empty_vec(raw_invocation_payload_id),
            },
        );
        // Continuation tools already name their terminal session in the request.
        // Create the session immediately so interrupted traces still group the
        // in-flight operation under the right terminal rail.
        if let Some(terminal_id) = terminal_id {
            self.ensure_terminal_session_for_operation(thread_id, &terminal_id, &operation_id);
        }
        Ok(Some(operation_id))
    }

    fn end_terminal_operation(
        &mut self,
        operation_id: &str,
        thread_id: &str,
        event: &CapturedEvent,
        raw_result_payload_id: &str,
    ) {
        let result_payload = self
            .payload_by_id(raw_result_payload_id)
            .ok()
            .flatten()
            .unwrap_or_else(|| json!({}));
        let existing_terminal_id = self
            .trace
            .terminal_operations
            .get(operation_id)
            .and_then(|operation| operation.terminal_id.clone());
        // Prefer real runtime ids over synthetic ids. Some continuation
        // results omit `session_id`, so preserve the id learned when the
        // operation started before inventing a one-shot synthetic session.
        let terminal_id = terminal_id_from_result_payload(&result_payload, "")
            .or(existing_terminal_id)
            .or_else(|| terminal_id_from_result_payload(&result_payload, operation_id));
        if let Some(operation) = self.trace.terminal_operations.get_mut(operation_id) {
            let status = field_str(event, "status").unwrap_or("completed");
            set_execution_end(
                &mut operation.execution,
                event.wall_time_unix_ms,
                event.seq,
                execution_status(status),
            );
            operation.terminal_id = terminal_id.clone();
            operation.result = Some(terminal_result_from_payload(
                &result_payload,
                field_str(event, "output_preview"),
            ));
            push_unique(&mut operation.raw_payload_ids, raw_result_payload_id);
        }
        if let Some(terminal_id) = terminal_id {
            self.ensure_terminal_session_for_operation(thread_id, &terminal_id, operation_id);
        }
    }

    fn ensure_terminal_session_for_operation(
        &mut self,
        thread_id: &str,
        terminal_id: &str,
        operation_id: &str,
    ) {
        if terminal_id.is_empty() || operation_id.is_empty() {
            return;
        }
        let operation_execution = self
            .trace
            .terminal_operations
            .get(operation_id)
            .map(|operation| operation.execution.clone())
            .unwrap_or_else(|| execution_window(0, None, ExecutionStatus::Running, 0, None));
        let session_execution = if terminal_id.starts_with("terminal:terminal_operation:") {
            operation_execution
        } else {
            let mut execution = operation_execution;
            execution.ended_at_unix_ms = None;
            execution.ended_seq = None;
            execution.status = ExecutionStatus::Running;
            execution
        };
        self.trace
            .terminal_sessions
            .entry(terminal_id.to_string())
            .or_insert_with(|| TerminalSession {
                terminal_id: terminal_id.to_string(),
                thread_id: thread_id.to_string(),
                created_by_operation_id: operation_id.to_string(),
                operation_ids: Vec::new(),
                execution: session_execution,
            });
        if let Some(session) = self.trace.terminal_sessions.get_mut(terminal_id) {
            push_unique(&mut session.operation_ids, operation_id);
        }
    }

    fn payload_by_id(&self, raw_payload_id: &str) -> Result<Option<Value>> {
        if raw_payload_id.is_empty() {
            return Ok(None);
        }
        let Some(payload) = self.trace.raw_payloads.get(raw_payload_id) else {
            return Ok(None);
        };
        read_json(self.bundle_dir.join(&payload.path)).map(Some)
    }

    fn link_tools_to_conversation_items(&mut self) {
        for tool in self.trace.tool_calls.values_mut() {
            let Some(call_id) = tool.model_visible_call_id.as_deref() else {
                continue;
            };
            let mut call_items = Vec::new();
            let mut output_items = Vec::new();
            for item in self.trace.conversation_items.values() {
                if item.call_id.as_deref() != Some(call_id) {
                    continue;
                }
                match item.kind {
                    ConversationItemKind::FunctionCall | ConversationItemKind::CustomToolCall => {
                        call_items.push(item.item_id.clone());
                    }
                    ConversationItemKind::FunctionCallOutput
                    | ConversationItemKind::CustomToolCallOutput => {
                        output_items.push(item.item_id.clone());
                    }
                    _ => {}
                }
            }
            tool.model_visible_call_item_ids = call_items;
            tool.model_visible_output_item_ids = output_items;
        }

        let links = self
            .trace
            .tool_calls
            .iter()
            .flat_map(|(tool_call_id, tool)| {
                tool.model_visible_output_item_ids
                    .iter()
                    .cloned()
                    .map(|item_id| (tool_call_id.clone(), item_id))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for (tool_call_id, item_id) in links {
            self.add_conversation_item_producer(&item_id, ProducerRef::Tool { tool_call_id });
        }
    }

    fn link_code_cells_to_conversation_items(&mut self) {
        let code_cell_ids = self.trace.code_cells.keys().cloned().collect::<Vec<_>>();
        for code_cell_id in code_cell_ids {
            let Some(model_visible_call_id) = self
                .trace
                .code_cells
                .get(&code_cell_id)
                .map(|cell| cell.model_visible_call_id.clone())
            else {
                continue;
            };

            let mut source_item_id = String::new();
            let mut output_item_ids = Vec::new();
            let mut output_call_ids = vec![model_visible_call_id.clone()];
            if let Some(cell) = self.trace.code_cells.get(&code_cell_id) {
                for wait_tool_call_id in &cell.wait_tool_call_ids {
                    if let Some(call_id) = wait_tool_call_id.strip_prefix("tool:") {
                        output_call_ids.push(call_id.to_string());
                    }
                }
            }
            // The code-cell start event gives us runtime identity, but the
            // provider payload gives us the canonical conversation item ids.
            // Link them after replay so we can see both sides of the turn. A
            // yielded cell may emit its final model-visible output through a
            // later `wait` call, so outputs are collected from both ids.
            for item in self.trace.conversation_items.values() {
                let Some(item_call_id) = item.call_id.as_deref() else {
                    continue;
                };
                if !output_call_ids
                    .iter()
                    .any(|call_id| call_id.as_str() == item_call_id)
                {
                    continue;
                }
                match item.kind {
                    ConversationItemKind::CustomToolCall
                        if item_call_id == model_visible_call_id =>
                    {
                        source_item_id = item.item_id.clone();
                    }
                    ConversationItemKind::CustomToolCallOutput
                    | ConversationItemKind::FunctionCallOutput => {
                        output_item_ids.push(item.item_id.clone());
                    }
                    _ => {}
                }
            }

            if let Some(cell) = self.trace.code_cells.get_mut(&code_cell_id) {
                cell.source_item_id = source_item_id;
                cell.output_item_ids = output_item_ids.clone();
            }
            for output_item_id in output_item_ids {
                self.add_conversation_item_producer(
                    &output_item_id,
                    ProducerRef::CodeCell {
                        code_cell_id: code_cell_id.clone(),
                    },
                );
            }
        }
    }

    fn drop_redundant_code_cell_tool_calls(&mut self) {
        let mut redundant_tool_call_ids = Vec::new();
        for (tool_call_id, tool) in &self.trace.tool_calls {
            if !matches!(&tool.kind, ToolCallKind::Other { name } if name == "exec") {
                continue;
            };
            let Some(model_visible_call_id) = tool.model_visible_call_id.as_deref() else {
                continue;
            };
            let code_cell_id = reduced_code_cell_id(model_visible_call_id);
            if self.trace.code_cells.contains_key(&code_cell_id) {
                redundant_tool_call_ids.push(tool_call_id.clone());
            }
        }

        for tool_call_id in redundant_tool_call_ids {
            // `exec` itself is represented by `code_cells`; keeping the generic
            // dispatch ToolCall makes viewers render two roots for the same
            // model-visible JavaScript cell.
            self.trace.tool_calls.remove(&tool_call_id);
            self.remove_conversation_item_producer(ProducerRef::Tool { tool_call_id });
        }
    }

    fn sync_terminal_model_observations(&mut self) {
        let operation_ids = self
            .trace
            .terminal_operations
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for operation_id in operation_ids {
            let Some(tool_call_id) = self
                .trace
                .terminal_operations
                .get(&operation_id)
                .map(|operation| operation.tool_call_id.clone())
            else {
                continue;
            };
            let Some(tool) = self.trace.tool_calls.get(&tool_call_id) else {
                continue;
            };
            // Direct shell tools are observed by the model through their own
            // call/output items. Nested shell tools are only observed through
            // the enclosing code cell's output, so terminal playback needs to
            // point at the code cell in that case.
            let observation = if let ToolCallRequester::CodeCell { code_cell_id } = &tool.requester
            {
                let Some(cell) = self.trace.code_cells.get(code_cell_id) else {
                    continue;
                };
                TerminalModelObservation {
                    call_item_ids: non_empty_vec(&cell.source_item_id),
                    output_item_ids: cell.output_item_ids.clone(),
                    source: TerminalObservationSource::CodeCellOutput,
                }
            } else {
                TerminalModelObservation {
                    call_item_ids: tool.model_visible_call_item_ids.clone(),
                    output_item_ids: tool.model_visible_output_item_ids.clone(),
                    source: TerminalObservationSource::DirectToolCall,
                }
            };
            if let Some(operation) = self.trace.terminal_operations.get_mut(&operation_id) {
                operation.model_observations = vec![observation];
            }
        }
    }

    fn resolve_agent_result_edges(&mut self) {
        // The producer emits agent results at the control-plane handoff. The
        // parent-visible mailbox item usually appears later, when a request
        // snapshot includes it, so final anchoring has to run after replay.
        let observations = self
            .agent_result_observations
            .iter()
            .map(|(edge_id, observation)| (edge_id.clone(), observation.clone()))
            .collect::<Vec<_>>();
        for (edge_id, observation) in observations {
            let source_item_id = self.latest_assistant_message_for_turn(
                &observation.child_thread_id,
                &observation.child_turn_id,
            );
            let target_item_id = self.agent_result_notification_item(
                &observation.parent_thread_id,
                &observation.message,
                observation.observed_at_unix_ms,
            );

            if let Some(edge) = self.trace.interaction_edges.get_mut(&edge_id) {
                edge.source = source_item_id
                    .as_ref()
                    .map(|item_id| TraceAnchor::ConversationItem {
                        item_id: item_id.clone(),
                    })
                    .unwrap_or_else(|| TraceAnchor::Thread {
                        thread_id: observation.child_thread_id.clone(),
                    });
                edge.target = target_item_id
                    .as_ref()
                    .map(|item_id| TraceAnchor::ConversationItem {
                        item_id: item_id.clone(),
                    })
                    .unwrap_or_else(|| TraceAnchor::Thread {
                        thread_id: observation.parent_thread_id.clone(),
                    });
                if let Some(item_id) = source_item_id.as_deref() {
                    push_unique(&mut edge.carried_item_ids, item_id);
                }
                if let Some(item_id) = target_item_id.as_deref() {
                    push_unique(&mut edge.carried_item_ids, item_id);
                }
            }

            if let Some(item_id) = target_item_id {
                self.add_conversation_item_producer(
                    &item_id,
                    ProducerRef::InteractionEdge { edge_id },
                );
            }
        }
    }

    fn latest_assistant_message_for_turn(
        &self,
        thread_id: &str,
        codex_turn_id: &str,
    ) -> Option<String> {
        self.trace
            .conversation_items
            .values()
            .filter(|item| {
                item.thread_id == thread_id
                    && item.codex_turn_id.as_deref() == Some(codex_turn_id)
                    && item.role == ConversationRole::Assistant
                    && item.kind == ConversationItemKind::Message
            })
            .max_by_key(|item| {
                (
                    item.first_seen_at_unix_ms,
                    conversation_item_ordinal(&item.item_id),
                )
            })
            .map(|item| item.item_id.clone())
    }

    fn agent_result_notification_item(
        &self,
        parent_thread_id: &str,
        message: &str,
        observed_at_unix_ms: i64,
    ) -> Option<String> {
        if message.is_empty() {
            return None;
        }
        let matching_items = self
            .trace
            .conversation_items
            .values()
            .filter(|item| {
                item.thread_id == parent_thread_id
                    && conversation_item_contains_agent_result_message(item, message)
            })
            .collect::<Vec<_>>();

        matching_items
            .iter()
            .copied()
            .filter(|item| item.first_seen_at_unix_ms >= observed_at_unix_ms)
            .min_by_key(|item| {
                (
                    item.first_seen_at_unix_ms,
                    conversation_item_ordinal(&item.item_id),
                )
            })
            .or_else(|| {
                // Partial traces can contain the parent request without the
                // exact producer event ordering. Keep the edge useful, but only
                // after the ordered match above fails.
                matching_items.iter().copied().max_by_key(|item| {
                    (
                        item.first_seen_at_unix_ms,
                        conversation_item_ordinal(&item.item_id),
                    )
                })
            })
            .map(|item| item.item_id.clone())
    }

    fn add_conversation_item_producer(&mut self, item_id: &str, producer: ProducerRef) {
        let Some(item) = self.trace.conversation_items.get_mut(item_id) else {
            return;
        };
        if !item
            .produced_by
            .iter()
            .any(|existing| existing == &producer)
        {
            item.produced_by.push(producer);
        }
    }

    fn remove_conversation_item_producer(&mut self, producer: ProducerRef) {
        for item in self.trace.conversation_items.values_mut() {
            item.produced_by.retain(|existing| existing != &producer);
        }
    }

    fn attach_tool_payloads_to_interaction_edges(&mut self) {
        let mut tool_payloads = BTreeMap::new();
        for (tool_call_id, tool) in &self.trace.tool_calls {
            let raw_payload_ids = tool_raw_payload_ids(tool);
            let mut item_ids = Vec::new();
            extend_unique(&mut item_ids, &tool.model_visible_call_item_ids);
            extend_unique(&mut item_ids, &tool.model_visible_output_item_ids);
            tool_payloads.insert(tool_call_id.clone(), (raw_payload_ids, item_ids));
        }
        for edge in self.trace.interaction_edges.values_mut() {
            let mut raw_payload_ids = edge.carried_raw_payload_ids.clone();
            let mut item_ids = edge.carried_item_ids.clone();
            for anchor in [&edge.source, &edge.target] {
                let TraceAnchor::ToolCall { tool_call_id } = anchor else {
                    continue;
                };
                if let Some((tool_raw_payload_ids, tool_item_ids)) = tool_payloads.get(tool_call_id)
                {
                    push_unique_all(&mut raw_payload_ids, tool_raw_payload_ids);
                    push_unique_all(&mut item_ids, tool_item_ids);
                }
            }
            edge.carried_raw_payload_ids = raw_payload_ids;
            edge.carried_item_ids = item_ids;
        }
    }
}

fn normalize_conversation_item(
    item_id: &str,
    thread_id: &str,
    first_seen_at_unix_ms: i64,
    item: &Value,
    produced_by: Vec<ProducerRef>,
    raw_payload_id: &str,
) -> ConversationItem {
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("other");
    let role = conversation_role(
        item.get("role")
            .and_then(Value::as_str)
            .unwrap_or(match kind {
                "function_call" | "custom_tool_call" | "local_shell_call" | "tool_search_call" => {
                    "assistant"
                }
                "function_call_output" | "custom_tool_call_output" | "tool_search_output" => "tool",
                _ => "assistant",
            }),
    );
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let channel = if kind == "reasoning" {
        Some(ConversationChannel::Analysis)
    } else if role == ConversationRole::Assistant && kind == "message" {
        match item.get("phase").and_then(Value::as_str) {
            Some("analysis") => Some(ConversationChannel::Analysis),
            Some("commentary") => Some(ConversationChannel::Commentary),
            Some("final_answer") => Some(ConversationChannel::Final),
            Some("summary") => Some(ConversationChannel::Summary),
            _ => Some(ConversationChannel::Final),
        }
    } else {
        None
    };
    ConversationItem {
        item_id: item_id.to_string(),
        thread_id: thread_id.to_string(),
        codex_turn_id: None,
        first_seen_at_unix_ms,
        role,
        channel,
        kind: conversation_item_kind(kind),
        body: normalize_body(kind, item, raw_payload_id),
        call_id,
        produced_by,
    }
}

fn normalize_body(kind: &str, item: &Value, raw_payload_id: &str) -> ConversationBody {
    match kind {
        "message" => {
            let parts =
                item.get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|content| {
                        content.get("text").and_then(Value::as_str).map(|text| {
                            ConversationPart::Text {
                                text: text.to_string(),
                            }
                        })
                    })
                    .collect::<Vec<_>>();
            ConversationBody { parts }
        }
        "custom_tool_call" => {
            let input = item.get("input").and_then(Value::as_str).unwrap_or("");
            let part = if item.get("name").and_then(Value::as_str) == Some("exec") {
                ConversationPart::Code {
                    language: "javascript".to_string(),
                    source: input.to_string(),
                }
            } else {
                ConversationPart::Text {
                    text: input.to_string(),
                }
            };
            ConversationBody { parts: vec![part] }
        }
        "function_call"
        | "tool_search_call"
        | "web_search_call"
        | "image_generation_call"
        | "local_shell_call" => ConversationBody {
            parts: vec![ConversationPart::Json {
                summary: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("function_call")
                    .to_string(),
                raw_payload_id: raw_payload_id.to_string(),
            }],
        },
        "function_call_output"
        | "custom_tool_call_output"
        | "tool_search_output"
        | "mcp_tool_call_output" => ConversationBody {
            parts: vec![ConversationPart::Text {
                text: tool_output_text(item.get("output")),
            }],
        },
        "reasoning" => reasoning_body(item, raw_payload_id),
        _ => ConversationBody {
            parts: vec![ConversationPart::Json {
                summary: kind.to_string(),
                raw_payload_id: raw_payload_id.to_string(),
            }],
        },
    }
}

fn reasoning_body(item: &Value, raw_payload_id: &str) -> ConversationBody {
    let mut parts = Vec::new();
    parts.extend(
        reasoning_texts(item, "content", &["reasoning_text", "text"])
            .into_iter()
            .map(|text| ConversationPart::Text { text }),
    );
    parts.extend(
        reasoning_texts(item, "summary", &["summary_text"])
            .into_iter()
            .map(|text| ConversationPart::Summary { text }),
    );

    if let Some(encrypted_content) = item
        .get("encrypted_content")
        .and_then(Value::as_str)
        .filter(|encrypted_content| !encrypted_content.is_empty())
    {
        // The encrypted blob is the stable model-visible identity for many
        // reasoning items. Keep it inline rather than only behind a raw payload
        // ref so replayed snapshots can be compared by content.
        parts.push(ConversationPart::Encoded {
            label: "encrypted_content".to_string(),
            value: encrypted_content.to_string(),
        });
    }

    if parts.is_empty() {
        // Malformed or empty reasoning should still be represented in the
        // conversation graph; the raw payload keeps the original bytes.
        parts.push(ConversationPart::PayloadRef {
            label: "reasoning".to_string(),
            raw_payload_id: raw_payload_id.to_string(),
        });
    }

    ConversationBody { parts }
}

fn reasoning_texts(item: &Value, key: &str, accepted_types: &[&str]) -> Vec<String> {
    item.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| {
            let part_type = part.get("type").and_then(Value::as_str)?;
            if accepted_types.contains(&part_type) {
                part.get("text").and_then(Value::as_str).map(str::to_string)
            } else {
                None
            }
        })
        .collect()
}

fn conversation_item_matches(existing: &ConversationItem, incoming: &ConversationItem) -> bool {
    let body_matches = if existing.kind == ConversationItemKind::Reasoning
        && incoming.kind == ConversationItemKind::Reasoning
    {
        reasoning_body_matches(&existing.body, &incoming.body)
    } else {
        conversation_body_matches(&existing.body, &incoming.body)
    };

    existing.role == incoming.role
        && existing.channel == incoming.channel
        && existing.kind == incoming.kind
        && body_matches
        && existing.call_id == incoming.call_id
}

fn conversation_body_matches(left: &ConversationBody, right: &ConversationBody) -> bool {
    left.parts.len() == right.parts.len()
        && left
            .parts
            .iter()
            .zip(&right.parts)
            .all(|(left, right)| match (left, right) {
                (
                    ConversationPart::Json {
                        summary: left_summary,
                        raw_payload_id: _,
                    },
                    ConversationPart::Json {
                        summary: right_summary,
                        raw_payload_id: _,
                    },
                ) => left_summary == right_summary,
                _ => left == right,
            })
}

fn reasoning_body_matches(left: &ConversationBody, right: &ConversationBody) -> bool {
    if conversation_body_matches(left, right) {
        return true;
    }

    // Responses may return readable reasoning on completion, while later
    // request snapshots replay only the encrypted blob. The blob is the stable
    // model-visible identity; readable text/summary is extra evidence that must
    // agree whenever both sides provide it.
    let Some(left_encoded) = reasoning_encoded_part(left) else {
        return false;
    };
    let Some(right_encoded) = reasoning_encoded_part(right) else {
        return false;
    };

    let left_readable = readable_reasoning_parts(left);
    let right_readable = readable_reasoning_parts(right);
    left_encoded == right_encoded
        && (left_readable.is_empty()
            || right_readable.is_empty()
            || left_readable == right_readable)
}

fn reasoning_encoded_part(body: &ConversationBody) -> Option<(&str, &str)> {
    body.parts.iter().find_map(|part| {
        if let ConversationPart::Encoded { label, value } = part {
            Some((label.as_str(), value.as_str()))
        } else {
            None
        }
    })
}

fn readable_reasoning_parts(body: &ConversationBody) -> Vec<&ConversationPart> {
    body.parts
        .iter()
        .filter(|part| {
            matches!(
                part,
                ConversationPart::Text { .. } | ConversationPart::Summary { .. }
            )
        })
        .collect()
}

fn conversation_item_contains_agent_result_message(
    item: &ConversationItem,
    expected_message: &str,
) -> bool {
    item.body.parts.iter().any(|part| {
        let ConversationPart::Text { text } = part else {
            return false;
        };
        if text == expected_message {
            return true;
        }
        // MultiAgentV2 delivers the notification as an InterAgentCommunication
        // JSON message. Avoid depending on the protocol crate here; the reducer
        // only needs the stable `content` field to identify the parent item.
        serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| {
                value
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .is_some_and(|content| content == expected_message)
    })
}

fn conversation_item_ordinal(item_id: &str) -> u64 {
    item_id
        .strip_prefix("item:")
        .and_then(|ordinal| ordinal.parse::<u64>().ok())
        .unwrap_or(0)
}

fn reduced_code_cell_id(model_visible_call_id: &str) -> String {
    format!("code_cell:{model_visible_call_id}")
}

fn conversation_role(role: &str) -> ConversationRole {
    match role {
        "system" => ConversationRole::System,
        "developer" => ConversationRole::Developer,
        "user" => ConversationRole::User,
        "tool" => ConversationRole::Tool,
        _ => ConversationRole::Assistant,
    }
}

fn conversation_item_kind(kind: &str) -> ConversationItemKind {
    match kind {
        "reasoning" => ConversationItemKind::Reasoning,
        "function_call"
        | "tool_search_call"
        | "web_search_call"
        | "image_generation_call"
        | "local_shell_call" => ConversationItemKind::FunctionCall,
        "function_call_output" | "tool_search_output" | "mcp_tool_call_output" => {
            ConversationItemKind::FunctionCallOutput
        }
        "custom_tool_call" => ConversationItemKind::CustomToolCall,
        "custom_tool_call_output" => ConversationItemKind::CustomToolCallOutput,
        "compaction_marker" => ConversationItemKind::CompactionMarker,
        _ => ConversationItemKind::Message,
    }
}

fn tool_call_kind(tool_name: &str) -> ToolCallKind {
    if is_terminal_tool(tool_name) {
        return match terminal_operation_kind(tool_name) {
            TerminalOperationKind::WriteStdin => ToolCallKind::WriteStdin,
            TerminalOperationKind::ExecCommand => ToolCallKind::ExecCommand,
        };
    }

    match tool_name {
        "apply_patch" => ToolCallKind::ApplyPatch,
        "web" => ToolCallKind::Web,
        "image_generation" => ToolCallKind::ImageGeneration,
        "spawn_agent" => ToolCallKind::SpawnAgent,
        "followup_task" => ToolCallKind::AssignAgentTask,
        "send_message" => ToolCallKind::SendMessage,
        "wait_agent" => ToolCallKind::WaitAgent,
        "close_agent" => ToolCallKind::CloseAgent,
        _ => ToolCallKind::Other {
            name: tool_name.to_string(),
        },
    }
}

fn is_terminal_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "exec_command" | "local_shell" | "shell" | "shell_command" | "write_stdin"
    )
}

fn terminal_operation_kind(tool_name: &str) -> TerminalOperationKind {
    match tool_name {
        "write_stdin" => TerminalOperationKind::WriteStdin,
        "exec_command" | "local_shell" | "shell" | "shell_command" => {
            TerminalOperationKind::ExecCommand
        }
        _ => TerminalOperationKind::ExecCommand,
    }
}

fn terminal_request_from_invocation(invocation: &Value) -> TerminalRequest {
    let tool_name = invocation
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let payload = invocation.get("payload").unwrap_or(&Value::Null);
    match payload.get("type").and_then(Value::as_str) {
        Some("function") => {
            let arguments = payload
                .get("arguments")
                .and_then(Value::as_str)
                .and_then(parse_json_object)
                .unwrap_or_default();
            terminal_request_from_arguments(tool_name, &arguments)
        }
        Some("local_shell") => {
            let command = payload.get("command").cloned().unwrap_or_else(|| json!([]));
            TerminalRequest::ExecCommand {
                command: string_vec(&command),
                display_command: display_command(&command),
                cwd: payload
                    .get("workdir")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                yield_time_ms: None,
                max_output_tokens: None,
            }
        }
        _ => TerminalRequest::ExecCommand {
            command: Vec::new(),
            display_command: terminal_operation_kind_label(terminal_operation_kind(tool_name))
                .to_string(),
            cwd: String::new(),
            yield_time_ms: None,
            max_output_tokens: None,
        },
    }
}

fn terminal_id_from_invocation_payload(invocation: &Value) -> Option<String> {
    invocation
        .get("payload")
        .and_then(|payload| payload.get("arguments"))
        .and_then(Value::as_str)
        .and_then(parse_json_object)
        .and_then(|arguments| {
            arguments.get("session_id").and_then(|session_id| {
                session_id
                    .as_u64()
                    .map(|id| id.to_string())
                    .or_else(|| session_id.as_str().map(str::to_string))
            })
        })
}

fn terminal_request_from_arguments(
    tool_name: &str,
    arguments: &Map<String, Value>,
) -> TerminalRequest {
    if tool_name == "write_stdin" {
        return TerminalRequest::WriteStdin {
            stdin: arguments
                .get("chars")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            yield_time_ms: arguments.get("yield_time_ms").and_then(Value::as_u64),
            max_output_tokens: arguments
                .get("max_output_tokens")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok()),
        };
    }

    let command = arguments
        .get("command")
        .cloned()
        .or_else(|| arguments.get("cmd").map(|cmd| json!([cmd])))
        .unwrap_or_else(|| json!([]));
    TerminalRequest::ExecCommand {
        command: string_vec(&command),
        display_command: display_command(&command),
        cwd: arguments
            .get("workdir")
            .or_else(|| arguments.get("cwd"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        yield_time_ms: arguments.get("yield_time_ms").and_then(Value::as_u64),
        max_output_tokens: arguments
            .get("max_output_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok()),
    }
}

fn parse_json_object(raw: &str) -> Option<Map<String, Value>> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.as_object().cloned())
}

fn display_command(command: &Value) -> String {
    if let Some(text) = command.as_str() {
        return text.to_string();
    }

    command
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" ")
}

fn terminal_id_from_result_payload(payload: &Value, operation_id: &str) -> Option<String> {
    let runtime_terminal_id = payload
        .get("code_mode_result")
        .and_then(|result| result.get("session_id"))
        .and_then(|session_id| {
            session_id
                .as_u64()
                .map(|id| id.to_string())
                .or_else(|| session_id.as_str().map(str::to_string))
        });
    if runtime_terminal_id.is_some() {
        return runtime_terminal_id;
    }

    // One-shot commands finish before the runtime has a persistent session id.
    // Still create a terminal session so every terminal operation has a parent
    // session object for viewers to hang lifecycle UI from.
    payload
        .get("code_mode_result")
        .filter(|result| result.is_object())
        .and_then(|_| (!operation_id.is_empty()).then(|| format!("terminal:{operation_id}")))
}

fn tool_raw_payload_ids(tool: &ToolCall) -> Vec<String> {
    let mut raw_payload_ids = Vec::new();
    for raw_payload_id in [
        tool.raw_invocation_payload_id.as_deref(),
        tool.raw_result_payload_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        push_unique(&mut raw_payload_ids, raw_payload_id);
    }
    push_unique_all(&mut raw_payload_ids, &tool.raw_runtime_payload_ids);
    raw_payload_ids
}

fn terminal_result_from_payload(payload: &Value, output_preview: Option<&str>) -> TerminalResult {
    let code_mode_result = payload.get("code_mode_result").unwrap_or(&Value::Null);
    TerminalResult {
        exit_code: code_mode_result
            .get("exit_code")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        stdout: code_mode_result
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                payload
                    .get("output_preview")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            })
            .to_string(),
        stderr: String::new(),
        formatted_output: output_preview.map(str::to_string),
        original_token_count: code_mode_result
            .get("original_token_count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok()),
        chunk_id: code_mode_result
            .get("chunk_id")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn code_cell_execution_status(runtime_status: &str) -> ExecutionStatus {
    match runtime_status {
        "failed" => ExecutionStatus::Failed,
        "terminated" => ExecutionStatus::Cancelled,
        "yielded" | "running" => ExecutionStatus::Running,
        _ => ExecutionStatus::Completed,
    }
}

fn tool_output_text(output: Option<&Value>) -> String {
    let Some(output) = output else {
        return String::new();
    };
    if let Some(text) = output.as_str() {
        return text.to_string();
    }
    if let Some(items) = output.as_array() {
        return items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");
    }
    if let Some(text) = output.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(text) = output.get("body").and_then(Value::as_str) {
        return text.to_string();
    }
    serde_json::to_string_pretty(output).unwrap_or_default()
}

fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    serde_json::from_reader(file).with_context(|| format!("parse {}", path.display()))
}

fn parse_captured_event(line: &str, line_number: usize) -> Result<CapturedEvent> {
    let value: Value = serde_json::from_str(line)
        .with_context(|| format!("parse trace event line {line_number}"))?;
    let Some(object) = value.as_object() else {
        anyhow::bail!("trace event line {line_number} is not a JSON object");
    };

    if let Some(fields) = object.get("fields").and_then(Value::as_object) {
        return Ok(CapturedEvent {
            seq: object
                .get("seq")
                .and_then(Value::as_u64)
                .unwrap_or(line_number as u64),
            wall_time_unix_ms: object
                .get("wall_time_unix_ms")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            fields: fields.clone(),
        });
    }

    Ok(CapturedEvent {
        seq: object
            .get("trace_seq")
            .and_then(Value::as_u64)
            .unwrap_or(line_number as u64),
        wall_time_unix_ms: object
            .get("wall_time_unix_ms")
            .and_then(Value::as_i64)
            .or_else(|| timestamp_unix_ms(object.get("timestamp")))
            .unwrap_or(0),
        fields: object.clone(),
    })
}

fn event_name(event: &CapturedEvent) -> Option<&str> {
    field_str(event, "event.name").or_else(|| field_str(event, "trace_event"))
}

fn trace_field_str<'a>(event: &'a CapturedEvent, namespace: &str, name: &str) -> Option<&'a str> {
    field_str(event, &format!("{namespace}.{name}"))
        .or_else(|| field_str(event, &format!("{}_{}", namespace.replace('.', "_"), name)))
        .or_else(|| match (namespace, name) {
            ("inference", "id") => field_str(event, "inference_call_id"),
            ("tool", "call_id") => field_str(event, "tool_call_id"),
            ("response", "id") => field_str(event, "response_id"),
            ("thread", "source") => field_str(event, "source"),
            _ => None,
        })
}

fn normalized_tool_call_id(event: &CapturedEvent) -> Option<String> {
    let call_id =
        trace_field_str(event, "tool", "call_id").filter(|call_id| !call_id.is_empty())?;
    Some(normalize_tool_call_id(call_id))
}

fn normalize_tool_call_id(call_id: &str) -> String {
    if call_id.starts_with("tool:") {
        call_id.to_string()
    } else {
        format!("tool:{call_id}")
    }
}

fn interaction_edge_id(kind: &str, tool_call_id: &str) -> String {
    format!("edge:{kind}:{tool_call_id}")
}

fn interaction_edge_kind(kind: &str) -> InteractionEdgeKind {
    match kind {
        "spawn_agent" => InteractionEdgeKind::SpawnAgent,
        "assign_agent_task" | "followup_task" => InteractionEdgeKind::AssignAgentTask,
        "agent_result" => InteractionEdgeKind::AgentResult,
        "close_agent" => InteractionEdgeKind::CloseAgent,
        _ => InteractionEdgeKind::SendMessage,
    }
}

fn agent_result_observed_edge_id(
    child_thread_id: &str,
    child_turn_id: &str,
    parent_thread_id: &str,
) -> String {
    format!("edge:agent_result:{child_thread_id}:{child_turn_id}:{parent_thread_id}")
}

fn thread_anchor(thread_id: &str) -> Option<TraceAnchor> {
    (!thread_id.is_empty()).then(|| TraceAnchor::Thread {
        thread_id: thread_id.to_string(),
    })
}

fn task_name_from_agent_path(agent_path: &str) -> String {
    agent_path
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(agent_path)
        .to_string()
}

fn push_unique_all(values: &mut Vec<String>, new_values: &[String]) {
    for value in new_values {
        push_unique(values, value);
    }
}

fn extend_unique(values: &mut Vec<String>, new_values: &[String]) {
    push_unique_all(values, new_values);
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !value.is_empty() && !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn raw_payload_field_str<'a>(
    event: &'a CapturedEvent,
    payload_kind: &str,
    name: &str,
) -> Option<&'a str> {
    field_str(event, &format!("raw_payload.{payload_kind}.{name}"))
        .or_else(|| field_str(event, &format!("raw_{payload_kind}_payload_{name}")))
}

fn timestamp_unix_ms(value: Option<&Value>) -> Option<i64> {
    let timestamp = value?.as_str()?;
    let parsed = DateTime::parse_from_rfc3339(timestamp).ok()?;
    Some(parsed.timestamp_millis())
}

fn field_str<'a>(event: &'a CapturedEvent, name: &str) -> Option<&'a str> {
    event.fields.get(name).and_then(Value::as_str)
}

fn set_execution_start(execution: &mut ExecutionWindow, started_at_unix_ms: i64, started_seq: u64) {
    execution.started_at_unix_ms = started_at_unix_ms;
    execution.started_seq = started_seq;
}

fn set_execution_end(
    execution: &mut ExecutionWindow,
    ended_at_unix_ms: i64,
    ended_seq: u64,
    status: ExecutionStatus,
) {
    execution.ended_at_unix_ms = Some(ended_at_unix_ms);
    execution.ended_seq = Some(ended_seq);
    execution.status = status;
}

fn execution_window(
    started_at_unix_ms: i64,
    ended_at_unix_ms: Option<i64>,
    status: ExecutionStatus,
    started_seq: u64,
    ended_seq: Option<u64>,
) -> ExecutionWindow {
    ExecutionWindow {
        started_at_unix_ms,
        started_seq,
        ended_at_unix_ms,
        ended_seq,
        status,
    }
}

fn execution_status(status: &str) -> ExecutionStatus {
    match status {
        "failed" => ExecutionStatus::Failed,
        "cancelled" | "terminated" => ExecutionStatus::Cancelled,
        "aborted" => ExecutionStatus::Aborted,
        "running" | "yielded" => ExecutionStatus::Running,
        _ => ExecutionStatus::Completed,
    }
}

fn code_cell_runtime_status(status: &str) -> CodeCellRuntimeStatus {
    match status {
        "starting" => CodeCellRuntimeStatus::Starting,
        "running" => CodeCellRuntimeStatus::Running,
        "yielded" => CodeCellRuntimeStatus::Yielded,
        "failed" => CodeCellRuntimeStatus::Failed,
        "terminated" => CodeCellRuntimeStatus::Terminated,
        _ => CodeCellRuntimeStatus::Completed,
    }
}

fn raw_payload_kind(kind: &str) -> RawPayloadKind {
    match kind {
        "inference_request" => RawPayloadKind::InferenceRequest,
        "inference_response" | "inference_response_summary" => RawPayloadKind::InferenceResponse,
        "compaction_request" => RawPayloadKind::CompactionRequest,
        "compaction_checkpoint" => RawPayloadKind::CompactionCheckpoint,
        "compaction_response" => RawPayloadKind::CompactionResponse,
        "tool_invocation" => RawPayloadKind::ToolInvocation,
        "tool_result" => RawPayloadKind::ToolResult,
        "tool_runtime_event" => RawPayloadKind::ToolRuntimeEvent,
        "terminal_runtime_event" => RawPayloadKind::TerminalRuntimeEvent,
        "session_metadata" => RawPayloadKind::SessionMetadata,
        "agent_result" => RawPayloadKind::AgentResult,
        "code_cell_invocation" | "code_cell_result" => RawPayloadKind::ToolRuntimeEvent,
        _ => RawPayloadKind::ProtocolEvent,
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn non_empty_vec(value: &str) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        vec![value.to_string()]
    }
}

fn string_vec(value: &Value) -> Vec<String> {
    value
        .as_str()
        .map(|text| vec![text.to_string()])
        .unwrap_or_else(|| {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
}

fn terminal_operation_kind_label(kind: TerminalOperationKind) -> &'static str {
    match kind {
        TerminalOperationKind::ExecCommand => "exec_command",
        TerminalOperationKind::WriteStdin => "write_stdin",
    }
}

fn token_usage_from_value(value: Value) -> Option<TokenUsage> {
    Some(TokenUsage {
        input_tokens: value
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cached_input_tokens: value
            .get("cached_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: value
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        reasoning_output_tokens: value
            .get("reasoning_output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::Path;

    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

    use super::reduce_bundle;

    fn reduce_bundle_json(bundle_dir: &Path) -> anyhow::Result<serde_json::Value> {
        // Tests assert the public `state.json` contract instead of private Rust
        // field access. That catches serde-shape drift from PR 17982's model.
        Ok(serde_json::to_value(reduce_bundle(bundle_dir)?)?)
    }

    #[test]
    fn reduces_inference_and_tool_events() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&json!({
                "trace_id": "trace-test",
                "started_at_unix_ms": 10,
            }))?,
        )?;
        std::fs::create_dir(temp.path().join("payloads"))?;

        let user_message = json!({
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": "hi" }]
        });
        let tool_call = json!({
            "type": "function_call",
            "name": "shell",
            "arguments": "{}",
            "call_id": "call-1"
        });

        std::fs::write(
            temp.path().join("payloads/1.json"),
            serde_json::to_vec(&json!({ "input": [user_message] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/2.json"),
            serde_json::to_vec(&json!({
                "response_id": "resp-1",
                "token_usage": { "input_tokens": 1, "output_tokens": 2 },
                "output_items": [tool_call]
            }))?,
        )?;
        // The next request includes the whole model-visible input again. The
        // reducer should reuse the user message and tool call, then add only
        // the new tool output item.
        std::fs::write(
            temp.path().join("payloads/3.json"),
            serde_json::to_vec(&json!({
                "input": [
                    user_message,
                    tool_call,
                    {
                        "type": "function_call_output",
                        "call_id": "call-1",
                        "output": "ok\n"
                    }
                ]
            }))?,
        )?;
        let mut events = std::fs::File::create(temp.path().join("events.jsonl"))?;
        write_event(
            &mut events,
            1,
            "codex.thread.started",
            json!({"thread.id": "thread-1"}),
        )?;
        write_event(
            &mut events,
            2,
            "codex.turn.started",
            json!({"thread.id": "thread-1", "turn.id": "turn-1"}),
        )?;
        write_event(
            &mut events,
            3,
            "codex.inference.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "inference.id": "inf-1",
                "raw_payload.request.id": "raw_payload:1",
                "raw_payload.request.path": "payloads/1.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;
        write_event(
            &mut events,
            4,
            "codex.inference.completed",
            json!({
                "inference.id": "inf-1",
                "response.id": "resp-1",
                "raw_payload.response.id": "raw_payload:2",
                "raw_payload.response.path": "payloads/2.json",
                "raw_payload.response.kind": "inference_response_summary"
            }),
        )?;
        write_event(
            &mut events,
            5,
            "codex.tool.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "tool.call_id": "tool-1",
                "tool.name": "shell",
                "model_visible_call.id": "call-1"
            }),
        )?;
        write_event(
            &mut events,
            6,
            "codex.turn.started",
            json!({"thread.id": "thread-1", "turn.id": "turn-2"}),
        )?;
        write_event(
            &mut events,
            7,
            "codex.inference.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-2",
                "inference.id": "inf-2",
                "raw_payload.request.id": "raw_payload:3",
                "raw_payload.request.path": "payloads/3.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;
        let trace = reduce_bundle_json(temp.path())?;

        assert_eq!(
            json!({
                "thread_count": trace["threads"].as_object().unwrap().len(),
                "inference_count": trace["inference_calls"].as_object().unwrap().len(),
                "conversation_item_count": trace["conversation_items"].as_object().unwrap().len(),
                "thread_item_ids": trace["threads"]["thread-1"]["conversation_item_ids"].clone(),
                "second_request_item_ids": trace["inference_calls"]["inf-2"]["request_item_ids"].clone(),
                "tool_call_item_ids": trace["tool_calls"]["tool-1"]["model_visible_call_item_ids"].clone(),
            }),
            json!({
                "thread_count": 1,
                "inference_count": 2,
                "conversation_item_count": 3,
                "thread_item_ids": ["item:1", "item:2", "item:3"],
                "second_request_item_ids": ["item:1", "item:2", "item:3"],
                "tool_call_item_ids": ["item:2"],
            })
        );
        Ok(())
    }

    #[test]
    fn captures_analysis_reasoning_and_reuses_replayed_reasoning() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&json!({
                "trace_id": "trace-test",
                "started_at_unix_ms": 10,
            }))?,
        )?;
        std::fs::create_dir(temp.path().join("payloads"))?;

        let analysis_message = json!({
            "type": "message",
            "role": "assistant",
            "phase": "analysis",
            "content": [{ "type": "output_text", "text": "analysis note" }]
        });
        let reasoning_item = json!({
            "type": "reasoning",
            "content": [
                { "type": "reasoning_text", "text": "raw reasoning" },
                { "type": "text", "text": "raw text reasoning" }
            ],
            "summary": [{ "type": "summary_text", "text": "short summary" }],
            "encrypted_content": "encrypted-blob"
        });
        let replayed_reasoning_item = json!({
            "type": "reasoning",
            "summary": [],
            "encrypted_content": "encrypted-blob"
        });

        std::fs::write(
            temp.path().join("payloads/request.json"),
            serde_json::to_vec(&json!({ "input": [] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/response.json"),
            serde_json::to_vec(&json!({ "output_items": [analysis_message, reasoning_item] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/request-2.json"),
            serde_json::to_vec(&json!({ "input": [replayed_reasoning_item] }))?,
        )?;
        let mut events = std::fs::File::create(temp.path().join("events.jsonl"))?;
        for (seq, event, fields) in [
            (1, "codex.thread.started", json!({"thread.id": "thread-1"})),
            (
                2,
                "codex.turn.started",
                json!({"thread.id": "thread-1", "turn.id": "turn-1"}),
            ),
            (
                3,
                "codex.inference.started",
                json!({
                    "thread.id": "thread-1",
                    "turn.id": "turn-1",
                    "inference.id": "inf-1",
                    "raw_payload.request.id": "raw_payload:request",
                    "raw_payload.request.path": "payloads/request.json",
                    "raw_payload.request.kind": "inference_request"
                }),
            ),
            (
                4,
                "codex.inference.completed",
                json!({
                    "inference.id": "inf-1",
                    "raw_payload.response.id": "raw_payload:response",
                    "raw_payload.response.path": "payloads/response.json",
                    "raw_payload.response.kind": "inference_response_summary"
                }),
            ),
            (
                5,
                "codex.turn.started",
                json!({"thread.id": "thread-1", "turn.id": "turn-2"}),
            ),
            (
                6,
                "codex.inference.started",
                json!({
                    "thread.id": "thread-1",
                    "turn.id": "turn-2",
                    "inference.id": "inf-2",
                    "raw_payload.request.id": "raw_payload:request-2",
                    "raw_payload.request.path": "payloads/request-2.json",
                    "raw_payload.request.kind": "inference_request"
                }),
            ),
        ] {
            write_event(&mut events, seq, event, fields)?;
        }

        let trace = reduce_bundle_json(temp.path())?;

        // The response has the rich readable reasoning; the next request only
        // replays the encrypted blob. Both sightings should resolve to item:2.
        assert_eq!(
            json!({
                "item_count": trace["conversation_items"].as_object().unwrap().len(),
                "analysis_channel": trace["conversation_items"]["item:1"]["channel"].clone(),
                "reasoning_parts": trace["conversation_items"]["item:2"]["body"]["parts"].clone(),
                "first_response_items": trace["inference_calls"]["inf-1"]["response_item_ids"].clone(),
                "second_request_items": trace["inference_calls"]["inf-2"]["request_item_ids"].clone(),
            }),
            json!({
                "item_count": 2,
                "analysis_channel": "analysis",
                "reasoning_parts": [
                    { "type": "text", "text": "raw reasoning" },
                    { "type": "text", "text": "raw text reasoning" },
                    { "type": "summary", "text": "short summary" },
                    {
                        "type": "encoded",
                        "label": "encrypted_content",
                        "value": "encrypted-blob"
                    }
                ],
                "first_response_items": ["item:1", "item:2"],
                "second_request_items": ["item:2"],
            })
        );
        Ok(())
    }

    #[test]
    fn reduces_collab_events_to_interaction_edges() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&json!({
                "trace_id": "trace-test",
                "started_at_unix_ms": 10,
            }))?,
        )?;
        std::fs::create_dir(temp.path().join("payloads"))?;
        let child_message = "finished counting files";
        let result_message = "<subagent_notification>\n\
{\"agent_path\":\"/root/worker\",\"status\":{\"completed\":\"finished counting files\"}}\n\
</subagent_notification>";
        let parent_notification = json!({
            "author": "/root/worker",
            "recipient": "/root",
            "other_recipients": [],
            "content": result_message,
            "trigger_turn": false,
        })
        .to_string();
        std::fs::write(
            temp.path().join("payloads/child-request.json"),
            serde_json::to_vec(&json!({ "input": [] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/child-response.json"),
            serde_json::to_vec(&json!({
                "output_items": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": child_message }]
                }]
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/parent-request.json"),
            serde_json::to_vec(&json!({
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": parent_notification }]
                }]
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/agent-result.json"),
            serde_json::to_vec(&json!({
                "child_agent_path": "/root/worker",
                "message": result_message,
                "status": { "completed": child_message }
            }))?,
        )?;
        let mut events = std::fs::File::create(temp.path().join("events.jsonl"))?;
        write_event(
            &mut events,
            1,
            "codex.thread.started",
            json!({"thread.id": "thread-parent"}),
        )?;
        write_event(
            &mut events,
            2,
            "codex.turn.started",
            json!({"thread.id": "thread-parent", "turn.id": "turn-parent"}),
        )?;
        write_event(
            &mut events,
            3,
            "codex.tool.started",
            json!({
                "thread.id": "thread-parent",
                "turn.id": "turn-parent",
                "tool.call_id": "tool:call-spawn",
                "tool.name": "spawn_agent",
                "model_visible_call.id": "call-spawn",
                "raw_payload.invocation.id": "raw_payload:spawn-invocation",
                "raw_payload.invocation.path": "payloads/spawn-invocation.json",
                "raw_payload.invocation.kind": "tool_invocation"
            }),
        )?;
        write_event(
            &mut events,
            4,
            "codex.collab.spawn.started",
            json!({
                "tool.call_id": "call-spawn",
                "sender.thread.id": "thread-parent"
            }),
        )?;
        write_event(
            &mut events,
            5,
            "codex.thread.started",
            json!({
                "thread.id": "thread-child",
                "parent_thread.id": "thread-parent",
                "agent.path": "/root/worker"
            }),
        )?;
        write_event(
            &mut events,
            6,
            "codex.collab.spawn.ended",
            json!({
                "tool.call_id": "call-spawn",
                "sender.thread.id": "thread-parent",
                "target.thread.id": "thread-child",
                "target.agent.nickname": "Euclid",
                "target.agent.role": "worker"
            }),
        )?;
        write_event(
            &mut events,
            7,
            "codex.tool.ended",
            json!({
                "tool.call_id": "tool:call-spawn",
                "status": "completed",
                "raw_payload.result.id": "raw_payload:spawn-result",
                "raw_payload.result.path": "payloads/spawn-result.json",
                "raw_payload.result.kind": "tool_result"
            }),
        )?;
        write_event(
            &mut events,
            8,
            "codex.tool.started",
            json!({
                "thread.id": "thread-parent",
                "turn.id": "turn-parent",
                "tool.call_id": "tool:call-wait",
                "tool.name": "wait_agent",
                "model_visible_call.id": "call-wait",
                "raw_payload.invocation.id": "raw_payload:wait-invocation",
                "raw_payload.invocation.path": "payloads/wait-invocation.json",
                "raw_payload.invocation.kind": "tool_invocation"
            }),
        )?;
        write_event(
            &mut events,
            9,
            "codex.collab.wait.started",
            json!({
                "tool.call_id": "tool:call-wait",
                "sender.thread.id": "thread-parent",
                "target.thread.id": "thread-child"
            }),
        )?;
        write_event(
            &mut events,
            10,
            "codex.collab.wait.ended",
            json!({
                "tool.call_id": "tool:call-wait",
                "sender.thread.id": "thread-parent",
                "target.thread.id": "thread-child"
            }),
        )?;
        write_event(
            &mut events,
            11,
            "codex.tool.ended",
            json!({
                "tool.call_id": "tool:call-wait",
                "status": "completed",
                "raw_payload.result.id": "raw_payload:wait-result",
                "raw_payload.result.path": "payloads/wait-result.json",
                "raw_payload.result.kind": "tool_result"
            }),
        )?;
        write_event(
            &mut events,
            12,
            "codex.turn.started",
            json!({"thread.id": "thread-child", "turn.id": "turn-child"}),
        )?;
        write_event(
            &mut events,
            13,
            "codex.inference.started",
            json!({
                "thread.id": "thread-child",
                "turn.id": "turn-child",
                "inference.id": "inf-child",
                "raw_payload.request.id": "raw_payload:child-request",
                "raw_payload.request.path": "payloads/child-request.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;
        write_event(
            &mut events,
            14,
            "codex.inference.completed",
            json!({
                "inference.id": "inf-child",
                "raw_payload.response.id": "raw_payload:child-response",
                "raw_payload.response.path": "payloads/child-response.json",
                "raw_payload.response.kind": "inference_response_summary"
            }),
        )?;
        write_event(
            &mut events,
            15,
            "codex.turn.ended",
            json!({
                "thread.id": "thread-child",
                "turn.id": "turn-child",
                "status": "completed"
            }),
        )?;
        // The result edge comes from the explicit control-plane observation,
        // not from wait-tool timing. This mirrors PR 17982 and avoids guessing
        // which child completed when a parent waits on "any" or many children.
        write_event(
            &mut events,
            16,
            "codex.collab.agent_result.observed",
            json!({
                "child.thread.id": "thread-child",
                "child.turn.id": "turn-child",
                "parent.thread.id": "thread-parent",
                "message": result_message,
                "raw_payload.agent_result.id": "raw_payload:agent-result",
                "raw_payload.agent_result.path": "payloads/agent-result.json",
                "raw_payload.agent_result.kind": "agent_result"
            }),
        )?;
        write_event(
            &mut events,
            17,
            "codex.inference.started",
            json!({
                "thread.id": "thread-parent",
                "turn.id": "turn-parent",
                "inference.id": "inf-parent-after-result",
                "raw_payload.request.id": "raw_payload:parent-request",
                "raw_payload.request.path": "payloads/parent-request.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;

        let trace = reduce_bundle_json(temp.path())?;

        assert_eq!(
            trace["interaction_edges"]["edge:spawn_agent:tool:call-spawn"],
            json!({
                "edge_id": "edge:spawn_agent:tool:call-spawn",
                "kind": "spawn_agent",
                "source": { "type": "tool_call", "tool_call_id": "tool:call-spawn" },
                "target": { "type": "thread", "thread_id": "thread-child" },
                "started_at_unix_ms": 1776420000000i64,
                "ended_at_unix_ms": 1776420000000i64,
                "carried_item_ids": [],
                "carried_raw_payload_ids": [
                    "raw_payload:spawn-invocation",
                    "raw_payload:spawn-result"
                ],
            })
        );
        assert_eq!(
            trace["threads"]["thread-child"]["nickname"],
            json!("Euclid")
        );
        assert_eq!(
            trace["threads"]["thread-child"]["origin"]["agent_role"],
            json!("worker")
        );
        assert_eq!(
            trace["threads"]["thread-child"]["origin"]["spawn_edge_id"],
            json!("edge:spawn_agent:tool:call-spawn")
        );
        assert_eq!(
            trace["threads"]["thread-child"]["origin"]["task_name"],
            json!("worker")
        );
        assert_eq!(
            trace["interaction_edges"]["edge:agent_result:thread-child:turn-child:thread-parent"],
            json!({
                "edge_id": "edge:agent_result:thread-child:turn-child:thread-parent",
                "kind": "agent_result",
                "source": { "type": "conversation_item", "item_id": "item:1" },
                "target": { "type": "conversation_item", "item_id": "item:2" },
                "started_at_unix_ms": 1776420000000i64,
                "ended_at_unix_ms": 1776420000000i64,
                "carried_item_ids": ["item:1", "item:2"],
                "carried_raw_payload_ids": [
                    "raw_payload:agent-result"
                ],
            })
        );
        assert_eq!(
            trace["interaction_edges"]["edge:agent_result:tool:call-wait:thread-child"],
            json!(null)
        );
        Ok(())
    }

    #[test]
    fn reduces_code_cell_nested_terminal_tool_relationships() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&json!({
                "trace_id": "trace-test",
                "started_at_unix_ms": 10,
            }))?,
        )?;
        std::fs::create_dir(temp.path().join("payloads"))?;

        let user_message = json!({
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": "count files" }]
        });
        let code_call = json!({
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": "call-code",
            "name": "exec",
            "input": "await tools.exec_command({ cmd: \"find . -type f | wc -l\" });"
        });
        let code_output = json!({
            "type": "custom_tool_call_output",
            "call_id": "call-code",
            "output": [{ "type": "input_text", "text": "Script completed\nOutput:\n42" }]
        });

        std::fs::write(
            temp.path().join("payloads/1.json"),
            serde_json::to_vec(&json!({ "input": [user_message] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/2.json"),
            serde_json::to_vec(&json!({ "output_items": [code_call] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/3.json"),
            serde_json::to_vec(&json!({
                "call_id": "call-code",
                "tool_name": "exec",
                "payload": { "type": "custom", "input": code_call["input"] }
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/4.json"),
            serde_json::to_vec(&json!({
                "call_id": "runtime-tool-1",
                "tool_name": "exec_command",
                "payload": {
                    "type": "function",
                    "arguments": "{\"cmd\":\"find . -type f | wc -l\",\"workdir\":\"/repo\",\"max_output_tokens\":200,\"yield_time_ms\":1000}"
                },
                "source": {
                    "type": "code_cell",
                    "runtime_cell_id": "runtime-cell-1",
                    "runtime_tool_call_id": "runtime-tool-1"
                }
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/5.json"),
            serde_json::to_vec(&json!({
                "status": "completed",
                "success": true,
                "output_preview": "Chunk ID: chunk-1\nWall time: 1.0 seconds\nOutput:\n",
                "code_mode_result": {
                    "chunk_id": "chunk-1",
                    "wall_time_seconds": 1.0,
                    "session_id": 55007,
                    "original_token_count": 0,
                    "output": ""
                }
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/6.json"),
            serde_json::to_vec(&json!({
                "status": "completed",
                "success": true,
                "output_preview": "Script completed\nOutput:\n42"
            }))?,
        )?;
        // Code-cell outputs are model-visible only when the follow-up request
        // sends the complete conversation snapshot back to the provider.
        std::fs::write(
            temp.path().join("payloads/7.json"),
            serde_json::to_vec(&json!({
                "input": [user_message, code_call, code_output]
            }))?,
        )?;

        let mut events = std::fs::File::create(temp.path().join("events.jsonl"))?;
        write_event(
            &mut events,
            1,
            "codex.thread.started",
            json!({"thread.id": "thread-1"}),
        )?;
        write_event(
            &mut events,
            2,
            "codex.turn.started",
            json!({"thread.id": "thread-1", "turn.id": "turn-1"}),
        )?;
        write_event(
            &mut events,
            3,
            "codex.inference.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "inference.id": "inf-1",
                "raw_payload.request.id": "raw_payload:1",
                "raw_payload.request.path": "payloads/1.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;
        write_event(
            &mut events,
            4,
            "codex.inference.completed",
            json!({
                "inference.id": "inf-1",
                "response.id": "resp-1",
                "raw_payload.response.id": "raw_payload:2",
                "raw_payload.response.path": "payloads/2.json",
                "raw_payload.response.kind": "inference_response_summary"
            }),
        )?;
        write_event(
            &mut events,
            5,
            "codex.tool.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "tool.call_id": "tool:call-code",
                "tool.name": "exec",
                "model_visible_call.id": "call-code",
                "raw_payload.invocation.id": "raw_payload:3",
                "raw_payload.invocation.path": "payloads/3.json",
                "raw_payload.invocation.kind": "tool_invocation"
            }),
        )?;
        write_event(
            &mut events,
            6,
            "codex.code_cell.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "code_cell.runtime_id": "runtime-cell-1",
                "model_visible_call.id": "call-code",
                "code_cell.source_js": "await tools.exec_command({ cmd: \"find . -type f | wc -l\" });"
            }),
        )?;
        write_event(
            &mut events,
            7,
            "codex.tool.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "tool.call_id": "tool:nested-exec",
                "tool.name": "exec_command",
                "tool.requester.type": "code_cell",
                "code_cell.runtime_id": "runtime-cell-1",
                "code_mode_runtime_tool.id": "runtime-tool-1",
                "raw_payload.invocation.id": "raw_payload:4",
                "raw_payload.invocation.path": "payloads/4.json",
                "raw_payload.invocation.kind": "tool_invocation"
            }),
        )?;
        write_event(
            &mut events,
            8,
            "codex.tool.ended",
            json!({
                "tool.call_id": "tool:nested-exec",
                "status": "completed",
                "output_preview": "Chunk ID: chunk-1\nWall time: 1.0 seconds\nOutput:\n",
                "raw_payload.result.id": "raw_payload:5",
                "raw_payload.result.path": "payloads/5.json",
                "raw_payload.result.kind": "tool_result"
            }),
        )?;
        write_event(
            &mut events,
            9,
            "codex.code_cell.ended",
            json!({
                "thread.id": "thread-1",
                "code_cell.runtime_id": "runtime-cell-1",
                "status": "completed"
            }),
        )?;
        write_event(
            &mut events,
            10,
            "codex.tool.ended",
            json!({
                "tool.call_id": "tool:call-code",
                "status": "completed",
                "raw_payload.result.id": "raw_payload:6",
                "raw_payload.result.path": "payloads/6.json",
                "raw_payload.result.kind": "tool_result"
            }),
        )?;
        write_event(
            &mut events,
            11,
            "codex.inference.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "inference.id": "inf-2",
                "raw_payload.request.id": "raw_payload:7",
                "raw_payload.request.path": "payloads/7.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;

        let trace = reduce_bundle_json(temp.path())?;

        assert_eq!(
            trace["code_cells"]["code_cell:call-code"],
            json!({
                "code_cell_id": "code_cell:call-code",
                "model_visible_call_id": "call-code",
                "thread_id": "thread-1",
                "codex_turn_id": "turn-1",
                "source_item_id": "item:2",
                "output_item_ids": ["item:3"],
                "runtime_cell_id": "runtime-cell-1",
                "execution": {
                    "started_at_unix_ms": 1776420000000i64,
                    "ended_at_unix_ms": 1776420000000i64,
                    "status": "completed",
                    "started_seq": 6,
                    "ended_seq": 9,
                },
                "runtime_status": "completed",
                "initial_response_at_unix_ms": null,
                "initial_response_seq": null,
                "yielded_at_unix_ms": null,
                "yielded_seq": null,
                "source_js": "await tools.exec_command({ cmd: \"find . -type f | wc -l\" });",
                "nested_tool_call_ids": ["tool:nested-exec"],
                "wait_tool_call_ids": [],
            })
        );
        assert_eq!(
            json!({
                "has_outer_exec_tool": trace["tool_calls"].as_object().unwrap().contains_key("tool:call-code"),
                "nested_tool": trace["tool_calls"]["tool:nested-exec"].clone(),
                "terminal_operation": trace["terminal_operations"]["terminal_operation:1"].clone(),
                "terminal_session": trace["terminal_sessions"]["55007"].clone(),
                "output_producers": trace["conversation_items"]["item:3"]["produced_by"].clone(),
            }),
            json!({
                "has_outer_exec_tool": false,
                "nested_tool": {
                    "tool_call_id": "tool:nested-exec",
                    "thread_id": "thread-1",
                    "started_by_codex_turn_id": "turn-1",
                    "model_visible_call_id": null,
                    "code_mode_runtime_tool_id": "runtime-tool-1",
                    "model_visible_call_item_ids": [],
                    "model_visible_output_item_ids": [],
                    "requester": {
                        "type": "code_cell",
                        "code_cell_id": "code_cell:call-code",
                    },
                    "kind": { "type": "exec_command" },
                    "terminal_operation_id": "terminal_operation:1",
                    "summary": {
                        "type": "terminal",
                        "operation_id": "terminal_operation:1",
                    },
                    "execution": {
                        "started_at_unix_ms": 1776420000000i64,
                        "ended_at_unix_ms": 1776420000000i64,
                        "status": "completed",
                        "started_seq": 7,
                        "ended_seq": 8,
                    },
                    "raw_invocation_payload_id": "raw_payload:4",
                    "raw_result_payload_id": "raw_payload:5",
                    "raw_runtime_payload_ids": [],
                },
                "terminal_operation": {
                    "operation_id": "terminal_operation:1",
                    "terminal_id": "55007",
                    "tool_call_id": "tool:nested-exec",
                    "kind": "exec_command",
                    "execution": {
                        "started_at_unix_ms": 1776420000000i64,
                        "ended_at_unix_ms": 1776420000000i64,
                        "status": "completed",
                        "started_seq": 7,
                        "ended_seq": 8,
                    },
                    "request": {
                        "type": "exec_command",
                        "command": ["find . -type f | wc -l"],
                        "display_command": "find . -type f | wc -l",
                        "cwd": "/repo",
                        "yield_time_ms": 1000,
                        "max_output_tokens": 200,
                    },
                    "result": {
                        "exit_code": null,
                        "stdout": "",
                        "stderr": "",
                        "formatted_output": "Chunk ID: chunk-1\nWall time: 1.0 seconds\nOutput:\n",
                        "original_token_count": 0,
                        "chunk_id": "chunk-1",
                    },
                    "model_observations": [{
                        "call_item_ids": ["item:2"],
                        "output_item_ids": ["item:3"],
                        "source": "code_cell_output",
                    }],
                    "raw_payload_ids": ["raw_payload:4", "raw_payload:5"],
                },
                "terminal_session": {
                    "terminal_id": "55007",
                    "thread_id": "thread-1",
                    "created_by_operation_id": "terminal_operation:1",
                    "operation_ids": ["terminal_operation:1"],
                    "execution": {
                        "started_at_unix_ms": 1776420000000i64,
                        "ended_at_unix_ms": null,
                        "status": "running",
                        "started_seq": 7,
                        "ended_seq": null,
                    },
                },
                "output_producers": [
                    { "type": "code_cell", "code_cell_id": "code_cell:call-code" },
                ],
            })
        );
        Ok(())
    }

    #[test]
    fn links_wait_outputs_and_payloads_to_code_cell() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&json!({
                "trace_id": "trace-test",
                "started_at_unix_ms": 10,
            }))?,
        )?;
        std::fs::create_dir(temp.path().join("payloads"))?;

        let code_call = json!({
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": "call-code",
            "name": "exec",
            "input": "await sleep(1); text('done');"
        });
        let yielded_output = json!({
            "type": "custom_tool_call_output",
            "call_id": "call-code",
            "output": [{ "type": "input_text", "text": "Execution yielded. Call wait." }]
        });
        let wait_call = json!({
            "type": "function_call",
            "status": "completed",
            "call_id": "call-wait",
            "name": "wait",
            "arguments": "{\"cell_id\":\"runtime-cell-1\"}"
        });
        let wait_output = json!({
            "type": "function_call_output",
            "call_id": "call-wait",
            "output": [{ "type": "input_text", "text": "done" }]
        });

        std::fs::write(
            temp.path().join("payloads/code-cell-invocation.json"),
            serde_json::to_vec(&json!({ "source_js": code_call["input"] }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/code-cell-yielded.json"),
            serde_json::to_vec(&json!({
                "cell_id": "runtime-cell-1",
                "status": "yielded",
                "content_items": [{ "type": "input_text", "text": "Execution yielded. Call wait." }]
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/wait-invocation.json"),
            serde_json::to_vec(&json!({
                "call_id": "call-wait",
                "tool_name": "wait",
                "payload": {
                    "type": "function",
                    "arguments": "{\"cell_id\":\"runtime-cell-1\"}"
                }
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/wait-result.json"),
            serde_json::to_vec(&json!({
                "status": "completed",
                "success": true,
                "output_preview": "done"
            }))?,
        )?;
        // Wait outputs are only canonical conversation items after the next
        // provider request carries the full snapshot back to the model.
        std::fs::write(
            temp.path().join("payloads/request.json"),
            serde_json::to_vec(&json!({
                "input": [code_call, yielded_output, wait_call, wait_output]
            }))?,
        )?;

        let mut events = std::fs::File::create(temp.path().join("events.jsonl"))?;
        write_event(
            &mut events,
            1,
            "codex.thread.started",
            json!({"thread.id": "thread-1"}),
        )?;
        write_event(
            &mut events,
            2,
            "codex.turn.started",
            json!({"thread.id": "thread-1", "turn.id": "turn-1"}),
        )?;
        write_event(
            &mut events,
            3,
            "codex.code_cell.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "code_cell.runtime_id": "runtime-cell-1",
                "model_visible_call.id": "call-code",
                "code_cell.source_js": "await sleep(1); text('done');",
                "raw_payload.invocation.id": "raw_payload:code-cell-invocation",
                "raw_payload.invocation.path": "payloads/code-cell-invocation.json",
                "raw_payload.invocation.kind": "code_cell_invocation"
            }),
        )?;
        // The initial custom `exec` call can return before the cell finishes.
        // This event records that yield point without closing the full runtime
        // execution window.
        write_event(
            &mut events,
            4,
            "codex.code_cell.ended",
            json!({
                "thread.id": "thread-1",
                "code_cell.runtime_id": "runtime-cell-1",
                "status": "yielded",
                "raw_payload.result.id": "raw_payload:code-cell-yielded",
                "raw_payload.result.path": "payloads/code-cell-yielded.json",
                "raw_payload.result.kind": "code_cell_result"
            }),
        )?;
        write_event(
            &mut events,
            5,
            "codex.tool.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "tool.call_id": "tool:call-wait",
                "tool.name": "wait",
                "tool.requester.type": "model",
                "model_visible_call.id": "call-wait",
                "raw_payload.invocation.id": "raw_payload:wait-invocation",
                "raw_payload.invocation.path": "payloads/wait-invocation.json",
                "raw_payload.invocation.kind": "tool_invocation"
            }),
        )?;
        write_event(
            &mut events,
            6,
            "codex.tool.ended",
            json!({
                "tool.call_id": "tool:call-wait",
                "status": "completed",
                "output_preview": "done",
                "raw_payload.result.id": "raw_payload:wait-result",
                "raw_payload.result.path": "payloads/wait-result.json",
                "raw_payload.result.kind": "tool_result"
            }),
        )?;
        write_event(
            &mut events,
            7,
            "codex.code_cell.ended",
            json!({
                "thread.id": "thread-1",
                "code_cell.runtime_id": "runtime-cell-1",
                "status": "completed",
                "model_visible_wait_call.id": "call-wait"
            }),
        )?;
        write_event(
            &mut events,
            8,
            "codex.inference.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "inference.id": "inf-1",
                "raw_payload.request.id": "raw_payload:request",
                "raw_payload.request.path": "payloads/request.json",
                "raw_payload.request.kind": "inference_request"
            }),
        )?;

        let trace = reduce_bundle_json(temp.path())?;

        assert_eq!(
            json!({
                "execution": trace["code_cells"]["code_cell:call-code"]["execution"].clone(),
                "runtime_status": trace["code_cells"]["code_cell:call-code"]["runtime_status"].clone(),
                "initial_response_at_unix_ms": trace["code_cells"]["code_cell:call-code"]["initial_response_at_unix_ms"].clone(),
                "initial_response_seq": trace["code_cells"]["code_cell:call-code"]["initial_response_seq"].clone(),
                "yielded_at_unix_ms": trace["code_cells"]["code_cell:call-code"]["yielded_at_unix_ms"].clone(),
                "yielded_seq": trace["code_cells"]["code_cell:call-code"]["yielded_seq"].clone(),
                "output_item_ids": trace["code_cells"]["code_cell:call-code"]["output_item_ids"].clone(),
                "wait_tool_call_ids": trace["code_cells"]["code_cell:call-code"]["wait_tool_call_ids"].clone(),
                "wait_output_producers": trace["conversation_items"]["item:4"]["produced_by"].clone(),
            }),
            json!({
                "execution": {
                    "started_at_unix_ms": 1776420000000i64,
                    "ended_at_unix_ms": 1776420000000i64,
                    "status": "completed",
                    "started_seq": 3,
                    "ended_seq": 7,
                },
                "runtime_status": "completed",
                "initial_response_at_unix_ms": 1776420000000i64,
                "initial_response_seq": 4,
                "yielded_at_unix_ms": 1776420000000i64,
                "yielded_seq": 4,
                "output_item_ids": ["item:2", "item:4"],
                "wait_tool_call_ids": ["tool:call-wait"],
                "wait_output_producers": [
                    { "type": "tool", "tool_call_id": "tool:call-wait" },
                    { "type": "code_cell", "code_cell_id": "code_cell:call-code" },
                ],
            })
        );
        Ok(())
    }

    #[test]
    fn write_stdin_terminal_operation_uses_request_session_id() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&json!({
                "trace_id": "trace-test",
                "started_at_unix_ms": 10,
            }))?,
        )?;
        std::fs::create_dir(temp.path().join("payloads"))?;

        std::fs::write(
            temp.path().join("payloads/1.json"),
            serde_json::to_vec(&json!({
                "call_id": "call-write",
                "tool_name": "write_stdin",
                "payload": {
                    "type": "function",
                    "arguments": "{\"session_id\":12121,\"chars\":\"\"}"
                }
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/2.json"),
            serde_json::to_vec(&json!({
                "status": "completed",
                "success": true,
                "output_preview": "Chunk ID: chunk-1\nWall time: 2.0 seconds\nProcess exited with code 0\nOutput:\n42\n",
                "code_mode_result": {
                    "chunk_id": "chunk-1",
                    "wall_time_seconds": 2.0,
                    "exit_code": 0,
                    "original_token_count": 1,
                    "output": "42\n"
                }
            }))?,
        )?;

        let mut events = std::fs::File::create(temp.path().join("events.jsonl"))?;
        write_event(
            &mut events,
            1,
            "codex.thread.started",
            json!({"thread.id": "thread-1"}),
        )?;
        write_event(
            &mut events,
            2,
            "codex.turn.started",
            json!({"thread.id": "thread-1", "turn.id": "turn-1"}),
        )?;
        write_event(
            &mut events,
            3,
            "codex.tool.started",
            json!({
                "thread.id": "thread-1",
                "turn.id": "turn-1",
                "tool.call_id": "tool:call-write",
                "tool.name": "write_stdin",
                "tool.requester.type": "model",
                "model_visible_call.id": "call-write",
                "raw_payload.invocation.id": "raw_payload:1",
                "raw_payload.invocation.path": "payloads/1.json",
                "raw_payload.invocation.kind": "tool_invocation"
            }),
        )?;
        write_event(
            &mut events,
            4,
            "codex.tool.ended",
            json!({
                "tool.call_id": "tool:call-write",
                "status": "completed",
                "output_preview": "Chunk ID: chunk-1\nWall time: 2.0 seconds\nProcess exited with code 0\nOutput:\n42\n",
                "raw_payload.result.id": "raw_payload:2",
                "raw_payload.result.path": "payloads/2.json",
                "raw_payload.result.kind": "tool_result"
            }),
        )?;

        let trace = reduce_bundle_json(temp.path())?;

        // `write_stdin` is a continuation of an existing terminal session. The
        // final result can omit `session_id`, so the reducer must preserve the
        // request-side id instead of creating a one-shot synthetic session.
        assert_eq!(
            trace["terminal_operations"]["terminal_operation:1"]["terminal_id"],
            json!("12121")
        );
        assert_eq!(
            trace["terminal_sessions"]["12121"],
            json!({
                "terminal_id": "12121",
                "thread_id": "thread-1",
                "created_by_operation_id": "terminal_operation:1",
                "operation_ids": ["terminal_operation:1"],
                "execution": {
                    "started_at_unix_ms": 1776420000000i64,
                    "ended_at_unix_ms": null,
                    "status": "running",
                    "started_seq": 3,
                    "ended_seq": null,
                },
            })
        );
        assert!(
            !trace["terminal_sessions"]
                .as_object()
                .unwrap()
                .contains_key("terminal:terminal_operation:1")
        );
        Ok(())
    }

    fn write_event(
        file: &mut std::fs::File,
        _seq: u64,
        event: &str,
        mut line: serde_json::Value,
    ) -> anyhow::Result<()> {
        line["timestamp"] = json!("2026-04-17T10:00:00.000000Z");
        line["level"] = json!("INFO");
        line["target"] = json!(crate::LOCAL_TRACE_TARGET);
        line["event.name"] = json!(event);
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
        Ok(())
    }
}
