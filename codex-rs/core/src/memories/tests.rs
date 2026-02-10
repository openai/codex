use super::MEMORY_SCOPE_KIND_CWD;
use super::PHASE_ONE_MAX_ROLLOUT_AGE_DAYS;
use super::StageOneResponseItemKinds;
use super::StageOneRolloutFilter;
use super::ensure_layout;
use super::memory_root_for_cwd;
use super::memory_scope_key_for_cwd;
use super::memory_summary_file;
use super::parse_stage_one_output;
use super::prune_to_recent_memories_and_rebuild_summary;
use super::raw_memories_dir;
use super::select_rollout_candidates_from_db;
use super::serialize_filtered_rollout_response_items;
use super::wipe_consolidation_outputs;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::RolloutItem;
use codex_state::ThreadMemory;
use codex_state::ThreadMetadata;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::tempdir;

fn thread_metadata(
    thread_id: ThreadId,
    path: PathBuf,
    cwd: PathBuf,
    title: &str,
    updated_at_secs: i64,
) -> ThreadMetadata {
    let updated_at = Utc
        .timestamp_opt(updated_at_secs, 0)
        .single()
        .expect("timestamp");
    ThreadMetadata {
        id: thread_id,
        rollout_path: path,
        created_at: updated_at,
        updated_at,
        source: "cli".to_string(),
        model_provider: "openai".to_string(),
        cwd,
        cli_version: "test".to_string(),
        title: title.to_string(),
        sandbox_policy: "read_only".to_string(),
        approval_mode: "on_request".to_string(),
        tokens_used: 0,
        first_user_message: None,
        archived_at: None,
        git_branch: None,
        git_sha: None,
        git_origin_url: None,
    }
}

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
fn memory_scope_key_uses_normalized_cwd() {
    let dir = tempdir().expect("tempdir");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("mkdir workspace");
    std::fs::create_dir_all(workspace.join("nested")).expect("mkdir nested");

    let alias = workspace.join("nested").join("..");
    let normalized = workspace
        .canonicalize()
        .expect("canonical workspace path should resolve");
    let alias_key = memory_scope_key_for_cwd(&alias);
    let normalized_key = memory_scope_key_for_cwd(&normalized);
    assert_eq!(alias_key, normalized_key);
}

#[test]
fn parse_stage_one_output_accepts_fenced_json() {
    let raw = "```json\n{\"rawMemory\":\"abc\",\"summary\":\"short\"}\n```";
    let parsed = parse_stage_one_output(raw).expect("parsed");
    assert!(parsed.raw_memory.contains("abc"));
    assert_eq!(parsed.summary, "short");
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

#[test]
fn select_rollout_candidates_filters_by_age_window() {
    let dir = tempdir().expect("tempdir");
    let cwd_a = dir.path().join("workspace-a");
    let cwd_b = dir.path().join("workspace-b");
    std::fs::create_dir_all(&cwd_a).expect("mkdir cwd a");
    std::fs::create_dir_all(&cwd_b).expect("mkdir cwd b");

    let now = Utc::now().timestamp();
    let current_thread_id = ThreadId::default();
    let recent_thread_id = ThreadId::default();
    let old_thread_id = ThreadId::default();
    let recent_two_thread_id = ThreadId::default();

    let current = thread_metadata(
        current_thread_id,
        dir.path().join("current.jsonl"),
        cwd_a.clone(),
        "current",
        now,
    );
    let recent = thread_metadata(
        recent_thread_id,
        dir.path().join("recent.jsonl"),
        cwd_a,
        "recent",
        now - 10,
    );
    let old = thread_metadata(
        old_thread_id,
        dir.path().join("old.jsonl"),
        cwd_b.clone(),
        "old",
        now - (PHASE_ONE_MAX_ROLLOUT_AGE_DAYS + 1) * 24 * 60 * 60,
    );
    let recent_two = thread_metadata(
        recent_two_thread_id,
        dir.path().join("recent-two.jsonl"),
        cwd_b,
        "recent-two",
        now - 20,
    );

    let candidates = select_rollout_candidates_from_db(
        &[current, recent, old, recent_two],
        current_thread_id,
        5,
        PHASE_ONE_MAX_ROLLOUT_AGE_DAYS,
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].thread_id, recent_thread_id);
    assert_eq!(candidates[1].thread_id, recent_two_thread_id);
}

#[tokio::test]
async fn prune_and_rebuild_summary_keeps_latest_memories_only() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().join("memory");
    ensure_layout(&root).await.expect("ensure layout");

    let keep_id = ThreadId::default().to_string();
    let drop_id = ThreadId::default().to_string();
    let keep_path = raw_memories_dir(&root).join(format!("{keep_id}.md"));
    let drop_path = raw_memories_dir(&root).join(format!("{drop_id}.md"));
    tokio::fs::write(&keep_path, "keep")
        .await
        .expect("write keep");
    tokio::fs::write(&drop_path, "drop")
        .await
        .expect("write drop");

    let memories = vec![ThreadMemory {
        thread_id: ThreadId::try_from(keep_id.clone()).expect("thread id"),
        scope_kind: MEMORY_SCOPE_KIND_CWD.to_string(),
        scope_key: "scope".to_string(),
        raw_memory: "raw memory".to_string(),
        memory_summary: "short summary".to_string(),
        updated_at: Utc.timestamp_opt(100, 0).single().expect("timestamp"),
        last_used_at: None,
        used_count: 0,
        invalidated_at: None,
        invalid_reason: None,
    }];

    prune_to_recent_memories_and_rebuild_summary(&root, &memories)
        .await
        .expect("prune and rebuild");

    assert!(keep_path.is_file());
    assert!(!drop_path.exists());

    let summary = tokio::fs::read_to_string(memory_summary_file(&root))
        .await
        .expect("read summary");
    assert!(summary.contains("short summary"));
    assert!(summary.contains(&keep_id));
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
