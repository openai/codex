use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_extension_items::ExtensionItem;
use codex_extension_items::image_generation::ImageGenerationFailure;
use codex_extension_items::image_generation::ImageGenerationItem;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::mcp::McpResourceOrigin;
use codex_protocol::mcp::McpResourceOriginCheckpoint;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ImageGenerationEndEvent;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::CompactedItem;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use super::LocalThreadStore;
use super::RolloutMigrationFailureReason;
use super::RolloutMigrationMode;
use super::RolloutMigrationOptions;
use super::RolloutMigrationProgress;
use super::RolloutMigrationStatus;
#[cfg(unix)]
use super::decompress_rollout_to_path;
use super::migration_journal_path;
use super::thread_history;
use super::write_migration_journal;
use crate::ItemSortKey;
use crate::ListItemsParams;
use crate::ListThreadsParams;
use crate::ListTurnsParams;
use crate::LoadThreadHistoryParams;
use crate::SortDirection;
use crate::StoredTurnItemsView;
use crate::ThreadMetadataPatch;
use crate::ThreadSortKey;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::TurnPage;
use crate::UpdateThreadMetadataParams;
use crate::local::test_support::test_config;

const TIMESTAMP: &str = "2025-01-03T12:00:00Z";

fn write_rollout(
    home: &Path,
    thread_id: ThreadId,
    source: SessionSource,
    items: Vec<RolloutItem>,
) -> PathBuf {
    write_rollout_with_fork(home, thread_id, source, /*forked_from_id*/ None, items)
}

fn write_rollout_with_fork(
    home: &Path,
    thread_id: ThreadId,
    source: SessionSource,
    forked_from_id: Option<ThreadId>,
    items: Vec<RolloutItem>,
) -> PathBuf {
    let directory = home.join("sessions/2025/01/03");
    fs::create_dir_all(&directory).expect("create rollout directory");
    let path = directory.join(format!("rollout-2025-01-03T12-00-00-{thread_id}.jsonl"));
    let mut file = fs::File::create(&path).expect("create legacy rollout");
    let metadata = SessionMeta {
        session_id: thread_id.into(),
        id: thread_id,
        forked_from_id,
        timestamp: TIMESTAMP.to_string(),
        cwd: home.to_path_buf(),
        originator: "test-originator".to_string(),
        cli_version: "0.0.0".to_string(),
        source,
        model_provider: Some("test-provider".to_string()),
        ..SessionMeta::default()
    };
    let items = std::iter::once(RolloutItem::SessionMeta(SessionMetaLine {
        meta: metadata,
        git: None,
    }))
    .chain(items);
    for item in items {
        let line = RolloutLine {
            timestamp: TIMESTAMP.to_string(),
            ordinal: None,
            item,
        };
        writeln!(
            file,
            "{}",
            serde_json::to_string(&line).expect("serialize legacy record")
        )
        .expect("write legacy record");
    }
    path
}

fn move_to_archived(home: &Path, path: PathBuf) -> PathBuf {
    let directory = home.join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR);
    fs::create_dir_all(&directory).expect("create archived rollout directory");
    let archived_path = directory.join(path.file_name().expect("rollout filename"));
    fs::rename(path, &archived_path).expect("archive rollout");
    archived_path
}

fn compress_rollout(path: &Path) -> PathBuf {
    let compressed_path = path.with_extension("jsonl.zst");
    let compressed = zstd::stream::encode_all(
        fs::File::open(path).expect("open rollout"),
        /*level*/ 3,
    )
    .expect("compress rollout");
    fs::write(&compressed_path, compressed).expect("write compressed rollout");
    fs::remove_file(path).expect("remove plain rollout");
    compressed_path
}

fn user_message(text: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        message: text.to_string(),
        ..UserMessageEvent::default()
    }))
}

fn agent_message(text: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
        message: text.to_string(),
        phase: None,
        memory_citation: None,
        delivery: None,
    }))
}

fn input_response_message(role: &str, text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn rollout_response_item(item: ResponseItem) -> RolloutItem {
    RolloutItem::ResponseItem(item.into())
}

fn exec_completion(turn_id: &str, call_id: &str) -> RolloutItem {
    serde_json::from_value(json!({
        "type": "event_msg",
        "payload": {
            "type": "exec_command_end",
            "call_id": call_id,
            "turn_id": turn_id,
            "command": ["echo", "ok"],
            "cwd": "file:///tmp",
            "parsed_cmd": [],
            "source": "agent",
            "stdout": "ok",
            "stderr": "",
            "aggregated_output": "ok",
            "exit_code": 0,
            "duration": {"secs": 0, "nanos": 0},
            "formatted_output": "ok",
            "status": "completed"
        }
    }))
    .expect("build legacy exec completion")
}

fn item_completed(turn_id: &str, item_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::new(),
        turn_id: turn_id.to_string(),
        item: TurnItem::Reasoning(ReasoningItem {
            id: item_id.to_string(),
            summary_text: vec!["summary".to_string()],
            raw_content: Vec::new(),
        }),
        started_at_ms: None,
        completed_at_ms: 1_735_905_601_000,
    }))
}

fn started(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: Some(1_735_905_600),
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
}

fn compacted(replacement_history: Vec<ResponseItem>) -> RolloutItem {
    RolloutItem::Compacted(CompactedItem {
        message: "checkpoint".to_string(),
        replacement_history: Some(replacement_history.into_iter().map(Into::into).collect()),
        mcp_resource_origins: None,
        window_number: Some(1),
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
    })
}

fn completed(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: turn_id.to_string(),
        last_agent_message: None,
        error: None,
        started_at: Some(1_735_905_600),
        completed_at: Some(1_735_905_601),
        duration_ms: Some(1_000),
        time_to_first_token_ms: None,
    }))
}

fn read_rollout(path: &Path) -> Vec<RolloutLine> {
    fs::read_to_string(path)
        .expect("read migrated rollout")
        .lines()
        .map(|line| serde_json::from_str(line).expect("parse migrated rollout"))
        .collect()
}

fn apply_options() -> RolloutMigrationOptions {
    RolloutMigrationOptions {
        mode: RolloutMigrationMode::Apply,
        max_mib_per_second: Some(1024),
        ..RolloutMigrationOptions::default()
    }
}

fn assert_failed_with_reason(
    outcome: &super::RolloutMigrationOutcome,
    failure_reason: RolloutMigrationFailureReason,
) {
    assert_eq!(
        (outcome.status, outcome.failure_reason),
        (RolloutMigrationStatus::Failed, Some(failure_reason))
    );
}

