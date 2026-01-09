#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use codex_core::find_thread_path_by_id_str;
use codex_core::find_thread_path_by_name_str;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

/// Create sessions/YYYY/MM/DD and write a minimal rollout file containing the
/// provided conversation id in the SessionMeta line. Returns the absolute path.
///
/// This test file covers the low-level "find rollout by X" helpers, so the
/// minimal rollout writer lives here to keep the lookup tests concise.
fn write_minimal_rollout_with_id(codex_home: &Path, id: Uuid) -> PathBuf {
    write_minimal_rollout(codex_home, "2024/01/01", "2024-01-01T00-00-00", id, None)
}

// Helper for name lookup tests: lets us create older/newer rollouts without
// duplicating JSONL construction logic.
fn write_minimal_rollout(
    codex_home: &Path,
    subdir: &str,
    filename_ts: &str,
    id: Uuid,
    name: Option<&str>,
) -> PathBuf {
    let sessions = codex_home.join(format!("sessions/{subdir}"));
    std::fs::create_dir_all(&sessions).unwrap();

    let file = sessions.join(format!("rollout-{filename_ts}-{id}.jsonl"));
    let mut f = std::fs::File::create(&file).unwrap();
    // Minimal first line: session_meta with the id so content search can find it
    let mut payload = serde_json::json!({
        "id": id,
        "timestamp": "2024-01-01T00:00:00Z",
        "instructions": null,
        "cwd": ".",
        "originator": "test",
        "cli_version": "test",
        "model_provider": "test-provider"
    });
    if let Some(name) = name {
        payload["name"] = serde_json::Value::String(name.to_string());
    }
    writeln!(
        f,
        "{}",
        serde_json::json!({
            "timestamp": "2024-01-01T00:00:00.000Z",
            "type": "session_meta",
            "payload": payload
        })
    )
    .unwrap();

    file
}

#[tokio::test]
async fn find_locates_rollout_file_by_id() {
    let home = TempDir::new().unwrap();
    let id = Uuid::new_v4();
    let expected = write_minimal_rollout_with_id(home.path(), id);

    let found = find_thread_path_by_id_str(home.path(), &id.to_string())
        .await
        .unwrap();

    assert_eq!(found.unwrap(), expected);
}

#[tokio::test]
async fn find_handles_gitignore_covering_codex_home_directory() {
    let repo = TempDir::new().unwrap();
    let codex_home = repo.path().join(".codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(repo.path().join(".gitignore"), ".codex/**\n").unwrap();
    let id = Uuid::new_v4();
    let expected = write_minimal_rollout_with_id(&codex_home, id);

    let found = find_thread_path_by_id_str(&codex_home, &id.to_string())
        .await
        .unwrap();

    assert_eq!(found, Some(expected));
}

#[tokio::test]
async fn find_ignores_granular_gitignore_rules() {
    let home = TempDir::new().unwrap();
    let id = Uuid::new_v4();
    let expected = write_minimal_rollout_with_id(home.path(), id);
    std::fs::write(home.path().join("sessions/.gitignore"), "*.jsonl\n").unwrap();

    let found = find_thread_path_by_id_str(home.path(), &id.to_string())
        .await
        .unwrap();

    assert_eq!(found, Some(expected));
}

#[tokio::test]
async fn find_locates_rollout_file_by_name_latest_first() {
    // This test lives here because it verifies the core "find rollout by name"
    // helper, including newest-first ordering and filename timestamp parsing.
    let home = TempDir::new().unwrap();
    let name = "release-notes";
    let older = write_minimal_rollout(
        home.path(),
        "2024/01/01",
        "2024-01-01T00-00-00",
        Uuid::new_v4(),
        Some(name),
    );
    let newer = write_minimal_rollout(
        home.path(),
        "2024/01/02",
        "2024-01-02T00-00-00",
        Uuid::new_v4(),
        Some(name),
    );

    let found = find_thread_path_by_name_str(home.path(), name)
        .await
        .unwrap();

    assert_eq!(found, Some(newer));
    assert_ne!(found, Some(older));
}

#[tokio::test]
async fn find_returns_none_for_unknown_name() {
    let home = TempDir::new().unwrap();
    write_minimal_rollout(
        home.path(),
        "2024/01/01",
        "2024-01-01T00-00-00",
        Uuid::new_v4(),
        Some("known"),
    );

    let found = find_thread_path_by_name_str(home.path(), "missing")
        .await
        .unwrap();

    assert_eq!(found, None);
}
