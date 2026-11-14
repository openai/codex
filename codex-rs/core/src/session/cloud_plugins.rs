//! Refreshes independent cloud catalogs concurrently, serialized with step preparation.

use super::Session;
use crate::McpRefreshReason;
use codex_extension_api::CloudCatalogRefreshInput;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;

#[derive(Default)]
pub(super) struct CloudCatalogPreparation {
    /// Serializes catalog refreshes and snapshot capture.
    refresh_lock: Mutex<()>,
    /// Requests resource invalidation on the next lock acquisition.
    /// Survives cancellation while waiting.
    refresh_pending: AtomicBool,
}

impl Session {
    pub(crate) async fn lock_cloud_catalogs(&self) -> tokio::sync::MutexGuard<'_, ()> {
        let guard = self.cloud_catalog_preparation.refresh_lock.lock().await;
        if self
            .cloud_catalog_preparation
            .refresh_pending
            .swap(false, Ordering::AcqRel)
        {
            self.services.mcp_runtime.invalidate_resource_caches();
        }
        guard
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "catalog publication must remain serialized with step preparation"
    )]
    pub(crate) async fn refresh_cloud_plugins(self: &Arc<Self>) -> anyhow::Result<()> {
        self.cloud_catalog_preparation
            .refresh_pending
            .store(true, Ordering::Release);
        let _catalogs = self.lock_cloud_catalogs().await;
        self.refresh_mcp_if_dirty().await;
        let results = futures::future::join_all(
            self.services
                .extensions
                .turn_lifecycle_contributors()
                .iter()
                .map(|contributor| async move {
                    let result = contributor
                        .refresh_cloud_catalog(CloudCatalogRefreshInput {
                            session_store: &self.services.session_extension_data,
                            thread_store: &self.services.thread_extension_data,
                        })
                        .await;
                    // Retain publication even if another provider is still pending at cancellation.
                    self.mark_mcp_runtime_dirty(McpRefreshReason::ExtensionCatalogChanged);
                    result
                }),
        )
        .await;
        drop(_catalogs);
        self.schedule_mcp_prewarm();
        let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
