use super::*;

const LOG_RETENTION_DAYS: i64 = 10;

impl StateRuntime {
    pub async fn insert_log(&self, entry: &LogEntry) -> anyhow::Result<()> {
        self.insert_logs(std::slice::from_ref(entry)).await
    }

    /// Insert a batch of log entries into the logs table.
    pub async fn insert_logs(&self, entries: &[LogEntry]) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut tx = self.logs_pool.begin().await?;
        let mut builder = QueryBuilder::<Sqlite>::new(
            "INSERT INTO logs (ts, ts_nanos, level, target, feedback_log_body, thread_id, process_uuid, module_path, file, line, estimated_bytes) ",
        );
        builder.push_values(entries, |mut row, entry| {
            let feedback_log_body = entry.feedback_log_body.as_ref().or(entry.message.as_ref());
            // Keep about 10 MiB of reader-visible log content per partition.
            // Both `query_logs` and `/feedback` read the persisted
            // `feedback_log_body`, while `LogEntry.message` is only a write-time
            // fallback for callers that still populate the old field.
            let estimated_bytes = feedback_log_body.map_or(0, String::len) as i64
                + entry.level.len() as i64
                + entry.target.len() as i64
                + entry.module_path.as_ref().map_or(0, String::len) as i64
                + entry.file.as_ref().map_or(0, String::len) as i64;
            row.push_bind(entry.ts)
                .push_bind(entry.ts_nanos)
                .push_bind(&entry.level)
                .push_bind(&entry.target)
                .push_bind(feedback_log_body)
                .push_bind(&entry.thread_id)
                .push_bind(&entry.process_uuid)
                .push_bind(&entry.module_path)
                .push_bind(&entry.file)
                .push_bind(entry.line)
                .push_bind(estimated_bytes);
        });
        builder.build().execute(&mut *tx).await?;
        self.prune_logs_after_insert(entries, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Enforce per-partition retained-log-content caps after a successful batch insert.
    ///
    /// We maintain two independent budgets:
    /// - Thread logs: rows with `thread_id IS NOT NULL`, capped per `thread_id`.
    /// - Threadless process logs: rows with `thread_id IS NULL` ("threadless"),
    ///   capped per `process_uuid` (including `process_uuid IS NULL` as its own
    ///   threadless partition).
    ///
    /// "Threadless" means the log row is not associated with any conversation
    /// thread, so retention is keyed by process identity instead.
    ///
    /// This runs inside the same transaction as the insert so callers never
    /// observe "inserted but not yet pruned" rows.
    async fn prune_logs_after_insert(
        &self,
        entries: &[LogEntry],
        tx: &mut SqliteConnection,
    ) -> anyhow::Result<()> {
        let thread_ids: BTreeSet<&str> = entries
            .iter()
            .filter_map(|entry| entry.thread_id.as_deref())
            .collect();
        if !thread_ids.is_empty() {
            // Cheap precheck: only run the heavier window-function prune for
            // threads that are currently above the cap.
            let mut over_limit_threads_query =
                QueryBuilder::<Sqlite>::new("SELECT thread_id FROM logs WHERE thread_id IN (");
            {
                let mut separated = over_limit_threads_query.separated(", ");
                for thread_id in &thread_ids {
                    separated.push_bind(*thread_id);
                }
            }
            over_limit_threads_query.push(") GROUP BY thread_id HAVING SUM(");
            over_limit_threads_query.push("estimated_bytes");
            over_limit_threads_query.push(") > ");
            over_limit_threads_query.push_bind(LOG_PARTITION_SIZE_LIMIT_BYTES);
            over_limit_threads_query.push(" OR COUNT(*) > ");
            over_limit_threads_query.push_bind(LOG_PARTITION_ROW_LIMIT);
            let over_limit_thread_ids: Vec<String> = over_limit_threads_query
                .build()
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|row| row.try_get("thread_id"))
                .collect::<Result<_, _>>()?;
            if !over_limit_thread_ids.is_empty() {
                // Enforce a strict per-thread cap by deleting every row whose
                // newest-first cumulative bytes exceed the partition budget.
                let mut prune_threads = QueryBuilder::<Sqlite>::new(
                    r#"
DELETE FROM logs
WHERE id IN (
    SELECT id
    FROM (
        SELECT
            id,
            SUM(
"#,
                );
                prune_threads.push("estimated_bytes");
                prune_threads.push(
                    r#"
            ) OVER (
                PARTITION BY thread_id
                ORDER BY ts DESC, ts_nanos DESC, id DESC
            ) AS cumulative_bytes,
            ROW_NUMBER() OVER (
                PARTITION BY thread_id
                ORDER BY ts DESC, ts_nanos DESC, id DESC
            ) AS row_number
        FROM logs
        WHERE thread_id IN (
"#,
                );
                {
                    let mut separated = prune_threads.separated(", ");
                    for thread_id in &over_limit_thread_ids {
                        separated.push_bind(thread_id);
                    }
                }
                prune_threads.push(
                    r#"
        )
    )
    WHERE cumulative_bytes >
"#,
                );
                prune_threads.push_bind(LOG_PARTITION_SIZE_LIMIT_BYTES);
                prune_threads.push(" OR row_number > ");
                prune_threads.push_bind(LOG_PARTITION_ROW_LIMIT);
                prune_threads.push("\n)");
                prune_threads.build().execute(&mut *tx).await?;
            }
        }

        let threadless_process_uuids: BTreeSet<&str> = entries
            .iter()
            .filter(|entry| entry.thread_id.is_none())
            .filter_map(|entry| entry.process_uuid.as_deref())
            .collect();
        let has_threadless_null_process_uuid = entries
            .iter()
            .any(|entry| entry.thread_id.is_none() && entry.process_uuid.is_none());
        if !threadless_process_uuids.is_empty() {
            // Threadless logs are budgeted separately per process UUID.
            let mut over_limit_processes_query = QueryBuilder::<Sqlite>::new(
                "SELECT process_uuid FROM logs WHERE thread_id IS NULL AND process_uuid IN (",
            );
            {
                let mut separated = over_limit_processes_query.separated(", ");
                for process_uuid in &threadless_process_uuids {
                    separated.push_bind(*process_uuid);
                }
            }
            over_limit_processes_query.push(") GROUP BY process_uuid HAVING SUM(");
            over_limit_processes_query.push("estimated_bytes");
            over_limit_processes_query.push(") > ");
            over_limit_processes_query.push_bind(LOG_PARTITION_SIZE_LIMIT_BYTES);
            over_limit_processes_query.push(" OR COUNT(*) > ");
            over_limit_processes_query.push_bind(LOG_PARTITION_ROW_LIMIT);
            let over_limit_process_uuids: Vec<String> = over_limit_processes_query
                .build()
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|row| row.try_get("process_uuid"))
                .collect::<Result<_, _>>()?;
            if !over_limit_process_uuids.is_empty() {
                // Same strict cap policy as thread pruning, but only for
                // threadless rows in the affected process UUIDs.
                let mut prune_threadless_process_logs = QueryBuilder::<Sqlite>::new(
                    r#"
DELETE FROM logs
WHERE id IN (
    SELECT id
    FROM (
        SELECT
            id,
            SUM(
"#,
                );
                prune_threadless_process_logs.push("estimated_bytes");
                prune_threadless_process_logs.push(
                    r#"
            ) OVER (
                PARTITION BY process_uuid
                ORDER BY ts DESC, ts_nanos DESC, id DESC
            ) AS cumulative_bytes,
            ROW_NUMBER() OVER (
                PARTITION BY process_uuid
                ORDER BY ts DESC, ts_nanos DESC, id DESC
            ) AS row_number
        FROM logs
        WHERE thread_id IS NULL
          AND process_uuid IN (
"#,
                );
                {
                    let mut separated = prune_threadless_process_logs.separated(", ");
                    for process_uuid in &over_limit_process_uuids {
                        separated.push_bind(process_uuid);
                    }
                }
                prune_threadless_process_logs.push(
                    r#"
          )
    )
    WHERE cumulative_bytes >
"#,
                );
                prune_threadless_process_logs.push_bind(LOG_PARTITION_SIZE_LIMIT_BYTES);
                prune_threadless_process_logs.push(" OR row_number > ");
                prune_threadless_process_logs.push_bind(LOG_PARTITION_ROW_LIMIT);
                prune_threadless_process_logs.push("\n)");
                prune_threadless_process_logs
                    .build()
                    .execute(&mut *tx)
                    .await?;
            }
        }
        if has_threadless_null_process_uuid {
            // Rows without a process UUID still need a cap; treat NULL as its
            // own threadless partition.
            let mut null_process_usage_query = QueryBuilder::<Sqlite>::new("SELECT SUM(");
            null_process_usage_query.push("estimated_bytes");
            null_process_usage_query.push(
                ") AS total_bytes, COUNT(*) AS row_count FROM logs WHERE thread_id IS NULL AND process_uuid IS NULL",
            );
            let null_process_usage = null_process_usage_query.build().fetch_one(&mut *tx).await?;
            let total_null_process_bytes: Option<i64> =
                null_process_usage.try_get("total_bytes")?;
            let null_process_row_count: i64 = null_process_usage.try_get("row_count")?;

            if total_null_process_bytes.unwrap_or(0) > LOG_PARTITION_SIZE_LIMIT_BYTES
                || null_process_row_count > LOG_PARTITION_ROW_LIMIT
            {
                let mut prune_threadless_null_process_logs = QueryBuilder::<Sqlite>::new(
                    r#"
DELETE FROM logs
WHERE id IN (
    SELECT id
    FROM (
        SELECT
            id,
            SUM(
"#,
                );
                prune_threadless_null_process_logs.push("estimated_bytes");
                prune_threadless_null_process_logs.push(
                    r#"
            ) OVER (
                PARTITION BY process_uuid
                ORDER BY ts DESC, ts_nanos DESC, id DESC
            ) AS cumulative_bytes,
            ROW_NUMBER() OVER (
                PARTITION BY process_uuid
                ORDER BY ts DESC, ts_nanos DESC, id DESC
            ) AS row_number
        FROM logs
        WHERE thread_id IS NULL
          AND process_uuid IS NULL
    )
    WHERE cumulative_bytes >
"#,
                );
                prune_threadless_null_process_logs.push_bind(LOG_PARTITION_SIZE_LIMIT_BYTES);
                prune_threadless_null_process_logs.push(" OR row_number > ");
                prune_threadless_null_process_logs.push_bind(LOG_PARTITION_ROW_LIMIT);
                prune_threadless_null_process_logs.push("\n)");
                prune_threadless_null_process_logs
                    .build()
                    .execute(&mut *tx)
                    .await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn delete_logs_before(&self, cutoff_ts: i64) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM logs WHERE ts < ?")
            .bind(cutoff_ts)
            .execute(self.logs_pool.as_ref())
            .await?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn run_logs_startup_maintenance(&self) -> anyhow::Result<()> {
        let Some(cutoff) =
            Utc::now().checked_sub_signed(chrono::Duration::days(LOG_RETENTION_DAYS))
        else {
            return Ok(());
        };
        self.delete_logs_before(cutoff.timestamp()).await?;
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(self.logs_pool.as_ref())
            .await?;
        sqlx::query("PRAGMA incremental_vacuum")
            .execute(self.logs_pool.as_ref())
            .await?;
        Ok(())
    }

    /// Query logs with optional filters.
    pub async fn query_logs(&self, query: &LogQuery) -> anyhow::Result<Vec<LogRow>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT id, ts, ts_nanos, level, target, feedback_log_body AS message, thread_id, process_uuid, file, line FROM logs WHERE 1 = 1",
        );
        push_log_filters(&mut builder, query);
        if query.descending {
            builder.push(" ORDER BY id DESC");
        } else {
            builder.push(" ORDER BY id ASC");
        }
        if let Some(limit) = query.limit {
            builder.push(" LIMIT ").push_bind(limit as i64);
        }

        let rows = builder
            .build_query_as::<LogRow>()
            .fetch_all(self.logs_pool.as_ref())
            .await?;
        Ok(rows)
    }

    /// Query feedback logs for a set of threads, capped to the SQLite retention budget.
    pub async fn query_feedback_logs_for_threads(
        &self,
        thread_ids: &[&str],
    ) -> anyhow::Result<Vec<u8>> {
        if thread_ids.is_empty() {
            return Ok(Vec::new());
        }

        let max_bytes = usize::try_from(LOG_PARTITION_SIZE_LIMIT_BYTES).unwrap_or(usize::MAX);
        // Bound the fetched rows in SQL first so over-retained partitions do not have to load
        // every row into memory, then apply the exact whole-line byte cap after formatting.
        let requested_threads = vec!["(?)"; thread_ids.len()].join(", ");
        let query = format!(
            r#"
WITH requested_threads(thread_id) AS (
    VALUES {requested_threads}
),
latest_processes AS (
    SELECT (
        SELECT process_uuid
        FROM logs
        WHERE logs.thread_id = requested_threads.thread_id AND process_uuid IS NOT NULL
        ORDER BY ts DESC, ts_nanos DESC, id DESC
        LIMIT 1
    ) AS process_uuid
    FROM requested_threads
),
feedback_logs AS (
    SELECT ts, ts_nanos, level, feedback_log_body, estimated_bytes, id
    FROM logs
    WHERE feedback_log_body IS NOT NULL AND (
        thread_id IN (SELECT thread_id FROM requested_threads)
        OR (
            thread_id IS NULL
            AND process_uuid IN (
                SELECT process_uuid
                FROM latest_processes
                WHERE process_uuid IS NOT NULL
            )
        )
    )
),
bounded_feedback_logs AS (
    SELECT
        ts,
        ts_nanos,
        level,
        feedback_log_body,
        id,
        SUM(estimated_bytes) OVER (
            ORDER BY ts DESC, ts_nanos DESC, id DESC
        ) AS cumulative_estimated_bytes
    FROM feedback_logs
)
SELECT ts, ts_nanos, level, feedback_log_body
FROM bounded_feedback_logs
WHERE cumulative_estimated_bytes <= ?
ORDER BY ts DESC, ts_nanos DESC, id DESC
"#
        );
        let mut sql = sqlx::query_as::<_, FeedbackLogRow>(query.as_str());
        for thread_id in thread_ids {
            sql = sql.bind(thread_id);
        }
        let rows = sql
            .bind(LOG_PARTITION_SIZE_LIMIT_BYTES)
            .fetch_all(self.logs_pool.as_ref())
            .await?;

        let mut lines = Vec::new();
        let mut total_bytes = 0usize;
        for row in rows {
            let line =
                format_feedback_log_line(row.ts, row.ts_nanos, &row.level, &row.feedback_log_body);
            if total_bytes.saturating_add(line.len()) > max_bytes {
                break;
            }
            total_bytes += line.len();
            lines.push(line);
        }

        let mut ordered_bytes = Vec::with_capacity(total_bytes);
        for line in lines.into_iter().rev() {
            ordered_bytes.extend_from_slice(line.as_bytes());
        }

        Ok(ordered_bytes)
    }

    /// Query per-thread feedback logs, capped to the per-thread SQLite retention budget.
    pub async fn query_feedback_logs(&self, thread_id: &str) -> anyhow::Result<Vec<u8>> {
        self.query_feedback_logs_for_threads(&[thread_id]).await
    }

    /// Return the max log id matching optional filters.
    pub async fn max_log_id(&self, query: &LogQuery) -> anyhow::Result<i64> {
        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT MAX(id) AS max_id FROM logs WHERE 1 = 1");
        push_log_filters(&mut builder, query);
        let row = builder.build().fetch_one(self.logs_pool.as_ref()).await?;
        let max_id: Option<i64> = row.try_get("max_id")?;
        Ok(max_id.unwrap_or(0))
    }
}

#[derive(sqlx::FromRow)]
struct FeedbackLogRow {
    ts: i64,
    ts_nanos: i64,
    level: String,
    feedback_log_body: String,
}

fn format_feedback_log_line(
    ts: i64,
    ts_nanos: i64,
    level: &str,
    feedback_log_body: &str,
) -> String {
    let nanos = u32::try_from(ts_nanos).unwrap_or(0);
    let timestamp = match DateTime::<Utc>::from_timestamp(ts, nanos) {
        Some(dt) => dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        None => format!("{ts}.{ts_nanos:09}Z"),
    };
    let mut line = format!("{timestamp} {level:>5} {feedback_log_body}");
    if !line.ends_with('\n') {
        line.push('\n');
    }
    line
}

fn push_log_filters<'a>(builder: &mut QueryBuilder<'a, Sqlite>, query: &'a LogQuery) {
    if let Some(level_upper) = query.level_upper.as_ref() {
        builder
            .push(" AND UPPER(level) = ")
            .push_bind(level_upper.as_str());
    }
    if let Some(from_ts) = query.from_ts {
        builder.push(" AND ts >= ").push_bind(from_ts);
    }
    if let Some(to_ts) = query.to_ts {
        builder.push(" AND ts <= ").push_bind(to_ts);
    }
    push_like_filters(builder, "module_path", &query.module_like);
    push_like_filters(builder, "file", &query.file_like);
    let has_thread_filter = !query.thread_ids.is_empty() || query.include_threadless;
    if has_thread_filter {
        builder.push(" AND (");
        let mut needs_or = false;
        for thread_id in &query.thread_ids {
            if needs_or {
                builder.push(" OR ");
            }
            builder.push("thread_id = ").push_bind(thread_id.as_str());
            needs_or = true;
        }
        if query.include_threadless {
            if needs_or {
                builder.push(" OR ");
            }
            builder.push("thread_id IS NULL");
        }
        builder.push(")");
    }
    if let Some(after_id) = query.after_id {
        builder.push(" AND id > ").push_bind(after_id);
    }
    if let Some(search) = query.search.as_ref() {
        builder.push(" AND INSTR(COALESCE(feedback_log_body, ''), ");
        builder.push_bind(search.as_str());
        builder.push(") > 0");
    }
}

fn push_like_filters<'a>(
    builder: &mut QueryBuilder<'a, Sqlite>,
    column: &str,
    filters: &'a [String],
) {
    if filters.is_empty() {
        return;
    }
    builder.push(" AND (");
    for (idx, filter) in filters.iter().enumerate() {
        if idx > 0 {
            builder.push(" OR ");
        }
        builder
            .push(column)
            .push(" LIKE '%' || ")
            .push_bind(filter.as_str())
            .push(" || '%'");
    }
    builder.push(")");
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::format_feedback_log_line;
    use super::test_support::unique_temp_dir;
    use crate::LogEntry;
    use crate::LogQuery;
    use crate::logs_db_path;
    use crate::migrations::LOGS_MIGRATOR;
    use chrono::Utc;
    use pretty_assertions::assert_eq;
    use sqlx::SqlitePool;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::SqliteConnectOptions;
    use std::borrow::Cow;
    use std::path::Path;

    async fn open_db_pool(path: &Path) -> SqlitePool {
        SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(false),
        )
        .await
        .expect("open sqlite pool")
    }

