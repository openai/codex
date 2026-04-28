//! Parsing and export helpers for external-agent session histories.

mod detect;
mod export;
mod ledger;
mod records;

use codex_protocol::protocol::RolloutItem;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub use detect::detect_recent_sessions;
pub use export::load_session_for_import;
pub use ledger::has_current_session_been_imported;
pub use ledger::record_imported_session;
pub use records::SessionSummary;
pub use records::summarize_session;

const SESSION_TITLE_MAX_LEN: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentSessionMigration {
    pub path: PathBuf,
    pub cwd: PathBuf,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImportedExternalAgentSession {
    pub cwd: PathBuf,
    pub title: Option<String>,
    pub rollout_items: Vec<RolloutItem>,
}

#[derive(Debug, Clone)]
pub struct PendingSessionImport {
    pub source_path: PathBuf,
    pub session: ImportedExternalAgentSession,
}

#[derive(Debug)]
pub enum PrepareSessionImportsError {
    SessionNotDetected(PathBuf),
}

impl std::fmt::Display for PrepareSessionImportsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrepareSessionImportsError::SessionNotDetected(path) => {
                write!(
                    formatter,
                    "external agent session was not detected for import: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for PrepareSessionImportsError {}

pub fn prepare_pending_session_imports(
    codex_home: &Path,
    requested_sessions: Vec<ExternalAgentSessionMigration>,
    detected_sessions: Vec<ExternalAgentSessionMigration>,
) -> Result<Vec<PendingSessionImport>, PrepareSessionImportsError> {
    let detected_session_paths = detected_sessions
        .into_iter()
        .map(|session| session.path)
        .collect::<HashSet<_>>();
    let mut pending_session_imports = Vec::new();
    for session in requested_sessions {
        let has_been_imported = match has_current_session_been_imported(codex_home, &session.path) {
            Ok(has_been_imported) => has_been_imported,
            Err(_) => continue,
        };
        if !detected_session_paths.contains(&session.path) && !has_been_imported {
            return Err(PrepareSessionImportsError::SessionNotDetected(session.path));
        }
        if has_been_imported {
            continue;
        }
        let imported_session = match load_importable_session(&session.path) {
            Ok(Some(imported_session)) => imported_session,
            Ok(None) | Err(_) => continue,
        };
        pending_session_imports.push(PendingSessionImport {
            source_path: session.path,
            session: imported_session,
        });
    }
    Ok(pending_session_imports)
}

fn load_importable_session(path: &Path) -> io::Result<Option<ImportedExternalAgentSession>> {
    let Some(imported_session) = load_session_for_import(path)? else {
        return Ok(None);
    };
    Ok(imported_session.cwd.is_dir().then_some(imported_session))
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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ThreadId;
    use tempfile::TempDir;

    #[test]
    fn rejects_session_that_was_not_detected() {
        let root = TempDir::new().expect("tempdir");
        let codex_home = root.path().join("codex-home");
        let source_path = root.path().join("session.jsonl");
        std::fs::write(&source_path, "{}\n").expect("session");

        let err = prepare_pending_session_imports(
            &codex_home,
            vec![session_migration(&source_path)],
            Vec::new(),
        )
        .expect_err("undetected session should be rejected");

        match err {
            PrepareSessionImportsError::SessionNotDetected(path) => {
                assert_eq!(path, source_path);
            }
        }
    }

    #[test]
    fn skips_session_that_was_already_imported() {
        let root = TempDir::new().expect("tempdir");
        let codex_home = root.path().join("codex-home");
        let source_path = root.path().join("session.jsonl");
        std::fs::write(&source_path, "{}\n").expect("session");
        record_imported_session(&codex_home, &source_path, ThreadId::new()).expect("record import");

        let pending = prepare_pending_session_imports(
            &codex_home,
            vec![session_migration(&source_path)],
            Vec::new(),
        )
        .expect("already imported session should be skipped");

        assert!(pending.is_empty());
    }

    fn session_migration(path: &Path) -> ExternalAgentSessionMigration {
        ExternalAgentSessionMigration {
            path: path.to_path_buf(),
            cwd: path
                .parent()
                .expect("source path should have parent")
                .to_path_buf(),
            title: None,
        }
    }
}
