//! Parsing and export helpers for external-agent session histories.

mod append;
mod export;
pub(crate) mod ledger;
pub(crate) mod records_cla;
mod records_common;
pub(crate) mod records_cur;
mod title;

use codex_protocol::ThreadId;
use codex_rollout::RolloutItem;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub use crate::detect::sessions::ImportedSessionConnectorAttribution;
pub use crate::detect::sessions::detect_imported_cla_session_connectors;
pub use crate::detect::sessions::detect_imported_cla_session_connectors_by_source_path;
pub use crate::detect::sessions::detect_recent_cla_sessions;
pub use crate::detect::sessions::detect_recent_cur_sessions;
pub use append::ExistingSessionAppend;
pub use append::append_existing_session;
use export::load_session_for_import_with_content_sha256;
pub use ledger::CompletedExternalAgentSessionImport;
pub use ledger::ImportedConnectorCandidate;
pub use ledger::append_imported_session_connector_names;
pub use ledger::has_current_session_been_imported;
pub use ledger::read_imported_connector_candidates;
pub use ledger::record_completed_session_imports;
pub use ledger::record_detected_session_connectors;
pub use records_cla::summarize_session;

const SESSION_TITLE_MAX_LEN: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SessionRecordFormat {
    Cla,
    Cur,
}

pub struct SessionSummary {
    pub latest_timestamp: i64,
    pub migration: ExternalAgentSessionMigration,
}

struct ParsedSessionImport {
    cwd: Option<PathBuf>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    messages: Vec<ConversationMessage>,
    content_sha256: String,
    attributed_mcp_server_ids: BTreeSet<String>,
}

