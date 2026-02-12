use super::storage::rebuild_raw_memories_file_from_memories;
use super::storage::sync_rollout_summaries_from_memories;
use crate::memories::ensure_layout;
use crate::memories::memory_root;
use crate::memories::raw_memories_file;
use crate::memories::rollout_summaries_dir;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_state::Stage1Output;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn memory_root_uses_shared_global_path() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().join("codex");
    assert_eq!(memory_root(&codex_home), codex_home.join("memories"));
}

#[test]
fn stage_one_output_schema_requires_all_declared_properties() {
    let schema = crate::memories::phase1::output_schema();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties object");
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("required array");

    let mut property_keys = properties.keys().map(String::as_str).collect::<Vec<_>>();
    property_keys.sort_unstable();

    let mut required_keys = required
        .iter()
        .map(|key| key.as_str().expect("required key string"))
        .collect::<Vec<_>>();
    required_keys.sort_unstable();

    assert_eq!(required_keys, property_keys);
}

#[tokio::test]
async fn sync_rollout_summaries_and_raw_memories_file_keeps_latest_memories_only() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().join("memory");
    ensure_layout(&root).await.expect("ensure layout");

    let keep_id = ThreadId::default().to_string();
    let drop_id = ThreadId::default().to_string();
    let keep_path = rollout_summaries_dir(&root).join(format!("{keep_id}.md"));
    let drop_path = rollout_summaries_dir(&root).join(format!("{drop_id}.md"));
    tokio::fs::write(&keep_path, "keep")
        .await
        .expect("write keep");
    tokio::fs::write(&drop_path, "drop")
        .await
        .expect("write drop");

    let memories = vec![Stage1Output {
        thread_id: ThreadId::try_from(keep_id.clone()).expect("thread id"),
        source_updated_at: Utc.timestamp_opt(100, 0).single().expect("timestamp"),
        raw_memory: "raw memory".to_string(),
        rollout_summary: "short summary".to_string(),
        cwd: PathBuf::from("/tmp/workspace"),
        generated_at: Utc.timestamp_opt(101, 0).single().expect("timestamp"),
    }];

    sync_rollout_summaries_from_memories(&root, &memories)
        .await
        .expect("sync rollout summaries");
    rebuild_raw_memories_file_from_memories(&root, &memories)
        .await
        .expect("rebuild raw memories");

    assert!(keep_path.is_file());
    assert!(!drop_path.exists());

    let raw_memories = tokio::fs::read_to_string(raw_memories_file(&root))
        .await
        .expect("read raw memories");
    assert!(raw_memories.contains("raw memory"));
    assert!(raw_memories.contains(&keep_id));
    assert!(raw_memories.contains("cwd: /tmp/workspace"));
}