async fn indexed_store(home: &Path) -> LocalThreadStore {
    let config = test_config(home);
    let rollout_config = RolloutConfig {
        codex_home: config.codex_home.clone(),
        sqlite: config.sqlite.clone(),
        cwd: home.to_path_buf(),
        model_provider_id: config.default_model_provider_id.clone(),
        generate_memories: false,
    };
    let state_db = codex_rollout::state_db::try_init(&rollout_config)
        .await
        .expect("backfill legacy thread metadata");
    LocalThreadStore::new(config, Some(state_db))
}

async fn list_active_summary_turns(store: &LocalThreadStore, thread_id: ThreadId) -> TurnPage {
    store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::Summary,
        })
        .await
        .expect("read projected turns")
}

#[tokio::test]
async fn migration_publishes_canonical_projected_history_and_is_idempotent() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("first question"),
            agent_message("first answer"),
        ],
    );
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy rollout");
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);

    let lines = read_rollout(&path);
    assert_eq!(
        lines.iter().map(|line| line.ordinal).collect::<Vec<_>>(),
        (0..lines.len() as u64).map(Some).collect::<Vec<_>>()
    );
    assert!(matches!(
        &lines[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.history_mode == ThreadHistoryMode::Paginated
                && metadata.meta.id == thread_id
                && metadata.meta.history_base.is_none()
    ));
    assert_eq!(
        lines
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::EventMsg(EventMsg::ItemCompleted(_))))
            .count(),
        2
    );

    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 2);

    let bytes = fs::read(&path).expect("read first migration");
    let second = store
        .migrate_rollouts(apply_options())
        .await
        .expect("rerun migration");
    assert_eq!(
        second.outcomes[0].status,
        RolloutMigrationStatus::AlreadyPaginated
    );
    assert_eq!(fs::read(&path).expect("read idempotent rollout"), bytes);
}

#[tokio::test]
async fn migration_projects_explicit_and_implicit_legacy_completed_items() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let exec = exec_completion("explicit", "call-1");
    let reasoning = serde_json::from_value(json!({
        "type": "event_msg",
        "payload": {"type": "agent_reasoning", "text": "summary"}
    }))
    .expect("build legacy reasoning summary");
    let raw_reasoning = serde_json::from_value(json!({
        "type": "event_msg",
        "payload": {"type": "agent_reasoning_raw_content", "text": "raw"}
    }))
    .expect("build legacy reasoning content");
    write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("explicit"),
            exec,
            completed("explicit"),
            reasoning,
            raw_reasoning,
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy completion events");

    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(turns.turns.len(), 2);
    assert_eq!(turns.turns[0].turn_id, "explicit");
    let items = store
        .list_items(ListItemsParams {
            thread_id,
            turn_id: None,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            sort_key: ItemSortKey::CreatedAtOrdinal,
            after_updated_at_ordinal: None,
        })
        .await
        .expect("read projected items");
    assert_eq!(items.items.len(), 2);
    assert_eq!(items.items[0].turn_id, "explicit");
    assert_eq!(items.items[1].turn_id, turns.turns[1].turn_id);
    let command: serde_json::Value =
        serde_json::from_slice(&items.items[0].item_json).expect("parse projected command");
    let reasoning: serde_json::Value =
        serde_json::from_slice(&items.items[1].item_json).expect("parse projected reasoning");
    assert_eq!(command["type"], "commandExecution");
    assert_eq!(reasoning["type"], "reasoning");
    assert_eq!(reasoning["summary"], json!(["summary"]));
    assert_eq!(reasoning["content"], json!(["raw"]));
}

#[tokio::test]
async fn migration_preserves_image_generation_failure_metadata() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let expected_item = ImageGenerationItem {
        id: "image-call".to_string(),
        status: "failed".to_string(),
        revised_prompt: Some("paint a blue whale".to_string()),
        result: String::new(),
        transparent_background: None,
        failure: Some(ImageGenerationFailure::UsageLimitExceeded {
            limit_id: "image_gen".to_string(),
            resets_at: Some(1_786_150_800),
        }),
        saved_path: None,
    };
    let image_completion =
        RolloutItem::EventMsg(EventMsg::ImageGenerationEnd(ImageGenerationEndEvent {
            call_id: expected_item.id.clone(),
            status: expected_item.status.clone(),
            revised_prompt: expected_item.revised_prompt.clone(),
            result: expected_item.result.clone(),
            transparent_background: expected_item.transparent_background,
            failure: expected_item.failure.clone(),
            saved_path: expected_item.saved_path.clone(),
        }));
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("image-turn"),
            image_completion,
            completed("image-turn"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy image completion");

    let migrated_item = read_rollout(&path)
        .into_iter()
        .find_map(|line| match line.item {
            RolloutItem::EventMsg(EventMsg::ItemCompleted(event)) => Some(event.item),
            _ => None,
        })
        .expect("migrated image completion");
    let TurnItem::Extension(ExtensionItem::ImageGeneration(migrated_item)) = migrated_item else {
        panic!("expected migrated extension image-generation item");
    };
    assert_eq!(migrated_item, expected_item);
}

#[tokio::test]
async fn migration_keeps_late_completions_in_their_original_turn() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("old"),
            user_message("old question"),
            started("current"),
            user_message("current question"),
            exec_completion("old", "call-old"),
            agent_message("current answer"),
            completed("current"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate late completion");

    let started_turn_ids = read_rollout(&path)
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => Some(event.turn_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(started_turn_ids, vec!["old", "current"]);

    let items = store
        .list_items(ListItemsParams {
            thread_id,
            turn_id: None,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            sort_key: ItemSortKey::CreatedAtOrdinal,
            after_updated_at_ordinal: None,
        })
        .await
        .expect("read projected late completion");
    let item_turn_ids = items
        .items
        .iter()
        .map(|item| item.turn_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(item_turn_ids, vec!["old", "current", "old", "current"]);
}

#[tokio::test]
async fn migration_hoists_delayed_session_meta_before_paginated_history() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(home.path(), thread_id, SessionSource::Cli, Vec::new());
    let existing = fs::read_to_string(&path).expect("read legacy rollout");
    let pre_header = RolloutLine {
        timestamp: TIMESTAMP.to_string(),
        ordinal: None,
        item: user_message("before metadata"),
    };
    fs::write(
        &path,
        format!(
            "{}\n{existing}",
            serde_json::to_string(&pre_header).expect("serialize pre-header record")
        ),
    )
    .expect("write pre-header rollout");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate delayed session metadata");

    let lines = read_rollout(&path);
    assert_eq!(lines[0].ordinal, Some(0));
    assert!(matches!(
        &lines[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.id == thread_id
                && metadata.meta.history_mode == ThreadHistoryMode::Paginated
    ));
    let turns = store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::NotLoaded,
        })
        .await
        .expect("read migrated turns");
    assert_eq!(turns.turns.len(), 1);

    let second = store
        .migrate_rollouts(apply_options())
        .await
        .expect("rerun migration");
    assert_eq!(
        second.outcomes[0].status,
        RolloutMigrationStatus::AlreadyPaginated
    );
}

