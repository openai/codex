use crate::ConversationMessage;
use crate::ExternalAgentSessionMigration;
use crate::MessageRole;
use crate::summarize_for_label;
use crate::truncate;
use serde_json::Value as JsonValue;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

const NOTE_MAX_LEN: usize = 2_000;
const TOOL_RESULT_MAX_LEN: usize = 4_000;
const EXTERNAL_AGENT_TOOL_CALL_TAG: &str = "external_agent_tool_call";
const EXTERNAL_AGENT_TOOL_RESULT_TAG: &str = "external_agent_tool_result";

pub struct SessionSummary {
    pub latest_timestamp: i64,
    pub migration: ExternalAgentSessionMigration,
}

pub fn summarize_session(path: &Path) -> io::Result<Option<SessionSummary>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut cwd = None;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut title = None;
    let mut latest_timestamp = None;
    let mut saw_message = false;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<JsonValue>(trimmed) else {
            continue;
        };
        if cwd.is_none() {
            cwd = record
                .get("cwd")
                .and_then(JsonValue::as_str)
                .map(PathBuf::from);
        }
        if let Some(title) = custom_title_from_record(&record) {
            custom_title = Some(title.to_string());
        }
        if let Some(title) = ai_title_from_record(&record) {
            ai_title = Some(title.to_string());
        }
        let Some(message) = conversation_message_from_record(&record) else {
            continue;
        };
        saw_message = true;
        if title.is_none() && message.role == MessageRole::User {
            title = Some(summarize_for_label(&message.text));
        }
        if let Some(timestamp) = message.timestamp {
            latest_timestamp =
                Some(latest_timestamp.map_or(timestamp, |current: i64| current.max(timestamp)));
        }
    }

    let Some(cwd) = cwd else {
        return Ok(None);
    };
    if !saw_message {
        return Ok(None);
    }
    let Some(latest_timestamp) = latest_timestamp else {
        return Ok(None);
    };
    Ok(Some(SessionSummary {
        latest_timestamp,
        migration: ExternalAgentSessionMigration {
            path: path.to_path_buf(),
            cwd,
            title: custom_title.or(ai_title).or(title),
        },
    }))
}

pub(super) fn source_title_from_records(records: &[JsonValue]) -> Option<String> {
    latest_title_from_records(records, custom_title_from_record)
        .or_else(|| latest_title_from_records(records, ai_title_from_record))
}

pub(super) fn read_records(path: &Path) -> io::Result<Vec<JsonValue>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<JsonValue>(trimmed) else {
            continue;
        };
        if value.is_object() {
            records.push(value);
        }
    }
    Ok(records)
}

pub(super) fn project_root_from_records(records: &[JsonValue]) -> Option<PathBuf> {
    records
        .iter()
        .find_map(|record| record.get("cwd").and_then(JsonValue::as_str))
        .map(PathBuf::from)
}

pub(super) fn conversation_messages(records: &[JsonValue]) -> Vec<ConversationMessage> {
    records
        .iter()
        .filter_map(conversation_message_from_record)
        .collect()
}

fn latest_title_from_records<'a>(
    records: &'a [JsonValue],
    title_from_record: impl Fn(&'a JsonValue) -> Option<&'a str>,
) -> Option<String> {
    records
        .iter()
        .filter_map(title_from_record)
        .next_back()
        .map(ToOwned::to_owned)
}

fn custom_title_from_record(record: &JsonValue) -> Option<&str> {
    title_from_record(record, "custom-title", "customTitle")
}

fn ai_title_from_record(record: &JsonValue) -> Option<&str> {
    title_from_record(record, "ai-title", "aiTitle")
}

fn title_from_record<'a>(record: &'a JsonValue, record_type: &str, field: &str) -> Option<&'a str> {
    (record.get("type").and_then(JsonValue::as_str) == Some(record_type))
        .then(|| record.get(field).and_then(JsonValue::as_str))
        .flatten()
        .map(str::trim)
        .filter(|title| !title.is_empty())
}

fn conversation_message_from_record(record: &JsonValue) -> Option<ConversationMessage> {
    let record_type = record.get("type")?.as_str()?;
    if record_type != "assistant" && record_type != "user" {
        return None;
    }
    if record.get("isMeta").and_then(JsonValue::as_bool) == Some(true)
        || record.get("isSidechain").and_then(JsonValue::as_bool) == Some(true)
    {
        return None;
    }

    let extracted = extract_message_text(record.get("message")?.get("content")?)?;
    let role = if record_type == "assistant" || extracted.only_tool_result {
        MessageRole::Assistant
    } else {
        MessageRole::User
    };
    let timestamp = record
        .get("timestamp")
        .and_then(JsonValue::as_str)
        .and_then(parse_timestamp);
    Some(ConversationMessage {
        role,
        text: extracted.text,
        timestamp,
    })
}

