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

pub(crate) fn summarize_session(
    path: &Path,
    fallback_cwd: Option<&Path>,
) -> io::Result<Option<SessionSummary>> {
    let file = File::open(path)?;
    let fallback_timestamp = fallback_cwd.and_then(|_| file_modified_at_seconds(&file));
    let reader = BufReader::new(file);
    let mut cwd = None;
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
                custom_title: None,
                ai_title: None,
                fallback_title: fallback_title.or_else(|| {
                    saw_user_message.then(|| IMPORTED_SESSION_FALLBACK_TITLE.to_string())
                }),
            }
            .select(),
        },
    }))
}

pub(super) fn read_session_import(
    path: &Path,
    fallback_cwd: Option<&Path>,
) -> io::Result<ParsedSessionImport> {
    let file = File::open(path)?;
    let fallback_timestamp = fallback_cwd.and_then(|_| file_modified_at_seconds(&file));
    let mut reader = BufReader::new(file);
    let mut cwd = None;
    let mut messages = Vec::new();
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
        if cwd.is_none() {
            cwd = record
                .get("cwd")
                .and_then(JsonValue::as_str)
                .map(PathBuf::from);
        }
        if let Some(message) =
            conversation_message_from_owned_record(&mut record, fallback_timestamp)
        {
            messages.push(message);
        }
    }
    Ok(ParsedSessionImport {
        cwd: cwd.or_else(|| fallback_cwd.map(Path::to_path_buf)),
        custom_title: None,
        ai_title: None,
        messages,
        content_sha256: format!("{:x}", hasher.finalize()),
        attributed_mcp_server_ids: BTreeSet::new(),
    })
}

fn conversation_message_from_owned_record(
    record: &mut JsonValue,
    fallback_timestamp: Option<i64>,
) -> Option<ConversationMessage> {
    let record_type = record.get("role").and_then(JsonValue::as_str)?;
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
    const USER_QUERY_OPEN: &str = "<user_query>";
    const USER_QUERY_CLOSE: &str = "</user_query>";
    const LEADING_CONTEXT_WRAPPERS: [(&str, &str); 2] = [
        ("<timestamp>", "</timestamp>"),
        ("<cursor_commands>", "</cursor_commands>"),
    ];

    let Some(query_start) = trimmed.find(USER_QUERY_OPEN) else {
        return text;
    };
    let query_content_start = query_start + USER_QUERY_OPEN.len();
    let Some(query_end_offset) = trimmed[query_content_start..].find(USER_QUERY_CLOSE) else {
        return text;
    };
    let query_end = query_content_start + query_end_offset;
    if !trimmed[query_end + USER_QUERY_CLOSE.len()..]
        .trim()
        .is_empty()
    {
        return text;
    }

    let mut leading_context = trimmed[..query_start].trim();
    while !leading_context.is_empty() {
        let Some((opening, closing)) = LEADING_CONTEXT_WRAPPERS
            .iter()
            .find(|(opening, _)| leading_context.starts_with(*opening))
        else {
            return text;
        };
        let Some(wrapper_end_offset) = leading_context[opening.len()..].find(*closing) else {
            return text;
        };
        let wrapper_end = opening.len() + wrapper_end_offset + closing.len();
        leading_context = leading_context[wrapper_end..].trim();
    }

    let inner = trimmed[query_content_start..query_end].trim();
    if inner.is_empty() {
        text
    } else {
        inner.to_string()
    }
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

#[cfg(test)]
#[path = "records_cur_tests.rs"]
mod tests;
