//! Parsing and export helpers for external-agent session histories.

mod detect;
mod export;
mod ledger;
mod records;
mod title;

use codex_protocol::protocol::RolloutItem;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub use detect::detect_recent_sessions;
use export::load_session_for_import_with_content_sha256;
pub use ledger::CompletedExternalAgentSessionImport;
pub use ledger::has_current_session_been_imported;
pub use ledger::record_completed_session_imports;
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
    pub first_user_message: Option<String>,
    pub rollout_items: Vec<RolloutItem>,
}

#[derive(Debug, Clone)]
pub struct PendingSessionImport {
    pub source_path: PathBuf,
    pub source_content_sha256: String,
    pub session: ImportedExternalAgentSession,
}

pub fn prepare_validated_session_import(
    codex_home: &Path,
    session: ExternalAgentSessionMigration,
) -> io::Result<Option<PendingSessionImport>> {
    let has_been_imported = has_current_session_been_imported(codex_home, &session.path)?;
    if has_been_imported {
        return Ok(None);
    }
    let Some((source_path, imported_session, source_content_sha256)) =
        load_importable_session(&session.path)?
    else {
        return Ok(None);
    };
    Ok(Some(PendingSessionImport {
        source_path,
        source_content_sha256,
        session: imported_session,
    }))
}

fn load_importable_session(
    path: &Path,
) -> io::Result<Option<(PathBuf, ImportedExternalAgentSession, String)>> {
    let source_path = std::fs::canonicalize(path)?;
    let Some((imported_session, source_content_sha256)) =
        load_session_for_import_with_content_sha256(&source_path)?
    else {
        return Ok(None);
    };
    Ok(imported_session.cwd.is_dir().then_some((
        source_path,
        imported_session,
        source_content_sha256,
    )))
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
    use sha2::Digest;
    use sha2::Sha256;
    use tempfile::TempDir;

    #[test]
    fn skips_session_that_was_already_imported() {
        let root = TempDir::new().expect("tempdir");
        let codex_home = root.path().join("codex-home");
        let source_path = root.path().join("session.jsonl");
        std::fs::write(&source_path, "{}\n").expect("session");
        ledger::record_imported_session(&codex_home, &source_path, ThreadId::new())
            .expect("record import");

        let pending =
            prepare_validated_session_import(&codex_home, session_migration(&source_path))
                .expect("already imported session should be skipped");

        assert!(pending.is_none());
    }

    #[test]
    fn reports_session_preparation_errors() {
        let root = TempDir::new().expect("tempdir");
        let source_path = root.path().join("missing-session.jsonl");

        let err = prepare_validated_session_import(root.path(), session_migration(&source_path))
            .expect_err("missing session should fail preparation");

        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn prepares_one_validated_session_import_with_content_hash() {
        let root = TempDir::new().expect("tempdir");
        let source_path = root.path().join("session.jsonl");
        let contents = serde_json::json!({
            "type": "user",
            "cwd": root.path(),
            "timestamp": "2026-06-03T12:00:00Z",
            "message": { "content": "first request" },
        })
        .to_string();
        std::fs::write(&source_path, &contents).expect("session");

        let pending =
            prepare_validated_session_import(root.path(), session_migration(&source_path))
                .expect("prepare session")
                .expect("pending import");

        assert_eq!(
            pending.source_content_sha256,
            format!("{:x}", Sha256::digest(contents))
        );
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