#[tokio::test]
async fn migration_preserves_valid_final_record_without_newline() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("question")],
    );
    let final_line = RolloutLine {
        timestamp: TIMESTAMP.to_string(),
        ordinal: None,
        item: agent_message("answer"),
    };
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open legacy rollout")
        .write_all(
            serde_json::to_string(&final_line)
                .expect("serialize final record")
                .as_bytes(),
        )
        .expect("append final record");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy rollout");

    assert_eq!(
        read_rollout(&path)
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::EventMsg(EventMsg::ItemCompleted(_))))
            .count(),
        2
    );
}

#[tokio::test]
async fn migration_applies_historical_rollbacks_before_sqlite_projection() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("keep"),
            user_message("keep question"),
            agent_message("keep answer"),
            completed("keep"),
            started("remove"),
            user_message("remove question"),
            agent_message("remove answer"),
            completed("remove"),
            started("shell"),
            completed("shell"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            started("replacement"),
            user_message("replacement question"),
            agent_message("replacement answer"),
            completed("replacement"),
        ],
    );
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate rolled-back thread");
    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);

    let lines = read_rollout(&path);
    assert!(!lines.iter().any(|line| matches!(
        line.item,
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))
    )));
    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(
        turns
            .turns
            .iter()
            .map(|turn| turn.turn_id.as_str())
            .collect::<Vec<_>>(),
        vec!["keep", "replacement"]
    );
}

