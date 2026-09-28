use super::*;
use crate::migrations::runtime_logs_migrator;
use crate::runtime::test_support::unique_temp_dir;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn interrupted_reclamation_releases_writer_and_resumes_without_data_loss()
-> anyhow::Result<()> {
    let sqlite = SqliteConfig::new_for_testing(unique_temp_dir().abs());
    tokio::fs::create_dir_all(sqlite.home()).await?;
    let pool = sqlite
        .open_logs_db(&runtime_logs_migrator(), /*telemetry_override*/ None)
        .await?;
    sqlx::query(
        "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i < 4096) \
         INSERT INTO logs(ts,ts_nanos,level,target,feedback_log_body,thread_id,process_uuid) \
         SELECT unixepoch(),0,'INFO','fixture',printf('%032768d',i),'thread-'||i,'process-'||(i%17) FROM n",
    )
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM logs WHERE id%4 != 0")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await?;
    let rows = "SELECT id, feedback_log_body FROM logs ORDER BY id";
    let mut expected: Vec<(i64, String)> = sqlx::query_as(rows).fetch_all(&pool).await?;
    let free_before: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&pool)
        .await?;
    let (shutdown, receiver) = watch::channel(());
    let mut connection = pool.acquire().await?.detach();
    sqlx::raw_sql(
        "PRAGMA busy_timeout = 0; PRAGMA wal_autocheckpoint = 0; PRAGMA cache_size = -16384;",
    )
    .execute(&mut connection)
    .await?;
    // Request shutdown inside the first vacuum commit, while SQLite still owns the
    // writer lock. The production progress handler must stop this pass safely.
    connection.lock_handle().await?.set_commit_hook(move || {
        shutdown.send_replace(());
        true
    });
    let interrupted = reclaim_pages(
        &mut connection,
        Budget {
            // Shutdown, rather than machine speed, determines the interruption point.
            deadline: Some(Instant::now() + Duration::from_secs(30)),
            pages: PASS_PAGES,
        },
        &receiver,
    )
    .await?;
    assert!(receiver.has_changed()?, "must reach the vacuum commit");
    assert!(interrupted.pages <= BATCH_PAGES);
    connection.close().await?;
    let free_after: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&pool)
        .await?;
    // An interruption at COMMIT may leave the first batch committed or rolled back.
    assert!((0..=i64::from(BATCH_PAGES)).contains(&(free_before - free_after)));
    assert_eq!(
        sqlx::query_as::<_, (i64, String)>(rows)
            .fetch_all(&pool)
            .await?,
        expected
    );

    // A foreground write must acquire the lock immediately, without a busy retry.
    let mut writer = pool.acquire().await?;
    sqlx::query("PRAGMA busy_timeout = 0")
        .execute(&mut *writer)
        .await?;
    let mut transaction = writer.begin_with("BEGIN IMMEDIATE").await?;
    expected[0].1 = "foreground write after interruption".to_string();
    sqlx::query("UPDATE logs SET feedback_log_body = ? WHERE id = ?")
        .bind(&expected[0].1)
        .bind(expected[0].0)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    drop(writer);

    // Flush earlier writes so only the resumed pass can shrink the main file.
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await?;
    let bytes_before_retry = tokio::fs::metadata(sqlite.logs_db_path()).await?.len();
    let free_before_retry: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&pool)
        .await?;
    let (_shutdown, receiver) = watch::channel(());
    let resumed = reclaim(
        &sqlite.logs_db_path(),
        Budget {
            deadline: Some(Instant::now() + Duration::from_secs(30)),
            pages: BATCH_PAGES,
        },
        &receiver,
    )
    .await?;
    let free_after_retry: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&pool)
        .await?;
    assert!(resumed.pages > 0);
    assert_eq!(
        free_before_retry - free_after_retry,
        i64::from(resumed.pages)
    );
    let bytes_after_retry = tokio::fs::metadata(sqlite.logs_db_path()).await?.len();
    assert!(
        bytes_after_retry < bytes_before_retry,
        "reclamation must shrink the SQLite file: {bytes_before_retry} -> {bytes_after_retry} bytes"
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, String)>(rows)
            .fetch_all(&pool)
            .await?,
        expected
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_all(&pool)
            .await?,
        vec!["ok"]
    );
    pool.close().await;
    tokio::fs::remove_dir_all(sqlite.home()).await?;
    Ok(())
}
