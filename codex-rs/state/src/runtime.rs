use crate::StateDb;
use crate::ThreadMetadataBuilder;
use chrono::DateTime;
use chrono::Utc;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

pub const STATE_DB_FILENAME: &str = "state.sqlite";

const METRIC_DB_INIT: &str = "codex.db.init";

/// A runtime wrapper around [`StateDb`] that owns configuration.
#[derive(Clone)]
pub struct StateRuntime {
    db: Arc<StateDb>,
    codex_home: PathBuf,
    default_provider: String,
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
        let state_path = codex_home.join(STATE_DB_FILENAME);
        let existed = tokio::fs::try_exists(&state_path).await.unwrap_or(false);
        let db = match StateDb::open(&state_path).await {
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
            db,
            codex_home,
            default_provider,
        });
        if !existed
            && let Some(otel) = otel.as_ref() {
                otel.counter(METRIC_DB_INIT, 1, &[("status", "created")]);
            }
        Ok(runtime)
    }

    /// Return the configured Codex home directory for this runtime.
    pub fn codex_home(&self) -> &Path {
        self.codex_home.as_path()
    }

    /// Load thread metadata by id using the underlying database.
    pub async fn get_thread(&self, id: ThreadId) -> anyhow::Result<Option<crate::ThreadMetadata>> {
        self.db.get_thread(id).await
    }

    /// Find a rollout path by thread id using the underlying database.
    pub async fn find_rollout_path_by_id(
        &self,
        id: ThreadId,
        archived_only: Option<bool>,
    ) -> anyhow::Result<Option<PathBuf>> {
        self.db.find_rollout_path_by_id(id, archived_only).await
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
        self.db
            .list_threads(
                page_size,
                anchor,
                sort_key,
                allowed_sources,
                model_providers,
                archived_only,
            )
            .await
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
        self.db
            .list_thread_ids(
                limit,
                anchor,
                sort_key,
                allowed_sources,
                model_providers,
                archived_only,
            )
            .await
    }

    /// Insert or replace thread metadata directly.
    pub async fn upsert_thread(&self, metadata: &crate::ThreadMetadata) -> anyhow::Result<()> {
        self.db.upsert_thread(metadata).await
    }

    /// Apply rollout items incrementally using the underlying database.
    pub async fn apply_rollout_items(
        &self,
        builder: &ThreadMetadataBuilder,
        items: &[RolloutItem],
        otel: Option<&OtelManager>,
    ) -> anyhow::Result<()> {
        self.db
            .apply_rollout_items(builder, self.default_provider.as_str(), items, otel)
            .await
    }

    /// Mark a thread as archived using the underlying database.
    pub async fn mark_archived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        archived_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        self.db
            .mark_archived(thread_id, rollout_path, archived_at)
            .await
    }

    /// Mark a thread as unarchived using the underlying database.
    pub async fn mark_unarchived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
    ) -> anyhow::Result<()> {
        self.db.mark_unarchived(thread_id, rollout_path).await
    }

    /// Emit a startup summary when the runtime is enabled.
    pub async fn startup_summary(&self) {
        info!("state db enabled at {}", self.db.path().display());
    }
}
