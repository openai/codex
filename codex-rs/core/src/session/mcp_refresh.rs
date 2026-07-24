use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::AcquireError;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

/// Owns MCP invalidation and the single gate used to publish runtime updates.
pub(super) struct McpRefresh {
    pending: AtomicBool,
    gate: Semaphore,
}

impl McpRefresh {
    pub(super) fn new() -> Self {
        Self {
            pending: AtomicBool::new(false),
            gate: Semaphore::new(/*permits*/ 1),
        }
    }

    pub(super) fn invalidate(&self) {
        self.pending.store(true, Ordering::Release);
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    pub(super) fn claim(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    pub(super) async fn acquire(&self) -> Result<SemaphorePermit<'_>, AcquireError> {
        self.gate.acquire().await
    }

    pub(super) fn close(&self) {
        self.gate.close();
    }
}

/// Restores a claimed refresh when its task is cancelled before publication.
pub(super) struct McpRefreshInvalidationGuard<'a> {
    pub(super) refresh: &'a McpRefresh,
    pub(super) published: bool,
}

impl Drop for McpRefreshInvalidationGuard<'_> {
    fn drop(&mut self) {
        if !self.published {
            self.refresh.invalidate();
        }
    }
}
