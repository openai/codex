//! Plugin catalog listing interface and source inputs used to request snapshots.

use crate::ExecutorPluginProviderError;
use crate::PluginCatalog;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_mcp::McpResourceClient;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;

/// Identifiers, MCP client, and executor inventories used to request one catalog snapshot.
#[derive(Clone, Debug)]
pub struct PluginListQuery {
    pub thread_id: String,
    pub turn_id: String,
    pub mcp_resources: Option<Arc<McpResourceClient>>,
    /// Already-acquired V2 inventories; empty for cloud listing.
    pub snapshots: Vec<ExecutorCapabilityDiscoveryV2Snapshot>,
}

/// Failure to list plugin metadata.
#[derive(Debug, Error)]
pub enum PluginProviderError {
    #[error(transparent)]
    Executor(#[from] ExecutorPluginProviderError),
    #[error("{0}")]
    Message(String),
}

pub type PluginProviderResult<T> = Result<T, PluginProviderError>;

pub type PluginProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = PluginProviderResult<T>> + Send + 'a>>;

/// Lists plugin metadata without resolving packages or binding filesystems.
///
/// Cloud providers implement `list`; executor listing is not yet supported.
/// Package access uses [`crate::ExecutorPluginProvider::resolve_bound`] directly.
pub trait PluginProvider: Send + Sync {
    /// Returns a snapshot; on error, callers retain the previous snapshot.
    /// Providers set `is_partial` when listing omits entries; warnings are diagnostic only.
    fn list(&self, _query: PluginListQuery) -> PluginProviderFuture<'_, PluginCatalog> {
        Box::pin(async { Ok(PluginCatalog::default()) })
    }
}