#[tokio::test]
async fn migration_rolls_back_response_and_inter_agent_user_boundaries() {
    let home = TempDir::new().expect("create Codex home");
    let response_thread_id = ThreadId::new();
    let response_path = write_rollout(
        home.path(),
        response_thread_id,
        SessionSource::Cli,
        vec![
            rollout_response_item(input_response_message("user", "remove response boundary")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let communication_thread_id = ThreadId::new();
    let communication_path = write_rollout(
        home.path(),
        communication_thread_id,
        SessionSource::Cli,
        vec![
            RolloutItem::InterAgentCommunication(InterAgentCommunication::new(
                AgentPath::root(),
                AgentPath::root().join("worker").expect("worker path"),
                Vec::new(),
                "remove communication boundary".to_string(),
                /*trigger_turn*/ true,
            )),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let contextual_thread_id = ThreadId::new();
    let contextual_path = write_rollout(
        home.path(),
        contextual_thread_id,
        SessionSource::Cli,
        vec![
            rollout_response_item(input_response_message("user", "keep first boundary")),
            rollout_response_item(input_response_message(
                "developer",
                "<managed_developer_instructions>context only</managed_developer_instructions>",
            )),
            rollout_response_item(input_response_message(
                "developer",
                "<permissions instructions>context only</permissions instructions>",
            )),
            rollout_response_item(input_response_message(
                "user",
                "<environment_context>context only</environment_context>",
            )),
            rollout_response_item(input_response_message("user", "remove real user boundary")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy rollback boundaries");

    assert!(
        !read_rollout(&response_path)
            .iter()
            .any(|line| matches!(line.item, RolloutItem::ResponseItem(_)))
    );
    assert!(
        !read_rollout(&communication_path)
            .iter()
            .any(|line| matches!(line.item, RolloutItem::InterAgentCommunication(_)))
    );
    assert_eq!(
        read_rollout(&contextual_path)
            .into_iter()
            .filter_map(|line| match line.item {
                RolloutItem::ResponseItem(response) => Some(response.into_item()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![input_response_message("user", "keep first boundary")]
    );
}

#[tokio::test]
async fn migration_drops_trailing_context_when_rollback_arrives_before_next_turn() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            rollout_response_item(input_response_message("user", "keep question")),
            rollout_response_item(input_response_message("user", "remove question")),
            rollout_response_item(input_response_message(
                "user",
                "<turn_aborted>remove this context too</turn_aborted>",
            )),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate trailing rollback context");

    assert_eq!(
        read_rollout(&path)
            .into_iter()
            .filter_map(|line| match line.item {
                RolloutItem::ResponseItem(response) => Some(response.into_item()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![input_response_message("user", "keep question")]
    );
}

#[tokio::test]
async fn migration_coalesces_response_first_user_message_rollback_boundary() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            rollout_response_item(input_response_message("user", "remove question")),
            user_message("remove question"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate response-first rollback");

    assert!(
        !read_rollout(&path)
            .iter()
            .any(|line| matches!(line.item, RolloutItem::ResponseItem(_)))
    );
}

#[tokio::test]
async fn migration_does_not_coalesce_distinct_adjacent_user_records() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            rollout_response_item(input_response_message("user", "copied parent question")),
            user_message("child question"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate distinct adjacent user records");

    assert_eq!(
        read_rollout(&path)
            .into_iter()
            .filter_map(|line| match line.item {
                RolloutItem::ResponseItem(response) => Some(response.into_item()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![input_response_message("user", "copied parent question")]
    );
}

#[tokio::test]
async fn migration_keeps_late_completions_for_surviving_turns_across_rollback() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("old"),
            user_message("old question"),
            started("remove"),
            user_message("remove question"),
            completed("old"),
            exec_completion("old", "call-old"),
            item_completed("old", "reason-old"),
            exec_completion("remove", "call-remove"),
            completed("remove"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            started("replacement"),
            user_message("replacement question"),
            completed("replacement"),
        ],
    );
    let legacy_turns = build_turns_from_rollout_items(
        &read_rollout(&path)
            .into_iter()
            .map(|line| line.item)
            .collect::<Vec<_>>(),
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate late completion rollback");

    let lines = read_rollout(&path);
    assert!(!lines.iter().any(|line| matches!(
        &line.item,
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event))
            if event.item.id() == "call-remove"
    )));
    assert!(lines.iter().any(|line| matches!(
        &line.item,
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event))
            if event.turn_id == "old" && event.item.id() == "call-old"
    )));
    assert!(lines.iter().any(|line| matches!(
        &line.item,
        RolloutItem::EventMsg(EventMsg::ItemCompleted(event))
            if event.turn_id == "old" && event.item.id() == "reason-old"
    )));
    assert!(lines.iter().any(|line| matches!(
        &line.item,
        RolloutItem::EventMsg(EventMsg::TurnComplete(event)) if event.turn_id == "old"
    )));
    assert!(!lines.iter().any(|line| matches!(
        &line.item,
        RolloutItem::EventMsg(EventMsg::TurnComplete(event)) if event.turn_id == "remove"
    )));
    let turns = store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::Summary,
        })
        .await
        .expect("read late-completion rollback turns");
    let turn_ids = turns
        .turns
        .iter()
        .map(|turn| turn.turn_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        turn_ids,
        legacy_turns
            .iter()
            .map(|turn| turn.id.as_str())
            .collect::<Vec<_>>()
    );
    let items = store
        .list_items(ListItemsParams {
            thread_id,
            turn_id: None,
            include_archived: false,
            cursor: None,
            page_size: 20,
            sort_direction: SortDirection::Asc,
            sort_key: ItemSortKey::CreatedAtOrdinal,
            after_updated_at_ordinal: None,
        })
        .await
        .expect("read late-completion rollback items");
    for legacy_turn in &legacy_turns {
        for legacy_item in &legacy_turn.items {
            if legacy_item.id().starts_with("item-") {
                continue;
            }
            assert_eq!(
                items
                    .items
                    .iter()
                    .find(|item| item.item_id == legacy_item.id())
                    .map(|item| item.turn_id.as_str()),
                Some(legacy_turn.id.as_str())
            );
        }
    }
}

#[tokio::test]
async fn migration_rolls_back_inter_agent_metadata_with_its_delivery() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let delivery = InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root().join("worker").expect("worker path"),
        Vec::new(),
        "remove delivery".to_string(),
        /*trigger_turn*/ true,
    );
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            RolloutItem::InterAgentCommunicationMetadata { trigger_turn: true },
            rollout_response_item(delivery.to_model_input_item()),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate rolled-back inter-agent delivery");

    assert!(!read_rollout(&path).iter().any(|line| match &line.item {
        RolloutItem::InterAgentCommunicationMetadata { .. } => true,
        RolloutItem::ResponseItem(response_item) => {
            matches!(&response_item.item, ResponseItem::AgentMessage { .. })
        }
        _ => false,
    }));
}

#[tokio::test]
async fn migration_rolls_back_pre_compaction_turns_from_sqlite_history() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let RolloutItem::Compacted(mut checkpoint) = compacted(vec![
        input_response_message("user", "old question"),
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "old answer".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ]) else {
        unreachable!("compacted helper always creates a compaction checkpoint");
    };
    checkpoint.mcp_resource_origins = Some(McpResourceOriginCheckpoint {
        origins: vec![McpResourceOrigin {
            call_id: "widget-call".to_string(),
            turn_id: Some("keep-before-compaction".to_string()),
            tool: "_product_search".to_string(),
            connector_id: "shopping".to_string(),
            link_id: None,
            uri: "ui://shopping/widget".to_string(),
            ambiguous_account: false,
        }],
        turns: vec!["keep-before-compaction".to_string()],
        current_turn_id: Some("keep-before-compaction".to_string()),
    });
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("keep-before-compaction"),
            user_message("old question"),
            completed("keep-before-compaction"),
            RolloutItem::Compacted(checkpoint),
            started("remove-after-compaction"),
            user_message("new question"),
            completed("remove-after-compaction"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 2,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate rollback through compaction");

    let turns = store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::Summary,
        })
        .await
        .expect("read rollback-through-compaction turns");
    assert_eq!(turns.turns.len(), 1);
    let checkpoint = read_rollout(&path)
        .into_iter()
        .find_map(|line| match line.item {
            RolloutItem::Compacted(item) => Some(item),
            _ => None,
        })
        .expect("retained compaction");
    assert_eq!(checkpoint.replacement_history, Some(Vec::new()));
    assert_eq!(checkpoint.mcp_resource_origins, None);
}

#[tokio::test]
async fn migration_preserves_reverse_replay_anchor_after_pre_compaction_rollback() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            rollout_response_item(input_response_message("user", "remove question")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            compacted(vec![input_response_message("user", "old question")]),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate pre-compaction rollback");

    let replacement_history = read_rollout(&path)
        .into_iter()
        .find_map(|line| match line.item {
            RolloutItem::Compacted(item) => item.replacement_history,
            _ => None,
        })
        .expect("retained compaction");
    assert!(replacement_history.is_empty());
}

#[tokio::test]
async fn migration_keeps_empty_replay_anchor_from_rolled_back_turn() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("keep"),
            user_message("keep question"),
            completed("keep"),
            started("remove"),
            user_message("remove question"),
            compacted(vec![input_response_message("user", "old question")]),
            completed("remove"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            started("replacement"),
            user_message("replacement question"),
            completed("replacement"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate rolled-back replay anchor");

    let replacement_history = read_rollout(&path)
        .into_iter()
        .find_map(|line| match line.item {
            RolloutItem::Compacted(item) => item.replacement_history,
            _ => None,
        })
        .expect("retained compaction");
    assert!(replacement_history.is_empty());
}

#[tokio::test]
async fn migration_uses_turn_context_to_select_reverse_replay_anchor() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("keep"),
            user_message("keep question"),
            compacted(vec![input_response_message("user", "keep question")]),
            completed("keep"),
            started("remove"),
            user_message("remove question"),
            serde_json::from_value(json!({
                "type": "event_msg",
                "payload": {
                    "type": "turn_aborted",
                    "turn_id": "other",
                    "reason": "interrupted"
                }
            }))
            .expect("build mismatched turn abort"),
            serde_json::from_value(json!({
                "type": "turn_context",
                "payload": {
                    "turn_id": "remove",
                    "cwd": home.path(),
                    "approval_policy": "never",
                    "sandbox_policy": {"type": "read-only"},
                    "model": "test-model",
                    "summary": "auto"
                }
            }))
            .expect("build turn context"),
            compacted(vec![input_response_message("user", "remove question")]),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            started("replacement"),
            user_message("replacement question"),
            completed("replacement"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate turn-context replay anchor");

    let replacement_histories = read_rollout(&path)
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::Compacted(item) => item.replacement_history.map(|items| {
                items
                    .into_iter()
                    .map(codex_rollout::ResponseItemEnvelope::into_item)
                    .collect::<Vec<_>>()
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        replacement_histories,
        vec![vec![input_response_message("user", "keep question")]]
    );
}

#[tokio::test]
async fn migration_applies_cumulative_and_overflowing_rollbacks() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            started("first"),
            user_message("first question"),
            completed("first"),
            started("second"),
            user_message("second question"),
            completed("second"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            started("third"),
            user_message("third question"),
            completed("third"),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 99,
            })),
            user_message("replacement question"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate cumulative rollbacks");

    let turns = store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: false,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::Summary,
        })
        .await
        .expect("read cumulative rollback turns");
    assert_eq!(turns.turns.len(), 1);
    assert!(!read_rollout(&path).iter().any(|line| matches!(
        line.item,
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))
    )));
}

#[tokio::test]
async fn migration_drops_copied_user_fork_metadata_without_creating_a_history_base() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let parent_id = ThreadId::new();
    let copied_metadata = SessionMeta {
        session_id: parent_id.into(),
        id: parent_id,
        timestamp: TIMESTAMP.to_string(),
        cwd: home.path().to_path_buf(),
        source: SessionSource::Cli,
        ..SessionMeta::default()
    };
    let copied_response =
        rollout_response_item(input_response_message("user", "copied parent history"));
    let path = write_rollout_with_fork(
        home.path(),
        thread_id,
        SessionSource::Cli,
        Some(parent_id),
        vec![
            RolloutItem::SessionMeta(SessionMetaLine {
                meta: copied_metadata,
                git: None,
            }),
            copied_response,
            user_message("child question"),
            agent_message("child answer"),
        ],
    );
    let expected_responses = read_rollout(&path)
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::ResponseItem(item) => {
                Some(serde_json::to_value(item.item).expect("serialize copied response"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate copied user fork");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    let lines = read_rollout(&path);
    assert!(matches!(
        &lines[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.id == thread_id
                && metadata.meta.forked_from_id == Some(parent_id)
                && metadata.meta.history_mode == ThreadHistoryMode::Paginated
                && metadata.meta.history_base.is_none()
    ));
    assert_eq!(
        lines
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::SessionMeta(_)))
            .count(),
        1
    );
    assert_eq!(
        lines
            .into_iter()
            .filter_map(|line| match line.item {
                RolloutItem::ResponseItem(item) => {
                    Some(serde_json::to_value(item.item).expect("serialize migrated response"))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        expected_responses
    );
}

#[tokio::test]
async fn migration_compacts_subagent_prefix_and_does_not_project_it() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::SubAgent(SubAgentSource::Other("test".to_string())),
        vec![
            RolloutItem::Compacted(CompactedItem {
                message: "superseded checkpoint".repeat(1024),
                replacement_history: Some(Vec::new()),
                mcp_resource_origins: None,
                window_number: Some(1),
                first_window_id: None,
                previous_window_id: None,
                window_id: None,
            }),
            RolloutItem::Compacted(CompactedItem {
                message: "latest checkpoint".to_string(),
                replacement_history: Some(vec![
                    ResponseItem::Message {
                        id: None,
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText {
                            text: "latest compacted context".to_string(),
                        }],
                        phase: None,
                        internal_chat_message_metadata_passthrough: None,
                    }
                    .into(),
                ]),
                mcp_resource_origins: None,
                window_number: Some(2),
                first_window_id: None,
                previous_window_id: None,
                window_id: None,
            }),
            started("child-turn"),
            RolloutItem::TurnContext(TurnContextItem {
                turn_id: Some("child-turn".to_string()),
                cwd: serde_json::from_value(json!(home.path())).expect("absolute cwd"),
                workspace_roots: None,
                current_date: None,
                timezone: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: None,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                permission_profile: None,
                active_permission_profile: None,
                network: None,
                file_system_sandbox_policy: None,
                model: "test-model".to_string(),
                comp_hash: None,
                personality: None,
                collaboration_mode: None,
                multi_agent_version: None,
                multi_agent_mode: None,
                realtime_active: None,
                effort: None,
                summary: ReasoningSummary::Auto,
            }),
            user_message("child question"),
            agent_message("child answer"),
            completed("child-turn"),
        ],
    );
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open legacy subagent rollout"),
        "{{not valid rollout json"
    )
    .expect("append malformed record");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate legacy subagent");

    let lines = read_rollout(&path);
    let RolloutItem::SessionMeta(metadata) = &lines[0].item else {
        panic!("migrated rollout should start with session metadata");
    };
    assert_eq!(
        metadata.meta.subagent_history_start_ordinal,
        Some(lines.len() as u64)
    );
    assert!(
        !fs::read_to_string(&path)
            .expect("read migrated rollout")
            .contains("superseded checkpoint")
    );
    let context = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await
        .expect("load migrated model context");
    assert!(context.items.iter().any(|item| {
        matches!(item, RolloutItem::Compacted(compacted) if compacted.message == "latest checkpoint")
    }));
    assert!(
        list_active_summary_turns(&store, thread_id)
            .await
            .turns
            .is_empty()
    );
}

#[tokio::test]
async fn migration_keeps_small_uncompacted_subagent_replay_as_prefix() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::SubAgent(SubAgentSource::Other("test".to_string())),
        vec![
            user_message("child question"),
            agent_message("child answer"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate uncompacted legacy subagent");

    let lines = read_rollout(&path);
    let RolloutItem::SessionMeta(metadata) = &lines[0].item else {
        panic!("migrated rollout should start with session metadata");
    };
    assert_eq!(
        metadata.meta.subagent_history_start_ordinal,
        Some(lines.len() as u64)
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| matches!(line.item, RolloutItem::EventMsg(EventMsg::ItemCompleted(_))))
            .count(),
        2
    );
}

#[tokio::test]
async fn migration_projects_memory_consolidation_as_ordinary_history() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::SubAgent(SubAgentSource::MemoryConsolidation),
        vec![
            user_message("memory question"),
            agent_message("memory answer"),
        ],
    );
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate memory consolidation rollout");

    let lines = read_rollout(&path);
    assert!(matches!(
        &lines[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.history_mode == ThreadHistoryMode::Paginated
                && metadata.meta.subagent_history_start_ordinal.is_none()
    ));
    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 2);
}

