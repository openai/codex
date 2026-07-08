use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::AgentReasoningEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::LocalThreadStore;
use crate::CreateThreadParams;
use crate::LiveThread;
use crate::SearchThreadOccurrencesParams;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStore;
use crate::local::test_support::test_config;

#[tokio::test]
async fn searches_visible_occurrences_and_keeps_paginated_snapshot_stable() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(test_config(home.path()), None));
    let thread_id = ThreadId::default();
    let live_thread = LiveThread::create(store.clone(), create_thread_params(thread_id))
        .await
        .expect("create thread");

    live_thread
        .append_items(&[
            turn_started("turn-1", 1_700_000_000),
            user_message(&format!(
                "needle in hidden environment context\n{USER_MESSAGE_BEGIN}\nNeedle twice: NEEDLE"
            )),
            RolloutItem::EventMsg(EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "needle in hidden reasoning".to_string(),
            })),
            RolloutItem::ResponseItem(codex_protocol::models::ResponseItem::FunctionCallOutput {
                id: None,
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_text("needle in tool output".to_string()),
                internal_chat_message_metadata_passthrough: None,
            }),
            agent_message("needle in an earlier assistant message"),
            agent_message("**Needle** from assistant; [needle](https://example.com)"),
            turn_complete("turn-1"),
        ])
        .await
        .expect("append searchable history");
    live_thread.flush().await.expect("flush history");

    let first = store
        .search_thread_occurrences(search_params(thread_id, None, 2))
        .await
        .expect("first search page");
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0].turn_id, "turn-1");
    assert_eq!(first.items[0].item_id, "item-1");
    assert_eq!(first.items[0].occurrence_index, 0);
    assert_eq!(first.items[1].occurrence_index, 1);
    assert_eq!(first.items[0].turn_started_at, 1_700_000_000);
    assert_eq!(first.items[0].snippet_match_range.start, 0);
    assert_eq!(first.items[0].snippet_match_range.end, 6);
    let cursor = first.next_cursor.expect("second page cursor");

    live_thread
        .append_items(&[
            turn_started("turn-2", 1_700_000_100),
            agent_message("new needle after the snapshot"),
            turn_complete("turn-2"),
        ])
        .await
        .expect("append after snapshot");
    live_thread.flush().await.expect("flush appended history");

    let second = store
        .search_thread_occurrences(search_params(thread_id, Some(cursor), 2))
        .await
        .expect("second search page");
    assert_eq!(second.items.len(), 2);
    assert_eq!(second.items[0].turn_id, "turn-1");
    assert_eq!(second.items[0].item_id, "item-4");
    assert_eq!(second.items[1].item_id, "item-4");
    assert_eq!(second.next_cursor, None);

    let refreshed = store
        .search_thread_occurrences(search_params(thread_id, None, 10))
        .await
        .expect("refreshed search");
    assert_eq!(refreshed.items.len(), 5);
    assert_eq!(refreshed.items.last().expect("new match").turn_id, "turn-2");

    let capped = store
        .search_thread_occurrences(SearchThreadOccurrencesParams {
            max_results: 3,
            page_size: 3,
            ..search_params(thread_id, None, 3)
        })
        .await
        .expect("capped search");
    assert_eq!(capped.items.len(), 3);
    assert!(capped.is_capped);
}

#[test]
fn reports_utf16_ranges_for_javascript_clients() {
    let matcher = super::LiteralMatcher::new("needle");
    let matches = super::occurrences_in_item(
        ThreadId::default(),
        "turn-1".to_string(),
        "item-1".to_string(),
        "🙂NEEDLE needle",
        1,
        &matcher,
        usize::MAX,
    );

    assert_eq!(
        matches
            .iter()
            .map(|item| item.match_range.clone())
            .collect::<Vec<_>>(),
        vec![
            crate::SearchTextRange { start: 2, end: 8 },
            crate::SearchTextRange { start: 9, end: 15 },
        ]
    );

    let greek_matches = super::LiteralMatcher::new("ος").find_ranges("ΟΣ", usize::MAX);
    assert_eq!(greek_matches, vec![0..4]);
    let expanded_matches = super::LiteralMatcher::new("i").find_ranges("İ", usize::MAX);
    assert_eq!(expanded_matches, vec![0..2]);
    assert_eq!(
        super::markdown_to_search_text("before <kbd>needle</kbd>"),
        "before <kbd>needle</kbd>"
    );
    assert_eq!(super::markdown_to_search_text("foo  \nbar"), "foobar");
}

fn search_params(
    thread_id: ThreadId,
    cursor: Option<String>,
    page_size: usize,
) -> SearchThreadOccurrencesParams {
    SearchThreadOccurrencesParams {
        thread_id,
        search_term: "needle".to_string(),
        cursor,
        page_size,
        max_results: 250,
    }
}

fn create_thread_params(thread_id: ThreadId) -> CreateThreadParams {
    CreateThreadParams {
        session_id: thread_id.into(),
        thread_id,
        extra_config: None,
        forked_from_id: None,
        parent_thread_id: None,
        source: SessionSource::Exec,
        thread_source: None,
        originator: "test_originator".to_string(),
        base_instructions: BaseInstructions::default(),
        dynamic_tools: Vec::new(),
        selected_capability_roots: Vec::new(),
        multi_agent_version: None,
        history_mode: ThreadHistoryMode::Legacy,
        initial_window_id: uuid::Uuid::now_v7().to_string(),
        metadata: ThreadPersistenceMetadata {
            cwd: Some(std::env::current_dir().expect("cwd")),
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        },
    }
}

fn turn_started(turn_id: &str, started_at: i64) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: Some(started_at),
        model_context_window: None,
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

fn agent_message(message: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
        message: message.to_string(),
        phase: None,
        memory_citation: None,
    }))
}
