use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

pub const REDUCED_STATE_FILE_NAME: &str = "state.json";

#[derive(Debug)]
struct CapturedEvent {
    seq: u64,
    wall_time_unix_ms: i64,
    fields: Map<String, Value>,
}

#[derive(Debug, Serialize)]
struct ReducedTrace {
    schema_version: u32,
    trace_id: String,
    rollout_id: String,
    started_at_unix_ms: i64,
    ended_at_unix_ms: Option<i64>,
    status: String,
    root_thread_id: String,
    threads: BTreeMap<String, Value>,
    codex_turns: BTreeMap<String, Value>,
    conversation_items: BTreeMap<String, Value>,
    inference_calls: BTreeMap<String, Value>,
    code_cells: BTreeMap<String, Value>,
    tool_calls: BTreeMap<String, Value>,
    terminal_sessions: BTreeMap<String, Value>,
    terminal_operations: BTreeMap<String, Value>,
    compactions: BTreeMap<String, Value>,
    compaction_requests: BTreeMap<String, Value>,
    interaction_edges: BTreeMap<String, Value>,
    raw_payloads: BTreeMap<String, Value>,
}

struct Reducer<'a> {
    bundle_dir: &'a Path,
    trace: ReducedTrace,
    next_conversation_item_ordinal: u64,
}

pub fn reduce_bundle_to_path(bundle_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let trace = reduce_bundle(bundle_dir.as_ref())?;
    let file = File::create(output.as_ref())
        .with_context(|| format!("create {}", output.as_ref().display()))?;
    serde_json::to_writer_pretty(file, &trace)
        .with_context(|| format!("write {}", output.as_ref().display()))
}

fn reduce_bundle(bundle_dir: &Path) -> Result<ReducedTrace> {
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
        trace: ReducedTrace {
            schema_version: 1,
            trace_id: trace_id.clone(),
            rollout_id: trace_id,
            started_at_unix_ms,
            ended_at_unix_ms: None,
            status: "running".to_string(),
            root_thread_id: String::new(),
            threads: BTreeMap::new(),
            codex_turns: BTreeMap::new(),
            conversation_items: BTreeMap::new(),
            inference_calls: BTreeMap::new(),
            code_cells: BTreeMap::new(),
            tool_calls: BTreeMap::new(),
            terminal_sessions: BTreeMap::new(),
            terminal_operations: BTreeMap::new(),
            compactions: BTreeMap::new(),
            compaction_requests: BTreeMap::new(),
            interaction_edges: BTreeMap::new(),
            raw_payloads: BTreeMap::new(),
        },
        next_conversation_item_ordinal: 1,
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
    reducer.attach_tool_payloads_to_interaction_edges();
    if reducer.trace.started_at_unix_ms == 0 {
        reducer.trace.started_at_unix_ms = reducer
            .trace
            .threads
            .values()
            .filter_map(|thread| {
                execution(thread).and_then(|execution| execution.get("started_at_unix_ms"))
            })
            .filter_map(Value::as_i64)
            .min()
            .unwrap_or(0);
    }
    reducer.trace.ended_at_unix_ms = reducer
        .trace
        .threads
        .values()
        .filter_map(|thread| {
            execution(thread).and_then(|execution| execution.get("ended_at_unix_ms"))
        })
        .filter_map(Value::as_i64)
        .max();
    if reducer.trace.ended_at_unix_ms.is_some() {
        reducer.trace.status = "completed".to_string();
    }
    Ok(reducer.trace)
}

