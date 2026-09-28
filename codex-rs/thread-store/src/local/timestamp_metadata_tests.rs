//! Verifies narrow SQLite timestamp updates preserve metadata and repair fallbacks.

use super::test_support::test_config;
use super::test_support::write_session_file_with_history_mode;
use super::*;
use chrono::Utc;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn timestamp_updates_repair_missing_rows_then_touch_only_timestamp_columns() {
    for history_mode in [ThreadHistoryMode::Legacy, ThreadHistoryMode::Paginated] {
        let home = tempfile::TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("initialize state db");
        let pool = config
            .sqlite
            .open_read_write_pool(&config.sqlite.state_db_path())
            .await
            .expect("open SQL observer");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let id = uuid::Uuid::new_v4();
        let thread_id = ThreadId::from_string(&id.to_string()).expect("thread id");
        let path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            id,
            history_mode,
        )
        .expect("write canonical history");
        let params = UpdateThreadMetadataParams {
            thread_id,
            patch: ThreadMetadataPatch {
                updated_at: Some(Utc::now()),
                ..Default::default()
            },
            include_archived: false,
        };
        // Missing rows must still be reconstructed from their canonical history.
        store
            .update_thread_metadata(params.clone())
            .await
            .expect("reconstruct missing metadata");
        let mut expected = runtime
            .get_thread(thread_id)
            .await
            .expect("read repaired metadata")
            .expect("repaired row");
        assert_eq!(
            (&expected.rollout_path, expected.history_mode),
            (&path, history_mode)
        );
        sqlx::raw_sql(
            r#"
            CREATE TABLE timestamp_writes(kind TEXT);
            CREATE TRIGGER count_timestamp AFTER UPDATE OF updated_at_ms ON threads
            BEGIN INSERT INTO timestamp_writes VALUES ('timestamp'); END;
            CREATE TRIGGER count_full_row AFTER UPDATE OF title ON threads
            BEGIN INSERT INTO timestamp_writes VALUES ('full'); END;
            "#,
        )
        .execute(&pool)
        .await
        .expect("install write counters");

        store
            .update_thread_metadata(params.clone())
            .await
            .expect("touch existing metadata");
        let actual = runtime
            .get_thread(thread_id)
            .await
            .expect("read touched metadata")
            .expect("touched row");
        assert!(actual.updated_at > expected.updated_at);
        expected.updated_at = actual.updated_at;
        assert_eq!(actual, expected);
        let writes: Vec<String> =
            sqlx::query_scalar("SELECT kind FROM timestamp_writes ORDER BY rowid")
                .fetch_all(&pool)
                .await
                .expect("read write counters");
        assert_eq!(writes, vec!["timestamp"]);

        store
            .stage_pending_thread_metadata(
                thread_id,
                ThreadMetadataPatch {
                    model: Some("staged-model".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("stage metadata");
        store
            .update_thread_metadata(params)
            .await
            .expect("apply staged metadata");
        let actual = runtime
            .get_thread(thread_id)
            .await
            .expect("read staged metadata")
            .expect("updated row");
        expected.model = Some("staged-model".to_string());
        expected.updated_at = actual.updated_at;
        assert_eq!(actual, expected);
        assert!(
            store
                .pending_thread_metadata
                .lock(thread_id)
                .await
                .is_none()
        );
        let mut writes: Vec<String> = sqlx::query_scalar("SELECT kind FROM timestamp_writes")
            .fetch_all(&pool)
            .await
            .expect("read write counters");
        writes.sort();
        assert_eq!(writes, vec!["full", "timestamp", "timestamp"]);
        pool.close().await;
    }
}
