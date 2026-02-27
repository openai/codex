use crate::AgentJob;
use crate::AgentJobCreateParams;
use crate::AgentJobItem;
use crate::AgentJobItemCreateParams;
use crate::AgentJobItemStatus;
use crate::AgentJobProgress;
use crate::AgentJobStatus;
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
use crate::model::AgentJobRow;
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
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqliteJournalMode;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::sqlite::SqliteSynchronous;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

mod agent_jobs;
mod backfill;
mod logs;
mod memories;
#[cfg(test)]
mod test_support;
mod threads;

// "Partition" is the retention bucket we cap at 10 MiB:
// - one bucket per non-null thread_id
// - one bucket per threadless (thread_id IS NULL) non-null process_uuid
// - one bucket for threadless rows with process_uuid IS NULL
const LOG_PARTITION_SIZE_LIMIT_BYTES: i64 = 10 * 1024 * 1024;

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
