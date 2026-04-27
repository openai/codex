use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const SESSION_IMPORT_MAX_COUNT: usize = 50;
const SESSION_IMPORT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const SESSION_TITLE_MAX_LEN: usize = 120;
const NOTE_MAX_LEN: usize = 2_000;
const TOOL_RESULT_MAX_LEN: usize = 4_000;
const SESSION_IMPORT_LEDGER_FILE: &str = "external_agent_session_imports.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAgentSessionMigration {
    pub path: PathBuf,
    pub cwd: PathBuf,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportedExternalAgentSession {
    pub cwd: PathBuf,
    pub title: Option<String>,
    pub rollout_items: Vec<RolloutItem>,
}

#[derive(Debug)]
struct SessionCandidate {
    latest_timestamp: i64,
    migration: ExternalAgentSessionMigration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ImportedExternalAgentSessionLedger {
    records: Vec<ImportedExternalAgentSessionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ImportedExternalAgentSessionRecord {
    source_path: PathBuf,
    content_sha256: String,
    imported_thread_id: ThreadId,
    imported_at: i64,
}

#[derive(Debug, Clone)]
struct ConversationMessage {
    role: MessageRole,
    text: String,
    timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageRole {
    Assistant,
    User,
}

pub(crate) fn detect_recent_sessions(
    external_agent_home: &Path,
    codex_home: &Path,
) -> io::Result<Vec<ExternalAgentSessionMigration>> {
    let projects_root = external_agent_home.join("projects");
    if !projects_root.is_dir() {
        return Ok(Vec::new());
    }

    let now = now_unix_seconds();
    let ledger = load_import_ledger(codex_home)?;
    let mut candidates = Vec::new();
    for project_entry in fs::read_dir(projects_root)? {
        let Ok(project_entry) = project_entry else {
            continue;
        };
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(project_path) else {
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(Some(summary)) = summarize_session(&path) else {
                continue;
            };
            if ledger.contains_current_source(&path)? {
                continue;
            }
            if !is_recent_enough(now, summary.latest_timestamp) {
                continue;
            }
            let migration = summary.migration;
            if !migration.cwd.is_dir() {
                continue;
            }
            candidates.push(SessionCandidate {
                latest_timestamp: summary.latest_timestamp,
                migration,
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .latest_timestamp
            .cmp(&left.latest_timestamp)
            .then_with(|| left.migration.path.cmp(&right.migration.path))
    });
    candidates.truncate(SESSION_IMPORT_MAX_COUNT);
    Ok(candidates
        .into_iter()
        .map(|candidate| candidate.migration)
        .collect())
}

pub(crate) fn has_current_session_been_imported(
    codex_home: &Path,
    source_path: &Path,
) -> io::Result<bool> {
    load_import_ledger(codex_home)?.contains_current_source(source_path)
}

pub(crate) fn record_imported_session(
    codex_home: &Path,
    source_path: &Path,
    imported_thread_id: ThreadId,
) -> io::Result<()> {
    let mut ledger = load_import_ledger(codex_home)?;
    let source_path = canonical_source_path(source_path)?;
    let content_sha256 = session_content_sha256(&source_path)?;
    if ledger
        .records
        .iter()
        .any(|record| record.source_path == source_path && record.content_sha256 == content_sha256)
    {
        return Ok(());
    }
    ledger.records.push(ImportedExternalAgentSessionRecord {
        source_path,
        content_sha256,
        imported_thread_id,
        imported_at: now_unix_seconds(),
    });
    save_import_ledger(codex_home, &ledger)
}

pub(crate) fn load_session_for_import(
    path: &Path,
) -> io::Result<Option<ImportedExternalAgentSession>> {
    let records = read_records(path)?;
    let Some(cwd) = project_root_from_records(&records) else {
        return Ok(None);
    };
    let messages = conversation_messages(&records);
    let rollout_items = rollout_items_from_messages(&messages);
    if rollout_items.is_empty() {
        return Ok(None);
    }
    let title = custom_title_from_records(&records).or_else(|| {
        messages
            .iter()
            .find(|message| message.role == MessageRole::User)
            .map(|message| summarize_for_label(&message.text))
    });
    Ok(Some(ImportedExternalAgentSession {
        cwd,
        title,
        rollout_items,
    }))
}

struct SessionSummary {
    latest_timestamp: i64,
    migration: ExternalAgentSessionMigration,
}

fn summarize_session(path: &Path) -> io::Result<Option<SessionSummary>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut cwd = None;
    let mut custom_title = None;
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
            title: custom_title.or(title),
        },
    }))
}

fn custom_title_from_records(records: &[JsonValue]) -> Option<String> {
    records
        .iter()
        .filter_map(custom_title_from_record)
        .next_back()
        .map(ToOwned::to_owned)
}

