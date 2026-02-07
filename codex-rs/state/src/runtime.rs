use crate::DB_ERROR_METRIC;
use crate::LogEntry;
use crate::LogQuery;
use crate::LogRow;
use crate::SortKey;
use crate::ThreadMemory;
use crate::ThreadMetadata;
use crate::ThreadMetadataBuilder;
use crate::ThreadsPage;
use crate::apply_rollout_item;
use crate::migrations::MIGRATOR;
use crate::model::ThreadMemoryRow;
use crate::model::ThreadRow;
use crate::model::anchor_from_item;
use crate::model::datetime_to_epoch_seconds;
use crate::paths::file_modified_time_utc;
use chrono::DateTime;
use chrono::Utc;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::RolloutItem;
use log::LevelFilter;
use serde_json::Value;
use sqlx::ConnectOptions;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

pub const STATE_DB_FILENAME: &str = "state";
pub const STATE_DB_VERSION: u32 = 4;

const METRIC_DB_INIT: &str = "codex.db.init";

#[derive(Clone)]
pub struct StateRuntime {
    codex_home: PathBuf,
    default_provider: String,
    pool: Arc<sqlx::SqlitePool>,
}

impl StateRuntime {
    /// Initialize the state runtime using the provided Codex home and default provider.
    ///
    /// This opens (and migrates) the SQLite database at `codex_home/state.sqlite`.
    pub async fn init(
        codex_home: PathBuf,
        default_provider: String,
        otel: Option<OtelManager>,
    ) -> anyhow::Result<Arc<Self>> {
        tokio::fs::create_dir_all(&codex_home).await?;
        remove_legacy_state_files(&codex_home).await;
        let state_path = state_db_path(codex_home.as_path());
        let existed = tokio::fs::try_exists(&state_path).await.unwrap_or(false);
        let pool = match open_sqlite(&state_path).await {
            Ok(db) => Arc::new(db),
            Err(err) => {
                warn!("failed to open state db at {}: {err}", state_path.display());
                if let Some(otel) = otel.as_ref() {
                    otel.counter(METRIC_DB_INIT, 1, &[("status", "open_error")]);
                }
                return Err(err);
            }
        };
        if let Some(otel) = otel.as_ref() {
            otel.counter(METRIC_DB_INIT, 1, &[("status", "opened")]);
        }
        let runtime = Arc::new(Self {
            pool,
            codex_home,
            default_provider,
        });
        if !existed && let Some(otel) = otel.as_ref() {
            otel.counter(METRIC_DB_INIT, 1, &[("status", "created")]);
        }
        Ok(runtime)
    }

    /// Return the configured Codex home directory for this runtime.
    pub fn codex_home(&self) -> &Path {
        self.codex_home.as_path()
    }

    /// Get persisted rollout metadata backfill state.
    pub async fn get_backfill_state(&self) -> anyhow::Result<crate::BackfillState> {
        self.ensure_backfill_state_row().await?;
        let row = sqlx::query(
            r#"
SELECT status, last_watermark, last_success_at
FROM backfill_state
WHERE id = 1
            "#,
        )
        .fetch_one(self.pool.as_ref())
        .await?;
        crate::BackfillState::try_from_row(&row)
    }

