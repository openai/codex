use super::ConversationMessage;
use super::ExternalAgentSessionMigration;
use super::MessageRole;
use super::ParsedSessionImport;
use super::SessionSummary;
use super::records_common::ExtractedMessage;
use super::records_common::extract_message_text;
use super::records_common::parse_timestamp;
use super::title::IMPORTED_SESSION_FALLBACK_TITLE;
use super::title::SessionTitleCandidates;
use super::title::fallback_title_from_user_message;
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

pub fn summarize_session(path: &Path) -> io::Result<Option<SessionSummary>> {
    let file = File::open(path)?;
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
        let Some(message) = conversation_message_from_owned_record(&mut record) else {
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

pub(super) fn read_session_import(path: &Path) -> io::Result<ParsedSessionImport> {
    let file = File::open(path)?;
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
        if let Some(message) = conversation_message_from_owned_record(&mut record) {
            messages.push(message);
        }
    }
    Ok(ParsedSessionImport {
        cwd,
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

fn conversation_message_from_owned_record(record: &mut JsonValue) -> Option<ConversationMessage> {
    let record_type = record
        .get("type")
        .and_then(JsonValue::as_str)
        .filter(|record_type| matches!(*record_type, "assistant" | "user"))?;
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
        });
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

#[cfg(test)]
#[path = "records_cla_tests.rs"]
mod tests;