impl Reducer<'_> {
    fn apply_event(&mut self, event: CapturedEvent) -> Result<()> {
        self.insert_payload_ref(&event, "request");
        self.insert_payload_ref(&event, "response");
        self.insert_payload_ref(&event, "invocation");
        self.insert_payload_ref(&event, "result");

        match event_name(&event).unwrap_or_default() {
            "codex.thread.started" | "thread_started" => self.thread_started(&event),
            "codex.turn.started" | "turn_started" => self.turn_started(&event),
            "codex.turn.ended" | "turn_ended" => self.turn_ended(&event),
            "codex.inference.started" | "inference_started" => self.inference_started(&event)?,
            "codex.inference.completed" | "inference_completed" => {
                self.inference_completed(&event)?;
            }
            "codex.inference.failed" | "inference_failed" => self.inference_failed(&event),
            "codex.tool.started" | "tool_started" => self.tool_started(&event),
            "codex.tool.ended" | "tool_ended" => self.tool_ended(&event),
            "codex.collab.spawn.started" => self.tool_to_thread_edge_started(&event, "spawn_agent"),
            "codex.collab.spawn.ended" => self.tool_to_thread_edge_ended(&event, "spawn_agent"),
            "codex.collab.message.started" => {
                self.tool_to_thread_edge_started(&event, "send_message");
            }
            "codex.collab.message.ended" => self.tool_to_thread_edge_ended(&event, "send_message"),
            "codex.collab.wait.started" => self.agent_result_edge_started(&event),
            "codex.collab.wait.ended" => self.agent_result_edge_ended(&event),
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
            json!({
                "raw_payload_id": id,
                "kind": kind,
                "path": path,
            }),
        );
    }

    fn ensure_thread(&mut self, thread_id: &str, now: i64) {
        if self.trace.root_thread_id.is_empty() {
            self.trace.root_thread_id = thread_id.to_string();
        }
        self.trace
            .threads
            .entry(thread_id.to_string())
            .or_insert_with(|| {
                json!({
                    "thread_id": thread_id,
                    "agent_path": "/root",
                    "origin": { "type": "root" },
                    "execution": execution_json(now, None, "running", 0, None),
                    "conversation_item_ids": [],
                })
            });
    }

    fn thread_started(&mut self, event: &CapturedEvent) {
        let Some(thread_id) = trace_field_str(event, "thread", "id") else {
            return;
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        if let Some(thread) = self.trace.threads.get_mut(thread_id) {
            set_string(
                thread,
                "agent_path",
                trace_field_str(event, "agent", "path").unwrap_or("/root"),
            );
            set_string(
                thread,
                "default_model",
                field_str(event, "default_model")
                    .or_else(|| field_str(event, "model"))
                    .unwrap_or(""),
            );
            set_string(
                thread,
                "source",
                trace_field_str(event, "thread", "source").unwrap_or(""),
            );
            let parent_thread_id = trace_field_str(event, "parent_thread", "id").unwrap_or("");
            thread["origin"] = if parent_thread_id.is_empty() {
                json!({ "type": "root" })
            } else {
                json!({
                    "type": "spawned",
                    "parent_thread_id": parent_thread_id,
                })
            };
            set_execution_start(thread, event.wall_time_unix_ms, event.seq);
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
            json!({
                "codex_turn_id": turn_id,
                "thread_id": thread_id,
                "execution": execution_json(event.wall_time_unix_ms, None, "running", event.seq, None),
                "input_item_ids": [],
                "output_item_ids": [],
            }),
        );
    }

    fn turn_ended(&mut self, event: &CapturedEvent) {
        let Some(turn_id) = trace_field_str(event, "turn", "id") else {
            return;
        };
        if let Some(turn) = self.trace.codex_turns.get_mut(turn_id) {
            set_execution_end(
                turn,
                event.wall_time_unix_ms,
                event.seq,
                field_str(event, "status").unwrap_or("completed"),
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
            json!({
                "inference_call_id": inference_id,
                "thread_id": thread_id,
                "codex_turn_id": turn_id,
                "model": field_str(event, "model").unwrap_or(""),
                "provider_name": trace_field_str(event, "provider", "name").unwrap_or(""),
                "execution": execution_json(event.wall_time_unix_ms, None, "running", event.seq, None),
                "request_item_ids": request_item_ids,
                "response_item_ids": [],
                "raw_request_payload_id": empty_to_null(request_payload_id),
                "raw_response_payload_id": Value::Null,
            }),
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
            set_execution_end(inference, event.wall_time_unix_ms, event.seq, "completed");
            set_string(
                inference,
                "response_id",
                trace_field_str(event, "response", "id").unwrap_or(""),
            );
            inference["raw_response_payload_id"] = empty_to_null(response_payload_id);
            inference["response_item_ids"] = json!(response_item_ids);
            if let Some(usage) = usage {
                inference["usage"] = usage;
            }
            if let Some(turn_id) = inference.get("codex_turn_id").and_then(Value::as_str)
                && let Some(turn) = self.trace.codex_turns.get_mut(turn_id)
            {
                extend_array_unique(turn, "output_item_ids", response_item_ids.iter());
            }
        }
        if let Some(thread_id) = thread_id
            && let Some(thread) = self.trace.threads.get_mut(&thread_id)
        {
            extend_array_unique(thread, "conversation_item_ids", response_item_ids.iter());
        }
        Ok(())
    }

    fn inference_failed(&mut self, event: &CapturedEvent) {
        let Some(inference_id) = trace_field_str(event, "inference", "id") else {
            return;
        };
        if let Some(inference) = self.trace.inference_calls.get_mut(inference_id) {
            set_execution_end(inference, event.wall_time_unix_ms, event.seq, "failed");
            set_string(inference, "error", field_str(event, "error").unwrap_or(""));
        }
    }

    fn tool_started(&mut self, event: &CapturedEvent) {
        let (Some(tool_call_id), Some(thread_id), Some(turn_id)) = (
            trace_field_str(event, "tool", "call_id"),
            trace_field_str(event, "thread", "id"),
            trace_field_str(event, "turn", "id"),
        ) else {
            return;
        };
        self.ensure_thread(thread_id, event.wall_time_unix_ms);
        let tool_name = trace_field_str(event, "tool", "name").unwrap_or("tool");
        let raw_invocation_payload_id =
            raw_payload_field_str(event, "invocation", "id").unwrap_or("");
        self.trace.tool_calls.insert(
            tool_call_id.to_string(),
            json!({
                "tool_call_id": tool_call_id,
                "thread_id": thread_id,
                "codex_turn_id": turn_id,
                "started_by_codex_turn_id": turn_id,
                "model_visible_call_id": trace_field_str(event, "model_visible_call", "id").unwrap_or(""),
                "model_visible_call_item_ids": [],
                "model_visible_output_item_ids": [],
                "requester": { "type": "model" },
                "kind": { "type": "other", "name": tool_name },
                "summary": {
                    "input_preview": "",
                    "output_preview": "",
                },
                "execution": execution_json(event.wall_time_unix_ms, None, "running", event.seq, None),
                "raw_invocation_payload_id": empty_to_null(raw_invocation_payload_id),
                "raw_result_payload_id": Value::Null,
                "raw_runtime_payload_ids": [],
            }),
        );
    }

    fn tool_ended(&mut self, event: &CapturedEvent) {
        let Some(tool_call_id) = trace_field_str(event, "tool", "call_id") else {
            return;
        };
        if let Some(tool) = self.trace.tool_calls.get_mut(tool_call_id) {
            let status = field_str(event, "status").unwrap_or("completed");
            set_execution_end(tool, event.wall_time_unix_ms, event.seq, status);
            if let Some(summary) = tool.get_mut("summary").and_then(Value::as_object_mut) {
                summary.insert(
                    "output_preview".to_string(),
                    Value::String(field_str(event, "output_preview").unwrap_or("").to_string()),
                );
            }
            tool["raw_result_payload_id"] =
                empty_to_null(raw_payload_field_str(event, "result", "id").unwrap_or(""));
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
            kind,
            json!({ "type": "tool_call", "tool_call_id": tool_call_id }),
            thread_anchor(target_thread_id),
            event,
        );
    }

    fn tool_to_thread_edge_ended(&mut self, event: &CapturedEvent, kind: &str) {
        self.tool_to_thread_edge_started(event, kind);
        if let Some(tool_call_id) = normalized_tool_call_id(event) {
            let edge_id = interaction_edge_id(kind, &tool_call_id);
            if let Some(edge) = self.trace.interaction_edges.get_mut(&edge_id) {
                edge["ended_at_unix_ms"] = event.wall_time_unix_ms.into();
            }
        }
        self.apply_target_agent_metadata(event);
    }

    fn agent_result_edge_started(&mut self, event: &CapturedEvent) {
        let (Some(tool_call_id), Some(target_thread_id)) = (
            normalized_tool_call_id(event),
            trace_field_str(event, "target.thread", "id").filter(|thread_id| !thread_id.is_empty()),
        ) else {
            return;
        };
        let edge_id = agent_result_edge_id(&tool_call_id, target_thread_id);
        self.upsert_interaction_edge(
            edge_id,
            "agent_result",
            thread_anchor(target_thread_id),
            json!({ "type": "tool_call", "tool_call_id": tool_call_id }),
            event,
        );
    }

    fn agent_result_edge_ended(&mut self, event: &CapturedEvent) {
        self.agent_result_edge_started(event);
        let Some(tool_call_id) = normalized_tool_call_id(event) else {
            return;
        };
        if let Some(target_thread_id) =
            trace_field_str(event, "target.thread", "id").filter(|thread_id| !thread_id.is_empty())
        {
            let edge_id = agent_result_edge_id(&tool_call_id, target_thread_id);
            if let Some(edge) = self.trace.interaction_edges.get_mut(&edge_id) {
                edge["ended_at_unix_ms"] = event.wall_time_unix_ms.into();
            }
        } else {
            let edge_prefix = format!("edge:agent_result:{tool_call_id}:");
            for (edge_id, edge) in &mut self.trace.interaction_edges {
                if edge_id.starts_with(&edge_prefix) {
                    edge["ended_at_unix_ms"] = event.wall_time_unix_ms.into();
                }
            }
        }
        self.apply_target_agent_metadata(event);
    }

    fn upsert_interaction_edge(
        &mut self,
        edge_id: String,
        kind: &str,
        source: Value,
        target: Value,
        event: &CapturedEvent,
    ) {
        let edge = self
            .trace
            .interaction_edges
            .entry(edge_id.clone())
            .or_insert_with(|| {
                json!({
                    "interaction_edge_id": edge_id,
                    "kind": { "type": kind },
                    "source": Value::Null,
                    "target": Value::Null,
                    "started_at_unix_ms": event.wall_time_unix_ms,
                    "ended_at_unix_ms": Value::Null,
                    "carried_item_ids": [],
                    "carried_raw_payload_ids": [],
                })
            });
        edge["source"] = source;
        if !target.is_null() {
            edge["target"] = target;
        }
    }

    fn apply_target_agent_metadata(&mut self, event: &CapturedEvent) {
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
            set_string(thread, "nickname", nickname);
        }
        if let Some(agent_role) =
            trace_field_str(event, "target.agent", "role").filter(|role| !role.is_empty())
            && let Some(origin) = thread.get_mut("origin").and_then(Value::as_object_mut)
        {
            origin.insert(
                "agent_role".to_string(),
                Value::String(agent_role.to_string()),
            );
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
        let items = payload
            .get("input")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ids = self.add_conversation_items(
            thread_id,
            event.wall_time_unix_ms,
            items,
            None,
            raw_payload_id,
        );
        if let Some(thread) = self.trace.threads.get_mut(thread_id) {
            extend_array_unique(thread, "conversation_item_ids", ids.iter());
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
        let thread_id = inference
            .get("thread_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let Some(payload) = self.payload_by_id(raw_payload_id)? else {
            return Ok((thread_id, Vec::new(), None));
        };
        let items = payload
            .get("output_items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let producer = Some(json!({
            "type": "inference",
            "inference_call_id": inference_id,
        }));
        let ids = thread_id.as_deref().map_or_else(Vec::new, |thread_id| {
            self.add_conversation_items(
                thread_id,
                event.wall_time_unix_ms,
                items,
                producer,
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
        producer: Option<Value>,
        raw_payload_id: &str,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        for item in items {
            let item_id = format!("item:{}", self.next_conversation_item_ordinal);
            self.next_conversation_item_ordinal += 1;
            let produced_by = producer.clone().into_iter().collect::<Vec<_>>();
            let normalized = normalize_conversation_item(
                &item_id,
                thread_id,
                first_seen_at_unix_ms,
                &item,
                produced_by,
                raw_payload_id,
            );
            self.trace
                .conversation_items
                .insert(item_id.clone(), normalized);
            ids.push(item_id);
        }
        ids
    }

    fn payload_by_id(&self, raw_payload_id: &str) -> Result<Option<Value>> {
        if raw_payload_id.is_empty() {
            return Ok(None);
        }
        let Some(payload) = self.trace.raw_payloads.get(raw_payload_id) else {
            return Ok(None);
        };
        let Some(path) = payload.get("path").and_then(Value::as_str) else {
            return Ok(None);
        };
        read_json(self.bundle_dir.join(path)).map(Some)
    }

    fn link_tools_to_conversation_items(&mut self) {
        for tool in self.trace.tool_calls.values_mut() {
            let Some(call_id) = tool
                .get("model_visible_call_id")
                .and_then(Value::as_str)
                .filter(|call_id| !call_id.is_empty())
            else {
                continue;
            };
            let mut call_items = Vec::new();
            let mut output_items = Vec::new();
            for item in self.trace.conversation_items.values() {
                if item.get("call_id").and_then(Value::as_str) != Some(call_id) {
                    continue;
                }
                let Some(item_id) = item.get("item_id").and_then(Value::as_str) else {
                    continue;
                };
                match item.get("kind").and_then(Value::as_str) {
                    Some("function_call" | "custom_tool_call" | "local_shell_call") => {
                        call_items.push(item_id.to_string());
                    }
                    Some(
                        "function_call_output" | "custom_tool_call_output" | "tool_search_output",
                    ) => {
                        output_items.push(item_id.to_string());
                    }
                    _ => {}
                }
            }
            tool["model_visible_call_item_ids"] = json!(call_items);
            tool["model_visible_output_item_ids"] = json!(output_items);
        }
    }

    fn attach_tool_payloads_to_interaction_edges(&mut self) {
        let mut tool_payloads = BTreeMap::new();
        for (tool_call_id, tool) in &self.trace.tool_calls {
            let mut raw_payload_ids = Vec::new();
            for field in ["raw_invocation_payload_id", "raw_result_payload_id"] {
                if let Some(raw_payload_id) = tool.get(field).and_then(Value::as_str) {
                    push_unique(&mut raw_payload_ids, raw_payload_id);
                }
            }
            if let Some(runtime_payload_ids) = tool
                .get("raw_runtime_payload_ids")
                .and_then(Value::as_array)
            {
                for raw_payload_id in runtime_payload_ids.iter().filter_map(Value::as_str) {
                    push_unique(&mut raw_payload_ids, raw_payload_id);
                }
            }

            let mut item_ids = Vec::new();
            for field in [
                "model_visible_call_item_ids",
                "model_visible_output_item_ids",
            ] {
                if let Some(tool_item_ids) = tool.get(field).and_then(Value::as_array) {
                    for item_id in tool_item_ids.iter().filter_map(Value::as_str) {
                        push_unique(&mut item_ids, item_id);
                    }
                }
            }
            tool_payloads.insert(tool_call_id.clone(), (raw_payload_ids, item_ids));
        }
        for edge in self.trace.interaction_edges.values_mut() {
            let mut raw_payload_ids = string_array_field(edge, "carried_raw_payload_ids");
            let mut item_ids = string_array_field(edge, "carried_item_ids");
            for anchor_name in ["source", "target"] {
                let Some(tool_call_id) = edge
                    .get(anchor_name)
                    .and_then(Value::as_object)
                    .filter(|anchor| {
                        anchor.get("type").and_then(Value::as_str) == Some("tool_call")
                    })
                    .and_then(|anchor| anchor.get("tool_call_id").and_then(Value::as_str))
                else {
                    continue;
                };
                if let Some((tool_raw_payload_ids, tool_item_ids)) = tool_payloads.get(tool_call_id)
                {
                    push_unique_all(&mut raw_payload_ids, tool_raw_payload_ids);
                    push_unique_all(&mut item_ids, tool_item_ids);
                }
            }
            edge["carried_raw_payload_ids"] = json!(raw_payload_ids);
            edge["carried_item_ids"] = json!(item_ids);
        }
    }
}

fn normalize_conversation_item(
    item_id: &str,
    thread_id: &str,
    first_seen_at_unix_ms: i64,
    item: &Value,
    produced_by: Vec<Value>,
    raw_payload_id: &str,
) -> Value {
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("other");
    let role = item
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or(match kind {
            "function_call" | "custom_tool_call" | "local_shell_call" | "tool_search_call" => {
                "assistant"
            }
            "function_call_output" | "custom_tool_call_output" | "tool_search_output" => "tool",
            _ => "",
        });
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))
        .unwrap_or("");
    let channel = if role == "assistant" && kind == "message" {
        "final"
    } else {
        ""
    };
    json!({
        "item_id": item_id,
        "thread_id": thread_id,
        "role": role,
        "channel": channel,
        "kind": kind,
        "call_id": empty_string(call_id),
        "first_seen_at_unix_ms": first_seen_at_unix_ms,
        "body": normalize_body(kind, item, raw_payload_id),
        "produced_by": produced_by,
    })
}

fn normalize_body(kind: &str, item: &Value, raw_payload_id: &str) -> Value {
    match kind {
        "message" => {
            let parts = item
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|content| {
                    content
                        .get("text")
                        .and_then(Value::as_str)
                        .map(|text| json!({ "type": "text", "text": text }))
                })
                .collect::<Vec<_>>();
            json!({ "parts": parts })
        }
        "custom_tool_call" => json!({
            "parts": [{
                "type": if item.get("name").and_then(Value::as_str) == Some("exec") { "code" } else { "text" },
                "language": "javascript",
                "source": item.get("input").and_then(Value::as_str).unwrap_or(""),
                "text": item.get("input").and_then(Value::as_str).unwrap_or(""),
            }]
        }),
        "function_call" => json!({
            "parts": [{
                "type": "json",
                "summary": item.get("name").and_then(Value::as_str).unwrap_or("function_call"),
                "raw_payload_id": raw_payload_id,
            }]
        }),
        "function_call_output" | "custom_tool_call_output" => json!({
            "parts": [{
                "type": "text",
                "text": tool_output_text(item.get("output")),
            }]
        }),
        "reasoning" => json!({
            "parts": [{
                "type": "payload_ref",
                "label": "reasoning",
                "raw_payload_id": raw_payload_id,
            }]
        }),
        _ => json!({
            "parts": [{
                "type": "json",
                "summary": kind,
                "raw_payload_id": raw_payload_id,
            }]
        }),
    }
}