    async fn log_row_count(path: &Path) -> i64 {
        let pool = open_db_pool(path).await;
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs")
            .fetch_one(&pool)
            .await
            .expect("count log rows");
        pool.close().await;
        count
    }

    #[tokio::test]
    async fn insert_logs_use_dedicated_log_database() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_logs(&[LogEntry {
                ts: 1,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some("dedicated-log-db".to_string()),
                feedback_log_body: Some("dedicated-log-db".to_string()),
                thread_id: Some("thread-1".to_string()),
                process_uuid: Some("proc-1".to_string()),
                module_path: Some("mod".to_string()),
                file: Some("main.rs".to_string()),
                line: Some(7),
            }])
            .await
            .expect("insert test logs");

        let logs_count = log_row_count(logs_db_path(codex_home.as_path()).as_path()).await;

        assert_eq!(logs_count, 1);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn init_migrates_message_only_logs_db_to_feedback_log_body_schema() {
        let codex_home = unique_temp_dir();
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("create codex home");
        let logs_path = logs_db_path(codex_home.as_path());
        let old_logs_migrator = Migrator {
            migrations: Cow::Owned(vec![LOGS_MIGRATOR.migrations[0].clone()]),
            ignore_missing: false,
            locking: true,
            no_tx: false,
        };
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(&logs_path)
                .create_if_missing(true),
        )
        .await
        .expect("open old logs db");
        old_logs_migrator
            .run(&pool)
            .await
            .expect("apply old logs schema");
        sqlx::query(
            "INSERT INTO logs (ts, ts_nanos, level, target, message, module_path, file, line, thread_id, process_uuid, estimated_bytes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Utc::now().timestamp())
        .bind(0_i64)
        .bind("INFO")
        .bind("cli")
        .bind("legacy-body")
        .bind("mod")
        .bind("main.rs")
        .bind(7_i64)
        .bind("thread-1")
        .bind("proc-1")
        .bind(16_i64)
        .execute(&pool)
        .await
        .expect("insert legacy log row");
        pool.close().await;
        drop(pool);

        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let rows = runtime
            .query_logs(&LogQuery::default())
            .await
            .expect("query migrated logs");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message.as_deref(), Some("legacy-body"));

