#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::io::Write;
use std::path::PathBuf;

use codex_core::find_conversation_path_by_id_str;
use tempfile::TempDir;
use uuid::Uuid;

/// Create sessions/YYYY/MM/DD and write a minimal rollout file containing the
/// provided conversation id in the SessionMeta line. Returns the absolute path.
fn write_minimal_rollout_with_id(codex_home: &TempDir, id: Uuid) -> PathBuf {
    let sessions = codex_home.path().join("sessions/2024/01/01");
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
                "cli_version": "test"
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
    let expected = write_minimal_rollout_with_id(&home, id);

    let found = find_conversation_path_by_id_str(home.path(), &id.to_string())
        .await
        .unwrap();

    assert_eq!(found.unwrap(), expected);
}
