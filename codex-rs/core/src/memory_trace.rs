use std::path::Path;
use std::path::PathBuf;

use crate::ModelClient;
use crate::error::CodexErr;
use crate::error::Result;
use codex_api::RawMemory as ApiRawMemory;
use codex_api::RawMemoryMetadata as ApiRawMemoryMetadata;
use codex_otel::OtelManager;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use serde_json::Map;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltMemory {
    pub memory_id: String,
    pub source_path: PathBuf,
    pub raw_memory: String,
    pub memory_summary: String,
}

struct PreparedTrace {
    memory_id: String,
    source_path: PathBuf,
    payload: ApiRawMemory,
}

/// Loads raw trace files, normalizes items, and builds memory summaries.
///
/// The request/response wiring mirrors the memory summarize E2E flow:
/// `/v1/memories/trace_summarize` with one output object per input raw memory.
///
/// The caller provides the model selection, reasoning effort, and telemetry context explicitly so
/// the session-scoped [`ModelClient`] can be reused across turns.
pub async fn build_memories_from_trace_files(
    client: &ModelClient,
    trace_paths: &[PathBuf],
    model_info: &ModelInfo,
    effort: Option<ReasoningEffortConfig>,
    otel_manager: &OtelManager,
) -> Result<Vec<BuiltMemory>> {
    if trace_paths.is_empty() {
        return Ok(Vec::new());
    }

    let mut prepared = Vec::with_capacity(trace_paths.len());
    for (index, path) in trace_paths.iter().enumerate() {
        prepared.push(prepare_trace(index + 1, path).await?);
    }

    let raw_memories = prepared.iter().map(|trace| trace.payload.clone()).collect();
    let output = client
        .summarize_memories(raw_memories, model_info, effort, otel_manager)
        .await?;
    if output.len() != prepared.len() {
        return Err(CodexErr::InvalidRequest(format!(
            "unexpected memory summarize output length: expected {}, got {}",
            prepared.len(),
            output.len()
        )));
    }

    Ok(prepared
        .into_iter()
        .zip(output)
        .map(|(trace, summary)| BuiltMemory {
            memory_id: trace.memory_id,
            source_path: trace.source_path,
            raw_memory: summary.raw_memory,
            memory_summary: summary.memory_summary,
        })
        .collect())
}

async fn prepare_trace(index: usize, path: &Path) -> Result<PreparedTrace> {
    let text = load_trace_text(path).await?;
    let items = load_trace_items(path, &text)?;
    let memory_id = build_memory_id(index, path);
    let source_path = path.to_path_buf();

    Ok(PreparedTrace {
        memory_id: memory_id.clone(),
        source_path: source_path.clone(),
        payload: ApiRawMemory {
            id: memory_id,
            metadata: ApiRawMemoryMetadata {
                source_path: source_path.display().to_string(),
            },
            items,
        },
    })
}

async fn load_trace_text(path: &Path) -> Result<String> {
    let raw = tokio::fs::read(path).await?;
    Ok(decode_trace_bytes(&raw))
}

fn decode_trace_bytes(raw: &[u8]) -> String {
    if let Some(without_bom) = raw.strip_prefix(&[0xEF, 0xBB, 0xBF])
        && let Ok(text) = String::from_utf8(without_bom.to_vec())
    {
        return text;
    }
    if let Ok(text) = String::from_utf8(raw.to_vec()) {
        return text;
    }
    raw.iter().map(|b| char::from(*b)).collect()
}

fn load_trace_items(path: &Path, text: &str) -> Result<Vec<Value>> {
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) {
        let dict_items = items
            .into_iter()
            .filter(serde_json::Value::is_object)
            .collect::<Vec<_>>();
        if dict_items.is_empty() {
            return Err(CodexErr::InvalidRequest(format!(
                "no object items found in trace file: {}",
                path.display()
            )));
        }
        return normalize_trace_items(dict_items, path);
    }

    let mut parsed_items = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || (!line.starts_with('{') && !line.starts_with('[')) {
            continue;
        }

        let Ok(obj) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        match obj {
            Value::Object(_) => parsed_items.push(obj),
            Value::Array(inner) => {
                parsed_items.extend(inner.into_iter().filter(serde_json::Value::is_object))
            }
            _ => {}
        }
    }

    if parsed_items.is_empty() {
        return Err(CodexErr::InvalidRequest(format!(
            "no JSON items parsed from trace file: {}",
            path.display()
        )));
    }

    normalize_trace_items(parsed_items, path)
}