        let migrated_pool = open_db_pool(logs_path.as_path()).await;
        let columns = sqlx::query_scalar::<_, String>("SELECT name FROM pragma_table_info('logs')")
            .fetch_all(&migrated_pool)
            .await
            .expect("load migrated columns");
        assert_eq!(
            columns,
            vec![
                "id".to_string(),
                "ts".to_string(),
                "ts_nanos".to_string(),
                "level".to_string(),
                "target".to_string(),
                "feedback_log_body".to_string(),
                "module_path".to_string(),
                "file".to_string(),
                "line".to_string(),
                "thread_id".to_string(),
                "process_uuid".to_string(),
                "estimated_bytes".to_string(),
            ]
        );
        let indexes = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_index_list('logs') ORDER BY name",
        )
        .fetch_all(&migrated_pool)
        .await
        .expect("load migrated indexes");
        assert_eq!(
            indexes,
            vec![
                "idx_logs_process_uuid_threadless_ts".to_string(),
                "idx_logs_thread_id".to_string(),
                "idx_logs_thread_id_ts".to_string(),
                "idx_logs_ts".to_string(),
            ]
        );
        migrated_pool.close().await;

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn init_recreates_legacy_logs_db_when_log_version_changes() {
        let codex_home = unique_temp_dir();
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("create codex home");
        let legacy_logs_path = codex_home.join("logs_1.sqlite");
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(&legacy_logs_path)
                .create_if_missing(true),
        )
        .await
        .expect("open legacy logs db");
        LOGS_MIGRATOR
            .run(&pool)
            .await
            .expect("apply legacy logs schema");
        sqlx::query(
            "INSERT INTO logs (ts, ts_nanos, level, target, feedback_log_body, module_path, file, line, thread_id, process_uuid, estimated_bytes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(1_i64)
        .bind(0_i64)
        .bind("INFO")
        .bind("cli")
        .bind("legacy-log-row")
        .bind("mod")
        .bind("main.rs")
        .bind(7_i64)
        .bind("thread-1")
        .bind("proc-1")
        .bind(16_i64)
        .execute(&pool)
        .await
        .expect("insert legacy log row");
        pool.close().await;

        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        assert!(
            !legacy_logs_path.exists(),
            "legacy logs db should be removed when the version changes"
        );
        assert!(
            logs_db_path(codex_home.as_path()).exists(),
            "current logs db should be recreated during init"
        );
        assert!(
            runtime
                .query_logs(&LogQuery::default())
                .await
                .expect("query recreated logs db")
                .is_empty()
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn init_configures_logs_db_with_incremental_auto_vacuum() {
        let codex_home = unique_temp_dir();
        let _runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let pool = open_db_pool(logs_db_path(codex_home.as_path()).as_path()).await;
        let auto_vacuum = sqlx::query_scalar::<_, i64>("PRAGMA auto_vacuum")
            .fetch_one(&pool)
            .await
            .expect("read auto_vacuum pragma");
        assert_eq!(auto_vacuum, 2);
        pool.close().await;

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[test]
    fn format_feedback_log_line_matches_feedback_formatter_shape() {
        assert_eq!(
            format_feedback_log_line(
                /*ts*/ 1,
                /*ts_nanos*/ 123_456_000,
                "INFO",
                "alpha"
            ),
            "1970-01-01T00:00:01.123456Z  INFO alpha\n"
        );
    }

    #[test]
    fn format_feedback_log_line_preserves_existing_trailing_newline() {
        assert_eq!(
            format_feedback_log_line(
                /*ts*/ 1,
                /*ts_nanos*/ 123_456_000,
                "INFO",
                "alpha\n"
            ),
            "1970-01-01T00:00:01.123456Z  INFO alpha\n"
        );
    }

    #[tokio::test]
    async fn query_logs_with_search_matches_rendered_body_substring() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1_700_000_001,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("alpha".to_string()),
                    feedback_log_body: Some("foo=1 alpha".to_string()),
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: None,
                    file: Some("main.rs".to_string()),
                    line: Some(42),
                    module_path: None,
                },
                LogEntry {
                    ts: 1_700_000_002,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("alphabet".to_string()),
                    feedback_log_body: Some("foo=2 alphabet".to_string()),
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: None,
                    file: Some("main.rs".to_string()),
                    line: Some(43),
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                search: Some("foo=2".to_string()),
                ..Default::default()
            })
            .await
            .expect("query matching logs");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message.as_deref(), Some("foo=2 alphabet"));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_old_rows_when_thread_exceeds_size_limit() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let six_mebibytes = "a".repeat(6 * 1024 * 1024);
        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("small".to_string()),
                    feedback_log_body: Some(six_mebibytes.clone()),
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: Some("main.rs".to_string()),
                    line: Some(1),
                    module_path: Some("mod".to_string()),
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("small".to_string()),
                    feedback_log_body: Some(six_mebibytes.clone()),
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: Some("main.rs".to_string()),
                    line: Some(2),
                    module_path: Some("mod".to_string()),
                },
            ])
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                thread_ids: vec!["thread-1".to_string()],
                ..Default::default()
            })
            .await
            .expect("query thread logs");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ts, 2);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_single_thread_row_when_it_exceeds_size_limit() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let eleven_mebibytes = "d".repeat(11 * 1024 * 1024);
        runtime
            .insert_logs(&[LogEntry {
                ts: 1,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some("small".to_string()),
                feedback_log_body: Some(eleven_mebibytes),
                thread_id: Some("thread-oversized".to_string()),
                process_uuid: Some("proc-1".to_string()),
                file: Some("main.rs".to_string()),
                line: Some(1),
                module_path: Some("mod".to_string()),
            }])
            .await
            .expect("insert test log");

        let rows = runtime
            .query_logs(&LogQuery {
                thread_ids: vec!["thread-oversized".to_string()],
                ..Default::default()
            })
            .await
            .expect("query thread logs");

        assert!(rows.is_empty());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_threadless_rows_per_process_uuid_only() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let six_mebibytes = "b".repeat(6 * 1024 * 1024);
        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(six_mebibytes.clone()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: Some("main.rs".to_string()),
                    line: Some(1),
                    module_path: Some("mod".to_string()),
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(six_mebibytes.clone()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: Some("main.rs".to_string()),
                    line: Some(2),
                    module_path: Some("mod".to_string()),
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(six_mebibytes),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: Some("main.rs".to_string()),
                    line: Some(3),
                    module_path: Some("mod".to_string()),
                },
            ])
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                thread_ids: vec!["thread-1".to_string()],
                include_threadless: true,
                ..Default::default()
            })
            .await
            .expect("query thread and threadless logs");

        let mut timestamps: Vec<i64> = rows.into_iter().map(|row| row.ts).collect();
        timestamps.sort_unstable();
        assert_eq!(timestamps, vec![2, 3]);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_single_threadless_process_row_when_it_exceeds_size_limit() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let eleven_mebibytes = "e".repeat(11 * 1024 * 1024);
        runtime
            .insert_logs(&[LogEntry {
                ts: 1,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some("small".to_string()),
                feedback_log_body: Some(eleven_mebibytes),
                thread_id: None,
                process_uuid: Some("proc-oversized".to_string()),
                file: Some("main.rs".to_string()),
                line: Some(1),
                module_path: Some("mod".to_string()),
            }])
            .await
            .expect("insert test log");

        let rows = runtime
            .query_logs(&LogQuery {
                include_threadless: true,
                ..Default::default()
            })
            .await
            .expect("query threadless logs");

        assert!(rows.is_empty());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_threadless_rows_with_null_process_uuid() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let six_mebibytes = "c".repeat(6 * 1024 * 1024);
        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(six_mebibytes.clone()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: None,
                    file: Some("main.rs".to_string()),
                    line: Some(1),
                    module_path: Some("mod".to_string()),
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(six_mebibytes),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: None,
                    file: Some("main.rs".to_string()),
                    line: Some(2),
                    module_path: Some("mod".to_string()),
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("small".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: Some("main.rs".to_string()),
                    line: Some(3),
                    module_path: Some("mod".to_string()),
                },
            ])
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                include_threadless: true,
                ..Default::default()
            })
            .await
            .expect("query threadless logs");

        let mut timestamps: Vec<i64> = rows.into_iter().map(|row| row.ts).collect();
        timestamps.sort_unstable();
        assert_eq!(timestamps, vec![2, 3]);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_single_threadless_null_process_row_when_it_exceeds_limit() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let eleven_mebibytes = "f".repeat(11 * 1024 * 1024);
        runtime
            .insert_logs(&[LogEntry {
                ts: 1,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some("small".to_string()),
                feedback_log_body: Some(eleven_mebibytes),
                thread_id: None,
                process_uuid: None,
                file: Some("main.rs".to_string()),
                line: Some(1),
                module_path: Some("mod".to_string()),
            }])
            .await
            .expect("insert test log");

        let rows = runtime
            .query_logs(&LogQuery {
                include_threadless: true,
                ..Default::default()
            })
            .await
            .expect("query threadless logs");

        assert!(rows.is_empty());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_old_rows_when_thread_exceeds_row_limit() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let entries: Vec<LogEntry> = (1..=1_001)
            .map(|ts| LogEntry {
                ts,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some(format!("thread-row-{ts}")),
                feedback_log_body: None,
                thread_id: Some("thread-row-limit".to_string()),
                process_uuid: Some("proc-1".to_string()),
                file: Some("main.rs".to_string()),
                line: Some(ts),
                module_path: Some("mod".to_string()),
            })
            .collect();
        runtime
            .insert_logs(&entries)
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                thread_ids: vec!["thread-row-limit".to_string()],
                ..Default::default()
            })
            .await
            .expect("query thread logs");

        let timestamps: Vec<i64> = rows.into_iter().map(|row| row.ts).collect();
        assert_eq!(timestamps.len(), 1_000);
        assert_eq!(timestamps.first().copied(), Some(2));
        assert_eq!(timestamps.last().copied(), Some(1_001));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_old_threadless_rows_when_process_exceeds_row_limit() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let entries: Vec<LogEntry> = (1..=1_001)
            .map(|ts| LogEntry {
                ts,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some(format!("process-row-{ts}")),
                feedback_log_body: None,
                thread_id: None,
                process_uuid: Some("proc-row-limit".to_string()),
                file: Some("main.rs".to_string()),
                line: Some(ts),
                module_path: Some("mod".to_string()),
            })
            .collect();
        runtime
            .insert_logs(&entries)
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                include_threadless: true,
                ..Default::default()
            })
            .await
            .expect("query threadless logs");

        let timestamps: Vec<i64> = rows
            .into_iter()
            .filter(|row| row.process_uuid.as_deref() == Some("proc-row-limit"))
            .map(|row| row.ts)
            .collect();
        assert_eq!(timestamps.len(), 1_000);
        assert_eq!(timestamps.first().copied(), Some(2));
        assert_eq!(timestamps.last().copied(), Some(1_001));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn insert_logs_prunes_old_threadless_null_process_rows_when_row_limit_exceeded() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let entries: Vec<LogEntry> = (1..=1_001)
            .map(|ts| LogEntry {
                ts,
                ts_nanos: 0,
                level: "INFO".to_string(),
                target: "cli".to_string(),
                message: Some(format!("null-process-row-{ts}")),
                feedback_log_body: None,
                thread_id: None,
                process_uuid: None,
                file: Some("main.rs".to_string()),
                line: Some(ts),
                module_path: Some("mod".to_string()),
            })
            .collect();
        runtime
            .insert_logs(&entries)
            .await
            .expect("insert test logs");

        let rows = runtime
            .query_logs(&LogQuery {
                include_threadless: true,
                ..Default::default()
            })
            .await
            .expect("query threadless logs");

        let timestamps: Vec<i64> = rows
            .into_iter()
            .filter(|row| row.process_uuid.is_none())
            .map(|row| row.ts)
            .collect();
        assert_eq!(timestamps.len(), 1_000);
        assert_eq!(timestamps.first().copied(), Some(2));
        assert_eq!(timestamps.last().copied(), Some(1_001));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_returns_newest_lines_within_limit_in_order() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("alpha".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("bravo".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("charlie".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let bytes = runtime
            .query_feedback_logs("thread-1")
            .await
            .expect("query feedback logs");

        assert_eq!(
            String::from_utf8(bytes).expect("valid utf-8"),
            [
                format_feedback_log_line(/*ts*/ 1, /*ts_nanos*/ 0, "INFO", "alpha"),
                format_feedback_log_line(/*ts*/ 2, /*ts_nanos*/ 0, "INFO", "bravo"),
                format_feedback_log_line(/*ts*/ 3, /*ts_nanos*/ 0, "INFO", "charlie"),
            ]
            .concat()
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_excludes_oversized_newest_row() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");
        let eleven_mebibytes = "z".repeat(11 * 1024 * 1024);

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("small".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-oversized".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(eleven_mebibytes),
                    feedback_log_body: None,
                    thread_id: Some("thread-oversized".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let bytes = runtime
            .query_feedback_logs("thread-oversized")
            .await
            .expect("query feedback logs");

        assert_eq!(bytes, Vec::<u8>::new());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_includes_threadless_rows_from_same_process() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("threadless-before".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("thread-scoped".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("threadless-after".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 4,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("other-process-threadless".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-2".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let bytes = runtime
            .query_feedback_logs("thread-1")
            .await
            .expect("query feedback logs");

        assert_eq!(
            String::from_utf8(bytes).expect("valid utf-8"),
            [
                format_feedback_log_line(
                    /*ts*/ 1,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "threadless-before"
                ),
                format_feedback_log_line(
                    /*ts*/ 2,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "thread-scoped"
                ),
                format_feedback_log_line(
                    /*ts*/ 3,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "threadless-after"
                ),
            ]
            .concat()
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_excludes_threadless_rows_from_prior_processes() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("old-process-threadless".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-old".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("old-process-thread".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-old".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("new-process-thread".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-new".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 4,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("new-process-threadless".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-new".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let bytes = runtime
            .query_feedback_logs("thread-1")
            .await
            .expect("query feedback logs");

        assert_eq!(
            String::from_utf8(bytes).expect("valid utf-8"),
            [
                format_feedback_log_line(
                    /*ts*/ 2,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "old-process-thread"
                ),
                format_feedback_log_line(
                    /*ts*/ 3,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "new-process-thread"
                ),
                format_feedback_log_line(
                    /*ts*/ 4,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "new-process-threadless"
                ),
            ]
            .concat()
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_keeps_newest_suffix_across_thread_and_threadless_logs() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");
        let thread_marker = "thread-scoped-oldest";
        let threadless_older_marker = "threadless-older";
        let threadless_newer_marker = "threadless-newer";
        let five_mebibytes = format!("{threadless_older_marker} {}", "a".repeat(5 * 1024 * 1024));
        let four_and_half_mebibytes = format!(
            "{threadless_newer_marker} {}",
            "b".repeat((9 * 1024 * 1024) / 2)
        );
        let one_mebibyte = format!("{thread_marker} {}", "c".repeat(1024 * 1024));

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(one_mebibyte.clone()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(five_mebibytes),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some(four_and_half_mebibytes),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let bytes = runtime
            .query_feedback_logs("thread-1")
            .await
            .expect("query feedback logs");
        let logs = String::from_utf8(bytes).expect("valid utf-8");

        assert!(!logs.contains(thread_marker));
        assert!(logs.contains(threadless_older_marker));
        assert!(logs.contains(threadless_newer_marker));
        assert_eq!(logs.matches('\n').count(), 2);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_for_threads_merges_requested_threads_and_threadless_rows() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        runtime
            .insert_logs(&[
                LogEntry {
                    ts: 1,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("thread-1".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-1".to_string()),
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 2,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("thread-2".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-2".to_string()),
                    process_uuid: Some("proc-2".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 3,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("threadless-proc-1".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-1".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 4,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("threadless-proc-2".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-2".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 5,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("thread-3".to_string()),
                    feedback_log_body: None,
                    thread_id: Some("thread-3".to_string()),
                    process_uuid: Some("proc-3".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
                LogEntry {
                    ts: 6,
                    ts_nanos: 0,
                    level: "INFO".to_string(),
                    target: "cli".to_string(),
                    message: Some("threadless-proc-3".to_string()),
                    feedback_log_body: None,
                    thread_id: None,
                    process_uuid: Some("proc-3".to_string()),
                    file: None,
                    line: None,
                    module_path: None,
                },
            ])
            .await
            .expect("insert test logs");

        let bytes = runtime
            .query_feedback_logs_for_threads(&["thread-1", "thread-2"])
            .await
            .expect("query feedback logs");

        assert_eq!(
            String::from_utf8(bytes).expect("valid utf-8"),
            [
                format_feedback_log_line(/*ts*/ 1, /*ts_nanos*/ 0, "INFO", "thread-1"),
                format_feedback_log_line(/*ts*/ 2, /*ts_nanos*/ 0, "INFO", "thread-2"),
                format_feedback_log_line(
                    /*ts*/ 3,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "threadless-proc-1"
                ),
                format_feedback_log_line(
                    /*ts*/ 4,
                    /*ts_nanos*/ 0,
                    "INFO",
                    "threadless-proc-2"
                ),
            ]
            .concat()
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn query_feedback_logs_for_threads_returns_empty_for_empty_thread_list() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
            .await
            .expect("initialize runtime");

        let bytes = runtime
            .query_feedback_logs_for_threads(&[])
            .await
            .expect("query feedback logs");

        assert_eq!(bytes, Vec::<u8>::new());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }
}