#[tokio::test]
async fn dry_run_reports_migration_order() {
    let home = TempDir::new().expect("create Codex home");
    let root_id = ThreadId::new();
    let root = write_rollout(
        home.path(),
        root_id,
        SessionSource::Cli,
        vec![user_message("root question")],
    );
    let newest_directory = home.path().join("sessions/2025/01/04");
    fs::create_dir_all(&newest_directory).expect("create newest rollout directory");
    let newest_root = newest_directory.join(format!("rollout-2025-01-04T12-00-00-{root_id}.jsonl"));
    fs::rename(&root, &newest_root).expect("move root rollout to newest date");
    let root = newest_root;
    let subagent_id = ThreadId::new();
    let subagent = write_rollout(
        home.path(),
        subagent_id,
        SessionSource::SubAgent(SubAgentSource::Other("test".to_string())),
        vec![user_message("subagent question")],
    );
    let memory_id = ThreadId::new();
    let memory = write_rollout(
        home.path(),
        memory_id,
        SessionSource::SubAgent(SubAgentSource::MemoryConsolidation),
        vec![user_message("memory question")],
    );
    let memory_directory = home.path().join("sessions/2025/01/01");
    fs::create_dir_all(&memory_directory).expect("create memory rollout directory");
    let moved_memory = memory_directory.join(memory.file_name().expect("memory filename"));
    fs::rename(memory, &moved_memory).expect("move memory rollout");
    let compressed_id = ThreadId::new();
    let oldest_directory = home.path().join("sessions/2025/01/02");
    fs::create_dir_all(&oldest_directory).expect("create oldest rollout directory");
    let source_compressed_plain = write_rollout(
        home.path(),
        compressed_id,
        SessionSource::Cli,
        vec![user_message("compressed question")],
    );
    let compressed_plain =
        oldest_directory.join(format!("rollout-2025-01-02T12-00-00-{compressed_id}.jsonl"));
    fs::rename(source_compressed_plain, &compressed_plain).expect("move compressed rollout");
    let compressed = compress_rollout(&compressed_plain);
    let archived_id = ThreadId::new();
    let archived = move_to_archived(
        home.path(),
        write_rollout(
            home.path(),
            archived_id,
            SessionSource::Cli,
            vec![user_message("archived question")],
        ),
    );
    let original = fs::read(&root).expect("read original root rollout");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);

    let mut progress = Vec::new();
    let report = store
        .migrate_rollouts_with_progress(RolloutMigrationOptions::default(), |update| {
            progress.push(update);
        })
        .await
        .expect("inspect legacy rollouts");

    let expected = vec![
        (root.clone(), root_id, RolloutMigrationStatus::Eligible),
        (subagent, subagent_id, RolloutMigrationStatus::Eligible),
        (compressed, compressed_id, RolloutMigrationStatus::Eligible),
        (moved_memory, memory_id, RolloutMigrationStatus::Eligible),
        (archived, archived_id, RolloutMigrationStatus::Eligible),
    ];
    assert_eq!(
        report
            .outcomes
            .iter()
            .map(|outcome| (
                outcome.rollout_path.clone(),
                outcome.thread_id.expect("rollout thread ID"),
                outcome.status,
            ))
            .collect::<Vec<_>>(),
        expected,
    );
    assert_eq!(
        progress.last(),
        Some(&RolloutMigrationProgress {
            processed_paths: 5,
            total_paths: 5,
            outcome_status: Some(RolloutMigrationStatus::Eligible),
        })
    );
    assert_eq!(fs::read(&root).expect("read inspected rollout"), original);

    let selected = store
        .migrate_rollouts(RolloutMigrationOptions {
            thread_ids: vec![root_id],
            ..RolloutMigrationOptions::default()
        })
        .await
        .expect("inspect selected rollout");
    assert_eq!(selected.outcomes.len(), 1);
    assert_eq!(selected.outcomes[0].thread_id, Some(root_id));
    assert_eq!(
        selected.outcomes[0].status,
        RolloutMigrationStatus::Eligible
    );
}

