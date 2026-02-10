use super::rollout::StageOneResponseItemKinds;
use super::rollout::StageOneRolloutFilter;
use super::rollout::serialize_filtered_rollout_response_items;
use super::stage_one::parse_stage_one_output;
use super::storage::rebuild_raw_memories_file_from_memories;
use super::storage::sync_rollout_summaries_from_memories;
use super::storage::wipe_consolidation_outputs;
use crate::memories::layout::ensure_layout;
use crate::memories::layout::memory_root;
use crate::memories::layout::migrate_legacy_user_memory_root_if_needed;
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
fn memory_root_uses_shared_global_path() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().join("codex");
    assert_eq!(memory_root(&codex_home), codex_home.join("memories"));
}

#[tokio::test]
async fn migrate_legacy_user_memory_root_if_needed_copies_contents() {
    let dir = tempdir().expect("tempdir");
    let codex_home = dir.path().join("codex");
    let legacy_root = codex_home.join("memories").join("user").join("memory");
    tokio::fs::create_dir_all(legacy_root.join("rollout_summaries"))
        .await
        .expect("create legacy rollout summaries dir");
    tokio::fs::write(
        legacy_root.join("rollout_summaries").join("thread.md"),
        "summary",
    )
    .await
    .expect("write legacy rollout summary");
    tokio::fs::write(legacy_root.join("raw_memories.md"), "raw")
        .await
        .expect("write legacy raw memories");

    migrate_legacy_user_memory_root_if_needed(&codex_home)
        .await
        .expect("migrate legacy memory root");

    let root = memory_root(&codex_home);
    assert!(root.join("rollout_summaries").join("thread.md").is_file());
    assert!(root.join("raw_memories.md").is_file());
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
