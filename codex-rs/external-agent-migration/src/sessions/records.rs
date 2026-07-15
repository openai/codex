use super::ConversationMessage;
use super::ExternalAgentSessionMigration;
use super::MessageRole;
use super::title::IMPORTED_SESSION_FALLBACK_TITLE;
use super::title::SessionTitleCandidates;
use super::title::fallback_title_from_user_message;
use super::truncate;
use serde_json::Value as JsonValue;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeSet;
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

pub(super) struct ParsedSessionImport {
    pub cwd: Option<PathBuf>,
    pub custom_title: Option<String>,
    pub ai_title: Option<String>,
    pub messages: Vec<ConversationMessage>,
    pub content_sha256: String,
    pub attributed_mcp_server_ids: BTreeSet<String>,
}

pub fn summarize_session(path: &Path) -> io::Result<Option<SessionSummary>> {
    summarize_session_with_cwd(path, /*fallback_cwd*/ None)
}

pub(crate) fn summarize_session_with_cwd(
    path: &Path,
    fallback_cwd: Option<&Path>,
) -> io::Result<Option<SessionSummary>> {
    let file = File::open(path)?;
    let fallback_timestamp = fallback_cwd.and_then(|_| file_modified_at_seconds(&file));
    let reader = BufReader::new(file);
    let mut cwd = None;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut fallback_title = None;
    let mut saw_user_message = false;
    let mut latest_timestamp = None;
    let mut saw_message = false;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(mut record) = serde_json::from_str::<JsonValue>(trimmed) else {
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
        let Some(message) = conversation_message_from_owned_record(&mut record, fallback_timestamp)
        else {
            continue;
        };
        saw_message = true;
        if message.role == MessageRole::User {
            saw_user_message = true;
            if fallback_title.is_none() {
                fallback_title = fallback_title_from_user_message(&message.text);
            }
        }
        if let Some(timestamp) = message.timestamp {
            latest_timestamp =
                Some(latest_timestamp.map_or(timestamp, |current: i64| current.max(timestamp)));
        }
    }

    let Some(cwd) = cwd.or_else(|| fallback_cwd.map(Path::to_path_buf)) else {
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
            title: SessionTitleCandidates {
                custom_title,
                ai_title,
                fallback_title: fallback_title.or_else(|| {
                    saw_user_message.then(|| IMPORTED_SESSION_FALLBACK_TITLE.to_string())
                }),
            }
            .select(),
        },
    }))
}

pub(super) fn read_session_import_with_cwd(
    path: &Path,
    fallback_cwd: Option<&Path>,
) -> io::Result<ParsedSessionImport> {
    let file = File::open(path)?;
    let fallback_timestamp = fallback_cwd.and_then(|_| file_modified_at_seconds(&file));
    let mut reader = BufReader::new(file);
    let mut cwd = None;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut messages = Vec::new();
    let mut attributed_mcp_server_ids = BTreeSet::new();
    let mut line = String::new();
    let mut hasher = Sha256::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        hasher.update(line.as_bytes());
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(mut record) = serde_json::from_str::<JsonValue>(trimmed) else {
            continue;
        };
        if let Some(server_id) = record
            .get("attributionMcpServer")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|server_id| !server_id.is_empty())
        {
            attributed_mcp_server_ids.insert(server_id.to_string());
        }
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
        if let Some(message) =
            conversation_message_from_owned_record(&mut record, fallback_timestamp)
        {
            messages.push(message);
        }
    }
    Ok(ParsedSessionImport {
        cwd: cwd.or_else(|| fallback_cwd.map(Path::to_path_buf)),
        custom_title,
        ai_title,
        messages,
        content_sha256: format!("{:x}", hasher.finalize()),
        attributed_mcp_server_ids,
    })
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

