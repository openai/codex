//! Exercises the real SQLite writer, feedback ring buffer, and snapshot ordering.
use super::*;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tracing_subscriber::prelude::*;

#[derive(PartialEq)]
enum Scenario {
    Healthy,
    WriteFailure,
    Corruption,
}

#[tokio::test]
async fn feedback_includes_flush_and_query_failures_in_the_same_submission() -> anyhow::Result<()> {
    for scenario in [
        Scenario::Healthy,
        Scenario::WriteFailure,
        Scenario::Corruption,
    ] {
        let home = tempfile::tempdir()?;
        let sqlite = SqliteConfig::new_for_testing(home.path().abs());
        let runtime = StateRuntime::init(sqlite.clone(), "test-provider".to_string()).await?;
        let pool = sqlite.open_read_write_pool(&sqlite.logs_db_path()).await?;
        let feedback = CodexFeedback::new();
        let writer =
            codex_state::log_db::start(runtime.clone(), std::sync::Arc::new(feedback.clone()));
        let _guard = tracing_subscriber::registry()
            .with(feedback.logger_layer())
            .with(writer.clone())
            .set_default();
        let thread_id = ThreadId::new();
        tracing::info_span!("thread", thread_id = %thread_id)
            .in_scope(|| tracing::info!("persisted log"));
        writer.flush().await;
        match scenario {
            Scenario::WriteFailure => {
                sqlx::query("CREATE TRIGGER fail_log_insert BEFORE INSERT ON logs BEGIN SELECT RAISE(ABORT, 'synthetic-secret'); END").execute(&pool).await?;
            }
            Scenario::Corruption => {
                sqlx::raw_sql("PRAGMA writable_schema = ON; UPDATE sqlite_schema SET rootpage = 2147483647 WHERE name = 'logs'; PRAGMA schema_version = 1000000;").execute(&pool).await?;
            }
            Scenario::Healthy => {}
        }
        let snapshot = feedback.snapshot(Some(thread_id));
        tracing::info_span!("thread", thread_id = %thread_id)
            .in_scope(|| tracing::info!("latest log"));
        let (snapshot, sqlite_logs) = collect_feedback_logs(
            &feedback,
            Some(&writer),
            Some(&runtime),
            snapshot,
            &[thread_id],
        )
        .await;
        assert_eq!(sqlite_logs.is_some(), scenario == Scenario::Healthy);
        let logs = String::from_utf8(snapshot.log_attachment(sqlite_logs).buffer)?;
        assert!(logs.contains("persisted log"));
        assert!(logs.contains("latest log"));
        let expected_error = match scenario {
            Scenario::Healthy => None,
            Scenario::WriteFailure => Some("constraint"),
            Scenario::Corruption => Some("corrupt"),
        };
        assert_eq!(
            logs.matches("failed to flush logs to SQLite").count(),
            usize::from(expected_error.is_some())
        );
        if let Some(error) = expected_error {
            assert!(logs.contains(&format!("failed to flush logs to SQLite error={error:?}")));
        }
        assert_eq!(
            logs.contains("failed to query feedback logs from sqlite"),
            scenario == Scenario::Corruption
        );
        pool.close().await;
        runtime.close().await;
    }
    Ok(())
}