fn normalize_trace_items(items: Vec<Value>, path: &Path) -> Result<Vec<Value>> {
    let mut normalized = Vec::new();

    for item in items {
        let Value::Object(obj) = item else {
            continue;
        };

        if let Some(payload) = obj.get("payload") {
            if obj.get("type").and_then(Value::as_str) != Some("response_item") {
                continue;
            }

            match payload {
                Value::Object(payload_item) => {
                    if is_allowed_trace_item(payload_item) {
                        normalized.push(Value::Object(payload_item.clone()));
                    }
                }
                Value::Array(payload_items) => {
                    for payload_item in payload_items {
                        if let Value::Object(payload_item) = payload_item
                            && is_allowed_trace_item(payload_item)
                        {
                            normalized.push(Value::Object(payload_item.clone()));
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        if is_allowed_trace_item(&obj) {
            normalized.push(Value::Object(obj));
        }
    }

    if normalized.is_empty() {
        return Err(CodexErr::InvalidRequest(format!(
            "no valid trace items after normalization: {}",
            path.display()
        )));
    }
    Ok(normalized)
}

fn is_allowed_trace_item(item: &Map<String, Value>) -> bool {
    let Some(item_type) = item.get("type").and_then(Value::as_str) else {
        return false;
    };

    if item_type == "message" {
        return matches!(
            item.get("role").and_then(Value::as_str),
            Some("assistant" | "system" | "developer" | "user")
        );
    }

    true
}

fn build_memory_id(index: usize, path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "memory".to_string());
    format!("memory_{index}_{stem}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn normalize_trace_items_handles_payload_wrapper_and_message_role_filtering() {
        let items = vec![
            serde_json::json!({
                "type": "response_item",
                "payload": {"type": "message", "role": "assistant", "content": []}
            }),
            serde_json::json!({
                "type": "response_item",
                "payload": [
                    {"type": "message", "role": "user", "content": []},
                    {"type": "message", "role": "tool", "content": []},
                    {"type": "function_call", "name": "shell", "arguments": "{}", "call_id": "c1"}
                ]
            }),
            serde_json::json!({
                "type": "not_response_item",
                "payload": {"type": "message", "role": "assistant", "content": []}
            }),
            serde_json::json!({
                "type": "message",
                "role": "developer",
                "content": []
            }),
        ];

        let normalized = normalize_trace_items(items, Path::new("trace.json")).expect("normalize");
        let expected = vec![
            serde_json::json!({"type": "message", "role": "assistant", "content": []}),
            serde_json::json!({"type": "message", "role": "user", "content": []}),
            serde_json::json!({"type": "function_call", "name": "shell", "arguments": "{}", "call_id": "c1"}),
            serde_json::json!({"type": "message", "role": "developer", "content": []}),
        ];
        assert_eq!(normalized, expected);
    }

    #[test]
    fn load_trace_items_supports_jsonl_arrays_and_objects() {
        let text = r#"
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[]}}
[{"type":"message","role":"user","content":[]},{"type":"message","role":"tool","content":[]}]
"#;
        let loaded = load_trace_items(Path::new("trace.jsonl"), text).expect("load");
        let expected = vec![
            serde_json::json!({"type":"message","role":"assistant","content":[]}),
            serde_json::json!({"type":"message","role":"user","content":[]}),
        ];
        assert_eq!(loaded, expected);
    }

    #[tokio::test]
    async fn load_trace_text_decodes_utf8_sig() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("trace.json");
        tokio::fs::write(
            &path,
            [
                0xEF, 0xBB, 0xBF, b'[', b'{', b'"', b't', b'y', b'p', b'e', b'"', b':', b'"', b'm',
                b'e', b's', b's', b'a', b'g', b'e', b'"', b',', b'"', b'r', b'o', b'l', b'e', b'"',
                b':', b'"', b'u', b's', b'e', b'r', b'"', b',', b'"', b'c', b'o', b'n', b't', b'e',
                b'n', b't', b'"', b':', b'[', b']', b'}', b']',
            ],
        )
        .await
        .expect("write");

        let text = load_trace_text(&path).await.expect("decode");
        assert!(text.starts_with('['));
    }
}
