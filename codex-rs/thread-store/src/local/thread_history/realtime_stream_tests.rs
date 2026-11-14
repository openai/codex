#![cfg(test)]

//! Verifies snapshot reads, late seals, and projection coverage against SQLite.

use super::*;
use codex_protocol::realtime::RealtimeItemContent;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

async fn database() -> (TempDir, sqlx::SqlitePool) {
    let home = TempDir::new().unwrap();
    let config = crate::local::test_support::test_config(home.path());
    let pool = codex_state::open_thread_history_db(&config.sqlite)
        .await
        .unwrap();
    (home, pool)
}

fn params(thread_id: ThreadId) -> ListRealtimeParams {
    ListRealtimeParams {
        thread_id,
        session_id: None,
        from_observed_at_ms: None,
        to_observed_at_ms: None,
        order: ThreadRealtimeOrder::ObservedAt,
        after_position: None,
        event_id: None,
        cursor: None,
        page_size: 1,
        snapshot_end: None,
        required_through_position: None,
    }
}

async fn insert(pool: &sqlx::SqlitePool, thread_id: ThreadId, position: i64, time: Option<i64>) {
    let item = RealtimeItem {
        id: format!("event-{position}"),
        realtime_session_id: "voice".to_owned(),
        observed_at_ms: time,
        content: RealtimeItemContent::RealtimeSessionStarted,
    };
    sqlx::query("INSERT INTO thread_realtime_items (thread_id, item_id, rollout_ordinal, created_at_ms, item_type, item_json) VALUES (?, ?, ?, 0, 'realtime_session_started', ?)")
        .bind(thread_id.to_string()).bind(&item.id).bind(position)
        .bind(serde_json::to_string(&item).unwrap()).execute(pool).await.unwrap();
}

#[tokio::test]
async fn snapshot_stays_fixed_while_position_catchup_finds_a_late_seal() {
    let (_home, pool) = database().await;
    let thread = ThreadId::default();
    insert(&pool, thread, /*position*/ 0, Some(200)).await;
    insert(&pool, thread, /*position*/ 1, Some(200)).await;
    insert(&pool, thread, /*position*/ 2, Some(100)).await;
    let first = read_page(
        &pool,
        params(thread),
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 3,
    )
    .await
    .unwrap();
    assert_eq!(first.data[0].position, 2);
    assert_eq!(first.snapshot_end, 3);

    insert(&pool, thread, /*position*/ 3, Some(50)).await;
    let next = read_page(
        &pool,
        ListRealtimeParams {
            cursor: first.next_cursor.clone(),
            ..params(thread)
        },
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 4,
    )
    .await
    .unwrap();
    assert_eq!(
        next.data
            .iter()
            .map(|entry| entry.position)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(next.snapshot_end, 3);
    // Equal observation times on opposite sides of a page boundary use rollout position.
    let last = read_page(
        &pool,
        ListRealtimeParams {
            cursor: next.next_cursor,
            ..params(thread)
        },
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 4,
    )
    .await
    .unwrap();
    assert_eq!(
        last.data
            .iter()
            .map(|entry| entry.position)
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(last.next_cursor.is_none());

    let catchup = read_page(
        &pool,
        ListRealtimeParams {
            order: ThreadRealtimeOrder::Position,
            after_position: Some(first.snapshot_end - 1),
            ..params(thread)
        },
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 4,
    )
    .await
    .unwrap();
    assert_eq!(catchup.data[0].position, 3);

    let error = read_page(
        &pool,
        ListRealtimeParams {
            cursor: first.next_cursor,
            session_id: Some("another-voice".into()),
            ..params(thread)
        },
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 4,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, ThreadStoreError::InvalidRequest { .. }));
}

#[tokio::test]
async fn projection_lag_is_explicit_and_descendants_hide_parent_positions() {
    let (_home, pool) = database().await;
    let thread = ThreadId::default();
    insert(&pool, thread, /*position*/ 1, Some(100)).await;
    let pending = read_page(
        &pool,
        ListRealtimeParams {
            required_through_position: Some(2),
            ..params(thread)
        },
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 2,
    )
    .await
    .unwrap();
    assert_eq!(pending.coverage, ThreadRealtimeCoverage::Pending);
    assert!(pending.data.is_empty());
    let hidden = read_page(
        &pool,
        ListRealtimeParams {
            order: ThreadRealtimeOrder::Position,
            ..params(thread)
        },
        thread,
        /*first_position*/ 2,
        /*projected_end*/ 2,
    )
    .await
    .unwrap();
    assert!(hidden.data.is_empty());
}

#[tokio::test]
async fn exact_event_id_waits_for_the_acknowledged_seal_to_be_projected() {
    let (_home, pool) = database().await;
    let thread = ThreadId::default();
    let query = ListRealtimeParams {
        event_id: Some("event-1".into()),
        ..params(thread)
    };
    let pending = read_page(
        &pool,
        query.clone(),
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 1,
    )
    .await
    .unwrap();
    assert_eq!(
        pending,
        empty_page(
            ThreadRealtimeCoverage::Pending,
            /*snapshot_end*/ 1,
            /*projected_through_item_index*/ 1
        )
    );

    insert(&pool, thread, /*position*/ 1, Some(100)).await;
    let stale = read_page(
        &pool,
        query.clone(),
        thread,
        /*first_position*/ 0,
        /*projected_end*/ 1,
    )
    .await
    .unwrap();
    assert_eq!(
        stale,
        empty_page(
            ThreadRealtimeCoverage::Pending,
            /*snapshot_end*/ 1,
            /*projected_through_item_index*/ 1
        )
    );
    for excluded in [
        ListRealtimeParams {
            snapshot_end: Some(1),
            ..query.clone()
        },
        ListRealtimeParams {
            from_observed_at_ms: Some(200),
            ..query.clone()
        },
    ] {
        let page = read_page(
            &pool, excluded, thread, /*first_position*/ 0, /*projected_end*/ 1,
        )
        .await
        .unwrap();
        assert_eq!(
            page,
            empty_page(
                ThreadRealtimeCoverage::Ready,
                /*snapshot_end*/ 1,
                /*projected_through_item_index*/ 1
            )
        );
    }
    let ready = read_page(
        &pool, query, thread, /*first_position*/ 0, /*projected_end*/ 2,
    )
    .await
    .unwrap();
    assert_eq!(ready.coverage, ThreadRealtimeCoverage::Ready);
    assert_eq!(ready.data[0].position, 1);
    assert_eq!(ready.snapshot_end, 2);
}
