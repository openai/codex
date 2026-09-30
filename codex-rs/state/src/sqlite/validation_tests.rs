//! Exercises startup validation, per-file caching, and report-only lazy databases.

use super::QuickCheckOutcome;
use super::SqliteQuickCheckManager;
use super::quick_check;
use crate::SqliteConfig;
use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use anyhow::Context;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Eq, PartialEq)]
struct CorruptionEvent {
    count: i64,
    tags: BTreeMap<String, String>,
}

#[derive(Default)]
struct CorruptionTelemetry(Mutex<Vec<CorruptionEvent>>);

impl crate::DbTelemetry for CorruptionTelemetry {
    fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) {
        if name == crate::DB_CORRUPTION_METRIC {
            self.0
                .lock()
                .expect("telemetry lock")
                .push(CorruptionEvent {
                    count: inc,
                    tags: tags
                        .iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                });
        }
    }

    fn histogram(&self, _name: &str, _value: i64, _tags: &[(&str, &str)]) {}

    fn record_duration(&self, _name: &str, _duration: Duration, _tags: &[(&str, &str)]) {}
}

#[tokio::test]
async fn runtime_opens_recover_or_report_corruption_by_database_policy() -> anyhow::Result<()> {
    let root = unique_temp_dir();
    let _cleanup = scopeguard::guard(root.clone(), |root| {
        let _ = std::fs::remove_dir_all(root);
    });
    for spec in super::super::RUNTIME_DBS {
        let home = root.join(spec.label);
        tokio::fs::create_dir_all(&home).await?;
        let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
        let path = spec.path(&home);
        let fixture = path.with_extension("fixture");
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&fixture)
                .create_if_missing(true),
        )
        .await?;
        sqlx::raw_sql(
            "CREATE TABLE sample(value INTEGER);
             INSERT INTO sample VALUES (NULL);
             PRAGMA writable_schema=ON;
             UPDATE sqlite_schema SET sql='CREATE TABLE sample(value INTEGER NOT NULL)' WHERE name='sample';
             PRAGMA writable_schema=OFF;",
        )
        .execute(&pool)
        .await?;
        pool.close().await;
        tokio::fs::rename(&fixture, &path).await?;

        if path == sqlite.thread_history_db_path() {
            let telemetry = CorruptionTelemetry::default();
            // Unrelated corruption must not disable lazy history reads when the
            // caller has no recovery path. Exercise both migration and reopening.
            for _ in 0..2 {
                let pool = sqlite
                    .open_thread_history_db(
                        &crate::migrations::runtime_thread_history_migrator(),
                        Some(&telemetry),
                    )
                    .await?;
                assert_eq!(
                    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM thread_turns")
                        .fetch_one(&pool)
                        .await?,
                    0
                );
                assert_eq!(
                    quick_check(&pool, Instant::now() + Duration::from_secs(/*secs*/ 5)).await?,
                    QuickCheckOutcome::CorruptedNeedsFixed
                );
                pool.close().await;
            }
            assert_eq!(
                *telemetry.0.lock().expect("telemetry lock"),
                vec![CorruptionEvent {
                    count: 1,
                    tags: BTreeMap::from([("db".to_string(), "thread_history".to_string())]),
                }]
            );
            continue;
        }

        // Recovery happens inside the database opener, without a CLI or app server.
        for _ in 0..2 {
            let runtime = StateRuntime::init(sqlite.clone(), "openai".to_string()).await?;
            runtime.close().await;
        }
        let backups = std::fs::read_dir(home.join("db-backups"))
            .with_context(|| format!("{} corruption did not create a backup", spec.label))?
            .collect::<std::io::Result<Vec<_>>>()?;
        assert_eq!(backups.len(), 1);
        let backup_path = backups[0]
            .path()
            .join(path.file_name().expect("database filename"));
        let pool = sqlite
            .open_read_only_pool(&backup_path, /*busy_timeout*/ None)
            .await?;
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA quick_check(1)")
                .fetch_one(&pool)
                .await?,
            "NULL value in sample.value"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT name FROM sqlite_schema WHERE type = 'table'")
                .fetch_all(&pool)
                .await?,
            vec!["sample".to_string()]
        );
        pool.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn quick_check_failure_is_recovered_before_runtime_init_returns() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    let _cleanup = scopeguard::guard(home.clone(), |home| {
        let _ = std::fs::remove_dir_all(home);
    });
    let telemetry = CorruptionTelemetry::default();
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let runtime = StateRuntime::init_with_telemetry_for_tests(
        sqlite.clone(),
        "openai".to_string(),
        &telemetry,
    )
    .await?;
    runtime.close().await;
    let state_pool = sqlite.open_read_write_pool(&sqlite.state_db_path()).await?;
    sqlx::query("CREATE TABLE preserved(value); INSERT INTO preserved VALUES ('keep me')")
        .execute(&state_pool)
        .await?;
    state_pool.close().await;
    let logs_path = sqlite.logs_db_path();
    sqlite
        .open_logs_db(
            &crate::migrations::runtime_logs_migrator(),
            /*telemetry_override*/ None,
        )
        .await?
        .close()
        .await;
    let fixture = logs_path.with_extension("fixture");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&fixture)
            .create_if_missing(true),
    )
    .await?;
    sqlx::raw_sql(
        "CREATE TABLE sample(value INTEGER);
         INSERT INTO sample VALUES (NULL);
         PRAGMA writable_schema=ON;
         UPDATE sqlite_schema SET sql='CREATE TABLE sample(value INTEGER NOT NULL)' WHERE name='sample';
         PRAGMA writable_schema=OFF;",
    )
    .execute(&pool)
    .await?;
    pool.close().await;
    // Replacing an already-validated file must trigger a new check at the same path.
    tokio::fs::remove_file(&logs_path).await?;
    tokio::fs::rename(&fixture, &logs_path).await?;
    let runtime = StateRuntime::init_with_telemetry_for_tests(
        sqlite.clone(),
        "openai".to_string(),
        &telemetry,
    )
    .await?;
    runtime.close().await;
    let runtime = StateRuntime::init_with_telemetry_for_tests(
        sqlite.clone(),
        "openai".to_string(),
        &telemetry,
    )
    .await?;
    runtime.close().await;
    assert_eq!(
        *telemetry.0.lock().expect("telemetry lock"),
        vec![CorruptionEvent {
            count: 1,
            tags: BTreeMap::from([("db".to_string(), "logs".to_string())]),
        }]
    );
    let backups =
        std::fs::read_dir(home.join("db-backups"))?.collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(backups.len(), 1);
    let backup_path = backups[0]
        .path()
        .join(logs_path.file_name().expect("logs filename"));
    let backup_pool = sqlite
        .open_read_only_pool(&backup_path, /*busy_timeout*/ None)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>("SELECT value FROM sample")
            .fetch_all(&backup_pool)
            .await?,
        vec![None],
    );
    backup_pool.close().await;
    let pool = sqlite.open_read_write_pool(&sqlite.state_db_path()).await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM preserved")
            .fetch_all(&pool)
            .await?,
        vec!["keep me".to_string()]
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn repeated_pool_opens_share_completed_and_incomplete_attempts() -> anyhow::Result<()> {
    for budget in [Duration::from_secs(/*secs*/ 5), Duration::ZERO] {
        let home = unique_temp_dir();
        tokio::fs::create_dir_all(&home).await?;
        let _cleanup = scopeguard::guard(home.clone(), |home| {
            let _ = std::fs::remove_dir_all(home);
        });
        let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
        let fixture = home.join("fixture.sqlite");
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&fixture)
                .create_if_missing(true),
        )
        .await?;
        sqlx::raw_sql(
            "CREATE TABLE sample(value INTEGER);
             WITH RECURSIVE rows(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM rows WHERE id < 2048)
             INSERT INTO sample SELECT id FROM rows;",
        )
        .execute(&pool)
        .await?;
        pool.close().await;
        let path = sqlite.logs_db_path();
        tokio::fs::rename(&fixture, &path).await?;
        let pool = sqlite
            .open_read_only_pool(&path, /*busy_timeout*/ None)
            .await?;
        sqlite
            .quick_check_manager
            .quick_check_once(&pool, &path, budget)
            .await?;
        pool.close().await;

        let migrator = crate::migrations::runtime_logs_migrator();
        let pool = sqlite
            .open_logs_db(&migrator, /*telemetry_override*/ None)
            .await?;
        sqlx::raw_sql(
            "UPDATE sample SET value=NULL WHERE value=1;
             PRAGMA writable_schema=ON;
             UPDATE sqlite_schema SET sql='CREATE TABLE sample(value INTEGER NOT NULL)' WHERE name='sample';
             PRAGMA writable_schema=OFF;",
        )
        .execute(&pool)
        .await?;
        pool.close().await;
        // Corruption introduced after the first attempt exposes any repeated scan.
        let cloned_sqlite = sqlite.clone();
        let (first, second) = tokio::join!(
            sqlite.open_logs_db(&migrator, /*telemetry_override*/ None),
            cloned_sqlite.open_logs_db(&migrator, /*telemetry_override*/ None),
        );
        let first = first?;
        let second = second?;
        assert_eq!(
            quick_check(&first, Instant::now() + Duration::from_secs(/*secs*/ 5)).await?,
            QuickCheckOutcome::CorruptedNeedsFixed,
            "an uncached check must still see the corruption"
        );
        first.close().await;
        second.close().await;

        // A separately constructed configuration must check and recover the file.
        let independent_sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
        let repaired = independent_sqlite
            .open_logs_db(&migrator, /*telemetry_override*/ None)
            .await?;
        assert_eq!(
            quick_check(&repaired, Instant::now() + Duration::from_secs(/*secs*/ 5)).await?,
            QuickCheckOutcome::Complete
        );
        assert!(home.join("db-backups").is_dir());
        repaired.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn concurrent_first_checks_share_an_in_flight_attempt() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    tokio::fs::create_dir_all(&home).await?;
    let _cleanup = scopeguard::guard(home.clone(), |home| {
        let _ = std::fs::remove_dir_all(home);
    });
    let validation = SqliteQuickCheckManager::new();
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let path = sqlite.logs_db_path();
    sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await?
    .close()
    .await;
    let path = tokio::fs::canonicalize(path).await?;
    let first = sqlite
        .open_read_only_pool(&path, /*busy_timeout*/ None)
        .await?;
    let second = sqlite
        .open_read_only_pool(&path, /*busy_timeout*/ None)
        .await?;
    // A second scan would fail on this pool instead of silently passing twice.
    second.close().await;
    let mut connection = first.acquire().await?;
    let id = file_id::get_file_id(&path)?;
    let check = tokio::spawn({
        let validation = validation.clone();
        let pool = first.clone();
        let path = path.clone();
        async move {
            validation
                .quick_check_once(&pool, &path, Duration::from_secs(/*secs*/ 5))
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        while !validation.0.lock().await.contains(&id) {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    // The concurrent caller skips immediately, while the first check is blocked.
    assert_eq!(
        validation
            .quick_check_once(&second, &path, Duration::from_secs(/*secs*/ 5))
            .await?,
        QuickCheckOutcome::Skipped
    );
    connection.return_to_pool().await;
    assert_eq!(check.await??, QuickCheckOutcome::Complete);
    first.close().await;
    Ok(())
}

#[tokio::test]
async fn cancelled_check_counts_as_an_attempt() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    tokio::fs::create_dir_all(&home).await?;
    let _cleanup = scopeguard::guard(home.clone(), |home| {
        let _ = std::fs::remove_dir_all(home);
    });
    let validation = SqliteQuickCheckManager::new();
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let path = sqlite.logs_db_path();
    sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await?
    .close()
    .await;
    let id = file_id::get_file_id(&path)?;
    let pool = sqlite
        .open_read_only_pool(&path, /*busy_timeout*/ None)
        .await?;
    // Occupy the sole connection so cancellation happens during initialization.
    let mut connection = pool.acquire().await?;
    let check = tokio::spawn({
        let validation = validation.clone();
        let pool = pool.clone();
        let path = path.clone();
        async move {
            validation
                .quick_check_once(&pool, &path, Duration::from_secs(/*secs*/ 5))
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            if validation.0.lock().await.contains(&id) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    check.abort();
    assert!(
        check
            .await
            .expect_err("check should be cancelled")
            .is_cancelled()
    );
    connection.return_to_pool().await;
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(/*secs*/ 5),
            validation.quick_check_once(&pool, &path, Duration::from_secs(/*secs*/ 5)),
        )
        .await??,
        QuickCheckOutcome::Skipped
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn corruption_attempts_are_cached_per_owner() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    tokio::fs::create_dir_all(&home).await?;
    let _cleanup = scopeguard::guard(home.clone(), |home| {
        let _ = std::fs::remove_dir_all(home);
    });
    let validation = SqliteQuickCheckManager::new();
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let path = sqlite.logs_db_path();
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await?;
    let mut connection = pool.acquire().await?;
    sqlx::raw_sql(
        "CREATE TABLE sample(value INTEGER);
         PRAGMA writable_schema=ON;
         UPDATE sqlite_schema SET rootpage=2147483647 WHERE name='sample';
         PRAGMA writable_schema=OFF;
         PRAGMA schema_version=99;",
    )
    .execute(&mut *connection)
    .await?;
    connection.return_to_pool().await;

    assert_eq!(
        validation
            .quick_check_once(&pool, &path, Duration::from_secs(/*secs*/ 5))
            .await?,
        QuickCheckOutcome::CorruptedNeedsFixed
    );
    // Another owner must check the same file independently.
    let independent_validation = SqliteQuickCheckManager::new();
    assert_eq!(
        independent_validation
            .quick_check_once(&pool, &path, Duration::from_secs(/*secs*/ 5))
            .await?,
        QuickCheckOutcome::CorruptedNeedsFixed
    );
    pool.close().await;
    // A closed pool proves the completed attempt is skipped without another query.
    assert_eq!(
        validation
            .quick_check_once(&pool, &path, Duration::from_secs(/*secs*/ 5))
            .await?,
        QuickCheckOutcome::Skipped
    );
    Ok(())
}

#[tokio::test]
async fn interrupted_check_leaves_connection_usable() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    tokio::fs::create_dir_all(&home).await?;
    let _cleanup = scopeguard::guard(home.clone(), |home| {
        let _ = std::fs::remove_dir_all(home);
    });
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let path = sqlite.logs_db_path();
    let pool = sqlite.open_read_write_pool(&path).await?;
    sqlx::raw_sql(
        "CREATE TABLE sample(value INTEGER);
         WITH RECURSIVE rows(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM rows WHERE id < 2048)
         INSERT INTO sample SELECT id FROM rows;",
    )
    .execute(&pool)
    .await?;
    pool.close().await;
    // One connection ensures every assertion exercises the connection that was interrupted.
    let pool = sqlite
        .open_read_only_pool(&path, Some(Duration::from_secs(/*secs*/ 5)))
        .await?;
    assert_eq!(
        quick_check(&pool, Instant::now()).await?,
        QuickCheckOutcome::Incomplete
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT sum(value) FROM sample")
            .fetch_one(&pool)
            .await?,
        2_098_176
    );
    assert_eq!(
        quick_check(&pool, Instant::now() + Duration::from_secs(/*secs*/ 5)).await?,
        QuickCheckOutcome::Complete
    );
    pool.close().await;

    let pool = sqlite.open_read_write_pool(&path).await?;
    sqlx::raw_sql(
        "UPDATE sample SET value=NULL WHERE value=1;
         PRAGMA writable_schema=ON;
         UPDATE sqlite_schema SET sql='CREATE TABLE sample(value INTEGER NOT NULL)' WHERE name='sample';
         PRAGMA writable_schema=OFF;",
    )
        .execute(&pool)
        .await?;
    pool.close().await;
    let pool = sqlite
        .open_read_only_pool(&path, /*busy_timeout*/ None)
        .await?;
    assert_eq!(
        quick_check(&pool, Instant::now()).await?,
        QuickCheckOutcome::CorruptedNeedsFixed,
        "retain a finding even if a later scan step exceeds the budget"
    );
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn locked_database_does_not_count_as_corruption() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    tokio::fs::create_dir_all(&home).await?;
    let _cleanup = scopeguard::guard(home.clone(), |home| {
        let _ = std::fs::remove_dir_all(home);
    });
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let path = sqlite.logs_db_path();
    let writer_pool = sqlite.open_read_write_pool(&path).await?;
    let mut writer = writer_pool.acquire().await?;
    sqlx::query("PRAGMA journal_mode=DELETE")
        .execute(&mut *writer)
        .await?;
    let reader_pool = sqlite
        .open_read_only_pool(&path, Some(Duration::from_millis(/*millis*/ 50)))
        .await?;
    sqlx::query("BEGIN EXCLUSIVE").execute(&mut *writer).await?;
    let started = Instant::now();
    let result = quick_check(&reader_pool, started + Duration::from_millis(/*millis*/ 50)).await;
    sqlx::query("ROLLBACK").execute(&mut *writer).await?;
    drop(writer);
    writer_pool.close().await;
    assert_eq!(result?, QuickCheckOutcome::Incomplete);
    assert_eq!(
        quick_check(
            &reader_pool,
            Instant::now() + Duration::from_secs(/*secs*/ 5)
        )
        .await?,
        QuickCheckOutcome::Complete
    );
    reader_pool.close().await;
    Ok(())
}