#[tokio::test]
async fn migration_preserves_compressed_rollouts_during_publish_and_recovery() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("compressed question"),
            agent_message("compressed answer"),
        ],
    );
    let compressed_path = compress_rollout(&path);
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate compressed rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!path.exists());
    assert!(compressed_path.exists());
    assert_eq!(
        codex_rollout::read_session_meta_line(&compressed_path)
            .await
            .expect("read compressed metadata")
            .meta
            .history_mode,
        ThreadHistoryMode::Paginated
    );
    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 2);

    thread_history::delete_thread(&store, thread_id)
        .await
        .expect("simulate missing projection");
    let journal_path = migration_journal_path(home.path(), thread_id);
    write_migration_journal(&journal_path)
        .await
        .expect("simulate pending migration journal");
    let recovered = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover compressed rollout");

    assert_eq!(
        recovered.outcomes[0].status,
        RolloutMigrationStatus::Migrated
    );
    assert!(recovered.outcomes[0].bytes_processed > 0);
    assert!(compressed_path.exists());
    assert!(!path.exists());
    assert!(!journal_path.exists());
}

#[tokio::test]
async fn migration_migrates_archived_rollouts_without_unarchiving_them() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let active_path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("archived question"),
            agent_message("archived answer"),
        ],
    );
    let archived_path = move_to_archived(home.path(), active_path.clone());
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate archived rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!active_path.exists());
    assert!(archived_path.exists());
    assert!(matches!(
        &read_rollout(&archived_path)[0].item,
        RolloutItem::SessionMeta(metadata)
            if metadata.meta.history_mode == ThreadHistoryMode::Paginated
    ));
    let turns = store
        .list_turns(ListTurnsParams {
            thread_id,
            include_archived: true,
            cursor: None,
            page_size: 10,
            sort_direction: SortDirection::Asc,
            items_view: StoredTurnItemsView::Summary,
        })
        .await
        .expect("read archived projected turns");
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 2);
}