struct ExtractedMessage {
    text: String,
    only_tool_result: bool,
}

fn extract_message_text(content: &JsonValue) -> Option<ExtractedMessage> {
    let blocks = content_blocks(content);
    let mut parts = Vec::new();
    let mut only_tool_result = !blocks.is_empty();

    for block in &blocks {
        let block_type = block.get("type").and_then(JsonValue::as_str);
        match block_type {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(JsonValue::as_str)
                    && !text.is_empty()
                {
                    parts.push(text.to_string());
                    only_tool_result = false;
                }
            }
            Some("tool_use") => {
                parts.push(tool_call_note(block));
                only_tool_result = false;
            }
            Some("tool_result") => {
                parts.push(tool_result_note(block));
            }
            Some("thinking") => {}
            Some(other) => {
                parts.push(format!("[external unsupported block: {other}]"));
                only_tool_result = false;
            }
            None => {}
        }
    }

    let text = parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() {
        None
    } else {
        Some(ExtractedMessage {
            text,
            only_tool_result,
        })
    }
}

fn content_blocks(content: &JsonValue) -> Vec<JsonValue> {
    if let Some(text) = content.as_str() {
        return vec![serde_json::json!({
            "type": "text",
            "text": text,
        })];
    }
    content
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn tool_call_note(block: &JsonValue) -> String {
    let name = block
        .get("name")
        .and_then(JsonValue::as_str)
        .unwrap_or("unknown");
    let mut lines = vec![format!("[{EXTERNAL_AGENT_TOOL_CALL_TAG}: {name}]")];
    if let Some(input) = block.get("input").and_then(JsonValue::as_object) {
        if let Some(description) = input.get("description").and_then(JsonValue::as_str) {
            lines.push(format!("description: {description}"));
        }
        if let Some(command) = input.get("command").and_then(JsonValue::as_str) {
            lines.push(format!("command: {command}"));
        }
        if let Some(file) = input
            .get("file_path")
            .or_else(|| input.get("file"))
            .and_then(JsonValue::as_str)
        {
            lines.push(format!("file: {file}"));
        }
        if lines.len() == 1 {
            lines.push(format!(
                "input: {}",
                truncate(&JsonValue::Object(input.clone()).to_string(), NOTE_MAX_LEN)
            ));
        }
    } else if let Some(input) = block.get("input") {
        lines.push(format!(
            "input: {}",
            truncate(&input.to_string(), NOTE_MAX_LEN)
        ));
    }
    lines.push(format!("[/{EXTERNAL_AGENT_TOOL_CALL_TAG}]"));
    lines.join("\n")
}

fn tool_result_note(block: &JsonValue) -> String {
    let label = if block.get("is_error").and_then(JsonValue::as_bool) == Some(true) {
        format!("[{EXTERNAL_AGENT_TOOL_RESULT_TAG}: error]")
    } else {
        format!("[{EXTERNAL_AGENT_TOOL_RESULT_TAG}]")
    };
    let text = tool_result_text(block.get("content"));
    if text.is_empty() {
        format!("{label}\n[/{EXTERNAL_AGENT_TOOL_RESULT_TAG}]")
    } else {
        format!(
            "{label}\n{}\n[/{EXTERNAL_AGENT_TOOL_RESULT_TAG}]",
            truncate(&text, TOOL_RESULT_MAX_LEN)
        )
    }
}

fn tool_result_text(content: Option<&JsonValue>) -> String {
    match content {
        Some(JsonValue::String(text)) => text.clone(),
        Some(JsonValue::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(JsonValue::as_str))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn parse_timestamp(timestamp: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_tool_use_blocks_to_bounded_external_agent_tags() {
        let block = serde_json::json!({
            "type": "tool_use",
            "name": "Bash",
            "input": {
                "description": "Check repo status",
                "command": "git status --short"
            }
        });

        assert_eq!(
            tool_call_note(&block),
            "[external_agent_tool_call: Bash]\n\
             description: Check repo status\n\
             command: git status --short\n\
             [/external_agent_tool_call]"
        );
    }

    #[test]
    fn converts_tool_result_blocks_to_bounded_external_agent_tags() {
        let block = serde_json::json!({
            "type": "tool_result",
            "content": "codex-rs/external-agent-sessions/src/records.rs"
        });

        assert_eq!(
            tool_result_note(&block),
            "[external_agent_tool_result]\n\
             codex-rs/external-agent-sessions/src/records.rs\n\
             [/external_agent_tool_result]"
        );
    }

    #[test]
    fn converts_error_tool_result_blocks_to_bounded_external_agent_tags() {
        let block = serde_json::json!({
            "type": "tool_result",
            "is_error": true,
            "content": "command failed"
        });

        assert_eq!(
            tool_result_note(&block),
            "[external_agent_tool_result: error]\n\
             command failed\n\
             [/external_agent_tool_result]"
        );
    }
}