fn conversation_message_from_owned_record(
    record: &mut JsonValue,
    fallback_timestamp: Option<i64>,
) -> Option<ConversationMessage> {
    let record_type = record
        .get("type")
        .and_then(JsonValue::as_str)
        .filter(|record_type| matches!(*record_type, "assistant" | "user"))
        .or_else(|| record.get("role").and_then(JsonValue::as_str))?;
    if !matches!(record_type, "assistant" | "user") {
        return None;
    }
    if record.get("isMeta").and_then(JsonValue::as_bool) == Some(true)
        || record.get("isSidechain").and_then(JsonValue::as_bool) == Some(true)
    {
        return None;
    }

    let is_assistant = record_type == "assistant";
    let timestamp = record
        .get("timestamp")
        .and_then(JsonValue::as_str)
        .and_then(parse_timestamp)
        .or_else(|| {
            record
                .get("timestamp_ms")
                .and_then(JsonValue::as_i64)
                .map(|value| value / 1_000)
        })
        .or(fallback_timestamp);
    let content = record.get_mut("message")?.get_mut("content")?.take();
    let extracted = match content {
        JsonValue::String(text) => {
            if text.trim().is_empty() {
                return None;
            }
            ExtractedMessage {
                text,
                only_tool_result: false,
            }
        }
        content => extract_message_text(&content)?,
    };
    let role = if is_assistant || extracted.only_tool_result {
        MessageRole::Assistant
    } else {
        MessageRole::User
    };
    let text = if role == MessageRole::User {
        unwrap_user_query(extracted.text)
    } else {
        extracted.text
    };
    Some(ConversationMessage {
        role,
        text,
        timestamp,
    })
}

fn unwrap_user_query(text: String) -> String {
    let trimmed = text.trim();
    let Some(inner) = trimmed
        .strip_prefix("<user_query>")
        .and_then(|inner| inner.strip_suffix("</user_query>"))
        .map(str::trim)
        .filter(|inner| !inner.is_empty())
    else {
        return text;
    };
    inner.to_string()
}

fn file_modified_at_seconds(file: &File) -> Option<i64> {
    file.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
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
    use tempfile::TempDir;

    #[test]
    fn reads_session_import_in_one_pass() {
        let root = TempDir::new().expect("tempdir");
        let path = root.path().join("session.jsonl");
        let contents = [
            serde_json::json!({
                "type": "user",
                "cwd": root.path(),
                "timestamp": "2026-06-03T12:00:00Z",
                "message": { "content": "<user_query>\nfirst request\n</user_query>" },
            })
            .to_string(),
            "not json".to_string(),
            serde_json::json!({
                "type": "ai-title",
                "aiTitle": "generated title",
            })
            .to_string(),
            serde_json::json!({
                "type": "custom-title",
                "customTitle": "custom title",
            })
            .to_string(),
        ]
        .join("\n");
        std::fs::write(&path, &contents).expect("session");

        let parsed =
            read_session_import_with_cwd(&path, /*fallback_cwd*/ None).expect("parse session");

        assert_eq!(parsed.cwd.as_deref(), Some(root.path()));
        assert_eq!(parsed.custom_title.as_deref(), Some("custom title"));
        assert_eq!(parsed.ai_title.as_deref(), Some("generated title"));
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].text, "first request");
        assert_eq!(
            parsed.content_sha256,
            format!("{:x}", Sha256::digest(contents))
        );
    }

    #[test]
    fn embedded_cwd_overrides_migration_fallback() {
        let root = TempDir::new().expect("tempdir");
        let embedded_cwd = root.path().join("embedded");
        let fallback_cwd = root.path().join("fallback");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            serde_json::json!({
                "cwd": embedded_cwd,
                "role": "user",
                "message": {"content": "first request"},
            })
            .to_string(),
        )
        .expect("session");

        let parsed =
            read_session_import_with_cwd(&path, Some(&fallback_cwd)).expect("parse session");
        let summary = summarize_session_with_cwd(&path, Some(&fallback_cwd))
            .expect("summarize session")
            .expect("session summary");

        assert_eq!(parsed.cwd.as_deref(), Some(embedded_cwd.as_path()));
        assert_eq!(summary.migration.cwd, embedded_cwd);
    }

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
            "content": "codex-rs/external-agent-migration/src/sessions/records.rs"
        });

        assert_eq!(
            tool_result_note(&block),
            "[external_agent_tool_result]\n\
             codex-rs/external-agent-migration/src/sessions/records.rs\n\
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
