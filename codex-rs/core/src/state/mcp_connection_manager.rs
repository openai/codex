use std::sync::Arc;
use std::sync::RwLock;

use codex_mcp::McpConnectionManager;

/// Reloadable session-owned MCP connection manager.
///
/// The lock only protects publication of the current manager. Callers retain an
/// owned handle and perform MCP work without holding the slot lock.
pub(crate) struct McpConnectionManagerSlot {
    current: RwLock<Arc<McpConnectionManager>>,
}

impl McpConnectionManagerSlot {
    pub(crate) fn new(manager: McpConnectionManager) -> Self {
        Self {
            current: RwLock::new(Arc::new(manager)),
        }
    }

    /// Clones the published manager while holding the slot lock briefly.
    ///
    /// MCP work happens through the returned handle without retaining the read
    /// lock, so long-running requests do not hold the publication lock.
    pub(crate) fn current(&self) -> Arc<McpConnectionManager> {
        let current = self
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(&*current)
    }

    /// Publishes a new manager using latest-wins semantics.
    ///
    /// Existing callers retain their snapshot of the previous manager until
    /// their work finishes. New callers immediately observe the replacement.
    pub(crate) fn replace(&self, manager: McpConnectionManager) {
        let manager = Arc::new(manager);
        let previous = {
            let mut current = self
                .current
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::replace(&mut *current, manager)
        };
        drop(previous);
    }
}