fn custom_title_from_record(record: &JsonValue) -> Option<&str> {
    (record.get("type").and_then(JsonValue::as_str) == Some("custom-title"))
        .then(|| record.get("customTitle").and_then(JsonValue::as_str))
        .flatten()
        .map(str::trim)
        .filter(|title| !title.is_empty())
}

fn read_records(path: &Path) -> io::Result<Vec<JsonValue>> {
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

fn project_root_from_records(records: &[JsonValue]) -> Option<PathBuf> {
    records
        .iter()
        .find_map(|record| record.get("cwd").and_then(JsonValue::as_str))
        .map(PathBuf::from)
}

fn conversation_messages(records: &[JsonValue]) -> Vec<ConversationMessage> {
    records
        .iter()
        .filter_map(conversation_message_from_record)
        .collect()
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
    let mut lines = vec![format!("[external tool call: {name}]")];
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
    lines.join("\n")
}

fn tool_result_note(block: &JsonValue) -> String {
    let label = if block.get("is_error").and_then(JsonValue::as_bool) == Some(true) {
        "[external tool result: error]"
    } else {
        "[external tool result]"
    };
    let text = tool_result_text(block.get("content"));
    if text.is_empty() {
        label.to_string()
    } else {
        format!("{label}\n{}", truncate(&text, TOOL_RESULT_MAX_LEN))
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

fn rollout_items_from_messages(messages: &[ConversationMessage]) -> Vec<RolloutItem> {
    let mut items = Vec::new();
    let mut current_turn: Option<(String, Option<String>)> = None;
    let mut user_turn_count = 0usize;

    for message in messages {
        match message.role {
            MessageRole::User => {
                if let Some((turn_id, last_agent_message)) = current_turn.take() {
                    items.push(turn_complete_item(turn_id, last_agent_message, None));
                }
                user_turn_count += 1;
                let turn_id = format!("external-import-turn-{user_turn_count}");
                items.push(RolloutItem::EventMsg(EventMsg::TurnStarted(
                    TurnStartedEvent {
                        turn_id: turn_id.clone(),
                        started_at: message.timestamp,
                        model_context_window: None,
                        collaboration_mode_kind: Default::default(),
                    },
                )));
                items.push(response_item(message));
                items.push(RolloutItem::EventMsg(EventMsg::UserMessage(
                    UserMessageEvent {
                        message: message.text.clone(),
                        images: None,
                        local_images: Vec::new(),
                        text_elements: Vec::new(),
                    },
                )));
                current_turn = Some((turn_id, None));
            }
            MessageRole::Assistant => {
                let Some((_, last_agent_message)) = current_turn.as_mut() else {
                    continue;
                };
                items.push(response_item(message));
                items.push(RolloutItem::EventMsg(EventMsg::AgentMessage(
                    AgentMessageEvent {
                        message: message.text.clone(),
                        phase: None,
                        memory_citation: None,
                    },
                )));
                *last_agent_message = Some(message.text.clone());
            }
        }
    }

    if let Some((turn_id, last_agent_message)) = current_turn {
        let completed_at = messages.last().and_then(|message| message.timestamp);
        items.push(turn_complete_item(
            turn_id,
            last_agent_message,
            completed_at,
        ));
    }

    items
}

fn response_item(message: &ConversationMessage) -> RolloutItem {
    let content = match message.role {
        MessageRole::Assistant => ContentItem::OutputText {
            text: message.text.clone(),
        },
        MessageRole::User => ContentItem::InputText {
            text: message.text.clone(),
        },
    };
    RolloutItem::ResponseItem(ResponseItem::Message {
        id: None,
        role: match message.role {
            MessageRole::Assistant => "assistant".to_string(),
            MessageRole::User => "user".to_string(),
        },
        content: vec![content],
        phase: None,
    })
}

fn turn_complete_item(
    turn_id: String,
    last_agent_message: Option<String>,
    completed_at: Option<i64>,
) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id,
        last_agent_message,
        completed_at,
        duration_ms: None,
        time_to_first_token_ms: None,
    }))
}

fn summarize_for_label(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or_default().trim();
    truncate(first_line, SESSION_TITLE_MAX_LEN)
}

fn truncate(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let prefix = text
        .chars()
        .take(max_len.saturating_sub(3))
        .collect::<String>();
    format!("{prefix}...")
}

fn parse_timestamp(timestamp: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.timestamp())
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn is_recent_enough(now: i64, latest_timestamp: i64) -> bool {
    latest_timestamp >= now.saturating_sub(SESSION_IMPORT_MAX_AGE.as_secs() as i64)
}

impl ImportedExternalAgentSessionLedger {
    fn contains_current_source(&self, source_path: &Path) -> io::Result<bool> {
        let source_path = canonical_source_path(source_path)?;
        let content_sha256 = session_content_sha256(&source_path)?;
        Ok(self.records.iter().any(|record| {
            record.source_path == source_path && record.content_sha256 == content_sha256
        }))
    }
}