pub(crate) fn normalized_connector_display_name(name: Option<&str>) -> Option<String> {
    name.map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

/// Selects whether session records must carry their own project metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMetadataMode {
    /// Read the project path only from the session records.
    Embedded,
    /// Use the detected migration path when the session records omit a project path.
    MigrationFallback,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionImportTarget {
    New,
    Existing {
        thread_id: ThreadId,
        expected_source_content_sha256: String,
    },
}

#[derive(Debug, Clone)]
pub struct PendingSessionImport {
    pub source_path: PathBuf,
    pub source_content_sha256: String,
    pub target: SessionImportTarget,
    pub attributed_mcp_server_ids: BTreeSet<String>,
    pub session: ImportedExternalAgentSession,
}

pub fn prepare_validated_session_import(
    codex_home: &Path,
    session: ExternalAgentSessionMigration,
) -> io::Result<Option<PendingSessionImport>> {
    prepare_validated_session_import_with_metadata_mode(
        codex_home,
        session,
        SessionMetadataMode::Embedded,
    )
}

pub fn prepare_validated_session_import_with_metadata_mode(
    codex_home: &Path,
    session: ExternalAgentSessionMigration,
    metadata_mode: SessionMetadataMode,
) -> io::Result<Option<PendingSessionImport>> {
    let Some(mut pending) = load_importable_session(&session.path, &session.cwd, metadata_mode)?
    else {
        return Ok(None);
    };
    pending.target = match ledger::find_existing_session_import(codex_home, &pending.source_path)? {
        ledger::SessionImportSourceMapping::None => SessionImportTarget::New,
        ledger::SessionImportSourceMapping::Unique {
            source_content_sha256,
            imported_thread_id,
        } => {
            if source_content_sha256 == pending.source_content_sha256 {
                return Ok(None);
            }
            SessionImportTarget::Existing {
                thread_id: imported_thread_id,
                expected_source_content_sha256: source_content_sha256,
            }
        }
        ledger::SessionImportSourceMapping::Ambiguous => return Ok(None),
    };
    Ok(Some(pending))
}

fn load_importable_session(
    path: &Path,
    fallback_cwd: &Path,
    metadata_mode: SessionMetadataMode,
) -> io::Result<Option<PendingSessionImport>> {
    let source_path = std::fs::canonicalize(path)?;
    let (record_format, fallback_cwd) = match metadata_mode {
        SessionMetadataMode::Embedded => (SessionRecordFormat::Cla, None),
        SessionMetadataMode::MigrationFallback => (SessionRecordFormat::Cur, Some(fallback_cwd)),
    };
    let Some((imported_session, source_content_sha256, attributed_mcp_server_ids)) =
        load_session_for_import_with_content_sha256(&source_path, record_format, fallback_cwd)?
    else {
        return Ok(None);
    };
    Ok(imported_session
        .cwd
        .is_dir()
        .then_some(PendingSessionImport {
            source_path,
            source_content_sha256,
            target: SessionImportTarget::New,
            attributed_mcp_server_ids,
            session: imported_session,
        }))
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

pub(crate) fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use sha2::Digest;
    use sha2::Sha256;
    use tempfile::TempDir;

    #[test]
    fn skips_session_that_was_already_imported() {
        let root = TempDir::new().expect("tempdir");
        let codex_home = root.path().join("codex-home");
        let source_path = root.path().join("session.jsonl");
        std::fs::write(&source_path, session_record(root.path(), "first request"))
            .expect("session");
        ledger::record_imported_session(&codex_home, &source_path, ThreadId::new())
            .expect("record import");

        let pending =
            prepare_validated_session_import(&codex_home, session_migration(&source_path))
                .expect("already imported session should be skipped");

        assert!(pending.is_none());
    }

    #[test]
    fn prepares_changed_session_for_its_unique_import() {
        let root = TempDir::new().expect("tempdir");
        let codex_home = root.path().join("codex-home");
        let source_path = root.path().join("session.jsonl");
        let initial = session_record(root.path(), "first request");
        let expected_source_content_sha256 = format!("{:x}", Sha256::digest(&initial));
        std::fs::write(&source_path, initial).expect("initial session");
        let imported_thread_id = ThreadId::new();
        ledger::record_imported_session(&codex_home, &source_path, imported_thread_id)
            .expect("record import");
        std::fs::write(&source_path, session_record(root.path(), "changed request"))
            .expect("changed session");

        let pending =
            prepare_validated_session_import(&codex_home, session_migration(&source_path))
                .expect("prepare changed session")
                .expect("changed session should be eligible");

        assert_eq!(
            pending.target,
            SessionImportTarget::Existing {
                thread_id: imported_thread_id,
                expected_source_content_sha256,
            }
        );
    }

    #[test]
    fn skips_changed_session_with_ambiguous_imports() {
        let root = TempDir::new().expect("tempdir");
        let codex_home = root.path().join("codex-home");
        let source_path = root.path().join("session.jsonl");
        std::fs::write(&source_path, session_record(root.path(), "changed request"))
            .expect("session");
        let source_path = std::fs::canonicalize(source_path).expect("canonical source");
        let import = |source_content_sha256: &str| ledger::CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_content_sha256: source_content_sha256.to_string(),
            imported_thread_id: ThreadId::new(),
            connector_names: Vec::new(),
            title: None,
        };
        ledger::record_completed_session_imports(
            &codex_home,
            vec![import("first hash"), import("second hash")],
        )
        .expect("record ambiguous imports");

        let pending =
            prepare_validated_session_import(&codex_home, session_migration(&source_path))
                .expect("prepare ambiguous session");

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
        assert_eq!(pending.target, SessionImportTarget::New);
    }

    #[test]
    fn migration_fallback_metadata_is_opt_in() {
        let root = TempDir::new().expect("tempdir");
        let source_path = root.path().join("session.jsonl");
        std::fs::write(
            &source_path,
            serde_json::json!({
                "type": "message",
                "role": "user",
                "timestamp_ms": 1_782_817_200_000_i64,
                "message": {"content": "first request"},
            })
            .to_string(),
        )
        .expect("session");
        let migration = session_migration(&source_path);

        assert!(
            prepare_validated_session_import_with_metadata_mode(
                root.path(),
                migration.clone(),
                SessionMetadataMode::Embedded,
            )
            .expect("embedded metadata mode")
            .is_none()
        );
        let pending = prepare_validated_session_import_with_metadata_mode(
            root.path(),
            migration,
            SessionMetadataMode::MigrationFallback,
        )
        .expect("fallback metadata mode")
        .expect("pending import");

        assert_eq!(pending.session.cwd, root.path());
        assert_eq!(
            pending.session.first_user_message.as_deref(),
            Some("first request")
        );
        assert!(!pending.session.rollout_items.is_empty());
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

    fn session_record(cwd: &Path, text: &str) -> String {
        serde_json::json!({
            "type": "user",
            "cwd": cwd,
            "timestamp": "2026-06-03T12:00:00Z",
            "message": { "content": text },
        })
        .to_string()
    }
}
