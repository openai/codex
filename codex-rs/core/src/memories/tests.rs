use super::rollout::StageOneResponseItemKinds;
use super::rollout::StageOneRolloutFilter;
use super::rollout::serialize_filtered_rollout_response_items;
use super::stage_one::parse_stage_one_output;
use super::storage::rebuild_raw_memories_file_from_memories;
use super::storage::sync_rollout_summaries_from_memories;
use super::storage::wipe_consolidation_outputs;
use crate::memories::layout::ensure_layout;
use crate::memories::layout::memory_root_for_cwd;
use crate::memories::layout::raw_memories_file;
use crate::memories::layout::rollout_summaries_dir;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::RolloutItem;
use codex_state::Stage1Output;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn memory_root_varies_by_cwd() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().join("codex");
    let cwd_a = dir.path().join("workspace-a");
    let cwd_b = dir.path().join("workspace-b");

    std::fs::create_dir_all(&cwd_a).expect("mkdir a");
    std::fs::create_dir_all(&cwd_b).expect("mkdir b");

    let root_a = memory_root_for_cwd(&codex_home, &cwd_a);
    let root_b = memory_root_for_cwd(&codex_home, &cwd_b);
    assert!(root_a.starts_with(codex_home.join("memories")));
    assert!(root_b.starts_with(codex_home.join("memories")));
    assert!(root_a.ends_with("memory"));
    assert!(root_b.ends_with("memory"));
    assert_ne!(root_a, root_b);

    let bucket_a = root_a
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .expect("cwd bucket");
    assert_eq!(bucket_a.len(), 16);
    assert!(bucket_a.chars().all(|ch| ch.is_ascii_hexdigit()));
}

#[test]
fn memory_root_encoding_avoids_component_collisions() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().join("codex");

    let cwd_question = dir.path().join("workspace?one");
    let cwd_hash = dir.path().join("workspace#one");

    let root_question = memory_root_for_cwd(&codex_home, &cwd_question);
    let root_hash = memory_root_for_cwd(&codex_home, &cwd_hash);

    assert_ne!(root_question, root_hash);
    assert!(!root_question.display().to_string().contains("workspace"));
    assert!(!root_hash.display().to_string().contains("workspace"));
}

#[test]
fn parse_stage_one_output_accepts_fenced_json() {
    let raw = "```json\n{\"raw_memory\":\"abc\",\"rollout_summary\":\"short\",\"rollout_slug\":\"slug\"}\n```";
    let parsed = parse_stage_one_output(raw).expect("parsed");
    assert!(parsed.raw_memory.contains("abc"));
    assert_eq!(parsed.rollout_summary, "short");
    assert_eq!(parsed.rollout_slug, Some("slug".to_string()));
}

#[test]
fn parse_stage_one_output_accepts_legacy_keys() {
    let raw = r#"{"rawMemory":"abc","summary":"short"}"#;
    let parsed = parse_stage_one_output(raw).expect("parsed");
    assert!(parsed.raw_memory.contains("abc"));
    assert_eq!(parsed.rollout_summary, "short");
    assert_eq!(parsed.rollout_slug, None);
}

#[test]
fn serialize_filtered_rollout_response_items_keeps_response_and_compacted() {
    let input = vec![
        RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "user input".to_string(),
            }],
            end_turn: None,
            phase: None,
        }),
        RolloutItem::Compacted(CompactedItem {
            message: "compacted summary".to_string(),
            replacement_history: None,
        }),
    ];

    let serialized = serialize_filtered_rollout_response_items(
        &input,
        StageOneRolloutFilter::response_and_compacted_items(),
    )
    .expect("serialize");
    let parsed: Vec<ResponseItem> = serde_json::from_str(&serialized).expect("deserialize");

    assert_eq!(parsed.len(), 2);
    assert!(matches!(parsed[0], ResponseItem::Message { .. }));
    assert!(matches!(parsed[1], ResponseItem::Message { .. }));
}

#[test]
fn serialize_filtered_rollout_response_items_supports_response_only_filter() {
    let input = vec![
        RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "user input".to_string(),
            }],
            end_turn: None,
            phase: None,
        }),
        RolloutItem::Compacted(CompactedItem {
            message: "compacted summary".to_string(),
            replacement_history: None,
        }),
    ];

    let serialized = serialize_filtered_rollout_response_items(
        &input,
        StageOneRolloutFilter {
            keep_response_items: true,
            keep_compacted_items: false,
            response_item_kinds: StageOneResponseItemKinds::all(),
            max_items: None,
        },
    )
    .expect("serialize");
    let parsed: Vec<ResponseItem> = serde_json::from_str(&serialized).expect("deserialize");

    assert_eq!(parsed.len(), 1);
    assert!(matches!(parsed[0], ResponseItem::Message { .. }));
}

#[test]
fn serialize_filtered_rollout_response_items_filters_by_response_item_kind() {
    let input = vec![
        RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "user input".to_string(),
            }],
            end_turn: None,
            phase: None,
        }),
        RolloutItem::ResponseItem(ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            arguments: "{\"cmd\":\"pwd\"}".to_string(),
            call_id: "call-1".to_string(),
        }),
    ];

    let serialized = serialize_filtered_rollout_response_items(
        &input,
        StageOneRolloutFilter {
            keep_response_items: true,
            keep_compacted_items: false,
            response_item_kinds: StageOneResponseItemKinds::messages_only(),
            max_items: None,
        },
    )
    .expect("serialize");
    let parsed: Vec<ResponseItem> = serde_json::from_str(&serialized).expect("deserialize");

    assert_eq!(parsed.len(), 1);
    assert!(matches!(parsed[0], ResponseItem::Message { .. }));
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
        summary: "short summary".to_string(),
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
}

#[tokio::test]
async fn wipe_consolidation_outputs_removes_registry_skills_and_legacy_file() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().join("memory");
    ensure_layout(&root).await.expect("ensure layout");

    let memory_registry = root.join("MEMORY.md");
    let legacy_consolidated = root.join("consolidated.md");
    let skills_dir = root.join("skills").join("example");

    tokio::fs::create_dir_all(&skills_dir)
        .await
        .expect("create skills dir");
    tokio::fs::write(&memory_registry, "memory")
        .await
        .expect("write memory registry");
    tokio::fs::write(&legacy_consolidated, "legacy")
        .await
        .expect("write legacy consolidated");

    wipe_consolidation_outputs(&root)
        .await
        .expect("wipe consolidation outputs");

    assert!(!memory_registry.exists());
    assert!(!legacy_consolidated.exists());
    assert!(!root.join("skills").exists());
}