fn load_import_ledger(codex_home: &Path) -> io::Result<ImportedExternalAgentSessionLedger> {
    let path = import_ledger_path(codex_home);
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(ImportedExternalAgentSessionLedger::default());
        }
        Err(err) => return Err(err),
    };
    serde_json::from_str(&raw).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid external agent session import ledger: {err}"),
        )
    })
}

fn save_import_ledger(
    codex_home: &Path,
    ledger: &ImportedExternalAgentSessionLedger,
) -> io::Result<()> {
    fs::create_dir_all(codex_home)?;
    let path = import_ledger_path(codex_home);
    let raw = serde_json::to_vec_pretty(ledger).map_err(io::Error::other)?;
    fs::write(path, raw)
}

fn import_ledger_path(codex_home: &Path) -> PathBuf {
    codex_home.join(SESSION_IMPORT_LEDGER_FILE)
}

fn canonical_source_path(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path)
}

fn session_content_sha256(path: &Path) -> io::Result<String> {
    let contents = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(contents)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::build_turns_from_rollout_items;
    use tempfile::TempDir;

    #[test]
    fn detects_recent_sessions_with_existing_roots() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let projects_dir = external_agent_home.join("projects").join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&projects_dir).expect("projects dir");
        let session_path = projects_dir.join("session.jsonl");
        fs::write(
            &session_path,
            jsonl(&[
                record("user", "hello there", project_root.as_path()),
                record("assistant", "ack", project_root.as_path()),
            ]),
        )
        .expect("session");

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("hello there".to_string()),
            }]
        );
    }

    #[test]
    fn prefers_latest_custom_title_over_first_user_message() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let projects_dir = external_agent_home.join("projects").join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&projects_dir).expect("projects dir");
        let session_path = projects_dir.join("session.jsonl");
        fs::write(
            &session_path,
            jsonl(&[
                record("user", "hello there", project_root.as_path()),
                custom_title_record("first title"),
                custom_title_record("final title"),
            ]),
        )
        .expect("session");

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("final title".to_string()),
            }]
        );
    }

    #[test]
    fn ignores_old_sessions() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let projects_dir = external_agent_home.join("projects").join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&projects_dir).expect("projects dir");
        let session_path = projects_dir.join("session.jsonl");
        fs::write(
            &session_path,
            jsonl(&[record_at(
                "user",
                "hello",
                &project_root,
                "2020-01-01T00:00:00Z",
            )]),
        )
        .expect("session");

        assert!(
            detect_recent_sessions(&external_agent_home, root.path())
                .expect("detect")
                .is_empty()
        );
    }

    #[test]
    fn skips_already_imported_current_session_versions() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let projects_dir = external_agent_home.join("projects").join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&projects_dir).expect("projects dir");
        let session_path = projects_dir.join("session.jsonl");
        fs::write(
            &session_path,
            jsonl(&[record("user", "hello there", project_root.as_path())]),
        )
        .expect("session");

        record_imported_session(root.path(), &session_path, ThreadId::new())
            .expect("record import");

        assert!(
            detect_recent_sessions(&external_agent_home, root.path())
                .expect("detect")
                .is_empty()
        );
    }

    #[test]
    fn redetects_sessions_when_source_contents_change_after_import() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let projects_dir = external_agent_home.join("projects").join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&projects_dir).expect("projects dir");
        let session_path = projects_dir.join("session.jsonl");
        fs::write(
            &session_path,
            jsonl(&[record("user", "hello there", project_root.as_path())]),
        )
        .expect("session");
        record_imported_session(root.path(), &session_path, ThreadId::new())
            .expect("record import");

        fs::write(
            &session_path,
            jsonl(&[
                record("user", "hello there", project_root.as_path()),
                record("assistant", "new reply", project_root.as_path()),
            ]),
        )
        .expect("update session");

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");
        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("hello there".to_string()),
            }]
        );
    }

    #[test]
    fn builds_visible_turns_for_imported_history() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                record("assistant", "first answer", &project_root),
                record("user", "second request", &project_root),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");
        let turns = build_turns_from_rollout_items(&imported.rollout_items);

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].items.len(), 2);
        assert_eq!(turns[1].items.len(), 1);
    }

    #[test]
    fn loads_custom_title_for_imported_session() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                custom_title_record("named by source app"),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");

        assert_eq!(imported.title.as_deref(), Some("named by source app"));
    }

    fn record(role: &str, text: &str, cwd: &Path) -> JsonValue {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        record_at(role, text, cwd, &timestamp)
    }

    fn record_at(role: &str, text: &str, cwd: &Path, timestamp: &str) -> JsonValue {
        serde_json::json!({
            "type": role,
            "cwd": cwd,
            "timestamp": timestamp,
            "message": { "content": text }
        })
    }

    fn custom_title_record(title: &str) -> JsonValue {
        serde_json::json!({
            "type": "custom-title",
            "customTitle": title,
        })
    }

    fn jsonl(records: &[JsonValue]) -> String {
        records
            .iter()
            .map(JsonValue::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
