mod detect;
mod import;
mod ledger;
mod records;

use codex_protocol::protocol::RolloutItem;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub(crate) use detect::detect_recent_sessions;
pub(crate) use import::load_session_for_import;
pub(crate) use ledger::has_current_session_been_imported;
pub(crate) use ledger::record_imported_session;

const SESSION_TITLE_MAX_LEN: usize = 120;

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

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
