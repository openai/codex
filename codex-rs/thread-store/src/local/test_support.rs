use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use uuid::Uuid;

use super::LocalThreadStoreConfig;

pub(super) fn test_config(codex_home: &Path) -> LocalThreadStoreConfig {
    LocalThreadStoreConfig {
        codex_home: codex_home.to_path_buf(),
        sqlite_home: codex_home.to_path_buf(),
        default_model_provider_id: "test-provider".to_string(),
    }
}

pub(super) fn write_session_file(root: &Path, ts: &str, uuid: Uuid) -> std::io::Result<PathBuf> {
    write_session_file_with_history_mode(root, ts, uuid, ThreadHistoryMode::Legacy)
}

pub(super) fn write_session_file_with_history_mode(
    root: &Path,
    ts: &str,
    uuid: Uuid,
    history_mode: ThreadHistoryMode,
) -> std::io::Result<PathBuf> {
    write_session_file_with(
        root,
        root.join("sessions/2025/01/03"),
        ts,
        uuid,
        "Hello from user",
        Some("test-provider"),
        history_mode,
    )
}

pub(super) fn write_archived_session_file(
    root: &Path,
    ts: &str,
    uuid: Uuid,
) -> std::io::Result<PathBuf> {
    write_session_file_with(
        root,
        root.join(ARCHIVED_SESSIONS_SUBDIR),
        ts,
        uuid,
        "Archived user message",
        Some("test-provider"),
        ThreadHistoryMode::Legacy,
    )
}

pub(super) fn write_session_file_with(
    root: &Path,
    day_dir: PathBuf,
    ts: &str,
    uuid: Uuid,
    first_user_message: &str,
    model_provider: Option<&str>,
    history_mode: ThreadHistoryMode,
) -> std::io::Result<PathBuf> {
    write_session_file_with_fork(
        root,
        day_dir,
        ts,
        uuid,
        first_user_message,
        model_provider,
        /*forked_from_id*/ None,
        history_mode,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_session_file_with_fork(
    root: &Path,
    day_dir: PathBuf,
    ts: &str,
    uuid: Uuid,
    first_user_message: &str,
    model_provider: Option<&str>,
    forked_from_id: Option<Uuid>,
    history_mode: ThreadHistoryMode,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(&day_dir)?;
    let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
    let mut file = fs::File::create(&path)?;
    let mut meta = serde_json::json!({
        "timestamp": ts,
        "type": "session_meta",
        "payload": {
            "session_id": uuid,
            "id": uuid,
            "forked_from_id": forked_from_id,
            "timestamp": ts,
            "cwd": root,
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "cli",
            "model_provider": model_provider,
            "history_mode": history_mode,
            "git": {
                "commit_hash": "abcdef",
                "branch": "main",
                "repository_url": "https://example.com/repo.git"
            }
        },
    });
    if matches!(history_mode, ThreadHistoryMode::Paginated) {
        meta["ordinal"] = serde_json::json!(0);
    }
    writeln!(file, "{meta}")?;
    if matches!(history_mode, ThreadHistoryMode::Legacy) {
        let user_event = serde_json::json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": first_user_message,
                "kind": "plain",
            },
        });
        writeln!(file, "{user_event}")?;
    }
    Ok(path)
}
