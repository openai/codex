use crate::DB_ERROR_METRIC;
use crate::extract::apply_rollout_item;
use crate::extract::rollout_has_user_event;
use crate::migrations::MIGRATOR;
use crate::model::Anchor;
use crate::model::SortKey;
use crate::model::ThreadMetadata;
use crate::model::ThreadMetadataBuilder;
use crate::model::ThreadsPage;
use crate::paths::file_modified_time_utc;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tracing::warn;
use uuid::Uuid;

/// A SQLite-backed store for rollout-derived thread metadata.
///
/// This type is intentionally low-level: it focuses on persistence and queries.
/// Most consumers should prefer [`crate::StateRuntime`], which owns configuration,
/// metrics, and first-run backfill behavior.
#[derive(Clone)]
pub struct StateDb {
    pool: sqlx::SqlitePool,
    path: PathBuf,
}

impl StateDb {
    /// Open (and migrate) the state database at `path`.
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self {
            pool,
            path: path.to_path_buf(),
        })
    }

    /// Return the on-disk path of this database.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load thread metadata by id.
    pub async fn get_thread(&self, id: ThreadId) -> Result<Option<ThreadMetadata>> {
        let row = sqlx::query_as::<_, ThreadRow>(
            r#"
SELECT
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(ThreadMetadata::try_from).transpose()
    }

    /// Resolve a rollout path by id, optionally constrained by archive state.
    pub async fn find_rollout_path_by_id(
        &self,
        id: ThreadId,
        archived_only: Option<bool>,
    ) -> Result<Option<PathBuf>> {
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
        let row = builder.build().fetch_optional(&self.pool).await?;
        Ok(row
            .and_then(|r| r.try_get::<String, _>("rollout_path").ok())
            .map(PathBuf::from))
    }

    /// Insert or replace a thread metadata row.
    pub async fn upsert_thread(&self, metadata: &ThreadMetadata) -> Result<()> {
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
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    archived,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    rollout_path = excluded.rollout_path,
    created_at = excluded.created_at,
    updated_at = excluded.updated_at,
    source = excluded.source,
    model_provider = excluded.model_provider,
    cwd = excluded.cwd,
    title = excluded.title,
    sandbox_policy = excluded.sandbox_policy,
    approval_mode = excluded.approval_mode,
    tokens_used = excluded.tokens_used,
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
        .bind(metadata.title.as_str())
        .bind(metadata.sandbox_policy.as_str())
        .bind(metadata.approval_mode.as_str())
        .bind(metadata.tokens_used)
        .bind(metadata.archived_at.is_some())
        .bind(metadata.archived_at.map(datetime_to_epoch_seconds))
        .bind(metadata.git_sha.as_deref())
        .bind(metadata.git_branch.as_deref())
        .bind(metadata.git_origin_url.as_deref())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List threads using keyset pagination, filtering to rollouts with user events.
    pub async fn list_threads(
        &self,
        page_size: usize,
        anchor: Option<&Anchor>,
        sort_key: SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
    ) -> Result<ThreadsPage> {
        let batch_size = page_size.saturating_mul(4).clamp(64, 512);
        let mut scan_anchor = anchor.cloned();
        let mut num_scanned_rows = 0usize;
        let mut items = Vec::new();

        loop {
            let batch = self
                .list_threads_batch(
                    batch_size,
                    scan_anchor.as_ref(),
                    sort_key,
                    allowed_sources,
                    model_providers,
                    archived_only,
                )
                .await?;
            let batch_len = batch.len();
            if batch_len == 0 {
                break;
            }
            num_scanned_rows = num_scanned_rows.saturating_add(batch_len);

            for candidate in batch {
                let candidate_anchor = anchor_from_item(&candidate, sort_key);
                scan_anchor = candidate_anchor;
                let has_user_event =
                    match rollout_has_user_event(candidate.rollout_path.as_path()).await {
                        Ok(has_user_event) => has_user_event,
                        Err(err) => {
                            warn!(
                                "failed to scan rollout for user events {}: {err}",
                                candidate.rollout_path.display()
                            );
                            true
                        }
                    };
                if has_user_event {
                    items.push(candidate);
                }
                if items.len() > page_size {
                    break;
                }
            }

            if items.len() > page_size {
                break;
            }
            if batch_len < batch_size {
                break;
            }
        }

        let next_anchor = if items.len() > page_size {
            match items.pop() {
                Some(extra) => anchor_from_item(&extra, sort_key),
                None => None,
            }
        } else {
            None
        };
        Ok(ThreadsPage {
            items,
            next_anchor,
            num_scanned_rows,
        })
    }

    /// List thread ids using keyset pagination without rollout scanning.
    pub async fn list_thread_ids(
        &self,
        limit: usize,
        anchor: Option<&Anchor>,
        sort_key: SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
    ) -> Result<Vec<ThreadId>> {
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT id FROM threads WHERE 1 = 1");
        if archived_only {
            builder.push(" AND archived = 1");
        } else {
            builder.push(" AND archived = 0");
        }
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
        let order_column = match sort_key {
            SortKey::CreatedAt => "created_at",
            SortKey::UpdatedAt => "updated_at",
        };
        builder.push(" ORDER BY ");
        builder.push(order_column);
        builder.push(" DESC, id DESC");
        builder.push(" LIMIT ");
        builder.push_bind(limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                Ok(ThreadId::try_from(id)?)
            })
            .collect()
    }

    /// Mark a thread as archived, ignoring requests for missing rows.
    pub async fn mark_archived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        archived_at: DateTime<Utc>,
    ) -> Result<()> {
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

    /// Mark a thread as unarchived, ignoring requests for missing rows.
    pub async fn mark_unarchived(&self, thread_id: ThreadId, rollout_path: &Path) -> Result<()> {
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

    /// Apply incremental rollout items to the stored metadata.
    pub async fn apply_rollout_items(
        &self,
        builder: &ThreadMetadataBuilder,
        default_provider: &str,
        items: &[RolloutItem],
        otel: Option<&OtelManager>,
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut metadata = self
            .get_thread(builder.id)
            .await?
            .unwrap_or_else(|| builder.build(default_provider));
        metadata.rollout_path = builder.rollout_path.clone();
        for item in items {
            apply_rollout_item(&mut metadata, item, default_provider);
        }
        if let Some(updated_at) = file_modified_time_utc(builder.rollout_path.as_path()).await {
            metadata.updated_at = updated_at;
        }
        if let Err(err) = self.upsert_thread(&metadata).await {
            if let Some(otel) = otel {
                otel.counter(DB_ERROR_METRIC, 1, &[("stage", "apply_rollout_items")]);
            }
            return Err(err);
        }
        Ok(())
    }
}

impl StateDb {
    async fn list_threads_batch(
        &self,
        limit: usize,
        anchor: Option<&Anchor>,
        sort_key: SortKey,
        allowed_sources: &[String],
        model_providers: Option<&[String]>,
        archived_only: bool,
    ) -> Result<Vec<ThreadMetadata>> {
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
    title,
    sandbox_policy,
    approval_mode,
    tokens_used,
    archived_at,
    git_sha,
    git_branch,
    git_origin_url
FROM threads
WHERE 1 = 1
            "#,
        );
        if archived_only {
            builder.push(" AND archived = 1");
        } else {
            builder.push(" AND archived = 0");
        }
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
        let order_column = match sort_key {
            SortKey::CreatedAt => "created_at",
            SortKey::UpdatedAt => "updated_at",
        };
        builder.push(" ORDER BY ");
        builder.push(order_column);
        builder.push(" DESC, id DESC");
        builder.push(" LIMIT ");
        builder.push_bind(limit as i64);
        let rows = builder
            .build_query_as::<ThreadRow>()
            .fetch_all(&self.pool)
            .await?;
        let rows = rows
            .into_iter()
            .map(ThreadMetadata::try_from)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ThreadRow {
    id: String,
    rollout_path: String,
    created_at: i64,
    updated_at: i64,
    source: String,
    model_provider: String,
    cwd: String,
    title: String,
    sandbox_policy: String,
    approval_mode: String,
    tokens_used: i64,
    archived_at: Option<i64>,
    git_sha: Option<String>,
    git_branch: Option<String>,
    git_origin_url: Option<String>,
}

impl TryFrom<ThreadRow> for ThreadMetadata {
    type Error = anyhow::Error;

    fn try_from(row: ThreadRow) -> std::result::Result<Self, Self::Error> {
        let ThreadRow {
            id,
            rollout_path,
            created_at,
            updated_at,
            source,
            model_provider,
            cwd,
            title,
            sandbox_policy,
            approval_mode,
            tokens_used,
            archived_at,
            git_sha,
            git_branch,
            git_origin_url,
        } = row;
        Ok(Self {
            id: ThreadId::try_from(id)?,
            rollout_path: PathBuf::from(rollout_path),
            created_at: epoch_seconds_to_datetime(created_at)?,
            updated_at: epoch_seconds_to_datetime(updated_at)?,
            source,
            model_provider,
            cwd: PathBuf::from(cwd),
            title,
            sandbox_policy,
            approval_mode,
            tokens_used,
            archived_at: archived_at.map(epoch_seconds_to_datetime).transpose()?,
            git_sha,
            git_branch,
            git_origin_url,
        })
    }
}

fn anchor_from_item(item: &ThreadMetadata, sort_key: SortKey) -> Option<Anchor> {
    let id = Uuid::parse_str(&item.id.to_string()).ok()?;
    let ts = match sort_key {
        SortKey::CreatedAt => item.created_at,
        SortKey::UpdatedAt => item.updated_at,
    };
    Some(Anchor { ts, id })
}

fn datetime_to_epoch_seconds(dt: DateTime<Utc>) -> i64 {
    dt.timestamp()
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
