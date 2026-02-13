use crate::DB_ERROR_METRIC;
use crate::LogEntry;
use crate::LogQuery;
use crate::LogRow;
use crate::METRIC_DB_INIT;
use crate::STATE_DB_FILENAME;
use crate::STATE_DB_VERSION;
use crate::SortKey;
use crate::ThreadMetadata;
use crate::ThreadMetadataBuilder;
use crate::ThreadsPage;
use crate::apply_rollout_item;
use crate::migrations::MIGRATOR;
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
use uuid::Uuid;

mod memories;
// Memory-specific CRUD and phase job lifecycle methods live in `runtime/memories.rs`.

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

    /// Attempt to claim ownership of rollout metadata backfill.
    ///
    /// Returns `true` when this runtime claimed the backfill worker slot.
    /// Returns `false` if backfill is already complete or currently owned by a
    /// non-expired worker.
    pub async fn try_claim_backfill(&self, lease_seconds: i64) -> anyhow::Result<bool> {
        self.ensure_backfill_state_row().await?;
        let now = Utc::now().timestamp();
        let lease_cutoff = now.saturating_sub(lease_seconds.max(0));
        let result = sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, updated_at = ?
WHERE id = 1
  AND status != ?
  AND (status != ? OR updated_at <= ?)
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(now)
        .bind(crate::BackfillStatus::Complete.as_str())
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(lease_cutoff)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
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

    /// Delete a thread metadata row by id.
    pub async fn delete_thread(&self, thread_id: ThreadId) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM threads WHERE id = ?")
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected())
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
    use super::StateRuntime;
    use super::ThreadMetadata;
    use super::state_db_filename;
    use crate::STATE_DB_FILENAME;
    use crate::STATE_DB_VERSION;
    use crate::model::Phase2JobClaimOutcome;
    use crate::model::Stage1JobClaimOutcome;
    use crate::model::Stage1StartupClaimParams;
    use chrono::DateTime;
    use chrono::Duration;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use sqlx::Row;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
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
    async fn backfill_claim_is_singleton_until_stale_and_blocked_when_complete() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let claimed = runtime
            .try_claim_backfill(3600)
            .await
            .expect("initial backfill claim");
        assert_eq!(claimed, true);

        let duplicate_claim = runtime
            .try_claim_backfill(3600)
            .await
            .expect("duplicate backfill claim");
        assert_eq!(duplicate_claim, false);

        let stale_updated_at = Utc::now().timestamp().saturating_sub(10_000);
        sqlx::query(
            r#"
UPDATE backfill_state
SET status = ?, updated_at = ?
WHERE id = 1
            "#,
        )
        .bind(crate::BackfillStatus::Running.as_str())
        .bind(stale_updated_at)
        .execute(runtime.pool.as_ref())
        .await
        .expect("force stale backfill lease");

        let stale_claim = runtime
            .try_claim_backfill(10)
            .await
            .expect("stale backfill claim");
        assert_eq!(stale_claim, true);

        runtime
            .mark_backfill_complete(None)
            .await
            .expect("mark complete");
        let claim_after_complete = runtime
            .try_claim_backfill(3600)
            .await
            .expect("claim after complete");
        assert_eq!(claim_after_complete, false);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_claim_skips_when_up_to_date() {
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

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        let claim = runtime
            .try_claim_stage1_job(thread_id, owner_a, 100, 3600, 64)
            .await
            .expect("claim stage1 job");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };

        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_id, ownership_token.as_str(), 100, "raw", "sum")
                .await
                .expect("mark stage1 succeeded"),
            "stage1 success should finalize for current token"
        );

        let up_to_date = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim stage1 up-to-date");
        assert_eq!(up_to_date, Stage1JobClaimOutcome::SkippedUpToDate);

        let needs_rerun = runtime
            .try_claim_stage1_job(thread_id, owner_b, 101, 3600, 64)
            .await
            .expect("claim stage1 newer source");
        assert!(
            matches!(needs_rerun, Stage1JobClaimOutcome::Claimed { .. }),
            "newer source_updated_at should be claimable"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_running_stale_can_be_stolen_but_fresh_running_is_skipped() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let cwd = codex_home.join("workspace");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, thread_id, cwd))
            .await
            .expect("upsert thread");

        let claim_a = runtime
            .try_claim_stage1_job(thread_id, owner_a, 100, 3600, 64)
            .await
            .expect("claim a");
        assert!(matches!(claim_a, Stage1JobClaimOutcome::Claimed { .. }));

        let claim_b_fresh = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim b fresh");
        assert_eq!(claim_b_fresh, Stage1JobClaimOutcome::SkippedRunning);

        sqlx::query("UPDATE jobs SET lease_until = 0 WHERE kind = 'memory_stage1' AND job_key = ?")
            .bind(thread_id.to_string())
            .execute(runtime.pool.as_ref())
            .await
            .expect("force stale lease");

        let claim_b_stale = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim b stale");
        assert!(matches!(
            claim_b_stale,
            Stage1JobClaimOutcome::Claimed { .. }
        ));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_concurrent_claim_for_same_thread_is_conflict_safe() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let thread_id_a = thread_id;
        let thread_id_b = thread_id;
        let runtime_a = Arc::clone(&runtime);
        let runtime_b = Arc::clone(&runtime);
        let claim_with_retry = |runtime: Arc<StateRuntime>,
                                thread_id: ThreadId,
                                owner: ThreadId| async move {
            for attempt in 0..5 {
                match runtime
                    .try_claim_stage1_job(thread_id, owner, 100, 3_600, 64)
                    .await
                {
                    Ok(outcome) => return outcome,
                    Err(err) if err.to_string().contains("database is locked") && attempt < 4 => {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(err) => panic!("claim stage1 should not fail: {err}"),
                }
            }
            panic!("claim stage1 should have returned within retry budget")
        };

        let (claim_a, claim_b) = tokio::join!(
            claim_with_retry(runtime_a, thread_id_a, owner_a),
            claim_with_retry(runtime_b, thread_id_b, owner_b),
        );

        let claim_outcomes = vec![claim_a, claim_b];
        let claimed_count = claim_outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Stage1JobClaimOutcome::Claimed { .. }))
            .count();
        assert_eq!(claimed_count, 1);
        assert!(
            claim_outcomes.iter().all(|outcome| {
                matches!(
                    outcome,
                    Stage1JobClaimOutcome::Claimed { .. } | Stage1JobClaimOutcome::SkippedRunning
                )
            }),
            "unexpected claim outcomes: {claim_outcomes:?}"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_concurrent_claims_respect_running_cap() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let runtime_a = Arc::clone(&runtime);
        let runtime_b = Arc::clone(&runtime);

        let (claim_a, claim_b) = tokio::join!(
            async move {
                runtime_a
                    .try_claim_stage1_job(thread_a, owner_a, 100, 3_600, 1)
                    .await
                    .expect("claim stage1 thread a")
            },
            async move {
                runtime_b
                    .try_claim_stage1_job(thread_b, owner_b, 101, 3_600, 1)
                    .await
                    .expect("claim stage1 thread b")
            },
        );

        let claim_outcomes = vec![claim_a, claim_b];
        let claimed_count = claim_outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Stage1JobClaimOutcome::Claimed { .. }))
            .count();
        assert_eq!(claimed_count, 1);
        assert!(
            claim_outcomes
                .iter()
                .any(|outcome| { matches!(outcome, Stage1JobClaimOutcome::SkippedRunning) }),
            "one concurrent claim should be throttled by running cap: {claim_outcomes:?}"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_filters_by_age_idle_and_current_thread() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let now = Utc::now();
        let fresh_at = now - Duration::hours(1);
        let just_under_idle_at = now - Duration::hours(12) + Duration::minutes(1);
        let eligible_idle_at = now - Duration::hours(12) - Duration::minutes(1);
        let old_at = now - Duration::days(31);

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        let fresh_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("fresh thread id");
        let just_under_idle_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("just under idle thread id");
        let eligible_idle_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("eligible idle thread id");
        let old_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("old thread id");

        let mut current =
            test_thread_metadata(&codex_home, current_thread_id, codex_home.join("current"));
        current.created_at = now;
        current.updated_at = now;
        runtime
            .upsert_thread(&current)
            .await
            .expect("upsert current");

        let mut fresh =
            test_thread_metadata(&codex_home, fresh_thread_id, codex_home.join("fresh"));
        fresh.created_at = fresh_at;
        fresh.updated_at = fresh_at;
        runtime.upsert_thread(&fresh).await.expect("upsert fresh");

        let mut just_under_idle = test_thread_metadata(
            &codex_home,
            just_under_idle_thread_id,
            codex_home.join("just-under-idle"),
        );
        just_under_idle.created_at = just_under_idle_at;
        just_under_idle.updated_at = just_under_idle_at;
        runtime
            .upsert_thread(&just_under_idle)
            .await
            .expect("upsert just-under-idle");

        let mut eligible_idle = test_thread_metadata(
            &codex_home,
            eligible_idle_thread_id,
            codex_home.join("eligible-idle"),
        );
        eligible_idle.created_at = eligible_idle_at;
        eligible_idle.updated_at = eligible_idle_at;
        runtime
            .upsert_thread(&eligible_idle)
            .await
            .expect("upsert eligible-idle");

        let mut old = test_thread_metadata(&codex_home, old_thread_id, codex_home.join("old"));
        old.created_at = old_at;
        old.updated_at = old_at;
        runtime.upsert_thread(&old).await.expect("upsert old");

        let allowed_sources = vec!["cli".to_string()];
        let claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 1,
                    max_claimed: 5,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 jobs");

        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].thread.id, eligible_idle_thread_id);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_prefilters_threads_with_up_to_date_memory() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let now = Utc::now();
        let eligible_newer_at = now - Duration::hours(13);
        let eligible_older_at = now - Duration::hours(14);

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        let up_to_date_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("up-to-date thread id");
        let stale_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("stale thread id");
        let worker_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("worker id");

        let mut current =
            test_thread_metadata(&codex_home, current_thread_id, codex_home.join("current"));
        current.created_at = now;
        current.updated_at = now;
        runtime
            .upsert_thread(&current)
            .await
            .expect("upsert current thread");

        let mut up_to_date = test_thread_metadata(
            &codex_home,
            up_to_date_thread_id,
            codex_home.join("up-to-date"),
        );
        up_to_date.created_at = eligible_newer_at;
        up_to_date.updated_at = eligible_newer_at;
        runtime
            .upsert_thread(&up_to_date)
            .await
            .expect("upsert up-to-date thread");

        let up_to_date_claim = runtime
            .try_claim_stage1_job(
                up_to_date_thread_id,
                worker_id,
                up_to_date.updated_at.timestamp(),
                3600,
                64,
            )
            .await
            .expect("claim up-to-date thread for seed");
        let up_to_date_token = match up_to_date_claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected seed claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    up_to_date_thread_id,
                    up_to_date_token.as_str(),
                    up_to_date.updated_at.timestamp(),
                    "raw",
                    "summary",
                )
                .await
                .expect("mark up-to-date thread succeeded"),
            "seed stage1 success should complete for up-to-date thread"
        );

        let mut stale =
            test_thread_metadata(&codex_home, stale_thread_id, codex_home.join("stale"));
        stale.created_at = eligible_older_at;
        stale.updated_at = eligible_older_at;
        runtime
            .upsert_thread(&stale)
            .await
            .expect("upsert stale thread");

        let allowed_sources = vec!["cli".to_string()];
        let claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 1,
                    max_claimed: 1,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 startup jobs");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].thread.id, stale_thread_id);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_enforces_global_running_cap() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                current_thread_id,
                codex_home.join("current"),
            ))
            .await
            .expect("upsert current");

        let now = Utc::now();
        let started_at = now.timestamp();
        let lease_until = started_at + 3600;
        let eligible_at = now - Duration::hours(13);
        let existing_running = 10usize;
        let total_candidates = 80usize;

        for idx in 0..total_candidates {
            let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
            let mut metadata = test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join(format!("thread-{idx}")),
            );
            metadata.created_at = eligible_at - Duration::seconds(idx as i64);
            metadata.updated_at = eligible_at - Duration::seconds(idx as i64);
            runtime
                .upsert_thread(&metadata)
                .await
                .expect("upsert thread");

            if idx < existing_running {
                sqlx::query(
                    r#"
INSERT INTO jobs (
    kind,
    job_key,
    status,
    worker_id,
    ownership_token,
    started_at,
    finished_at,
    lease_until,
    retry_at,
    retry_remaining,
    last_error,
    input_watermark,
    last_success_watermark
) VALUES (?, ?, 'running', ?, ?, ?, NULL, ?, NULL, ?, NULL, ?, NULL)
                    "#,
                )
                .bind("memory_stage1")
                .bind(thread_id.to_string())
                .bind(current_thread_id.to_string())
                .bind(Uuid::new_v4().to_string())
                .bind(started_at)
                .bind(lease_until)
                .bind(3)
                .bind(metadata.updated_at.timestamp())
                .execute(runtime.pool.as_ref())
                .await
                .expect("seed running stage1 job");
            }
        }

        let allowed_sources = vec!["cli".to_string()];
        let claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 200,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 jobs");
        assert_eq!(claims.len(), 54);

        let running_count = sqlx::query(
            r#"
SELECT COUNT(*) AS count
FROM jobs
WHERE kind = 'memory_stage1'
  AND status = 'running'
  AND lease_until IS NOT NULL
  AND lease_until > ?
            "#,
        )
        .bind(Utc::now().timestamp())
        .fetch_one(runtime.pool.as_ref())
        .await
        .expect("count running stage1 jobs")
        .try_get::<i64, _>("count")
        .expect("running count value");
        assert_eq!(running_count, 64);

        let more_claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 200,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3600,
                },
            )
            .await
            .expect("claim stage1 jobs with cap reached");
        assert_eq!(more_claims.len(), 0);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn claim_stage1_jobs_processes_two_full_batches_across_startup_passes() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let current_thread_id =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("current thread id");
        let mut current =
            test_thread_metadata(&codex_home, current_thread_id, codex_home.join("current"));
        current.created_at = Utc::now();
        current.updated_at = Utc::now();
        runtime
            .upsert_thread(&current)
            .await
            .expect("upsert current");

        let eligible_at = Utc::now() - Duration::hours(13);
        for idx in 0..200 {
            let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
            let mut metadata = test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join(format!("thread-{idx}")),
            );
            metadata.created_at = eligible_at - Duration::seconds(idx as i64);
            metadata.updated_at = eligible_at - Duration::seconds(idx as i64);
            runtime
                .upsert_thread(&metadata)
                .await
                .expect("upsert eligible thread");
        }

        let allowed_sources = vec!["cli".to_string()];
        let first_claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 5_000,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3_600,
                },
            )
            .await
            .expect("first stage1 startup claim");
        assert_eq!(first_claims.len(), 64);

        for claim in first_claims {
            assert!(
                runtime
                    .mark_stage1_job_succeeded(
                        claim.thread.id,
                        claim.ownership_token.as_str(),
                        claim.thread.updated_at.timestamp(),
                        "raw",
                        "summary",
                    )
                    .await
                    .expect("mark first-batch stage1 success"),
                "first batch stage1 completion should succeed"
            );
        }

        let second_claims = runtime
            .claim_stage1_jobs_for_startup(
                current_thread_id,
                Stage1StartupClaimParams {
                    scan_limit: 5_000,
                    max_claimed: 64,
                    max_age_days: 30,
                    min_rollout_idle_hours: 12,
                    allowed_sources: allowed_sources.as_slice(),
                    lease_seconds: 3_600,
                },
            )
            .await
            .expect("second stage1 startup claim");
        assert_eq!(second_claims.len(), 64);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_output_cascades_on_thread_delete() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let cwd = codex_home.join("workspace");
        runtime
            .upsert_thread(&test_thread_metadata(&codex_home, thread_id, cwd))
            .await
            .expect("upsert thread");

        let claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim stage1");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_id, ownership_token.as_str(), 100, "raw", "sum")
                .await
                .expect("mark stage1 succeeded"),
            "mark stage1 succeeded should write stage1_outputs"
        );

        let count_before =
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
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
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("count after delete")
                .try_get::<i64, _>("count")
                .expect("count value");
        assert_eq!(count_after, 0);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_stage1_job_succeeded_no_output_tracks_watermark_without_persisting_output() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        let claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3600, 64)
            .await
            .expect("claim stage1");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded_no_output(thread_id, ownership_token.as_str())
                .await
                .expect("mark stage1 succeeded without output"),
            "stage1 no-output success should complete the job"
        );

        let output_row_count =
            sqlx::query("SELECT COUNT(*) AS count FROM stage1_outputs WHERE thread_id = ?")
                .bind(thread_id.to_string())
                .fetch_one(runtime.pool.as_ref())
                .await
                .expect("load stage1 output count")
                .try_get::<i64, _>("count")
                .expect("stage1 output count");
        assert_eq!(
            output_row_count, 0,
            "stage1 no-output success should not persist empty stage1 outputs"
        );

        let up_to_date = runtime
            .try_claim_stage1_job(thread_id, owner_b, 100, 3600, 64)
            .await
            .expect("claim stage1 up-to-date");
        assert_eq!(up_to_date, Stage1JobClaimOutcome::SkippedUpToDate);

        let claim_phase2 = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (phase2_token, phase2_input_watermark) = match claim_phase2 {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome after no-output success: {other:?}"),
        };
        assert_eq!(phase2_input_watermark, 100);
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(phase2_token.as_str(), phase2_input_watermark,)
                .await
                .expect("mark phase2 succeeded after no-output")
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn stage1_retry_exhaustion_does_not_block_newer_watermark() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id,
                codex_home.join("workspace"),
            ))
            .await
            .expect("upsert thread");

        for attempt in 0..3 {
            let claim = runtime
                .try_claim_stage1_job(thread_id, owner, 100, 3_600, 64)
                .await
                .expect("claim stage1 for retry exhaustion");
            let ownership_token = match claim {
                Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
                other => panic!(
                    "attempt {} should claim stage1 before retries are exhausted: {other:?}",
                    attempt + 1
                ),
            };
            assert!(
                runtime
                    .mark_stage1_job_failed(thread_id, ownership_token.as_str(), "boom", 0)
                    .await
                    .expect("mark stage1 failed"),
                "attempt {} should decrement retry budget",
                attempt + 1
            );
        }

        let exhausted_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 100, 3_600, 64)
            .await
            .expect("claim stage1 after retry exhaustion");
        assert_eq!(
            exhausted_claim,
            Stage1JobClaimOutcome::SkippedRetryExhausted
        );

        let newer_source_claim = runtime
            .try_claim_stage1_job(thread_id, owner, 101, 3_600, 64)
            .await
            .expect("claim stage1 with newer source watermark");
        assert!(
            matches!(newer_source_claim, Stage1JobClaimOutcome::Claimed { .. }),
            "newer source watermark should reset retry budget and be claimable"
        );

        let job_row = sqlx::query(
            "SELECT retry_remaining, input_watermark FROM jobs WHERE kind = ? AND job_key = ?",
        )
        .bind("memory_stage1")
        .bind(thread_id.to_string())
        .fetch_one(runtime.pool.as_ref())
        .await
        .expect("load stage1 job row after newer-source claim");
        assert_eq!(
            job_row
                .try_get::<i64, _>("retry_remaining")
                .expect("retry_remaining"),
            3
        );
        assert_eq!(
            job_row
                .try_get::<i64, _>("input_watermark")
                .expect("input_watermark"),
            101
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_global_consolidation_reruns_when_watermark_advances() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        runtime
            .enqueue_global_consolidation(100)
            .await
            .expect("enqueue global consolidation");

        let claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2");
        let (ownership_token, input_watermark) = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected phase2 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(ownership_token.as_str(), input_watermark)
                .await
                .expect("mark phase2 succeeded"),
            "phase2 success should finalize for current token"
        );

        let claim_up_to_date = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2 up-to-date");
        assert_eq!(claim_up_to_date, Phase2JobClaimOutcome::SkippedNotDirty);

        runtime
            .enqueue_global_consolidation(101)
            .await
            .expect("enqueue global consolidation again");

        let claim_rerun = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim phase2 rerun");
        assert!(
            matches!(claim_rerun, Phase2JobClaimOutcome::Claimed { .. }),
            "advanced watermark should be claimable"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn list_stage1_outputs_for_global_returns_latest_outputs() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_id_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let claim = runtime
            .try_claim_stage1_job(thread_id_a, owner, 100, 3600, 64)
            .await
            .expect("claim stage1 a");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id_a,
                    ownership_token.as_str(),
                    100,
                    "raw memory a",
                    "summary a",
                )
                .await
                .expect("mark stage1 succeeded a"),
            "stage1 success should persist output a"
        );

        let claim = runtime
            .try_claim_stage1_job(thread_id_b, owner, 101, 3600, 64)
            .await
            .expect("claim stage1 b");
        let ownership_token = match claim {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(
                    thread_id_b,
                    ownership_token.as_str(),
                    101,
                    "raw memory b",
                    "summary b",
                )
                .await
                .expect("mark stage1 succeeded b"),
            "stage1 success should persist output b"
        );

        let outputs = runtime
            .list_stage1_outputs_for_global(10)
            .await
            .expect("list stage1 outputs for global");
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].thread_id, thread_id_b);
        assert_eq!(outputs[0].rollout_summary, "summary b");
        assert_eq!(outputs[0].cwd, codex_home.join("workspace-b"));
        assert_eq!(outputs[1].thread_id, thread_id_a);
        assert_eq!(outputs[1].rollout_summary, "summary a");
        assert_eq!(outputs[1].cwd, codex_home.join("workspace-a"));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn list_stage1_outputs_for_global_skips_empty_payloads() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_id_non_empty =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        let thread_id_empty =
            ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_non_empty,
                codex_home.join("workspace-non-empty"),
            ))
            .await
            .expect("upsert non-empty thread");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_id_empty,
                codex_home.join("workspace-empty"),
            ))
            .await
            .expect("upsert empty thread");

        sqlx::query(
            r#"
INSERT INTO stage1_outputs (thread_id, source_updated_at, raw_memory, rollout_summary, generated_at)
VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(thread_id_non_empty.to_string())
        .bind(100_i64)
        .bind("raw memory")
        .bind("summary")
        .bind(100_i64)
        .execute(runtime.pool.as_ref())
        .await
        .expect("insert non-empty stage1 output");
        sqlx::query(
            r#"
INSERT INTO stage1_outputs (thread_id, source_updated_at, raw_memory, rollout_summary, generated_at)
VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(thread_id_empty.to_string())
        .bind(101_i64)
        .bind("")
        .bind("")
        .bind(101_i64)
        .execute(runtime.pool.as_ref())
        .await
        .expect("insert empty stage1 output");

        let outputs = runtime
            .list_stage1_outputs_for_global(1)
            .await
            .expect("list stage1 outputs for global");
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].thread_id, thread_id_non_empty);
        assert_eq!(outputs[0].rollout_summary, "summary");
        assert_eq!(outputs[0].cwd, codex_home.join("workspace-non-empty"));

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn mark_stage1_job_succeeded_enqueues_global_consolidation() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        let thread_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id a");
        let thread_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("thread id b");
        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner id");

        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_a,
                codex_home.join("workspace-a"),
            ))
            .await
            .expect("upsert thread a");
        runtime
            .upsert_thread(&test_thread_metadata(
                &codex_home,
                thread_b,
                codex_home.join("workspace-b"),
            ))
            .await
            .expect("upsert thread b");

        let claim_a = runtime
            .try_claim_stage1_job(thread_a, owner, 100, 3600, 64)
            .await
            .expect("claim stage1 a");
        let token_a = match claim_a {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome for thread a: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_a, token_a.as_str(), 100, "raw-a", "summary-a")
                .await
                .expect("mark stage1 succeeded a"),
            "stage1 success should persist output for thread a"
        );

        let claim_b = runtime
            .try_claim_stage1_job(thread_b, owner, 101, 3600, 64)
            .await
            .expect("claim stage1 b");
        let token_b = match claim_b {
            Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
            other => panic!("unexpected stage1 claim outcome for thread b: {other:?}"),
        };
        assert!(
            runtime
                .mark_stage1_job_succeeded(thread_b, token_b.as_str(), 101, "raw-b", "summary-b")
                .await
                .expect("mark stage1 succeeded b"),
            "stage1 success should persist output for thread b"
        );

        let claim = runtime
            .try_claim_global_phase2_job(owner, 3600)
            .await
            .expect("claim global consolidation");
        let input_watermark = match claim {
            Phase2JobClaimOutcome::Claimed {
                input_watermark, ..
            } => input_watermark,
            other => panic!("unexpected global consolidation claim outcome: {other:?}"),
        };
        assert_eq!(input_watermark, 101);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_global_lock_allows_only_one_fresh_runner() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(200)
            .await
            .expect("enqueue global consolidation");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner a");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner b");

        let running_claim = runtime
            .try_claim_global_phase2_job(owner_a, 3600)
            .await
            .expect("claim global lock");
        assert!(
            matches!(running_claim, Phase2JobClaimOutcome::Claimed { .. }),
            "first owner should claim global lock"
        );

        let second_claim = runtime
            .try_claim_global_phase2_job(owner_b, 3600)
            .await
            .expect("claim global lock from second owner");
        assert_eq!(second_claim, Phase2JobClaimOutcome::SkippedRunning);

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_global_lock_stale_lease_allows_takeover() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(300)
            .await
            .expect("enqueue global consolidation");

        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner a");
        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner b");

        let initial_claim = runtime
            .try_claim_global_phase2_job(owner_a, 3600)
            .await
            .expect("claim initial global lock");
        let token_a = match initial_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected initial claim outcome: {other:?}"),
        };

        sqlx::query("UPDATE jobs SET lease_until = ? WHERE kind = ? AND job_key = ?")
            .bind(Utc::now().timestamp() - 1)
            .bind("memory_consolidate_global")
            .bind("global")
            .execute(runtime.pool.as_ref())
            .await
            .expect("expire global consolidation lease");

        let takeover_claim = runtime
            .try_claim_global_phase2_job(owner_b, 3600)
            .await
            .expect("claim stale global lock");
        let (token_b, input_watermark) = match takeover_claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => (ownership_token, input_watermark),
            other => panic!("unexpected takeover claim outcome: {other:?}"),
        };
        assert_ne!(token_a, token_b);
        assert_eq!(input_watermark, 300);

        assert_eq!(
            runtime
                .mark_global_phase2_job_succeeded(token_a.as_str(), 300)
                .await
                .expect("mark stale owner success result"),
            false,
            "stale owner should lose finalization ownership after takeover"
        );
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(token_b.as_str(), 300)
                .await
                .expect("mark takeover owner success"),
            "takeover owner should finalize consolidation"
        );

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_backfilled_inputs_below_last_success_still_become_dirty() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(500)
            .await
            .expect("enqueue initial consolidation");
        let owner_a = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner a");
        let claim_a = runtime
            .try_claim_global_phase2_job(owner_a, 3_600)
            .await
            .expect("claim initial consolidation");
        let token_a = match claim_a {
            Phase2JobClaimOutcome::Claimed {
                ownership_token,
                input_watermark,
            } => {
                assert_eq!(input_watermark, 500);
                ownership_token
            }
            other => panic!("unexpected initial phase2 claim outcome: {other:?}"),
        };
        assert!(
            runtime
                .mark_global_phase2_job_succeeded(token_a.as_str(), 500)
                .await
                .expect("mark initial phase2 success"),
            "initial phase2 success should finalize"
        );

        runtime
            .enqueue_global_consolidation(400)
            .await
            .expect("enqueue backfilled consolidation");

        let owner_b = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner b");
        let claim_b = runtime
            .try_claim_global_phase2_job(owner_b, 3_600)
            .await
            .expect("claim backfilled consolidation");
        match claim_b {
            Phase2JobClaimOutcome::Claimed {
                input_watermark, ..
            } => {
                assert!(
                    input_watermark > 500,
                    "backfilled enqueue should advance dirty watermark beyond last success"
                );
            }
            other => panic!("unexpected backfilled phase2 claim outcome: {other:?}"),
        }

        let _ = tokio::fs::remove_dir_all(codex_home).await;
    }

    #[tokio::test]
    async fn phase2_failure_fallback_updates_unowned_running_job() {
        let codex_home = unique_temp_dir();
        let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
            .await
            .expect("initialize runtime");

        runtime
            .enqueue_global_consolidation(400)
            .await
            .expect("enqueue global consolidation");

        let owner = ThreadId::from_string(&Uuid::new_v4().to_string()).expect("owner");
        let claim = runtime
            .try_claim_global_phase2_job(owner, 3_600)
            .await
            .expect("claim global consolidation");
        let ownership_token = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected claim outcome: {other:?}"),
        };

        sqlx::query("UPDATE jobs SET ownership_token = NULL WHERE kind = ? AND job_key = ?")
            .bind("memory_consolidate_global")
            .bind("global")
            .execute(runtime.pool.as_ref())
            .await
            .expect("clear ownership token");

        assert_eq!(
            runtime
                .mark_global_phase2_job_failed(ownership_token.as_str(), "lost", 3_600)
                .await
                .expect("mark phase2 failed with strict ownership"),
            false,
            "strict failure update should not match unowned running job"
        );
        assert!(
            runtime
                .mark_global_phase2_job_failed_if_unowned(ownership_token.as_str(), "lost", 3_600)
                .await
                .expect("fallback failure update should match unowned running job"),
            "fallback failure update should transition the unowned running job"
        );

        let claim = runtime
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim after fallback failure");
        assert_eq!(claim, Phase2JobClaimOutcome::SkippedNotDirty);

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
            sandbox_policy: crate::extract::enum_to_string(&SandboxPolicy::new_read_only_policy()),
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