fn tool_output_text(output: Option<&Value>) -> String {
    let Some(output) = output else {
        return String::new();
    };
    if let Some(text) = output.as_str() {
        return text.to_string();
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
    Some(if call_id.starts_with("tool:") {
        call_id.to_string()
    } else {
        format!("tool:{call_id}")
    })
}

fn interaction_edge_id(kind: &str, tool_call_id: &str) -> String {
    format!("edge:{kind}:{tool_call_id}")
}

fn agent_result_edge_id(tool_call_id: &str, thread_id: &str) -> String {
    format!("edge:agent_result:{tool_call_id}:{thread_id}")
}

fn thread_anchor(thread_id: &str) -> Value {
    if thread_id.is_empty() {
        Value::Null
    } else {
        json!({ "type": "thread", "thread_id": thread_id })
    }
}

fn string_array_field(object: &Value, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn push_unique_all(values: &mut Vec<String>, new_values: &[String]) {
    for value in new_values {
        push_unique(values, value);
    }
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

fn execution(value: &Value) -> Option<&Value> {
    value.get("execution")
}

fn set_string(object: &mut Value, key: &str, value: &str) {
    object[key] = Value::String(value.to_string());
}

fn set_execution_start(object: &mut Value, started_at_unix_ms: i64, started_seq: u64) {
    object["execution"]["started_at_unix_ms"] = started_at_unix_ms.into();
    object["execution"]["started_seq"] = started_seq.into();
}

fn set_execution_end(object: &mut Value, ended_at_unix_ms: i64, ended_seq: u64, status: &str) {
    object["execution"]["ended_at_unix_ms"] = ended_at_unix_ms.into();
    object["execution"]["ended_seq"] = ended_seq.into();
    object["execution"]["status"] = Value::String(status.to_string());
}

fn execution_json(
    started_at_unix_ms: i64,
    ended_at_unix_ms: Option<i64>,
    status: &str,
    started_seq: u64,
    ended_seq: Option<u64>,
) -> Value {
    json!({
        "started_at_unix_ms": started_at_unix_ms,
        "ended_at_unix_ms": ended_at_unix_ms,
        "status": status,
        "started_seq": started_seq,
        "ended_seq": ended_seq,
    })
}

fn extend_array_unique<'a>(
    object: &mut Value,
    key: &str,
    values: impl IntoIterator<Item = &'a String>,
) {
    let Some(array) = object.get_mut(key).and_then(Value::as_array_mut) else {
        return;
    };
    for value in values {
        if !array
            .iter()
            .any(|existing| existing.as_str() == Some(value))
        {
            array.push(Value::String(value.clone()));
        }
    }
}

fn empty_to_null(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

fn empty_string(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

    use super::reduce_bundle;

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
        std::fs::write(
            temp.path().join("payloads/1.json"),
            serde_json::to_vec(&json!({
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "hi" }]
                }]
            }))?,
        )?;
        std::fs::write(
            temp.path().join("payloads/2.json"),
            serde_json::to_vec(&json!({
                "response_id": "resp-1",
                "token_usage": { "input_tokens": 1, "output_tokens": 2 },
                "output_items": [{
                    "type": "function_call",
                    "name": "shell",
                    "arguments": "{}",
                    "call_id": "call-1"
                }]
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

        let trace = reduce_bundle(temp.path())?;

        assert_eq!(trace.threads.len(), 1);
        assert_eq!(trace.inference_calls.len(), 1);
        assert_eq!(trace.conversation_items.len(), 2);
        assert_eq!(
            trace.tool_calls["tool-1"]["model_visible_call_item_ids"],
            json!(["item:2"])
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
                "parent_thread.id": "thread-parent"
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

        let trace = reduce_bundle(temp.path())?;

        assert_eq!(
            trace.interaction_edges["edge:spawn_agent:tool:call-spawn"],
            json!({
                "interaction_edge_id": "edge:spawn_agent:tool:call-spawn",
                "kind": { "type": "spawn_agent" },
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
        assert_eq!(trace.threads["thread-child"]["nickname"], json!("Euclid"));
        assert_eq!(
            trace.threads["thread-child"]["origin"]["agent_role"],
            json!("worker")
        );
        assert_eq!(
            trace.interaction_edges["edge:agent_result:tool:call-wait:thread-child"],
            json!({
                "interaction_edge_id": "edge:agent_result:tool:call-wait:thread-child",
                "kind": { "type": "agent_result" },
                "source": { "type": "thread", "thread_id": "thread-child" },
                "target": { "type": "tool_call", "tool_call_id": "tool:call-wait" },
                "started_at_unix_ms": 1776420000000i64,
                "ended_at_unix_ms": 1776420000000i64,
                "carried_item_ids": [],
                "carried_raw_payload_ids": [
                    "raw_payload:wait-invocation",
                    "raw_payload:wait-result"
                ],
            })
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