#[tokio::test]
async fn migration_preserves_legacy_displayed_thread_names() {
    let home = TempDir::new().expect("create Codex home");
    let title_thread_id = ThreadId::new();
    write_rollout(
        home.path(),
        title_thread_id,
        SessionSource::Cli,
        vec![user_message("title question")],
    );
    let index_thread_id = ThreadId::new();
    write_rollout(
        home.path(),
        index_thread_id,
        SessionSource::Cli,
        vec![user_message("index question")],
    );
    let store = indexed_store(home.path()).await;
    store
        .update_thread_metadata(UpdateThreadMetadataParams {
            thread_id: title_thread_id,
            patch: ThreadMetadataPatch {
                name: Some(Some("renamed title".to_string())),
                ..Default::default()
            },
            include_archived: false,
        })
        .await
        .expect("rename legacy thread");
    codex_rollout::append_thread_name(home.path(), title_thread_id, "stale index title")
        .await
        .expect("write stale legacy index name");
    codex_rollout::append_thread_name(home.path(), index_thread_id, "indexed title")
        .await
        .expect("write legacy index name");

    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate named rollouts");

    let page = store
        .list_threads(ListThreadsParams {
            page_size: 10,
            cursor: None,
            sort_key: ThreadSortKey::CreatedAt,
            sort_direction: SortDirection::Desc,
            allowed_sources: Vec::new(),
            model_providers: None,
            cwd_filters: None,
            section: None,
            project_id: None,
            archived: false,
            search_term: None,
            relation_filter: None,
            use_state_db_only: true,
        })
        .await
        .expect("list migrated threads");
    let title_thread = page
        .items
        .iter()
        .find(|thread| thread.thread_id == title_thread_id)
        .expect("renamed title thread");
    let index_thread = page
        .items
        .iter()
        .find(|thread| thread.thread_id == index_thread_id)
        .expect("indexed title thread");

    assert_eq!(title_thread.name.as_deref(), Some("renamed title"));
    assert_eq!(index_thread.name.as_deref(), Some("indexed title"));
}

#[tokio::test]
async fn migration_repairs_a_missing_paginated_name_when_rerun() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("question")],
    );
    let store = indexed_store(home.path()).await;
    store
        .update_thread_metadata(UpdateThreadMetadataParams {
            thread_id,
            patch: ThreadMetadataPatch {
                name: Some(Some("renamed title".to_string())),
                ..Default::default()
            },
            include_archived: false,
        })
        .await
        .expect("rename legacy thread");
    store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate named rollout");
    let state_db = store.state_db().await.expect("state runtime");
    state_db
        .update_thread_title(thread_id, "question")
        .await
        .expect("restore derived title");
    state_db
        .update_thread_name(thread_id, /*name*/ None)
        .await
        .expect("clear migrated name");

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("repair migrated name");

    assert_eq!(
        report.outcomes[0].status,
        RolloutMigrationStatus::AlreadyPaginated
    );
    assert_eq!(
        state_db
            .get_thread(thread_id)
            .await
            .expect("read repaired metadata")
            .expect("repaired thread")
            .name
            .as_deref(),
        Some("renamed title")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn decompression_temporaries_are_owner_only() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let compressed_path = compress_rollout(&write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("compressed question")],
    ));
    let plain_path = home.path().join("decompressed.tmp");

    decompress_rollout_to_path(&compressed_path, &plain_path)
        .await
        .expect("decompress rollout");

    assert_eq!(
        fs::metadata(&plain_path)
            .expect("read temporary metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[tokio::test]
async fn migration_skips_threads_with_an_active_writer() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("active question")],
    );
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let _writer = store
        .writer_lock_coordinator
        .acquire(thread_id)
        .expect("acquire live writer lock");
    let original = fs::read(&path).expect("read active rollout");

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect active writer");

    assert_eq!(
        report.outcomes[0].status,
        RolloutMigrationStatus::SkippedBusy
    );
    assert_eq!(fs::read(&path).expect("read unmodified rollout"), original);
}

#[tokio::test]
async fn migration_apply_conflicts_with_rollout_maintenance() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("maintenance question")],
    );
    let original = fs::read(&path).expect("read legacy rollout");
    let _maintenance_guard = codex_rollout::try_acquire_rollout_maintenance_lock(home.path())
        .expect("acquire rollout maintenance lock")
        .expect("claim rollout maintenance lock");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);

    let error = store
        .migrate_rollouts(apply_options())
        .await
        .expect_err("reject concurrent rollout maintenance");

    assert!(matches!(error, ThreadStoreError::Conflict { .. }));
    assert_eq!(fs::read(&path).expect("read untouched rollout"), original);
}

#[tokio::test]
async fn migration_recovers_a_published_rollout_with_missing_projection() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("recover question"),
            agent_message("recover answer"),
        ],
    );
    let store = indexed_store(home.path()).await;
    store
        .migrate_rollouts(apply_options())
        .await
        .expect("publish canonical rollout");
    thread_history::delete_thread(&store, thread_id)
        .await
        .expect("simulate interrupted projection");
    let journal_path = migration_journal_path(home.path(), thread_id);
    write_migration_journal(&journal_path)
        .await
        .expect("simulate pending migration journal");

    let writer = store
        .writer_lock_coordinator
        .acquire(thread_id)
        .expect("acquire live writer lock");
    let busy = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect busy published recovery");
    assert_eq!(busy.outcomes[0].status, RolloutMigrationStatus::SkippedBusy);
    assert!(journal_path.exists());
    drop(writer);

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover published rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!journal_path.exists());
    let projection = thread_history::projection_state(&store, thread_id)
        .await
        .expect("read repaired projection")
        .expect("projection was rebuilt");
    assert_eq!(
        projection.next_byte_offset,
        fs::metadata(&path).expect("read rollout metadata").len()
    );
}

#[tokio::test]
async fn migration_recovers_pending_rollouts_before_new_work() {
    let home = TempDir::new().expect("create Codex home");
    let pending_thread_id = ThreadId::new();
    write_rollout(
        home.path(),
        pending_thread_id,
        SessionSource::Cli,
        vec![user_message("pending question")],
    );
    let new_thread_id = ThreadId::new();
    let new_path = write_rollout(
        home.path(),
        new_thread_id,
        SessionSource::Cli,
        vec![user_message("new question")],
    );
    let newer_directory = home.path().join("sessions/2025/01/04");
    fs::create_dir_all(&newer_directory).expect("create newer rollout directory");
    fs::rename(
        &new_path,
        newer_directory.join(new_path.file_name().expect("rollout filename")),
    )
    .expect("move newer rollout");
    let store = indexed_store(home.path()).await;

    store
        .migrate_rollouts(RolloutMigrationOptions {
            thread_ids: vec![pending_thread_id],
            ..apply_options()
        })
        .await
        .expect("publish pending rollout");
    thread_history::delete_thread(&store, pending_thread_id)
        .await
        .expect("simulate missing projection");
    write_migration_journal(&migration_journal_path(home.path(), pending_thread_id))
        .await
        .expect("simulate pending migration journal");

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover pending rollout before new work");

    assert_eq!(
        report
            .outcomes
            .iter()
            .map(|outcome| (
                outcome.thread_id.expect("rollout thread ID"),
                outcome.status
            ))
            .collect::<Vec<_>>(),
        vec![
            (pending_thread_id, RolloutMigrationStatus::Migrated),
            (new_thread_id, RolloutMigrationStatus::Migrated),
        ]
    );
}

