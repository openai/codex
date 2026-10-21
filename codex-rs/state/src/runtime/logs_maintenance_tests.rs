//! Exercise age/size pruning and automatic cleanup of an idle runtime.

use super::LOG_RETENTION_SECONDS;
use super::StateRuntime;
use super::prune_by_age_and_size;
use crate::SqliteConfig;
use crate::migrations::LOGS_MIGRATOR;
use chrono::Utc;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::time::Duration;

const NOW: i64 = 2_000_000_000;
const DAY: i64 = 24 * 60 * 60;

async fn database_with_logs(ages: &[i64]) -> (SqlitePool, PathBuf) {
    let directory = super::super::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let pool = SqliteConfig::new_for_testing(directory.as_path().abs())
        .open_read_write_pool(&directory.join("logs.sqlite"))
        .await
        .unwrap();
    LOGS_MIGRATOR.run(&pool).await.unwrap();
    for age in ages {
        sqlx::query(
            "INSERT INTO logs (ts, ts_nanos, level, target, feedback_log_body) \
             VALUES (?, 0, 'INFO', 'retention-test', ?)",
        )
        .bind(NOW - age)
        .bind("x".repeat(128 * 1024))
        .execute(&pool)
        .await
        .unwrap();
    }
    (pool, directory)
}

