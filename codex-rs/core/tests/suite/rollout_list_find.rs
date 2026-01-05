#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use codex_core::find_conversation_path_by_id_str;
use codex_core::find_conversation_path_by_selector;
use tempfile::TempDir;
use uuid::Uuid;

/// Create sessions/YYYY/MM/DD and write a minimal rollout file containing the
/// provided conversation id in the SessionMeta line. Returns the absolute path.
fn write_minimal_rollout_with_id(codex_home: &Path, id: Uuid) -> PathBuf {
    let sessions = codex_home.join("sessions/2024/01/01");
    std::fs::create_dir_all(&sessions).unwrap();

    let file = sessions.join(format!("rollout-2024-01-01T00-00-00-{id}.jsonl"));
    let mut f = std::fs::File::create(&file).unwrap();
    // Minimal first line: session_meta with the id so content search can find it
    writeln!(
        f,
        "{}",
        serde_json::json!({
            "timestamp": "2024-01-01T00:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "id": id,
                "timestamp": "2024-01-01T00:00:00Z",
                "instructions": null,
                "cwd": ".",
                "originator": "test",
                "cli_version": "test",
                "model_provider": "test-provider"
            }
        })
    )
    .unwrap();

    file
}

fn write_minimal_rollout_with_id_and_title(
    codex_home: &Path,
    id: Uuid,
    timestamp_fragment: &str,
    title: &str,
) -> PathBuf {
    let sessions = codex_home.join("sessions/2024/01/01");
    std::fs::create_dir_all(&sessions).unwrap();

    let file = sessions.join(format!(
        "rollout-2024-01-01T{timestamp_fragment}-{id}.jsonl"
    ));
    let mut f = std::fs::File::create(&file).unwrap();
    writeln!(
        f,
        "{}",
        serde_json::json!({
            "timestamp": "2024-01-01T00:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "id": id,
                "timestamp": "2024-01-01T00:00:00Z",
                "cwd": ".",
                "title": title,
                "originator": "test",
                "cli_version": "test",
                "instructions": null,
                "model_provider": "test-provider"
            }
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

    let found = find_conversation_path_by_id_str(home.path(), &id.to_string())
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

    let found = find_conversation_path_by_id_str(&codex_home, &id.to_string())
        .await
        .unwrap();

    assert_eq!(found, Some(expected));
}

#[tokio::test]
async fn resolve_selector_finds_rollout_file_by_title() {
    let home = TempDir::new().unwrap();
    let id = Uuid::new_v4();
    let expected = write_minimal_rollout_with_id_and_title(home.path(), id, "00-00-00", "My Title");

    let resolved = find_conversation_path_by_selector(home.path(), "my title")
        .await
        .unwrap();

    assert_eq!(resolved, Some(expected));
}

#[tokio::test]
async fn resolve_selector_picks_newest_when_titles_duplicate() {
    let home = TempDir::new().unwrap();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let _older = write_minimal_rollout_with_id_and_title(home.path(), id1, "00-00-00", "Dup");
    let newer = write_minimal_rollout_with_id_and_title(home.path(), id2, "00-00-01", "Dup");

    let resolved = find_conversation_path_by_selector(home.path(), "dup")
        .await
        .unwrap();

    assert_eq!(resolved, Some(newer));
}

#[tokio::test]
async fn find_ignores_granular_gitignore_rules() {
    let home = TempDir::new().unwrap();
    let id = Uuid::new_v4();
    let expected = write_minimal_rollout_with_id(home.path(), id);
    std::fs::write(home.path().join("sessions/.gitignore"), "*.jsonl\n").unwrap();

    let found = find_conversation_path_by_id_str(home.path(), &id.to_string())
        .await
        .unwrap();

    assert_eq!(found, Some(expected));
}
