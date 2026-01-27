use crate::DB_ERROR_METRIC;
use crate::DB_METRIC_BACKFILL;
use crate::DB_METRIC_COMPARE_ERROR;
use crate::StateDb;
use crate::extract_metadata_from_rollout;
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

/// A runtime wrapper around [`StateDb`] that owns configuration and metrics.
///
/// This should be the entry point of the crate.
#[derive(Clone)]
pub struct StateRuntime {
    db: Arc<StateDb>,
    codex_home: PathBuf,
    default_provider: String,
    otel: OtelManager,
}

impl StateRuntime {
    /// Initialize the state runtime using the provided Codex home and default provider.
    ///
    /// This opens (and migrates) the SQLite database at `codex_home/state.sqlite`.
    /// If the database did not previously exist, this also performs a full best-effort
    /// backfill by scanning `sessions/`.
    pub async fn init(
        codex_home: PathBuf,
        default_provider: String,
        otel: OtelManager,
    ) -> anyhow::Result<Arc<Self>> {
        let state_path = codex_home.join(STATE_DB_FILENAME);
        let existed = tokio::fs::try_exists(&state_path).await.unwrap_or(false);
        let db = match StateDb::open(&state_path).await {
            Ok(db) => Arc::new(db),
            Err(err) => {
                warn!("failed to open state db at {}: {err}", state_path.display());
                otel.counter(METRIC_DB_INIT, 1, &[("status", "open_error")]);
                return Err(err);
            }
        };
        otel.counter(METRIC_DB_INIT, 1, &[("status", "opened")]);
        let runtime = Arc::new(Self {
            db,
            codex_home,
            default_provider,
            otel,
        });
        if !existed {
            runtime
                .otel
                .counter(METRIC_DB_INIT, 1, &[("status", "created")]);
            warn!("state db created; performing initial backfill scan of sessions/ (may be slow)");
            match runtime
                .db
                .backfill_sessions(
                    runtime.codex_home.as_path(),
                    runtime.default_provider.as_str(),
                    Some(&runtime.otel),
                )
                .await
            {
                Ok(stats) => {
                    runtime.otel.counter(
                        DB_METRIC_BACKFILL,
                        stats.upserted as i64,
                        &[("status", "upserted")],
                    );
                    runtime.otel.counter(
                        DB_METRIC_BACKFILL,
                        stats.failed as i64,
                        &[("status", "failed")],
                    );
                }
                Err(err) => {
                    warn!("state db backfill failed: {err}");
                    runtime
                        .otel
                        .counter(DB_METRIC_BACKFILL, 1, &[("status", "error")]);
                }
            }
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

    /// Extract rollout metadata.
    pub async fn extract(&self, path: &Path) -> Option<crate::ThreadMetadata> {
        // TODO(jif) add a cache if this is too slow.
        let outcome = match extract_metadata_from_rollout(
            path,
            self.default_provider.as_str(),
            Some(&self.otel),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(err) => {
                warn!(
                    "failed to extract rollout metadata {}: {err}",
                    path.display()
                );
                self.otel
                    .counter(DB_METRIC_COMPARE_ERROR, 1, &[("stage", "extract")]);
                return None;
            }
        };

        if outcome.parse_errors > 0 {
            self.otel.counter(
                DB_METRIC_COMPARE_ERROR,
                outcome.parse_errors as i64,
                &[("stage", "extract_per_line")],
            );
        }
        Some(outcome.metadata)
    }

    /// Ingest a rollout file by extracting metadata and upserting it into the database.
    pub async fn ingest_rollout_file(&self, path: &Path) {
        let Some(metadata) = self.extract(path).await else {
            return;
        };
        if let Err(err) = self.db.upsert_thread(&metadata).await {
            warn!("failed to reconcile rollout {}: {err}", path.display());
            self.otel
                .counter(DB_ERROR_METRIC, 1, &[("stage", "ingest_rollout_file")]);
        }
    }

    /// Apply rollout items incrementally using the underlying database.
    pub async fn apply_rollout_items(&self, path: &Path, items: &[RolloutItem]) {
        if let Err(err) = self
            .db
            .apply_rollout_items(path, self.default_provider.as_str(), items, &self.otel)
            .await
        {
            warn!(
                "failed to apply rollout items to db {}: {err}",
                path.display()
            );
        }
    }

    /// Mark a thread as archived using the underlying database.
    pub async fn mark_archived(
        &self,
        thread_id: ThreadId,
        rollout_path: &Path,
        archived_at: &str,
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