#[tokio::test]
async fn migration_recovers_a_compressed_published_rollout() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![
            user_message("recover compressed question"),
            agent_message("recover compressed answer"),
        ],
    );
    let store = indexed_store(home.path()).await;
    store
        .migrate_rollouts(apply_options())
        .await
        .expect("publish canonical rollout");
    thread_history::delete_thread(&store, thread_id)
        .await
        .expect("simulate interrupted projection");
    let state_db = store.state_db.as_ref().expect("state db");
    let mut legacy_metadata = state_db
        .get_thread(thread_id)
        .await
        .expect("read thread metadata")
        .expect("thread metadata");
    legacy_metadata.history_mode = ThreadHistoryMode::Legacy;
    state_db
        .delete_thread(thread_id)
        .await
        .expect("remove paginated thread metadata");
    assert!(
        state_db
            .insert_thread_if_absent(&legacy_metadata)
            .await
            .expect("restore legacy thread metadata")
    );
    let journal_path = migration_journal_path(home.path(), thread_id);
    write_migration_journal(&journal_path)
        .await
        .expect("simulate pending migration journal");
    let compressed_path = compress_rollout(&path);

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("recover compressed published rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!path.exists());
    assert!(compressed_path.exists());
    assert!(!journal_path.exists());
    assert_eq!(
        store
            .state_db
            .as_ref()
            .expect("state db")
            .get_thread(thread_id)
            .await
            .expect("read repaired thread metadata")
            .expect("thread metadata")
            .history_mode,
        ThreadHistoryMode::Paginated
    );
    let projection = thread_history::projection_state(&store, thread_id)
        .await
        .expect("read repaired projection")
        .expect("projection was rebuilt");
    assert_eq!(
        projection.next_byte_offset,
        zstd::stream::decode_all(fs::File::open(&compressed_path).expect("open rollout"))
            .expect("decompress rollout")
            .len() as u64
    );
}

#[tokio::test]
async fn migration_skips_oversized_jsonl_records() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("kept question")],
    );
    let oversized_output = "x".repeat(super::MAX_ROLLOUT_LINE_BYTES);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open legacy rollout");
    writeln!(
        file,
        "{{\"timestamp\":\"{TIMESTAMP}\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"call_id\":\"call-1\",\"output\":\"{oversized_output}\"}}}}"
    )
    .expect("write oversized rollout record");
    drop(file);
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate oversized rollout record");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(
        fs::metadata(&path)
            .expect("read migrated rollout metadata")
            .len()
            < super::MAX_ROLLOUT_LINE_BYTES as u64
    );
    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 1);
}

#[tokio::test]
async fn migration_skips_empty_rollout_files() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let directory = home.path().join("sessions/2025/01/03");
    fs::create_dir_all(&directory).expect("create rollout directory");
    let path = directory.join(format!("rollout-2025-01-03T12-00-00-{thread_id}.jsonl"));
    fs::File::create(&path).expect("create empty rollout");
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect empty rollout");

    assert_eq!(
        report.outcomes[0].status,
        RolloutMigrationStatus::SkippedEmpty
    );
    assert_eq!(fs::metadata(&path).expect("read empty rollout").len(), 0);
    assert!(
        store
            .state_db
            .as_ref()
            .expect("state db")
            .get_thread(thread_id)
            .await
            .expect("read thread metadata")
            .is_none()
    );
}

#[tokio::test]
async fn migration_reports_missing_sqlite_metadata() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("question")],
    );
    let store = indexed_store(home.path()).await;
    store
        .state_db
        .as_ref()
        .expect("state db")
        .delete_thread(thread_id)
        .await
        .expect("remove thread metadata");

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect rollout with missing metadata");

    assert_failed_with_reason(
        &report.outcomes[0],
        RolloutMigrationFailureReason::MissingSqliteMetadata,
    );
}

#[tokio::test]
async fn migration_reports_invalid_session_metadata() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let directory = home.path().join("sessions/2025/01/03");
    fs::create_dir_all(&directory).expect("create rollout directory");
    let path = directory.join(format!("rollout-2025-01-03T12-00-00-{thread_id}.jsonl"));
    fs::write(path, "not a rollout record\n").expect("write malformed rollout");
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("inspect rollout with invalid metadata");

    assert_failed_with_reason(
        &report.outcomes[0],
        RolloutMigrationFailureReason::InvalidSessionMetadata,
    );
}

#[tokio::test]
async fn migration_skips_malformed_lines_and_trailing_partial_tail() {
    let home = TempDir::new().expect("create Codex home");
    let thread_id = ThreadId::new();
    let path = write_rollout(
        home.path(),
        thread_id,
        SessionSource::Cli,
        vec![user_message("kept question")],
    );
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open legacy rollout");
    writeln!(file, "{{not valid rollout json").expect("append malformed record");
    let later_valid_line = RolloutLine {
        timestamp: TIMESTAMP.to_string(),
        ordinal: None,
        item: agent_message("kept answer"),
    };
    writeln!(
        file,
        "{}",
        serde_json::to_string(&later_valid_line).expect("serialize later valid record")
    )
    .expect("append later valid record");
    file.write_all(br#"{"timestamp":"unterminated""#)
        .expect("append partial tail");
    drop(file);
    let store = indexed_store(home.path()).await;

    let report = store
        .migrate_rollouts(apply_options())
        .await
        .expect("migrate malformed legacy rollout");

    assert_eq!(report.outcomes[0].status, RolloutMigrationStatus::Migrated);
    assert!(!migration_journal_path(home.path(), thread_id).exists());
    let turns = list_active_summary_turns(&store, thread_id).await;
    assert_eq!(turns.turns.len(), 1);
    assert_eq!(turns.turns[0].items.len(), 2);
}