async fn retained_ages(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT ? - ts FROM logs ORDER BY ts DESC")
        .bind(NOW)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn keeps_ten_day_boundary_when_under_budget() {
    let (pool, directory) =
        database_with_logs(&[0, LOG_RETENTION_SECONDS, LOG_RETENTION_SECONDS + 1]).await;
    prune_by_age_and_size(&pool, NOW, /*budget_bytes*/ 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(retained_ages(&pool).await, vec![0, LOG_RETENTION_SECONDS]);
    pool.close().await;
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

#[tokio::test]
async fn halves_again_when_first_halving_deletes_nothing() {
    let (pool, directory) = database_with_logs(&[DAY, 2 * DAY, 4 * DAY]).await;
    // Three payloads exceed this budget; two fit, including table/index pages.
    prune_by_age_and_size(&pool, NOW, /*budget_bytes*/ 350 * 1024)
        .await
        .unwrap();
    assert_eq!(retained_ages(&pool).await, vec![DAY, 2 * DAY]);
    // A second cleanup must ignore free pages left in the file.
    let allocated: i64 = sqlx::query_scalar(
        "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(allocated > 350 * 1024);
    prune_by_age_and_size(&pool, NOW, /*budget_bytes*/ 350 * 1024)
        .await
        .unwrap();
    assert_eq!(retained_ages(&pool).await, vec![DAY, 2 * DAY]);
    pool.close().await;
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

#[tokio::test]
async fn stops_at_one_second_when_recent_rows_exceed_budget() {
    let (pool, directory) = database_with_logs(&[-1, 0, 1, 2, DAY]).await;
    prune_by_age_and_size(&pool, NOW, /*budget_bytes*/ 1)
        .await
        .unwrap();
    assert_eq!(retained_ages(&pool).await, vec![-1, 0, 1]);
    pool.close().await;
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

#[tokio::test]
async fn periodic_cleanup_prunes_without_new_log_writes_or_restart() {
    let directory = super::super::test_support::unique_temp_dir();
    let sqlite = SqliteConfig::new_for_testing(directory.as_path().abs());
    let runtime = StateRuntime::init(
        sqlite.clone().with_local_logs_home(directory.join("log")),
        "test-provider".to_string(),
    )
    .await
    .unwrap();
    // An older process can create the legacy database after the new runtime starts.
    let legacy_pool = sqlite
        .open_logs_db(&LOGS_MIGRATOR, /*telemetry_override*/ None)
        .await
        .unwrap();
    runtime.start_periodic_logs_maintenance(Duration::from_millis(10));
    let now = Utc::now().timestamp();
    for round in 1..=2 {
        for pool in [runtime.logs_pool.as_ref(), &legacy_pool] {
            for timestamp in [now - 11 * DAY, now] {
                sqlx::query(
                    "INSERT INTO logs (ts, ts_nanos, level, target) VALUES (?, 0, 'INFO', 'test')",
                )
                .bind(timestamp)
                .execute(pool)
                .await
                .unwrap();
            }
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let current: Vec<i64> = sqlx::query_scalar("SELECT ts FROM logs ORDER BY ts")
                    .fetch_all(runtime.logs_pool.as_ref())
                    .await
                    .unwrap();
                let legacy: Vec<i64> = sqlx::query_scalar("SELECT ts FROM logs ORDER BY ts")
                    .fetch_all(&legacy_pool)
                    .await
                    .unwrap();
                if (current, legacy) == (vec![now; round], vec![now; round]) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("periodic cleanup should remove expired logs from both locations while idle");
    }
    legacy_pool.close().await;
    runtime.close().await;
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

#[tokio::test]
async fn legacy_cleanup_skips_missing_and_shared_databases() -> anyhow::Result<()> {
    let directory = super::super::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&directory).await?;
    let sqlite = SqliteConfig::new_for_testing(directory.as_path().abs());
    let legacy_path = sqlite.logs_db_path();
    for logs_home in [directory.clone(), directory.join("log")] {
        let sqlite = sqlite.clone().with_local_logs_home(logs_home);
        assert!(sqlite.open_legacy_logs_for_cleanup().await?.is_none());
        assert!(!legacy_path.exists());
    }
    let pool = sqlite
        .open_logs_db(&LOGS_MIGRATOR, /*telemetry_override*/ None)
        .await?;
    assert!(sqlite.open_legacy_logs_for_cleanup().await?.is_none());
    pool.close().await;
    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}

#[tokio::test]
async fn legacy_writer_does_not_block_current_cleanup_and_is_retried() -> anyhow::Result<()> {
    let directory = super::super::test_support::unique_temp_dir();
    tokio::fs::create_dir_all(&directory).await?;
    let sqlite = SqliteConfig::new_for_testing(directory.as_path().abs());
    let legacy_pool = sqlite
        .open_logs_db(&LOGS_MIGRATOR, /*telemetry_override*/ None)
        .await?;
    sqlx::query("INSERT INTO logs (ts, ts_nanos, level, target) VALUES (0, 0, 'INFO', 'test')")
        .execute(&legacy_pool)
        .await?;
    let writer = legacy_pool.begin_with("BEGIN IMMEDIATE").await?;
    let runtime = StateRuntime::init(
        sqlite.with_local_logs_home(directory.join("log")),
        "test-provider".to_string(),
    )
    .await?;
    sqlx::query("INSERT INTO logs (ts, ts_nanos, level, target) VALUES (0, 0, 'INFO', 'test')")
        .execute(runtime.logs_pool.as_ref())
        .await?;
    runtime.start_periodic_logs_maintenance(Duration::from_millis(10));
    tokio::time::timeout(Duration::from_secs(5), async {
        while sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs")
            .fetch_one(runtime.logs_pool.as_ref())
            .await
            .unwrap()
            != 0
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs")
            .fetch_one(&legacy_pool)
            .await?,
        1
    );
    writer.rollback().await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs")
            .fetch_one(&legacy_pool)
            .await
            .unwrap()
            != 0
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    legacy_pool.close().await;
    runtime.close().await;
    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}

#[tokio::test]
async fn startup_cleanup_runs_without_waiting_for_the_period() {
    for separate_logs in [false, true] {
        let directory = super::super::test_support::unique_temp_dir();
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let sqlite = SqliteConfig::new_for_testing(directory.as_path().abs());
        let legacy_path = sqlite.logs_db_path();
        let legacy_pool = sqlite
            .open_logs_db(&LOGS_MIGRATOR, /*telemetry_override*/ None)
            .await
            .unwrap();
        let sqlite = if separate_logs {
            sqlite.with_local_logs_home(directory.join("log"))
        } else {
            sqlite
        };
        let pool = sqlite
            .open_logs_db(&LOGS_MIGRATOR, /*telemetry_override*/ None)
            .await
            .unwrap();
        for pool in [&pool, &legacy_pool] {
            sqlx::query(
                "INSERT INTO logs (ts, ts_nanos, level, target) VALUES (0, 0, 'INFO', 'test')",
            )
            .execute(pool)
            .await
            .unwrap();
        }
        pool.close().await;
        legacy_pool.close().await;

        let runtime = StateRuntime::init(sqlite, "test-provider".to_string())
            .await
            .unwrap();
        let legacy_pool = runtime
            .sqlite
            .open_read_only_pool(&legacy_path, /*busy_timeout*/ None)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let current: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
                    .fetch_one(runtime.logs_pool.as_ref())
                    .await
                    .unwrap();
                let legacy: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
                    .fetch_one(&legacy_pool)
                    .await
                    .unwrap();
                if (current, legacy) == (0, 0) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("startup cleanup should prune both locations before the 30-minute interval");
        legacy_pool.close().await;
        runtime.close().await;
        tokio::fs::remove_dir_all(directory).await.unwrap();
    }
}
