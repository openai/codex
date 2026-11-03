//! Configured thread-store construction and host-owned permanent-deletion cleanup.
//! Host cleanup runs under the store's existing deletion locks.

use crate::config::Config;
use crate::config::ThreadStoreConfig;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_rollout::state_db::StateDbHandle;
use codex_thread_store::InMemoryThreadStore;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::LocalThreadStoreConfig;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreFuture;
use std::sync::Arc;
use tracing::warn;

type AdditionalCleanup = dyn Fn(Vec<ThreadId>) -> ThreadStoreFuture<'static, ()> + Send + Sync;

/// Builds the configured thread store with optional host-owned data cleanup.
/// Hosts install cleanup for their services before sharing the store with callers.
pub struct ThreadStoreBuilder<'config> {
    config: &'config Config,
    state_db: Option<StateDbHandle>,
    additional_cleanup: Option<Arc<AdditionalCleanup>>,
}

impl<'config> ThreadStoreBuilder<'config> {
    pub fn from_config(config: &'config Config, state_db: Option<StateDbHandle>) -> Self {
        Self {
            config,
            state_db,
            additional_cleanup: None,
        }
    }

    /// Sets host-owned cleanup for permanent deletion in local stores.
    /// Runs before deleting rollouts, under the store's existing deletion locks.
    /// Cleanup must be idempotent because later deletion steps can fail and be retried.
    ///
    /// Cleanup must not call, or await work that calls, thread-store operations that reacquire
    /// lifecycle or writer locks for any supplied thread ID (for example, flush or shutdown).
    /// Deletion holds those locks until cleanup returns, so such calls would deadlock.
    /// Flush or shut down threads before starting deletion.
    ///
    /// In-memory stores do not run persistent-data cleanup.
    pub fn with_additional_cleanup(
        mut self,
        cleanup: impl Fn(Vec<ThreadId>) -> ThreadStoreFuture<'static, ()> + Send + Sync + 'static,
    ) -> Self {
        self.additional_cleanup = Some(Arc::new(cleanup));
        self
    }

    pub fn build(self) -> Arc<dyn ThreadStore> {
        let Self {
            config,
            state_db,
            additional_cleanup,
        } = self;
        match &config.experimental_thread_store {
            ThreadStoreConfig::Local => {
                let compression_enabled = config
                    .features
                    .enabled(Feature::LocalThreadStoreCompression);
                let background_migration_enabled = config
                    .features
                    .enabled(Feature::BackgroundPaginatedRolloutMigration);
                let has_state_db = state_db.is_some();
                let store = Arc::new(
                    LocalThreadStore::new(LocalThreadStoreConfig::from_config(config), state_db)
                        .with_thread_data_cleanup(move |thread_ids| {
                            let additional_cleanup = additional_cleanup.clone();
                            Box::pin(async move {
                                if let Some(cleanup) = additional_cleanup {
                                    cleanup(thread_ids).await?;
                                }
                                Ok(())
                            })
                        }),
                );
                if has_state_db && background_migration_enabled {
                    let startup_store = Arc::clone(&store);
                    let codex_home = config.codex_home.to_path_buf();
                    tokio::spawn(async move {
                        if let Err(err) = startup_store.migrate_rollouts_on_startup().await {
                            warn!("failed to migrate legacy rollouts on startup: {err}");
                        }
                        if compression_enabled {
                            codex_rollout::spawn_rollout_compression_worker(
                                codex_home,
                                codex_rollout::RolloutCompressionTrigger::Startup,
                            );
                        }
                    });
                } else if compression_enabled {
                    codex_rollout::spawn_rollout_compression_worker(
                        config.codex_home.to_path_buf(),
                        codex_rollout::RolloutCompressionTrigger::Startup,
                    );
                }
                store
            }
            ThreadStoreConfig::InMemory { id } => {
                Arc::new(InMemoryThreadStore::for_id(id).with_state_db(state_db))
            }
        }
    }
}
