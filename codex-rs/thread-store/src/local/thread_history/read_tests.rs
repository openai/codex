use std::fs;

use chrono::Utc;
use codex_app_server_protocol::CodexErrorInfo;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::ItemSortKey;
use crate::SortDirection;
use crate::StoredTurnError;
use crate::StoredTurnStatus;
use crate::local::test_support::test_config;

#[tokio::test]
async fn list_turns_pages_projected_rows_and_applies_item_views() {
    let (_home, store, thread_id) = store_with_mode(ThreadHistoryMode::Paginated).await;
    let db = history_db(&store).await;
    for (turn_id, ordinal, status, error, first_user, final_agent) in [
        (
            "turn-1",
            10,
            "completed",
            None,
            Some("user-1"),
            Some("agent-1"),
        ),
        (
            "turn-2",
            20,
            "failed",
            Some(
                r#"{"message":"turn failed","codexErrorInfo":"serverOverloaded","additionalDetails":"retry later"}"#,
            ),
            None,
            None,
        ),
        ("turn-3", 30, "inProgress", None, None, None),
    ] {
        insert_turn(
            db,
            thread_id,
            turn_id,
            ordinal,
            status,
            error,
            first_user,
            final_agent,
        )
        .await;
    }
    for (turn_id, item_id, ordinal) in [
        ("turn-1", "user-1", 11),
        ("turn-1", "middle-1", 12),
        ("turn-1", "agent-1", 13),
    ] {
        insert_item(db, thread_id, turn_id, item_id, ordinal).await;
    }

    let first_page = store
        .list_turns(turn_params(
            thread_id,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
            StoredTurnItemsView::Summary,
        ))
        .await
        .expect("first turns page");
    assert_eq!(turn_ids(&first_page), vec!["turn-1", "turn-2"]);
    assert_eq!(
        first_page.turns[0].items,
        vec![
            expected_item("turn-1", "user-1", /*rollout_ordinal*/ 11),
            expected_item("turn-1", "agent-1", /*rollout_ordinal*/ 13),
        ]
    );
    assert_eq!(
        first_page.turns[1].error,
        Some(StoredTurnError {
            message: "turn failed".to_string(),
            codex_error_info: Some(CodexErrorInfo::ServerOverloaded),
            additional_details: Some("retry later".to_string()),
        })
    );
    let second_page = store
        .list_turns(turn_params(
            thread_id,
            first_page.next_cursor,
            /*page_size*/ 2,
            SortDirection::Asc,
            StoredTurnItemsView::NotLoaded,
        ))
        .await
        .expect("second turns page");
    assert_eq!(turn_ids(&second_page), vec!["turn-3"]);
    assert_eq!(second_page.turns[0].items, Vec::new());
    assert_eq!(second_page.turns[0].status, StoredTurnStatus::InProgress);
    let backwards_page = store
        .list_turns(turn_params(
            thread_id,
            second_page.backwards_cursor,
            /*page_size*/ 2,
            SortDirection::Desc,
            StoredTurnItemsView::NotLoaded,
        ))
        .await
        .expect("backwards turns page");
    assert_eq!(turn_ids(&backwards_page), vec!["turn-3", "turn-2"]);
}

