//! Regression coverage for flush diagnostics, recovery, and sink recursion prevention.

use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use super::tests::SharedWriter;
use super::*;

#[tokio::test]
async fn failed_batch_is_reported_once_and_later_writes_recover() {
    let codex_home =
        std::env::temp_dir().join(format!("codex-state-log-failure-{}", Uuid::new_v4()));
    let _cleanup = scopeguard::guard(codex_home.clone(), |codex_home| {
        let _ = std::fs::remove_dir_all(codex_home);
    });
    let sqlite = crate::SqliteConfig::new_for_testing(codex_home.as_path().abs());
    let runtime = StateRuntime::init(sqlite.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");
    let pool = sqlite
        .open_read_write_pool(&sqlite.logs_db_path())
        .await
        .expect("open logs database");
    sqlx::query("CREATE TRIGGER fail_log_insert BEFORE INSERT ON logs BEGIN SELECT RAISE(ABORT, 'synthetic-secret'); END")
        .execute(&pool)
        .await
        .expect("install failure trigger");
    let diagnostics = SharedWriter::default();
    let layer = start(runtime.clone(), Arc::new(diagnostics.clone()));
    assert_eq!(layer.has_write_failure(), false);
    let writer = SharedWriter::default();
    let guard = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer.clone())
                .without_time()
                .with_level(/*display_level*/ false)
                .with_target(/*display_target*/ false)
                .with_ansi(/*ansi*/ false)
                .with_filter(LevelFilter::ERROR),
        )
        // No diagnostic filter is needed: the diagnostic writer bypasses tracing.
        .with(layer.clone())
        .set_default();

    tracing::info!("sensitive log body");
    layer.flush().await;
    assert_eq!(layer.has_write_failure(), true);
    let diagnostic = diagnostics.snapshot();
    assert_eq!(
        diagnostic.split_once(" ERROR ").unwrap().1,
        "failed to flush logs to SQLite error=\"constraint\" entries=1\n"
    );
    assert_eq!(writer.snapshot(), "");

    // Reporting the first failure must not enqueue a second failing batch.
    layer.flush().await;
    assert_eq!(diagnostics.snapshot(), diagnostic);
    assert_eq!(writer.snapshot(), "");

    sqlx::query("DROP TRIGGER fail_log_insert")
        .execute(&pool)
        .await
        .expect("remove failure trigger");
    tracing::info!("recovered log");
    layer.flush().await;
    assert_eq!(diagnostics.snapshot(), diagnostic);
    assert_eq!(writer.snapshot(), "");
    assert_eq!(layer.has_write_failure(), true);
    drop(guard);

    let logs = runtime
        .query_logs(&crate::LogQuery::default())
        .await
        .expect("query logs");
    assert_eq!(
        logs.iter()
            .map(|log| log.message.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("recovered log")]
    );
    pool.close().await;
    runtime.close().await;
}
