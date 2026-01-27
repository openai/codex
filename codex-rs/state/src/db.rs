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

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ThreadRow {
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

pub(crate) fn anchor_from_item(item: &ThreadMetadata, sort_key: SortKey) -> Option<Anchor> {
    let id = Uuid::parse_str(&item.id.to_string()).ok()?;
    let ts = match sort_key {
        SortKey::CreatedAt => item.created_at,
        SortKey::UpdatedAt => item.updated_at,
    };
    Some(Anchor { ts, id })
}

pub(crate) fn datetime_to_epoch_seconds(dt: DateTime<Utc>) -> i64 {
    dt.timestamp()
}

fn epoch_seconds_to_datetime(secs: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid unix timestamp: {secs}"))
}
