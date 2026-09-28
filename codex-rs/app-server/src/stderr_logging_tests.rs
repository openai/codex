//! Regressions for SQLite progress and stderr span diagnostics.

use crate::LogFormat;
use crate::StderrLogLayer;
use crate::stderr_span_events;
use anyhow::Context;
use anyhow::Result;
use codex_state::SqliteConfig;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

#[test_case(LogFormat::Json; "json")]
#[test_case(LogFormat::Default; "text")]
#[tokio::test]
async fn sqlite_transaction_completes_with_undrained_stderr(format: LogFormat) -> Result<()> {
    let dir = TempDir::new()?;
    let config = SqliteConfig::new_for_testing(dir.path().abs());
    let path = dir.path().join("logging.sqlite");
    let writer_pool = config.open_read_write_pool(&path).await?;
    sqlx::query("CREATE TABLE writes (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)")
        .execute(&writer_pool)
        .await?;
    // Open both pools before tracing so this isolates logging during a transaction.
    let contender_pool = config.open_read_write_pool(&path).await?;
    let (mut reader, writer) = std::io::pipe()?;
    let fmt = tracing_subscriber::fmt::layer()
        .with_writer(Mutex::new(writer))
        .with_span_events(stderr_span_events());
    let filter = EnvFilter::new("off,codex_app_server::stderr_logging_tests=info");
    let fmt: StderrLogLayer = match format {
        LogFormat::Json => fmt.json().with_filter(filter).boxed(),
        LogFormat::Default => fmt.with_filter(filter).boxed(),
    };
    let subscriber = tracing_subscriber::registry().with(fmt);
    let (locked_tx, locked_rx) = oneshot::channel();
    let (finished_tx, finished_rx) = oneshot::channel();

    // A synchronous stderr write can block a runtime thread as well as SQLx's
    // worker. Keep the timeout on a separate thread so regressions cannot hang it.
    let worker = std::thread::spawn(move || {
        let result = tracing::subscriber::with_default(subscriber, || -> Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let span = tracing::info_span!("sqlite_transaction");
            span.in_scope(|| {
                runtime.block_on(async {
                    let mut transaction = writer_pool.begin().await?;
                    sqlx::query("INSERT INTO writes VALUES (1, 0)")
                        .execute(&mut *transaction)
                        .await?;
                    let _ = locked_tx.send(());
                    // Enough commands to fill an unread OS pipe if every entry
                    // and exit of the caller's span produces a log record.
                    for _ in 0..4096 {
                        sqlx::query("UPDATE writes SET value = value + 1 WHERE id = 1")
                            .execute(&mut *transaction)
                            .await?;
                    }
                    transaction.commit().await?;
                    tracing::info!("transaction committed");
                    tracing::warn!("explicit warning");
                    writer_pool.close().await;
                    Ok(())
                })
            })
        });
        let _ = finished_tx.send(result);
    });

    let progress = timeout(Duration::from_secs(/*secs*/ 15), async {
        locked_rx.await?;
        let write = async {
            sqlx::query("INSERT INTO writes VALUES (2, 0)")
                .execute(&contender_pool)
                .await
                .context("competing writer could not acquire the SQLite lock")
        };
        tokio::try_join!(write, async { finished_rx.await? })?;
        Ok::<_, anyhow::Error>(())
    })
    .await;

    // Always drain before reporting a failure, releasing a worker blocked in
    // tracing if enter/exit logging is accidentally restored.
    let logs = tokio::task::spawn_blocking(move || {
        let mut logs = String::new();
        reader.read_to_string(&mut logs)?;
        Ok::<_, std::io::Error>(logs)
    })
    .await??;
    worker.join().expect("SQLite worker panicked");
    progress.context("SQLite transaction stalled with undrained stderr")??;
    let rows: Vec<(i64, i64)> = sqlx::query_as("SELECT id, value FROM writes ORDER BY id")
        .fetch_all(&contender_pool)
        .await?;
    contender_pool.close().await;
    assert_eq!(rows, vec![(1, 4096), (2, 0)]);

    let lines: Vec<_> = logs.lines().collect();
    assert_eq!(lines.len(), 4, "{logs}");
    for (line, message) in
        lines
            .iter()
            .zip(["new", "transaction committed", "explicit warning", "close"])
    {
        assert!(line.contains(message), "{logs}");
    }
    assert!(lines[2].contains("WARN"), "{logs}");
    for field in ["time.busy", "time.idle"] {
        assert!(lines[3].contains(field), "{logs}");
    }
    Ok(())
}
