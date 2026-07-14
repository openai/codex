use std::fs;
use std::io::Write;
use std::time::Duration;

use codex_app_server_protocol::ThreadItem;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout::RolloutRecorder;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::super::LocalThreadStore;
use super::super::test_support::test_config;
use crate::AppendThreadItemsParams;
use crate::CreateThreadParams;
use crate::DeleteThreadParams;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStore;

#[tokio::test]
async fn paginated_live_append_materializes_turn_items_and_state() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .persist_thread(thread_id)
        .await
        .expect("persist session metadata");

    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![
                turn_started("turn-1"),
                completed_item(
                    thread_id,
                    "turn-1",
                    TurnItem::UserMessage(UserMessageItem {
                        id: "user-1".to_string(),
                        client_id: None,
                        content: Vec::new(),
                    }),
                ),
                completed_item(
                    thread_id,
                    "turn-1",
                    TurnItem::AgentMessage(AgentMessageItem {
                        id: "agent-1".to_string(),
                        content: vec![AgentMessageContent::Text {
                            text: "done".to_string(),
                        }],
                        phase: None,
                        memory_citation: None,
                    }),
                ),
                turn_completed("turn-1"),
            ],
        })
        .await
        .expect("append paginated items");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let turn = sqlx::query_as::<
        _,
        (
            i64,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
SELECT
    rollout_ordinal,
    status,
    started_at,
    completed_at,
    duration_ms,
    first_user_item_id,
    final_agent_item_id
FROM thread_turns
WHERE thread_id = ? AND turn_id = ?
        "#,
    )
    .bind(thread_id.to_string())
    .bind("turn-1")
    .fetch_one(&pool)
    .await
    .expect("read projected turn");
    assert_eq!(
        turn,
        (
            1,
            "completed".to_string(),
            Some(10),
            Some(20),
            Some(10_000),
            Some("user-1".to_string()),
            Some("agent-1".to_string()),
        )
    );

    let items = sqlx::query_as::<_, (String, i64)>(
        r#"
SELECT item_id, rollout_ordinal
FROM thread_items
WHERE thread_id = ?
ORDER BY rollout_ordinal
        "#,
    )
    .bind(thread_id.to_string())
    .fetch_all(&pool)
    .await
    .expect("read projected items");
    assert_eq!(
        items,
        vec![("user-1".to_string(), 2), ("agent-1".to_string(), 3)]
    );

    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    let rollout_len = i64::try_from(fs::metadata(rollout_path).expect("rollout metadata").len())
        .expect("rollout length");
    let projection_state = sqlx::query_as::<_, (i64, i64)>(
        r#"
SELECT next_rollout_byte_offset, next_rollout_ordinal
FROM thread_history_projection_state
WHERE thread_id = ?
        "#,
    )
    .bind(thread_id.to_string())
    .fetch_one(&pool)
    .await
    .expect("read projection state");
    assert_eq!(projection_state, (rollout_len, 5));
}

#[tokio::test]
async fn replayed_item_snapshot_updates_content_without_reordering() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;

    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![
                turn_started("turn-1"),
                completed_item(
                    thread_id,
                    "turn-1",
                    TurnItem::UserMessage(UserMessageItem {
                        id: "user-1".to_string(),
                        client_id: None,
                        content: Vec::new(),
                    }),
                ),
            ],
        })
        .await
        .expect("append first item snapshot");
    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let first_created_at_ms = sqlx::query_scalar::<_, i64>(
        "SELECT created_at_ms FROM thread_items WHERE thread_id = ? AND turn_id = ? AND item_id = ?",
    )
    .bind(thread_id.to_string())
    .bind("turn-1")
    .bind("user-1")
    .fetch_one(&pool)
    .await
    .expect("read first item timestamp");
    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![completed_item(
                thread_id,
                "turn-1",
                TurnItem::UserMessage(UserMessageItem {
                    id: "user-1".to_string(),
                    client_id: Some("updated".to_string()),
                    content: Vec::new(),
                }),
            )],
        })
        .await
        .expect("append replayed item snapshot");

    let item = sqlx::query_as::<_, (i64, i64, String)>(
        r#"
SELECT rollout_ordinal, created_at_ms, item_json
FROM thread_items
WHERE thread_id = ? AND turn_id = ? AND item_id = ?
        "#,
    )
    .bind(thread_id.to_string())
    .bind("turn-1")
    .bind("user-1")
    .fetch_one(&pool)
    .await
    .expect("read projected item");
    assert_eq!(item.0, 2);
    assert_eq!(item.1, first_created_at_ms);
    assert_eq!(
        serde_json::from_str::<ThreadItem>(item.2.as_str()).expect("parse projected item"),
        ThreadItem::UserMessage {
            id: "user-1".to_string(),
            client_id: Some("updated".to_string()),
            content: Vec::new(),
        }
    );
}

