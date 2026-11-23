//! Refreshes independent cloud catalogs concurrently, serialized with step preparation.

use super::Session;
use crate::McpRefreshReason;
use codex_extension_api::CloudCatalogRefreshInput;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;

#[derive(Default)]
pub(super) struct CloudCatalogPreparation {
    /// Serializes catalog refreshes and snapshot capture.
    refresh_lock: Mutex<CatalogRefreshState>,
    /// Requests resource invalidation on the next lock acquisition.
    /// Survives cancellation while waiting.
    refresh_pending: AtomicBool,
}

#[derive(Default)]
pub(crate) struct CatalogRefreshState {
    // Leave the check for the next catalog lock holder if discovery is cancelled.
    startup_retry_pending: bool,
}

#[tracing::instrument(
    name = "cloud_catalog.refresh.acquire_lock",
    skip_all,
    fields(acquired)
)]
async fn acquire_refresh_lock(
    lock: &Mutex<CatalogRefreshState>,
) -> MutexGuard<'_, CatalogRefreshState> {
    let guard = lock.lock().await;
    tracing::Span::current().record("acquired", true);
    guard
}

impl Session {
    fn finish_catalog_startup(&self, catalogs: &mut CatalogRefreshState) {
        if std::mem::take(&mut catalogs.startup_retry_pending)
            && self.services.mcp_runtime.has_failed_startup_for_retry()
        {
            self.request_mcp_runtime_reprojection(McpRefreshReason::ServerStartupFailed);
        }
    }

    pub(crate) async fn lock_cloud_catalogs(&self) -> MutexGuard<'_, CatalogRefreshState> {
        let mut guard = self.cloud_catalog_preparation.refresh_lock.lock().await;
        self.prepare_locked_catalogs(&mut guard);
        guard
    }

    fn prepare_locked_catalogs(&self, catalogs: &mut CatalogRefreshState) {
        self.finish_catalog_startup(catalogs);
        if self
            .cloud_catalog_preparation
            .refresh_pending
            .swap(false, Ordering::AcqRel)
        {
            self.services.mcp_runtime.invalidate_resource_caches();
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "catalog publication must remain serialized with step preparation"
    )]
    pub(crate) async fn refresh_cloud_plugins(self: &Arc<Self>) -> anyhow::Result<()> {
        self.cloud_catalog_preparation
            .refresh_pending
            .store(true, Ordering::Release);
        let mut catalogs = acquire_refresh_lock(&self.cloud_catalog_preparation.refresh_lock).await;
        self.prepare_locked_catalogs(&mut catalogs);
        catalogs.startup_retry_pending = true;
        self.refresh_mcp_if_dirty().await;
        let results = futures::future::join_all(
            self.services
                .extensions
                .turn_lifecycle_contributors()
                .iter()
                .map(|contributor| async move {
                    let outcome = contributor
                        .refresh_cloud_catalog(CloudCatalogRefreshInput {
                            session_store: &self.services.session_extension_data,
                            thread_store: &self.services.thread_extension_data,
                        })
                        .await;
                    // Queue the work before a slower sibling can time out the whole refresh.
                    if outcome.needs_mcp_refresh {
                        self.request_mcp_runtime_reprojection(
                            McpRefreshReason::ExtensionCatalogChanged,
                        );
                    }
                    outcome.result
                }),
        )
        .await;
        self.finish_catalog_startup(&mut catalogs);
        drop(catalogs);
        let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