    /// Mark rollout metadata backfill as running.
    pub async fn mark_backfill_running(&self) -> anyhow::Result<()> {
        self.ensure_backfill_state_row().await?;
        sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Persist rollout metadata backfill progress.
    pub async fn checkpoint_backfill(&self, watermark: &str) -> anyhow::Result<()> {
        self.ensure_backfill_state_row().await?;
        sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, last_watermark = ?, updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(watermark)
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Mark rollout metadata backfill as complete.
    pub async fn mark_backfill_complete(&self, last_watermark: Option<&str>) -> anyhow::Result<()> {
        self.ensure_backfill_state_row().await?;
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
UPDATE backfill_state
SET
    status = ?,
    last_watermark = COALESCE(?, last_watermark),
    last_success_at = ?,
    updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Complete.as_str())
        .bind(last_watermark)
        .bind(now)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Load thread metadata by id using the underlying database.
    pub async fn get_thread(&self, id: ThreadId) -> anyhow::Result<Option<crate::ThreadMetadata>> {
        let row = sqlx::query(
            r#"
SELECT
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from))
            .transpose()
    }

    /// Get dynamic tools for a thread, if present.
    pub async fn get_dynamic_tools(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<Vec<DynamicToolSpec>>> {
        let rows = sqlx::query(
            r#"
SELECT name, description, input_schema
FROM thread_dynamic_tools
WHERE thread_id = ?
ORDER BY position ASC
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let mut tools = Vec::with_capacity(rows.len());
        for row in rows {
            let input_schema: String = row.try_get("input_schema")?;
            let input_schema = serde_json::from_str::<Value>(input_schema.as_str())?;
            tools.push(DynamicToolSpec {
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                input_schema,
            });
        }
        Ok(Some(tools))
    }

    /// Get memory summaries for a thread, if present.
    pub async fn get_thread_memory(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<ThreadMemory>> {
        let row = sqlx::query(
            r#"
SELECT thread_id, trace_summary, memory_summary, updated_at
FROM thread_memory
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| ThreadMemoryRow::try_from_row(&row).and_then(ThreadMemory::try_from))
            .transpose()
    }

    /// Find a rollout path by thread id using the underlying database.
    pub async fn find_rollout_path_by_id(
        &self,
        id: ThreadId,
        archived_only: Option<bool>,
    ) -> anyhow::Result<Option<PathBuf>> {
        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT rollout_path FROM threads WHERE id = ");
        builder.push_bind(id.to_string());
        match archived_only {
            Some(true) => {
                builder.push(" AND archived = 1");
            }
            Some(false) => {
                builder.push(" AND archived = 0");
            }
            None => {}
        }
        let row = builder.build().fetch_optional(self.pool.as_ref()).await?;
        Ok(row
            .and_then(|r| r.try_get::<String, _>("rollout_path").ok())
            .map(PathBuf::from))
    }

    /// List threads using the underlying database.
    pub async fn list_threads(
        &self,
        page_size: usize,
        anchor: Option<&crate::Anchor>,
        sort_key: crate::SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
    ) -> anyhow::Result<crate::ThreadsPage> {
        let limit = page_size.saturating_add(1);

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
SELECT
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
            "#,
        );
        push_thread_filters(
            &mut builder,
            archived_only,
            allowed_sources,
            model_providers,
            anchor,
            sort_key,
        );
        push_thread_order_and_limit(&mut builder, sort_key, limit);

        let rows = builder.build().fetch_all(self.pool.as_ref()).await?;
        let mut items = rows
            .into_iter()
            .map(|row| ThreadRow::try_from_row(&row).and_then(ThreadMetadata::try_from))
            .collect::<Result<Vec<_>, _>>()?;
        let num_scanned_rows = items.len();
        let next_anchor = if items.len() > page_size {
            items.pop();
            items
                .last()
                .and_then(|item| anchor_from_item(item, sort_key))
        } else {
            None
        };
        Ok(ThreadsPage {
            items,
            next_anchor,
            num_scanned_rows,
        })
    }

    /// Insert one log entry into the logs table.
    pub async fn insert_log(&self, entry: &LogEntry) -> anyhow::Result<()> {
        self.insert_logs(std::slice::from_ref(entry)).await
    }

    /// Insert a batch of log entries into the logs table.
    pub async fn insert_logs(&self, entries: &[LogEntry]) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            "INSERT INTO logs (ts, ts_nanos, level, target, message, thread_id, module_path, file, line) ",
        );
        builder.push_values(entries, |mut row, entry| {
            row.push_bind(entry.ts)
                .push_bind(entry.ts_nanos)
                .push_bind(&entry.level)
                .push_bind(&entry.target)
                .push_bind(&entry.message)
                .push_bind(&entry.thread_id)
                .push_bind(&entry.module_path)
                .push_bind(&entry.file)
                .push_bind(entry.line);
        });
        builder.build().execute(self.pool.as_ref()).await?;
        Ok(())
    }

    pub(crate) async fn delete_logs_before(&self, cutoff_ts: i64) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM logs WHERE ts < ?")
            .bind(cutoff_ts)
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected())
    }

    /// Query logs with optional filters.
    pub async fn query_logs(&self, query: &LogQuery) -> anyhow::Result<Vec<LogRow>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT id, ts, ts_nanos, level, target, message, thread_id, file, line FROM logs WHERE 1 = 1",
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
            .fetch_all(self.pool.as_ref())
            .await?;
        Ok(rows)
    }

    /// Return the max log id matching optional filters.
    pub async fn max_log_id(&self, query: &LogQuery) -> anyhow::Result<i64> {
        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT MAX(id) AS max_id FROM logs WHERE 1 = 1");
        push_log_filters(&mut builder, query);
        let row = builder.build().fetch_one(self.pool.as_ref()).await?;
        let max_id: Option<i64> = row.try_get("max_id")?;
        Ok(max_id.unwrap_or(0))
    }

    /// List thread ids using the underlying database (no rollout scanning).
    pub async fn list_thread_ids(
        &self,
        limit: usize,
        anchor: Option<&crate::Anchor>,
        sort_key: crate::SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
    ) -> anyhow::Result<Vec<ThreadId>> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT id FROM threads");
        push_thread_filters(
            &mut builder,
            archived_only,
            allowed_sources,
            model_providers,
            anchor,
            sort_key,
        );
        push_thread_order_and_limit(&mut builder, sort_key, limit);

        let rows = builder.build().fetch_all(self.pool.as_ref()).await?;
        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                Ok(ThreadId::try_from(id)?)
            })
            .collect()
    }

    /// Insert or replace thread metadata directly.
    pub async fn upsert_thread(&self, metadata: &crate::ThreadMetadata) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    cli_version,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    first_user_message,
    archived,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    rollout_path = excluded.rollout_path,
    created_at = excluded.created_at,
    updated_at = excluded.updated_at,
    source = excluded.source,
    model_provider = excluded.model_provider,
    cwd = excluded.cwd,
    cli_version = excluded.cli_version,
    title = excluded.title,
    sandbox_policy = excluded.sandbox_policy,
    approval_mode = excluded.approval_mode,
    tokens_used = excluded.tokens_used,
    first_user_message = excluded.first_user_message,
    archived = excluded.archived,
    archived_at = excluded.archived_at,
    git_sha = excluded.git_sha,
    git_branch = excluded.git_branch,
    git_origin_url = excluded.git_origin_url
            "#,
        )
        .bind(metadata.id.to_string())
        .bind(metadata.rollout_path.display().to_string())
        .bind(datetime_to_epoch_seconds(metadata.created_at))
        .bind(datetime_to_epoch_seconds(metadata.updated_at))
        .bind(metadata.source.as_str())
        .bind(metadata.model_provider.as_str())
        .bind(metadata.cwd.display().to_string())
        .bind(metadata.cli_version.as_str())
        .bind(metadata.title.as_str())
        .bind(metadata.sandbox_policy.as_str())
        .bind(metadata.approval_mode.as_str())
        .bind(metadata.tokens_used)
        .bind(metadata.first_user_message.as_deref().unwrap_or_default())
        .bind(metadata.archived_at.is_some())
        .bind(metadata.archived_at.map(datetime_to_epoch_seconds))
        .bind(metadata.git_sha.as_deref())
        .bind(metadata.git_branch.as_deref())
        .bind(metadata.git_origin_url.as_deref())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Insert or update memory summaries for a thread.
    ///
    /// This method always advances `updated_at`, even if summaries are unchanged.
    pub async fn upsert_thread_memory(
        &self,
        thread_id: ThreadId,
        trace_summary: &str,
        memory_summary: &str,
    ) -> anyhow::Result<ThreadMemory> {
        if self.get_thread(thread_id).await?.is_none() {
            return Err(anyhow::anyhow!("thread not found: {thread_id}"));
        }

        let updated_at = Utc::now().timestamp();
        sqlx::query(
            r#"
INSERT INTO thread_memory (
    thread_id,
    trace_summary,
    memory_summary,
    updated_at
) VALUES (?, ?, ?, ?)
ON CONFLICT(thread_id) DO UPDATE SET
    trace_summary = excluded.trace_summary,
    memory_summary = excluded.memory_summary,
    updated_at = CASE
        WHEN excluded.updated_at <= thread_memory.updated_at THEN thread_memory.updated_at + 1
        ELSE excluded.updated_at
    END
            "#,
        )
        .bind(thread_id.to_string())
        .bind(trace_summary)
        .bind(memory_summary)
        .bind(updated_at)
        .execute(self.pool.as_ref())
        .await?;

        self.get_thread_memory(thread_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to load upserted thread memory: {thread_id}"))
    }

    /// Get the last `n` memories for threads with an exact cwd match.
    pub async fn get_last_n_thread_memories_for_cwd(
        &self,
        cwd: &Path,
        n: usize,
    ) -> anyhow::Result<Vec<ThreadMemory>> {
        if n == 0 {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
SELECT
    m.thread_id,
    m.trace_summary,
    m.memory_summary,
    m.updated_at
FROM thread_memory AS m
INNER JOIN threads AS t ON t.id = m.thread_id
WHERE t.cwd = ?
ORDER BY m.updated_at DESC, m.thread_id DESC
LIMIT ?
            "#,
        )
        .bind(cwd.display().to_string())
        .bind(n as i64)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| ThreadMemoryRow::try_from_row(&row).and_then(ThreadMemory::try_from))
            .collect()
    }

    /// Persist dynamic tools for a thread if none have been stored yet.
    ///
    /// Dynamic tools are defined at thread start and should not change afterward.
    /// This only writes the first time we see tools for a given thread.
    pub async fn persist_dynamic_tools(
        &self,
        thread_id: ThreadId,
        tools: Option<&[DynamicToolSpec]>,
    ) -> anyhow::Result<()> {
        let Some(tools) = tools else {
            return Ok(());
        };
        if tools.is_empty() {
            return Ok(());
        }
        let thread_id = thread_id.to_string();
        let mut tx = self.pool.begin().await?;
        for (idx, tool) in tools.iter().enumerate() {
            let position = i64::try_from(idx).unwrap_or(i64::MAX);
            let input_schema = serde_json::to_string(&tool.input_schema)?;
            sqlx::query(
                r#"
INSERT INTO thread_dynamic_tools (
    thread_id,
    position,
    name,
    description,
    input_schema
) VALUES (?, ?, ?, ?, ?)
ON CONFLICT(thread_id, position) DO NOTHING
                "#,
            )
            .bind(thread_id.as_str())
            .bind(position)
            .bind(tool.name.as_str())
            .bind(tool.description.as_str())
            .bind(input_schema)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Apply rollout items incrementally using the underlying database.
    pub async fn apply_rollout_items(
        &self,
        builder: &ThreadMetadataBuilder,
        items: &[RolloutItem],
        otel: Option<&OtelManager>,
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut metadata = self
            .get_thread(builder.id)
            .await?
            .unwrap_or_else(|| builder.build(&self.default_provider));
        metadata.rollout_path = builder.rollout_path.clone();
        for item in items {
            apply_rollout_item(&mut metadata, item, &self.default_provider);
        }
        if let Some(updated_at) = file_modified_time_utc(builder.rollout_path.as_path()).await {
            metadata.updated_at = updated_at;
        }
        // Keep the thread upsert before dynamic tools to satisfy the foreign key constraint:
        // thread_dynamic_tools.thread_id -> threads.id.
        if let Err(err) = self.upsert_thread(&metadata).await {
            if let Some(otel) = otel {
                otel.counter(DB_ERROR_METRIC, 1, &[("stage", "apply_rollout_items")]);
            }
            return Err(err);
        }
        let dynamic_tools = extract_dynamic_tools(items);
        if let Some(dynamic_tools) = dynamic_tools
            && let Err(err) = self
                .persist_dynamic_tools(builder.id, dynamic_tools.as_deref())
                .await
        {
            if let Some(otel) = otel {
                otel.counter(DB_ERROR_METRIC, 1, &[("stage", "persist_dynamic_tools")]);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Mark a thread as archived using the underlying database.
    pub async fn mark_archived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        archived_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let Some(mut metadata) = self.get_thread(thread_id).await? else {
            return Ok(());
        };
        metadata.archived_at = Some(archived_at);
        metadata.rollout_path = rollout_path.to_path_buf();
        if let Some(updated_at) = file_modified_time_utc(rollout_path).await {
            metadata.updated_at = updated_at;
        }
        if metadata.id != thread_id {
            warn!(
                "thread id mismatch during archive: expected {thread_id}, got {}",
                metadata.id
            );
        }
        self.upsert_thread(&metadata).await
    }

    /// Mark a thread as unarchived using the underlying database.
    pub async fn mark_unarchived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
    ) -> anyhow::Result<()> {
        let Some(mut metadata) = self.get_thread(thread_id).await? else {
            return Ok(());
        };
        metadata.archived_at = None;
        metadata.rollout_path = rollout_path.to_path_buf();
        if let Some(updated_at) = file_modified_time_utc(rollout_path).await {
            metadata.updated_at = updated_at;
        }
        if metadata.id != thread_id {
            warn!(
                "thread id mismatch during unarchive: expected {thread_id}, got {}",
                metadata.id
            );
        }
        self.upsert_thread(&metadata).await
    }

    async fn ensure_backfill_state_row(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO backfill_state (id, status, last_watermark, last_success_at, updated_at)
VALUES (?, ?, NULL, NULL, ?)
ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(1_i64)
        .bind(crate::BackfillStatus::Pending.as_str())
        .bind(Utc::now().timestamp())
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
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

fn extract_dynamic_tools(items: &[RolloutItem]) -> Option<Option<Vec<DynamicToolSpec>>> {
    items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) => Some(meta_line.meta.dynamic_tools.clone()),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    })
}

async fn open_sqlite(path: &Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .log_statements(LevelFilter::Off);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

pub fn state_db_filename() -> String {
    format!("{STATE_DB_FILENAME}_{STATE_DB_VERSION}.sqlite")
}

pub fn state_db_path(codex_home: &Path) -> PathBuf {
    codex_home.join(state_db_filename())
}

async fn remove_legacy_state_files(codex_home: &Path) {
    let current_name = state_db_filename();
    let mut entries = match tokio::fs::read_dir(codex_home).await {
        Ok(entries) => entries,
        Err(err) => {
            warn!(
                "failed to read codex_home for state db cleanup {}: {err}",
                codex_home.display()
            );
            return;
        }
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if !entry
            .file_type()
            .await
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !should_remove_state_file(file_name.as_ref(), current_name.as_str()) {
            continue;
        }

        let legacy_path = entry.path();
        if let Err(err) = tokio::fs::remove_file(&legacy_path).await {
            warn!(
                "failed to remove legacy state db file {}: {err}",
                legacy_path.display()
            );
        }
    }
}

fn should_remove_state_file(file_name: &str, current_name: &str) -> bool {
    let mut base_name = file_name;
    for suffix in ["-wal", "-shm", "-journal"] {
        if let Some(stripped) = file_name.strip_suffix(suffix) {
            base_name = stripped;
            break;
        }
    }
    if base_name == current_name {
        return false;
    }
    let unversioned_name = format!("{STATE_DB_FILENAME}.sqlite");
    if base_name == unversioned_name {
        return true;
    }

    let Some(version_with_extension) = base_name.strip_prefix(&format!("{STATE_DB_FILENAME}_"))
    else {
        return false;
    };
    let Some(version_suffix) = version_with_extension.strip_suffix(".sqlite") else {
        return false;
    };
    !version_suffix.is_empty() && version_suffix.chars().all(|ch| ch.is_ascii_digit())
}

fn push_thread_filters<'a>(
    builder: &mut QueryBuilder<'a, Sqlite>,
    archived_only: bool,
    allowed_sources: &'a [String],
    model_providers: Option<&'a [String]>,
    anchor: Option<&crate::Anchor>,
    sort_key: SortKey,
) {
    builder.push(" WHERE 1 = 1");
    if archived_only {
        builder.push(" AND archived = 1");
    } else {
        builder.push(" AND archived = 0");
    }
    builder.push(" AND first_user_message <> ''");
    if !allowed_sources.is_empty() {
        builder.push(" AND source IN (");
        let mut separated = builder.separated(", ");
        for source in allowed_sources {
            separated.push_bind(source);
        }
        separated.push_unseparated(")");
    }
    if let Some(model_providers) = model_providers
        && !model_providers.is_empty()
    {
        builder.push(" AND model_provider IN (");
        let mut separated = builder.separated(", ");
        for provider in model_providers {
            separated.push_bind(provider);
        }
        separated.push_unseparated(")");
    }
    if let Some(anchor) = anchor {
        let anchor_ts = datetime_to_epoch_seconds(anchor.ts);
        let column = match sort_key {
            SortKey::CreatedAt => "created_at",
            SortKey::UpdatedAt => "updated_at",
        };
        builder.push(" AND (");
        builder.push(column);
        builder.push(" < ");
        builder.push_bind(anchor_ts);
        builder.push(" OR (");
        builder.push(column);
        builder.push(" = ");
        builder.push_bind(anchor_ts);
        builder.push(" AND id < ");
        builder.push_bind(anchor.id.to_string());
        builder.push("))");
    }
}

fn push_thread_order_and_limit(
    builder: &mut QueryBuilder<'_, Sqlite>,
    sort_key: SortKey,
    limit: usize,
) {
    let order_column = match sort_key {
        SortKey::CreatedAt => "created_at",
        SortKey::UpdatedAt => "updated_at",
    };
    builder.push(" ORDER BY ");
    builder.push(order_column);
    builder.push(" DESC, id DESC");
    builder.push(" LIMIT ");
    builder.push_bind(limit as i64);
}

#[cfg(test)]
mod tests {
    use super::STATE_DB_FILENAME;
    use super::STATE_DB_VERSION;
    use super::StateRuntime;
    use super::ThreadMetadata;
    use super::state_db_filename;
    use chrono::DateTime;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use sqlx::Row;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;
    use uuid::Uuid;

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!(
            "codex-state-runtime-test-{nanos}-{}",
            Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn init_removes_legacy_state_db_files() {
        let codex_home = unique_temp_dir();
        tokio::fs::create_dir_all(&codex_home)
            .await
            .expect("create codex_home");

        let current_name = state_db_filename();
        let previous_version = STATE_DB_VERSION.saturating_sub(1);
        let unversioned_name = format!("{STATE_DB_FILENAME}.sqlite");
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let path = codex_home.join(format!("{unversioned_name}{suffix}"));
            tokio::fs::write(path, b"legacy")
                .await
                .expect("write legacy");
            let old_version_path = codex_home.join(format!(
                "{STATE_DB_FILENAME}_{previous_version}.sqlite{suffix}"
            ));
            tokio::fs::write(old_version_path, b"old_version")
                .await
                .expect("write old version");
        }
        let unrelated_path = codex_home.join("state.sqlite_backup");
        tokio::fs::write(&unrelated_path, b"keep")
            .await
            .expect("write unrelated");
        let numeric_path = codex_home.join("123");
        tokio::fs::write(&numeric_path, b"keep")
            .await
            .expect("write numeric");

        let _runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        for suffix in ["", "-wal", "-shm", "-journal"] {
            let legacy_path = codex_home.join(format!("{unversioned_name}{suffix}"));
            assert_eq!(
                tokio::fs::try_exists(&legacy_path)
                    .await
                    .expect("check legacy path"),
                false
            );
            let old_version_path = codex_home.join(format!(
                "{STATE_DB_FILENAME}_{previous_version}.sqlite{suffix}"
            ));
            assert_eq!(
                tokio::fs::try_exists(&old_version_path)
                    .await
                    .expect("check old version path"),
                false
            );
        }
        assert_eq!(
            tokio::fs::try_exists(codex_home.join(current_name))
                .await
                .expect("check new db path"),
            true
        );
        assert_eq!(
            tokio::fs::try_exists(&unrelated_path)
                .await
                .expect("check unrelated path"),
            true
        );
        assert_eq!(
            tokio::fs::try_exists(&numeric_path)
                .await
                .expect("check numeric path"),
            true
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn backfill_state_persists_progress_and_completion() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let initial = runtime
            .get_backfill_state()
            .await
            .expect("get initial backfill state");
        assert_eq!(initial.status, crate::BackfillStatus::Pending);
        assert_eq!(initial.last_watermark, None);
        assert_eq!(initial.last_success_at, None);

        runtime
            .mark_backfill_running()
            .await
            .expect("mark backfill running");
        runtime
            .checkpoint_backfill("sessions/2026/01/27/rollout-a.jsonl")
            .await
            .expect("checkpoint backfill");

        let running = runtime
            .get_backfill_state()
            .await
            .expect("get running backfill state");
        assert_eq!(running.status, crate::BackfillStatus::Running);
        assert_eq!(
            running.last_watermark,
            Some("sessions/2026/01/27/rollout-a.jsonl".to_string())
        );
        assert_eq!(running.last_success_at, None);

        runtime
            .mark_backfill_complete(Some("sessions/2026/01/28/rollout-b.jsonl"))
            .await
            .expect("mark backfill complete");
        let completed = runtime
            .get_backfill_state()
            .await
            .expect("get completed backfill state");
        assert_eq!(completed.status, crate::BackfillStatus::Complete);
        assert_eq!(
            completed.last_watermark,
            Some("sessions/2026/01/28/rollout-b.jsonl".to_string())
        );
        assert!(completed.last_success_at.is_some());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn upsert_and_get_thread_memory() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let metadata = test_thread_metadata(&codex_home, thread_id, codex_home.join("a"));
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("upsert thread");

        assert_eq!(
            runtime
                .get_thread_memory(thread_id)
                .await
                .expect("get memory before insert"),
            None
        );

        let inserted = runtime
            .upsert_thread_memory(thread_id, "trace one", "memory one")
            .await
            .expect("upsert memory");
        assert_eq!(inserted.thread_id, thread_id);
        assert_eq!(inserted.trace_summary, "trace one");
        assert_eq!(inserted.memory_summary, "memory one");

        let updated = runtime
            .upsert_thread_memory(thread_id, "trace two", "memory two")
            .await
            .expect("update memory");
        assert_eq!(updated.thread_id, thread_id);
        assert_eq!(updated.trace_summary, "trace two");
        assert_eq!(updated.memory_summary, "memory two");
        assert!(
            updated.updated_at >= inserted.updated_at,
            "updated_at should not move backward"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_last_n_thread_memories_for_cwd_matches_exactly() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let cwd_a = codex_home.join("workspace-a");
        let cwd_b = codex_home.join("workspace-b");
        // Use deterministic IDs to keep secondary sort-order assertions stable when
        // multiple updates share the same second-level timestamp.
        let t1 = ThreadId::from_string("ffffffff-ffff-4fff-8fff-ffffffffffff").expect("thread id");
        let t2 = ThreadId::from_string("00000000-0000-4000-8000-000000000000").expect("thread id");
        let t3 = ThreadId::from_string("11111111-1111-4111-8111-111111111111").expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, t1, cwd_a.clone()))
            .await
            .expect("upsert thread t1");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, t2, cwd_a.clone()))
            .await
            .expect("upsert thread t2");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, t3, cwd_b.clone()))
            .await
            .expect("upsert thread t3");

        let first = runtime
            .upsert_thread_memory(t1, "trace-1", "memory-1")
            .await
            .expect("upsert t1 memory");
        runtime
            .upsert_thread_memory(t2, "trace-2", "memory-2")
            .await
            .expect("upsert t2 memory");
        runtime
            .upsert_thread_memory(t3, "trace-3", "memory-3")
            .await
            .expect("upsert t3 memory");
        // Ensure deterministic ordering even when updates happen in the same second.
        runtime
            .upsert_thread_memory(t1, "trace-1b", "memory-1b")
            .await
            .expect("upsert t1 memory again");

        let cwd_a_memories = runtime
            .get_last_n_thread_memories_for_cwd(cwd_a.as_path(), 2)
            .await
            .expect("list cwd a memories");
        assert_eq!(cwd_a_memories.len(), 2);
        assert_eq!(cwd_a_memories[0].thread_id, t1);
        assert_eq!(cwd_a_memories[0].trace_summary, "trace-1b");
        assert_eq!(cwd_a_memories[0].memory_summary, "memory-1b");
        assert_eq!(cwd_a_memories[1].thread_id, t2);
        assert!(cwd_a_memories[0].updated_at >= first.updated_at);

        let cwd_b_memories = runtime
            .get_last_n_thread_memories_for_cwd(cwd_b.as_path(), 10)
            .await
            .expect("list cwd b memories");
        assert_eq!(cwd_b_memories.len(), 1);
        assert_eq!(cwd_b_memories[0].thread_id, t3);

        let none = runtime
            .get_last_n_thread_memories_for_cwd(codex_home.join("missing").as_path(), 10)
            .await
            .expect("list missing cwd memories");
        assert_eq!(none, Vec::new());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn upsert_thread_memory_errors_for_unknown_thread() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let unknown_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let err = runtime
            .upsert_thread_memory(unknown_thread_id, "trace", "memory")
            .await
            .expect_err("unknown thread should fail");
        assert!(
            err.to_string().contains("thread not found"),
            "error should mention missing thread: {err}"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_last_n_thread_memories_for_cwd_zero_returns_empty() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let cwd = codex_home.join("workspace");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, thread_id, cwd.clone()))
            .await
            .expect("upsert thread");
        runtime
            .upsert_thread_memory(thread_id, "trace", "memory")
            .await
            .expect("upsert memory");

        let memories = runtime
            .get_last_n_thread_memories_for_cwd(cwd.as_path(), 0)
            .await
            .expect("query memories");
        assert_eq!(memories, Vec::new());

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn get_last_n_thread_memories_for_cwd_does_not_prefix_match() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let cwd_exact = codex_home.join("workspace");
        let cwd_prefix = codex_home.join("workspace-child");
        let t_exact = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let t_prefix = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                t_exact,
                cwd_exact.clone(),
            ))
            .await
            .expect("upsert exact thread");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                t_prefix,
                cwd_prefix.clone(),
            ))
            .await
            .expect("upsert prefix thread");
        runtime
            .upsert_thread_memory(t_exact, "trace-exact", "memory-exact")
            .await
            .expect("upsert exact memory");
        runtime
            .upsert_thread_memory(t_prefix, "trace-prefix", "memory-prefix")
            .await
            .expect("upsert prefix memory");

        let exact_only = runtime
            .get_last_n_thread_memories_for_cwd(cwd_exact.as_path(), 10)
            .await
            .expect("query exact cwd");
        assert_eq!(exact_only.len(), 1);
        assert_eq!(exact_only[0].thread_id, t_exact);
        assert_eq!(exact_only[0].memory_summary, "memory-exact");

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn deleting_thread_cascades_thread_memory() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let cwd = codex_home.join("workspace");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, thread_id, cwd))
            .await
            .expect("upsert thread");
        runtime
            .upsert_thread_memory(thread_id, "trace", "memory")
            .await
            .expect("upsert memory");

        let count_before =
            sqlx::query("SELECT COUNT(*) AS count FROM thread_memory WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("count before delete")
                .try_get::<i64, _>("count")
                .expect("count value");
        assert_eq!(count_before, 1);

        sqlx::query("DELETE FROM threads WHERE id = ?")
            .bind(thread_id.to_string())
            .execute(runtime.pool.as_ref())
            .await
            .expect("delete thread");

        let count_after =
            sqlx::query("SELECT COUNT(*) AS count FROM thread_memory WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("count after delete")
                .try_get::<i64, _>("count")
                .expect("count value");
        assert_eq!(count_after, 0);
        assert_eq!(
            runtime
                .get_thread_memory(thread_id)
                .await
                .expect("get memory after delete"),
            None
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    fn test_thread_metadata(
        codex_home: &Path,
        thread_id: ThreadId,
        cwd: PathBuf,
    ) -> ThreadMetadata {
        let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");
        ThreadMetadata {
            id: thread_id,
            rollout_path: codex_home.join(format!("rollout-{thread_id}.jsonl")),
            created_at: now,
            updated_at: now,
            source: "cli".to_string(),
            model_provider: "test-provider".to_string(),
            cwd,
            cli_version: "0.0.0".to_string(),
            title: String::new(),
            sandbox_policy: crate::extract::enum_to_string(&SandboxPolicy::ReadOnly),
            approval_mode: crate::extract::enum_to_string(&AskForApproval::OnRequest),
            tokens_used: 0,
            first_user_message: Some("hello".to_string()),
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        }
    }
}