#[tokio::test]
async fn turn_creation_recovers_summary_ids_from_earlier_items() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;

    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![
                completed_item(
                    thread_id,
                    "turn-1",
                    TurnItem::UserMessage(UserMessageItem {
                        id: "user-1".to_string(),
                        client_id: None,
                        content: Vec::new(),
                    }),
                ),
                completed_item(
                    thread_id,
                    "turn-1",
                    TurnItem::AgentMessage(AgentMessageItem {
                        id: "agent-1".to_string(),
                        content: vec![AgentMessageContent::Text {
                            text: "done".to_string(),
                        }],
                        phase: None,
                        memory_citation: None,
                    }),
                ),
            ],
        })
        .await
        .expect("append items before turn");
    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![turn_started("turn-1"), turn_completed("turn-1")],
        })
        .await
        .expect("append turn lifecycle");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let summary_ids = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT first_user_item_id, final_agent_item_id FROM thread_turns WHERE thread_id = ? AND turn_id = ?",
    )
    .bind(thread_id.to_string())
    .bind("turn-1")
    .fetch_one(&pool)
    .await
    .expect("read turn summary ids");
    assert_eq!(
        summary_ids,
        (Some("user-1".to_string()), Some("agent-1".to_string()))
    );
}

#[tokio::test]
async fn next_write_catches_up_unprojected_durable_suffix() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .persist_thread(thread_id)
        .await
        .expect("persist session metadata");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let checkpoint = projection_state(&pool, thread_id).await;
    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![turn_started("turn-1")],
        })
        .await
        .expect("append turn start");

    let thread_id_string = thread_id.to_string();
    sqlx::query("DELETE FROM thread_turns WHERE thread_id = ?")
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("remove projected turn");
    sqlx::query(
        r#"
UPDATE thread_history_projection_state
SET next_rollout_byte_offset = ?, next_rollout_ordinal = ?
WHERE thread_id = ?
        "#,
    )
    .bind(checkpoint.0)
    .bind(checkpoint.1)
    .bind(thread_id_string.as_str())
    .execute(&pool)
    .await
    .expect("rewind projection state");

    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![completed_item(
                thread_id,
                "turn-1",
                TurnItem::UserMessage(UserMessageItem {
                    id: "user-1".to_string(),
                    client_id: None,
                    content: Vec::new(),
                }),
            )],
        })
        .await
        .expect("append after simulated projection failure");

    let rows = sqlx::query_as::<_, (String, String)>(
        r#"
SELECT
    (SELECT status FROM thread_turns WHERE thread_id = ? AND turn_id = 'turn-1'),
    (SELECT item_id FROM thread_items WHERE thread_id = ? AND turn_id = 'turn-1')
        "#,
    )
    .bind(thread_id_string.as_str())
    .bind(thread_id_string.as_str())
    .fetch_one(&pool)
    .await
    .expect("read recovered rows");
    assert_eq!(rows, ("inProgress".to_string(), "user-1".to_string()));

    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    let rollout_len = i64::try_from(fs::metadata(rollout_path).expect("rollout metadata").len())
        .expect("rollout length");
    assert_eq!(projection_state(&pool, thread_id).await, (rollout_len, 3));
}

#[tokio::test]
async fn synchronized_catch_up_does_not_replay_old_rows() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![turn_started("turn-1")],
        })
        .await
        .expect("append turn start");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let before = projection_state(&pool, thread_id).await;
    sqlx::query("UPDATE thread_turns SET status = 'sentinel' WHERE thread_id = ?")
        .bind(thread_id.to_string())
        .execute(&pool)
        .await
        .expect("mark projected turn");
    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    super::materialize_to_sqlite(&store, thread_id, rollout_path.as_path())
        .await
        .expect("catch up synchronized rollout");

    assert_eq!(projection_state(&pool, thread_id).await, before);
    let status =
        sqlx::query_scalar::<_, String>("SELECT status FROM thread_turns WHERE thread_id = ?")
            .bind(thread_id.to_string())
            .fetch_one(&pool)
            .await
            .expect("read projected turn");
    assert_eq!(status, "sentinel");
}

