use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use uuid::Uuid;

use super::*;
use crate::ThreadStore;
use crate::local::test_support::test_config;
use crate::local::test_support::write_session_file_with_history_mode;

#[tokio::test]
async fn loads_latest_checkpoint_with_required_turn_metadata() {
    let home = TempDir::new().expect("temp dir");
    let uuid = Uuid::from_u128(1001);
    let thread_id = codex_protocol::ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let path = write_session_file_with_history_mode(
        home.path(),
        "2025-01-03T13-00-00",
        uuid,
        ThreadHistoryMode::Paginated,
    )
    .expect("write session file");
    append_items(
        path.as_path(),
        [
            turn_started("turn-1"),
            user_message("older turn"),
            turn_context(home.path(), "turn-1"),
            compacted("older checkpoint", Some(Vec::new())),
            turn_complete("turn-1"),
            turn_started("turn-2"),
            user_message("latest turn"),
            turn_context(home.path(), "turn-2"),
            compacted("latest checkpoint", Some(Vec::new())),
            turn_complete("turn-2"),
        ],
    );
    let store = LocalThreadStore::new(test_config(home.path()), None);

    let context = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await
        .expect("load model context");

    assert_eq!(context.thread_id, thread_id);
    assert!(matches!(
        context.items.first(),
        Some(RolloutItem::SessionMeta(_))
    ));
    assert!(context.items.iter().any(|item| {
        matches!(item, RolloutItem::Compacted(compacted) if compacted.message == "latest checkpoint")
    }));
    assert!(!context.items.iter().any(|item| {
        matches!(item, RolloutItem::Compacted(compacted) if compacted.message == "older checkpoint")
    }));
    assert!(context.items.iter().any(|item| {
        matches!(item, RolloutItem::TurnContext(context) if context.turn_id.as_deref() == Some("turn-2"))
    }));
}

#[tokio::test]
async fn falls_back_to_full_history_for_compaction_without_replacement_history() {
    let home = TempDir::new().expect("temp dir");
    let uuid = Uuid::from_u128(1002);
    let thread_id = codex_protocol::ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let path = write_session_file_with_history_mode(
        home.path(),
        "2025-01-03T13-00-01",
        uuid,
        ThreadHistoryMode::Paginated,
    )
    .expect("write session file");
    append_items(
        path.as_path(),
        [
            turn_started("turn-1"),
            user_message("turn"),
            turn_context(home.path(), "turn-1"),
            compacted("usable checkpoint", Some(Vec::new())),
            compacted("legacy checkpoint", None),
            turn_complete("turn-1"),
        ],
    );
    let full_items = read_thread::load_history_items(path.as_path())
        .await
        .expect("load full history");
    let store = LocalThreadStore::new(test_config(home.path()), None);

    let context = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await
        .expect("load model context");

    assert_eq!(context.items.len(), full_items.len());
    assert!(context.items.iter().any(|item| {
        matches!(item, RolloutItem::Compacted(compacted) if compacted.message == "usable checkpoint")
    }));
}

#[tokio::test]
async fn ignores_malformed_tail_lines_before_selecting_checkpoint() {
    let home = TempDir::new().expect("temp dir");
    let uuid = Uuid::from_u128(1003);
    let thread_id = codex_protocol::ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let path = write_session_file_with_history_mode(
        home.path(),
        "2025-01-03T13-00-02",
        uuid,
        ThreadHistoryMode::Paginated,
    )
    .expect("write session file");
    append_items(
        path.as_path(),
        [
            turn_started("turn-1"),
            user_message("turn"),
            turn_context(home.path(), "turn-1"),
            compacted("checkpoint", Some(Vec::new())),
            turn_complete("turn-1"),
        ],
    );
    let mut file = OpenOptions::new()
        .append(true)
        .open(path.as_path())
        .expect("open session file");
    writeln!(file, "not-json").expect("append malformed line");
    let store = LocalThreadStore::new(test_config(home.path()), None);

    let context = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await
        .expect("load model context");

    assert!(context.items.iter().any(|item| {
        matches!(item, RolloutItem::Compacted(compacted) if compacted.message == "checkpoint")
    }));
}

fn append_items<const N: usize>(path: &Path, items: [RolloutItem; N]) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open session file");
    for item in items {
        let line = RolloutLine {
            timestamp: "2025-01-03T13:00:01Z".to_string(),
            item,
        };
        writeln!(
            file,
            "{}",
            serde_json::to_string(&line).expect("serialize line")
        )
        .expect("append rollout line");
    }
}

fn turn_started(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: Some(128_000),
        collaboration_mode_kind: Default::default(),
    }))
}

fn turn_complete(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: turn_id.to_string(),
        last_agent_message: None,
        completed_at: None,
        duration_ms: None,
        time_to_first_token_ms: None,
    }))
}

fn user_message(message: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        message: message.to_string(),
        ..Default::default()
    }))
}

fn turn_context(root: &Path, turn_id: &str) -> RolloutItem {
    RolloutItem::TurnContext(TurnContextItem {
        turn_id: Some(turn_id.to_string()),
        cwd: serde_json::from_value(serde_json::json!(root)).expect("absolute cwd"),
        workspace_roots: None,
        current_date: None,
        timezone: None,
        approval_policy: AskForApproval::Never,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
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
    })
}

fn compacted(message: &str, replacement_history: Option<Vec<ResponseItem>>) -> RolloutItem {
    RolloutItem::Compacted(CompactedItem {
        message: message.to_string(),
        replacement_history,
        window_number: Some(1),
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
    })
}
