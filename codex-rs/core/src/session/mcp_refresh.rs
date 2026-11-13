use crate::McpRefreshReason;
use codex_protocol::ThreadId;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::AcquireError;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
use tokio_util::task::TaskTracker;

/// Owns MCP invalidation and the single gate used to publish runtime updates.
pub(super) struct McpRefresh {
    pending: Mutex<BTreeSet<McpRefreshReason>>,
    // Kept separate from pending so prewarm cannot claim work reserved for a real turn.
    on_next_turn: AtomicBool,
    gate: Semaphore,
    pub(super) apps_refreshes: TaskTracker,
}

impl McpRefresh {
    pub(super) fn new() -> Self {
        let apps_refreshes = TaskTracker::new();
        // Closed trackers still accept tokens and wake waiters when empty.
        apps_refreshes.close();
        Self {
            pending: Mutex::default(),
            on_next_turn: AtomicBool::new(false),
            gate: Semaphore::new(/*permits*/ 1),
            apps_refreshes,
        }
    }

    pub(super) fn request_for_next_turn(&self) {
        self.on_next_turn.store(true, Ordering::Release);
    }

    pub(super) fn claim_for_turn_start(&self) -> bool {
        self.on_next_turn.swap(false, Ordering::AcqRel)
    }

    pub(super) fn invalidate(&self, reason: McpRefreshReason) {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(reason);
    }

    pub(super) fn is_pending(&self) -> bool {
        !self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    /// Take only the causes known at the start of this refresh. Later invalidations
    /// belong to the next publication, even if they have the same cause.
    pub(super) fn claim(&self, thread_id: ThreadId) -> Option<McpRefreshInvalidationGuard<'_>> {
        let reasons = self.take_pending();
        (!reasons.is_empty()).then(|| self.start(thread_id, reasons))
    }

    pub(super) fn claim_forced(
        &self,
        thread_id: ThreadId,
        reason: McpRefreshReason,
    ) -> McpRefreshInvalidationGuard<'_> {
        let mut reasons = self.take_pending();
        reasons.insert(reason);
        self.start(thread_id, reasons)
    }

    pub(super) fn take_pending(&self) -> BTreeSet<McpRefreshReason> {
        std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    fn start(
        &self,
        thread_id: ThreadId,
        reasons: BTreeSet<McpRefreshReason>,
    ) -> McpRefreshInvalidationGuard<'_> {
        tracing::info!(%thread_id, ?reasons, "refreshing MCP runtime");
        McpRefreshInvalidationGuard {
            refresh: self,
            reasons,
            published: false,
        }
    }

    #[tracing::instrument(name = "mcp.runtime.refresh_wait", skip_all)]
    pub(super) async fn acquire(&self) -> Result<SemaphorePermit<'_>, AcquireError> {
        self.gate.acquire().await
    }

    pub(super) fn close(&self) {
        self.gate.close();
    }
}

/// Restores a claimed refresh when its task is cancelled before publication.
pub(super) struct McpRefreshInvalidationGuard<'a> {
    refresh: &'a McpRefresh,
    reasons: BTreeSet<McpRefreshReason>,
    pub(super) published: bool,
}

impl Drop for McpRefreshInvalidationGuard<'_> {
    fn drop(&mut self) {
        if !self.published {
            let mut pending = self
                .refresh
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.append(&mut self.reasons);
            pending.insert(McpRefreshReason::PreviousRefreshUnpublished);
        }
    }
}

#[cfg(test)]
#[path = "mcp_refresh_tests.rs"]
mod tests;