#[tokio::test]
async fn catch_up_leaves_trailing_partial_line_unprojected() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .persist_thread(thread_id)
        .await
        .expect("persist session metadata");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let before = projection_state(&pool, thread_id).await;
    let complete_line = rollout_line(Some(1), turn_started("turn-1"));
    let partial_line = rollout_line(
        Some(2),
        completed_item(
            thread_id,
            "turn-1",
            TurnItem::UserMessage(UserMessageItem {
                id: "user-1".to_string(),
                client_id: None,
                content: Vec::new(),
            }),
        ),
    );
    let complete_suffix = format!("{complete_line}\n");
    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    append_suffix(
        rollout_path.as_path(),
        format!("{complete_suffix}{partial_line}").as_str(),
    );

    super::materialize_to_sqlite(&store, thread_id, rollout_path.as_path())
        .await
        .expect("catch up complete suffix");

    let expected_offset =
        before.0 + i64::try_from(complete_suffix.len()).expect("complete suffix byte count");
    assert_eq!(
        projection_state(&pool, thread_id).await,
        (expected_offset, 2)
    );
    let counts = sqlx::query_as::<_, (i64, i64)>(
        r#"
SELECT
    (SELECT COUNT(*) FROM thread_turns WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_items WHERE thread_id = ?)
        "#,
    )
    .bind(thread_id.to_string())
    .bind(thread_id.to_string())
    .fetch_one(&pool)
    .await
    .expect("read projected row counts");
    assert_eq!(counts, (1, 0));
}

#[tokio::test]
async fn catch_up_rejects_invalid_complete_suffixes_without_advancing_state() {
    let cases = [
        (
            "missing ordinal",
            format!(
                "{}\n",
                rollout_line(/*ordinal*/ None, turn_started("turn-1"))
            ),
        ),
        (
            "duplicate ordinal",
            format!(
                "{}\n{}\n",
                rollout_line(Some(1), turn_started("turn-1")),
                rollout_line(Some(1), turn_started("turn-2")),
            ),
        ),
        (
            "out of order ordinal",
            format!("{}\n", rollout_line(Some(2), turn_started("turn-1"))),
        ),
    ];
    for (name, suffix) in cases {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let thread_id = ThreadId::default();
        create_paginated_thread(&store, thread_id).await;
        store
            .persist_thread(thread_id)
            .await
            .expect("persist session metadata");

        let pool = codex_state::open_thread_history_db(home.path())
            .await
            .expect("open thread history db");
        let before = projection_state(&pool, thread_id).await;
        let rollout_path = store
            .live_rollout_path(thread_id)
            .await
            .expect("rollout path");
        append_suffix(rollout_path.as_path(), suffix.as_str());

        super::materialize_to_sqlite(&store, thread_id, rollout_path.as_path())
            .await
            .expect_err(name);

        assert_eq!(
            projection_state(&pool, thread_id).await,
            before,
            "{name} should not advance projection state"
        );
        let counts = sqlx::query_as::<_, (i64, i64)>(
            r#"
SELECT
    (SELECT COUNT(*) FROM thread_turns WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_items WHERE thread_id = ?)
            "#,
        )
        .bind(thread_id.to_string())
        .bind(thread_id.to_string())
        .fetch_one(&pool)
        .await
        .expect("read projected row counts");
        assert_eq!(counts, (0, 0), "{name} should not project rows");
    }
}

#[tokio::test]
async fn jsonl_failure_does_not_create_projection_database() {
    let home = TempDir::new().expect("temp dir");
    fs::write(home.path().join("sessions"), "not a directory").expect("block sessions dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;

    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![turn_started("turn-1")],
        })
        .await
        .expect_err("JSONL append should fail");

    assert!(!codex_state::thread_history_db_path(home.path()).exists());
}

#[tokio::test]
async fn catch_up_rejects_missing_rollout_after_projection() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .persist_thread(thread_id)
        .await
        .expect("persist session metadata");
    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    store
        .shutdown_thread(thread_id)
        .await
        .expect("close rollout");
    fs::remove_file(rollout_path.as_path()).expect("remove rollout");

    super::materialize_to_sqlite(&store, thread_id, rollout_path.as_path())
        .await
        .expect_err("missing projected rollout should fail");
}

#[tokio::test]
async fn sqlite_failure_does_not_fail_durable_jsonl_write() {
    let home = TempDir::new().expect("temp dir");
    let sqlite_home = home.path().join("not-a-directory");
    fs::write(sqlite_home.as_path(), "not a directory").expect("block sqlite home");
    let mut config = test_config(home.path());
    config.sqlite_home = sqlite_home;
    let store = LocalThreadStore::new(config, /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;

    store
        .append_items(AppendThreadItemsParams {
            thread_id,
            items: vec![turn_started("turn-1")],
        })
        .await
        .expect("durable JSONL append should succeed");

    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    let (items, _, _) = RolloutRecorder::load_rollout_items(rollout_path.as_path())
        .await
        .expect("load durable rollout");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            RolloutItem::EventMsg(EventMsg::TurnStarted(event))
                if event.turn_id == "turn-1"
        )
    }));
}