#[tokio::test]
async fn list_items_pages_whole_thread_and_per_turn_rows() {
    let (_home, store, thread_id) = store_with_mode(ThreadHistoryMode::Paginated).await;
    let db = history_db(&store).await;
    for (turn_id, ordinal) in [("turn-1", 10), ("turn-2", 20)] {
        insert_turn(
            db,
            thread_id,
            turn_id,
            ordinal,
            "completed",
            /*error_json*/ None,
            /*first_user_item_id*/ None,
            /*final_agent_item_id*/ None,
        )
        .await;
    }
    for (turn_id, item_id, ordinal) in [
        ("turn-1", "item-1", 11),
        ("turn-1", "item-2", 12),
        ("turn-2", "item-3", 21),
        ("turn-2", "item-4", 22),
        ("turn-2", "item-5", 23),
    ] {
        insert_item(db, thread_id, turn_id, item_id, ordinal).await;
    }

    let first_page = store
        .list_items(item_params(
            thread_id,
            /*turn_id*/ None,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect("first item page");
    assert_eq!(
        first_page.items,
        vec![
            expected_item("turn-1", "item-1", /*rollout_ordinal*/ 11),
            expected_item("turn-1", "item-2", /*rollout_ordinal*/ 12),
        ]
    );
    let second_page = store
        .list_items(item_params(
            thread_id,
            /*turn_id*/ None,
            first_page.next_cursor,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect("second item page");
    assert_eq!(item_ids(&second_page), vec!["item-3", "item-4"]);
    let backwards_page = store
        .list_items(item_params(
            thread_id,
            /*turn_id*/ None,
            second_page.backwards_cursor,
            /*page_size*/ 2,
            SortDirection::Desc,
        ))
        .await
        .expect("backwards item page");
    assert_eq!(item_ids(&backwards_page), vec!["item-3", "item-2"]);

    let turn_page = store
        .list_items(item_params(
            thread_id,
            Some("turn-2"),
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Desc,
        ))
        .await
        .expect("turn item page");
    assert_eq!(item_ids(&turn_page), vec!["item-5", "item-4"]);
    let whole_thread_from_turn_cursor = store
        .list_items(item_params(
            thread_id,
            /*turn_id*/ None,
            turn_page.backwards_cursor.clone(),
            /*page_size*/ 2,
            SortDirection::Desc,
        ))
        .await
        .expect("whole-thread page from turn cursor");
    assert_eq!(
        item_ids(&whole_thread_from_turn_cursor),
        vec!["item-5", "item-4"]
    );
    let next_turn_page = store
        .list_items(item_params(
            thread_id,
            Some("turn-2"),
            turn_page.next_cursor,
            /*page_size*/ 2,
            SortDirection::Desc,
        ))
        .await
        .expect("next turn item page");
    assert_eq!(item_ids(&next_turn_page), vec!["item-3"]);
}

#[tokio::test]
async fn list_items_filters_exclusive_update_ordinals_across_pages_and_turns() {
    let (_home, store, thread_id) = store_with_mode(ThreadHistoryMode::Paginated).await;
    let db = history_db(&store).await;
    for (turn_id, item_id, ordinal) in [
        ("turn-1", "item-1", 1),
        ("turn-2", "item-2", 2),
        ("turn-2", "item-3", 3),
    ] {
        insert_item(db, thread_id, turn_id, item_id, ordinal).await;
    }
    sqlx::query(
        "UPDATE thread_items SET updated_at_ordinal = 4 WHERE thread_id = ? AND item_id = 'item-1'",
    )
    .bind(thread_id.to_string())
    .execute(db)
    .await
    .expect("advance first item update ordinal");

    let item_1 = StoredThreadItem {
        updated_at_ordinal: 4,
        ..expected_item("turn-1", "item-1", /*rollout_ordinal*/ 1)
    };
    let item_2 = expected_item("turn-2", "item-2", /*rollout_ordinal*/ 2);
    let item_3 = expected_item("turn-2", "item-3", /*rollout_ordinal*/ 3);
    let creation_page = store
        .list_items(item_params(
            thread_id,
            /*turn_id*/ None,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect("creation-ordered item page");
    assert_eq!(creation_page.items, vec![item_1.clone(), item_2.clone()]);
    for (sort_direction, expected) in [
        (SortDirection::Asc, vec![item_1.clone(), item_3.clone()]),
        (SortDirection::Desc, vec![item_3.clone(), item_1.clone()]),
    ] {
        let page = store
            .list_items(ListItemsParams {
                after_updated_at_ordinal: Some(2),
                ..item_params(
                    thread_id,
                    /*turn_id*/ None,
                    /*cursor*/ None,
                    /*page_size*/ 2,
                    sort_direction,
                )
            })
            .await
            .expect("creation-ordered filtered item page");
        assert_eq!(page.items, expected);
    }

    let first_page = store
        .list_items(updated_item_params(
            thread_id, /*after_updated_at_ordinal*/ 0,
        ))
        .await
        .expect("first filtered item page");
    assert_eq!(first_page.items, vec![item_2.clone(), item_3.clone()]);
    for params in [
        ListItemsParams {
            cursor: creation_page.next_cursor,
            ..updated_item_params(thread_id, /*after_updated_at_ordinal*/ 0)
        },
        item_params(
            thread_id,
            /*turn_id*/ None,
            first_page.next_cursor.clone(),
            /*page_size*/ 2,
            SortDirection::Asc,
        ),
    ] {
        let error = store
            .list_items(params)
            .await
            .expect_err("creation and update cursors should not be interchangeable");
        assert!(matches!(error, ThreadStoreError::InvalidRequest { .. }));
    }

    let second_page = store
        .list_items(ListItemsParams {
            cursor: first_page.next_cursor,
            ..updated_item_params(thread_id, /*after_updated_at_ordinal*/ 0)
        })
        .await
        .expect("second filtered item page");
    assert_eq!(second_page.items, vec![item_1.clone()]);
    assert!(second_page.next_cursor.is_none());

    let exclusive_page = store
        .list_items(ListItemsParams {
            turn_id: Some("turn-2".to_string()),
            ..updated_item_params(thread_id, /*after_updated_at_ordinal*/ 2)
        })
        .await
        .expect("exclusive filtered turn page");
    assert_eq!(exclusive_page.items, vec![item_3.clone()]);

    let descending_page = store
        .list_items(ListItemsParams {
            sort_direction: SortDirection::Desc,
            ..updated_item_params(thread_id, /*after_updated_at_ordinal*/ 0)
        })
        .await
        .expect("descending update-ordered item page");
    assert_eq!(descending_page.items, vec![item_1, item_3]);
    let descending_next_page = store
        .list_items(ListItemsParams {
            cursor: descending_page.next_cursor,
            sort_direction: SortDirection::Desc,
            ..updated_item_params(thread_id, /*after_updated_at_ordinal*/ 0)
        })
        .await
        .expect("next descending update-ordered item page");
    assert_eq!(descending_next_page.items, vec![item_2]);

    let error = store
        .list_items(ListItemsParams {
            sort_key: ItemSortKey::UpdatedAtOrdinal,
            ..item_params(
                thread_id,
                /*turn_id*/ None,
                /*cursor*/ None,
                /*page_size*/ 2,
                SortDirection::Asc,
            )
        })
        .await
        .expect_err("update-ordinal sorting should require a watermark");
    assert!(matches!(error, ThreadStoreError::InvalidRequest { .. }));
}

#[tokio::test]
async fn list_items_rejects_update_ordinals_outside_sqlite_integer_range() {
    let (_home, store, thread_id) = store_with_mode(ThreadHistoryMode::Paginated).await;

    for sort_key in [ItemSortKey::CreatedAtOrdinal, ItemSortKey::UpdatedAtOrdinal] {
        let error = store
            .list_items(ListItemsParams {
                sort_key,
                ..updated_item_params(thread_id, /*after_updated_at_ordinal*/ u64::MAX)
            })
            .await
            .expect_err("out-of-range SQLite update ordinal should fail");

        assert!(matches!(error, ThreadStoreError::InvalidRequest { .. }));
    }
}

#[tokio::test]
async fn list_history_keeps_legacy_threads_unsupported() {
    let (_home, store, thread_id) = store_with_mode(ThreadHistoryMode::Legacy).await;

    let error = store
        .list_turns(turn_params(
            thread_id,
            /*cursor*/ None,
            /*page_size*/ 1,
            SortDirection::Asc,
            StoredTurnItemsView::Summary,
        ))
        .await
        .expect_err("legacy turns remain unsupported");
    assert!(matches!(
        error,
        ThreadStoreError::Unsupported {
            operation: "list_turns"
        }
    ));

    let error = store
        .list_turns(turn_params(
            ThreadId::default(),
            /*cursor*/ None,
            /*page_size*/ 1,
            SortDirection::Asc,
            StoredTurnItemsView::Summary,
        ))
        .await
        .expect_err("unindexed threads remain unsupported");
    assert!(matches!(
        error,
        ThreadStoreError::Unsupported {
            operation: "list_turns"
        }
    ));
}

#[tokio::test]
async fn lineage_reads_page_across_parent_and_child_segments() {
    let (home, store, child_id) = store_with_mode(ThreadHistoryMode::Paginated).await;
    let root_id = ThreadId::default();
    let root_path = rollout_path(home.path(), root_id);
    write_rollout_with_end(
        root_path.as_path(),
        root_id,
        /*history_base*/ None,
        /*next_ordinal*/ 8,
    );
    write_rollout_with_end(
        rollout_path(home.path(), child_id).as_path(),
        child_id,
        Some(history_position(
            root_path.as_path(),
            root_id,
            /*end_ordinal_exclusive*/ 6,
        )),
        /*next_ordinal*/ 3,
    );
    let db = history_db(&store).await;
    for (thread_id, turn_id, ordinal, first_user, final_agent) in [
        (root_id, "root-1", 1, Some("root-user"), Some("root-agent")),
        (root_id, "root-2", 4, None, None),
        (root_id, "excluded-root", 6, None, None),
        (child_id, "child-1", 1, None, None),
    ] {
        insert_turn(
            db,
            thread_id,
            turn_id,
            ordinal,
            "completed",
            /*error_json*/ None,
            first_user,
            final_agent,
        )
        .await;
    }
    for (thread_id, turn_id, item_id, ordinal) in [
        (root_id, "root-1", "root-user", 2),
        (root_id, "root-1", "root-agent", 3),
        (root_id, "root-2", "root-2-item", 5),
        (root_id, "excluded-root", "excluded-item", 7),
        (child_id, "child-1", "child-item", 2),
    ] {
        insert_item(db, thread_id, turn_id, item_id, ordinal).await;
    }

    let first_turns = store
        .list_turns(turn_params(
            child_id,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
            StoredTurnItemsView::Summary,
        ))
        .await
        .expect("first lineage turns page");
    assert_eq!(turn_ids(&first_turns), vec!["root-1", "root-2"]);
    assert_eq!(
        first_turns.turns[0].items,
        vec![
            expected_item("root-1", "root-user", /*rollout_ordinal*/ 2),
            expected_item("root-1", "root-agent", /*rollout_ordinal*/ 3),
        ]
    );
    let second_turns = store
        .list_turns(turn_params(
            child_id,
            first_turns.next_cursor,
            /*page_size*/ 2,
            SortDirection::Asc,
            StoredTurnItemsView::NotLoaded,
        ))
        .await
        .expect("second lineage turns page");
    assert_eq!(turn_ids(&second_turns), vec!["child-1"]);
    let backwards_turns = store
        .list_turns(turn_params(
            child_id,
            second_turns.backwards_cursor,
            /*page_size*/ 2,
            SortDirection::Desc,
            StoredTurnItemsView::NotLoaded,
        ))
        .await
        .expect("backwards lineage turns page");
    assert_eq!(turn_ids(&backwards_turns), vec!["child-1", "root-2"]);

    let first_items = store
        .list_items(item_params(
            child_id,
            /*turn_id*/ None,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect("first lineage items page");
    assert_eq!(item_ids(&first_items), vec!["root-user", "root-agent"]);
    let second_items = store
        .list_items(item_params(
            child_id,
            /*turn_id*/ None,
            first_items.next_cursor,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect("second lineage items page");
    assert_eq!(item_ids(&second_items), vec!["root-2-item", "child-item"]);
    let descending_items = store
        .list_items(item_params(
            child_id,
            /*turn_id*/ None,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Desc,
        ))
        .await
        .expect("descending lineage items page");
    assert_eq!(
        item_ids(&descending_items),
        vec!["child-item", "root-2-item"]
    );
    let inherited_turn_items = store
        .list_items(item_params(
            child_id,
            Some("root-1"),
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect("inherited turn item page");
    assert_eq!(
        item_ids(&inherited_turn_items),
        vec!["root-user", "root-agent"]
    );

    for sort_key in [ItemSortKey::CreatedAtOrdinal, ItemSortKey::UpdatedAtOrdinal] {
        let error = store
            .list_items(ListItemsParams {
                sort_key,
                ..updated_item_params(child_id, /*after_updated_at_ordinal*/ 0)
            })
            .await
            .expect_err("incremental replay should reject forked lineages");
        assert!(matches!(error, ThreadStoreError::InvalidRequest { .. }));
    }

    let (_other_home, other_store, other_thread_id) =
        store_with_mode(ThreadHistoryMode::Paginated).await;
    let error = other_store
        .list_items(item_params(
            other_thread_id,
            /*turn_id*/ None,
            second_items.backwards_cursor,
            /*page_size*/ 2,
            SortDirection::Asc,
        ))
        .await
        .expect_err("lineage cursor belongs to requested thread");
    assert!(matches!(error, ThreadStoreError::InvalidRequest { .. }));
}

#[tokio::test]
async fn lineage_reads_nested_forks() {
    let (home, store, child_id) = store_with_mode(ThreadHistoryMode::Paginated).await;
    let root_id = ThreadId::default();
    let middle_id = ThreadId::default();
    let root_path = rollout_path(home.path(), root_id);
    write_rollout_with_end(
        root_path.as_path(),
        root_id,
        /*history_base*/ None,
        /*next_ordinal*/ 3,
    );
    let middle_path = rollout_path(home.path(), middle_id);
    write_rollout_with_end(
        middle_path.as_path(),
        middle_id,
        Some(history_position(
            root_path.as_path(),
            root_id,
            /*end_ordinal_exclusive*/ 3,
        )),
        /*next_ordinal*/ 2,
    );
    write_rollout_with_end(
        rollout_path(home.path(), child_id).as_path(),
        child_id,
        Some(history_position(
            middle_path.as_path(),
            middle_id,
            /*end_ordinal_exclusive*/ 2,
        )),
        /*next_ordinal*/ 2,
    );
    let db = history_db(&store).await;
    for (thread_id, turn_id) in [
        (root_id, "root"),
        (middle_id, "middle"),
        (child_id, "child"),
    ] {
        insert_turn(
            db,
            thread_id,
            turn_id,
            /*rollout_ordinal*/ 1,
            "completed",
            /*error_json*/ None,
            /*first_user_item_id*/ None,
            /*final_agent_item_id*/ None,
        )
        .await;
    }

    let first_descending_page = store
        .list_turns(turn_params(
            child_id,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Desc,
            StoredTurnItemsView::NotLoaded,
        ))
        .await
        .expect("first nested descending page");
    assert_eq!(turn_ids(&first_descending_page), vec!["child", "middle"]);
    let second_descending_page = store
        .list_turns(turn_params(
            child_id,
            first_descending_page.next_cursor,
            /*page_size*/ 2,
            SortDirection::Desc,
            StoredTurnItemsView::NotLoaded,
        ))
        .await
        .expect("second nested descending page");
    assert_eq!(turn_ids(&second_descending_page), vec!["root"]);
}

async fn store_with_mode(history_mode: ThreadHistoryMode) -> (TempDir, LocalThreadStore, ThreadId) {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let thread_id = ThreadId::default();
    let rollout_path = rollout_path(home.path(), thread_id);
    if history_mode == ThreadHistoryMode::Paginated {
        write_rollout(
            rollout_path.as_path(),
            thread_id,
            /*history_base*/ None,
        );
    }
    let runtime = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("state runtime");
    let mut builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        rollout_path,
        Utc::now(),
        SessionSource::Cli,
    );
    builder.history_mode = history_mode;
    runtime
        .upsert_thread(&builder.build(config.default_model_provider_id.as_str()))
        .await
        .expect("seed thread metadata");
    let store = LocalThreadStore::new(config, Some(runtime));
    (home, store, thread_id)
}

fn write_rollout(
    path: &std::path::Path,
    thread_id: ThreadId,
    history_base: Option<HistoryPosition>,
) {
    write_rollout_with_end(path, thread_id, history_base, /*next_ordinal*/ 1);
}

fn write_rollout_with_end(
    path: &std::path::Path,
    thread_id: ThreadId,
    history_base: Option<HistoryPosition>,
    next_ordinal: u64,
) {
    fs::create_dir_all(path.parent().expect("rollout parent")).expect("create rollout parent");
    let mut lines = vec![RolloutLine {
        timestamp: "2026-07-16T00:00:00.000Z".to_string(),
        ordinal: Some(0),
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                session_id: thread_id.into(),
                id: thread_id,
                history_mode: ThreadHistoryMode::Paginated,
                history_base,
                ..SessionMeta::default()
            },
            git: None,
        }),
    }];
    for ordinal in 1..next_ordinal {
        lines.push(RolloutLine {
            timestamp: "2026-07-16T00:00:00.000Z".to_string(),
            ordinal: Some(ordinal),
            item: RolloutItem::EventMsg(EventMsg::ShutdownComplete),
        });
    }
    fs::write(
        path,
        format!(
            "{}\n",
            lines
                .iter()
                .map(|line| serde_json::to_string(line).expect("serialize rollout"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write rollout");
}

fn rollout_path(home: &std::path::Path, thread_id: ThreadId) -> std::path::PathBuf {
    home.join(format!(
        "sessions/2026/07/16/rollout-2026-07-16T00-00-00-{thread_id}.jsonl"
    ))
}

fn history_position(
    path: &std::path::Path,
    thread_id: ThreadId,
    end_ordinal_exclusive: u64,
) -> HistoryPosition {
    HistoryPosition {
        thread_id,
        end_ordinal_exclusive,
        end_byte_offset: rollout_end_byte_offset(path, end_ordinal_exclusive),
    }
}

fn rollout_end_byte_offset(path: &std::path::Path, end_ordinal_exclusive: u64) -> u64 {
    let line_count = usize::try_from(end_ordinal_exclusive).expect("ordinal fits usize");
    let bytes = fs::read(path).expect("read rollout");
    let end_byte_offset = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .take(line_count)
        .map(<[u8]>::len)
        .sum::<usize>();
    u64::try_from(end_byte_offset).expect("rollout byte offset fits u64")
}

async fn history_db(store: &LocalThreadStore) -> &sqlx::SqlitePool {
    store
        .thread_history_db()
        .await
        .expect("open history fixture database")
}

#[allow(clippy::too_many_arguments)]
async fn insert_turn(
    db: &sqlx::SqlitePool,
    thread_id: ThreadId,
    turn_id: &str,
    rollout_ordinal: i64,
    status: &str,
    error_json: Option<&str>,
    first_user_item_id: Option<&str>,
    final_agent_item_id: Option<&str>,
) {
    sqlx::query(
        r#"
INSERT INTO thread_turns (
    thread_id,
    turn_id,
    rollout_ordinal,
    status,
    error_json,
    first_user_item_id,
    final_agent_item_id
) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(thread_id.to_string())
    .bind(turn_id)
    .bind(rollout_ordinal)
    .bind(status)
    .bind(error_json)
    .bind(first_user_item_id)
    .bind(final_agent_item_id)
    .execute(db)
    .await
    .expect("insert turn fixture");
}

async fn insert_item(
    db: &sqlx::SqlitePool,
    thread_id: ThreadId,
    turn_id: &str,
    item_id: &str,
    rollout_ordinal: i64,
) {
    sqlx::query(
        "INSERT INTO thread_items (thread_id, turn_id, item_id, rollout_ordinal, updated_at_ordinal, created_at_ms, item_json) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(thread_id.to_string())
    .bind(turn_id)
    .bind(item_id)
    .bind(rollout_ordinal)
    .bind(rollout_ordinal)
    .bind(rollout_ordinal * 1_000)
    .bind(format!(r#"{{"type":"userMessage","id":"{item_id}","content":[]}}"#))
    .execute(db)
    .await
    .expect("insert item fixture");
}

fn turn_params(
    thread_id: ThreadId,
    cursor: Option<String>,
    page_size: usize,
    sort_direction: SortDirection,
    items_view: StoredTurnItemsView,
) -> ListTurnsParams {
    ListTurnsParams {
        thread_id,
        include_archived: false,
        cursor,
        page_size,
        sort_direction,
        items_view,
    }
}

fn item_params(
    thread_id: ThreadId,
    turn_id: Option<&str>,
    cursor: Option<String>,
    page_size: usize,
    sort_direction: SortDirection,
) -> ListItemsParams {
    ListItemsParams {
        thread_id,
        turn_id: turn_id.map(str::to_owned),
        include_archived: false,
        cursor,
        page_size,
        sort_direction,
        sort_key: ItemSortKey::CreatedAtOrdinal,
        after_updated_at_ordinal: None,
    }
}

fn updated_item_params(thread_id: ThreadId, after_updated_at_ordinal: u64) -> ListItemsParams {
    ListItemsParams {
        sort_key: ItemSortKey::UpdatedAtOrdinal,
        after_updated_at_ordinal: Some(after_updated_at_ordinal),
        ..item_params(
            thread_id,
            /*turn_id*/ None,
            /*cursor*/ None,
            /*page_size*/ 2,
            SortDirection::Asc,
        )
    }
}

fn expected_item(turn_id: &str, item_id: &str, rollout_ordinal: u64) -> StoredThreadItem {
    StoredThreadItem {
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        updated_at_ordinal: rollout_ordinal,
        created_at_ms: i64::try_from(rollout_ordinal).expect("fixture ordinal fits i64") * 1_000,
        item_json: format!(r#"{{"type":"userMessage","id":"{item_id}","content":[]}}"#)
            .into_bytes(),
    }
}

fn turn_ids(page: &TurnPage) -> Vec<&str> {
    page.turns
        .iter()
        .map(|turn| turn.turn_id.as_str())
        .collect()
}

fn item_ids(page: &ItemPage) -> Vec<&str> {
    page.items
        .iter()
        .map(|item| item.item_id.as_str())
        .collect()
}
