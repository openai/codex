use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::RolloutConfig;
use uuid::Uuid;

pub(super) fn test_config(codex_home: &Path) -> RolloutConfig {
    RolloutConfig {
        codex_home: codex_home.to_path_buf(),
        sqlite_home: codex_home.to_path_buf(),
        cwd: codex_home.to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: true,
    }
}

pub(super) fn write_session_file(root: &Path, ts: &str, uuid: Uuid) -> std::io::Result<PathBuf> {
    write_session_file_with_fork(
        root,
        root.join("sessions/2025/01/03"),
        ts,
        uuid,
        "Hello from user",
        Some("test-provider"),
        /*forked_from_id*/ None,
    )
}

pub(super) fn write_archived_session_file(
    root: &Path,
    ts: &str,
    uuid: Uuid,
) -> std::io::Result<PathBuf> {
    write_session_file_with_fork(
        root,
        root.join(ARCHIVED_SESSIONS_SUBDIR),
        ts,
        uuid,
        "Archived user message",
        Some("test-provider"),
        /*forked_from_id*/ None,
    )
}

pub(super) fn write_session_file_with(
    root: &Path,
    day_dir: PathBuf,
    ts: &str,
    uuid: Uuid,
    first_user_message: &str,
    model_provider: Option<&str>,
) -> std::io::Result<PathBuf> {
    write_session_file_with_fork(
        root,
        day_dir,
        ts,
        uuid,
        first_user_message,
        model_provider,
        /*forked_from_id*/ None,
    )
}

pub(super) fn write_session_file_with_fork(
    root: &Path,
    day_dir: PathBuf,
    ts: &str,
    uuid: Uuid,
    first_user_message: &str,
    model_provider: Option<&str>,
    forked_from_id: Option<Uuid>,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(&day_dir)?;
    let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
    let mut file = fs::File::create(&path)?;
    let meta = serde_json::json!({
        "timestamp": ts,
        "type": "session_meta",
        "payload": {
            "id": uuid,
            "timestamp": ts,
            "cwd": root,
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "cli",
            "model_provider": model_provider,
            "forked_from_id": forked_from_id,
            "git": {
                "commit_hash": "abcdef",
                "branch": "main",
                "repository_url": "https://example.com/repo.git"
            }
        },
    });
    writeln!(file, "{meta}")?;
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
    Ok(path)
}