#[tokio::test]
async fn rejected_rollout_line_does_not_poison_projection() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .persist_thread(thread_id)
        .await
        .expect("persist session metadata");

    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(rollout_path.as_path())
        .expect("open rollout for rejected line");
    file.write_all(b"{not json}\n")
        .expect("append rejected line");
    file.flush().expect("flush rejected line");
    let recorder = store
        .live_recorders
        .lock()
        .await
        .get(&thread_id)
        .expect("live recorder")
        .recorder
        .clone();
    recorder
        .record_canonical_items(&[turn_started("turn-1")])
        .await
        .expect("queue valid retry");
    recorder.flush().await.expect("flush valid retry");

    super::materialize_to_sqlite(&store, thread_id, rollout_path.as_path())
        .await
        .expect("project valid retry after rejected line");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let projected_turns = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM thread_turns WHERE thread_id = ? AND turn_id = ?",
    )
    .bind(thread_id.to_string())
    .bind("turn-1")
    .fetch_one(&pool)
    .await
    .expect("read projected turns");
    assert_eq!(projected_turns, 1);
}

#[tokio::test]
async fn shutdown_materializes_items_queued_without_a_flush() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    let recorder = store
        .live_recorders
        .lock()
        .await
        .get(&thread_id)
        .expect("live recorder")
        .recorder
        .clone();
    recorder
        .record_canonical_items(&[turn_started("turn-1")])
        .await
        .expect("queue rollout item");

    store
        .shutdown_thread(thread_id)
        .await
        .expect("shutdown live thread");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let projected_turns = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM thread_turns WHERE thread_id = ? AND turn_id = ?",
    )
    .bind(thread_id.to_string())
    .bind("turn-1")
    .fetch_one(&pool)
    .await
    .expect("read projected turns");
    assert_eq!(projected_turns, 1);
}

#[tokio::test]
async fn delete_waits_for_in_flight_projection_before_removing_rows() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let thread_id = ThreadId::default();
    create_paginated_thread(&store, thread_id).await;
    store
        .persist_thread(thread_id)
        .await
        .expect("persist session metadata");
    let write_permit = store.live_writer_locks.lock(thread_id).await;

    let append_store = store.clone();
    let append = tokio::spawn(async move {
        append_store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![turn_started("turn-1")],
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
    let delete_store = store.clone();
    let delete = tokio::spawn(async move {
        delete_store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
    });
    tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
    assert!(!delete.is_finished());

    drop(write_permit);
    append
        .await
        .expect("join append")
        .expect("finish in-flight append");
    delete.await.expect("join delete").expect("delete thread");

    let pool = codex_state::open_thread_history_db(home.path())
        .await
        .expect("open thread history db");
    let counts = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
SELECT
    (SELECT COUNT(*) FROM thread_turns WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_items WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_history_projection_state WHERE thread_id = ?)
        "#,
    )
    .bind(thread_id.to_string())
    .bind(thread_id.to_string())
    .bind(thread_id.to_string())
    .fetch_one(&pool)
    .await
    .expect("read history row counts");
    assert_eq!(counts, (0, 0, 0));
}

async fn create_paginated_thread(store: &LocalThreadStore, thread_id: ThreadId) {
    store
        .create_thread(CreateThreadParams {
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
            history_mode: ThreadHistoryMode::Paginated,
            initial_window_id: "window-1".to_string(),
            metadata: ThreadPersistenceMetadata {
                cwd: Some(std::env::current_dir().expect("cwd")),
                model_provider: "test-provider".to_string(),
                memory_mode: ThreadMemoryMode::Enabled,
            },
        })
        .await
        .expect("create paginated thread");
}

fn turn_started(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: Some(10),
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
}

fn turn_completed(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: turn_id.to_string(),
        last_agent_message: None,
        error: None,
        started_at: Some(10),
        completed_at: Some(20),
        duration_ms: Some(10_000),
        time_to_first_token_ms: None,
    }))
}

fn completed_item(thread_id: ThreadId, turn_id: &str, item: TurnItem) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id,
        turn_id: turn_id.to_string(),
        item,
        completed_at_ms: 1,
    }))
}

async fn projection_state(pool: &sqlx::SqlitePool, thread_id: ThreadId) -> (i64, i64) {
    sqlx::query_as::<_, (i64, i64)>(
        r#"
SELECT next_rollout_byte_offset, next_rollout_ordinal
FROM thread_history_projection_state
WHERE thread_id = ?
        "#,
    )
    .bind(thread_id.to_string())
    .fetch_one(pool)
    .await
    .expect("read projection state")
}

fn rollout_line(ordinal: Option<u64>, item: RolloutItem) -> String {
    serde_json::to_string(&RolloutLine {
        timestamp: "2025-01-01T00:00:00.000Z".to_string(),
        ordinal,
        item,
    })
    .expect("serialize rollout line")
}

fn append_suffix(rollout_path: &std::path::Path, suffix: &str) {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(rollout_path)
        .expect("open rollout suffix");
    file.write_all(suffix.as_bytes()).expect("append suffix");
    file.flush().expect("flush suffix");
}
