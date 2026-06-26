use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::notify_cell_closed;
use super::session_registry::CellOwner;

struct CleanupInner {
    complete: CancellationToken,
}

#[derive(Clone)]
pub(in crate::remote_session) struct SessionCleanup {
    inner: Arc<CleanupInner>,
}

impl SessionCleanup {
    pub(in crate::remote_session) fn new() -> Self {
        Self {
            inner: Arc::new(CleanupInner {
                complete: CancellationToken::new(),
            }),
        }
    }

    pub(super) fn fail(&self, cells: Vec<CellOwner>) {
        for owner in cells {
            notify_cell_closed(&owner.delegate, &owner.cell_id);
        }
        self.inner.complete.cancel();
    }

    pub(in crate::remote_session) async fn wait(&self) {
        self.inner.complete.cancelled().await;
    }

    pub(in crate::remote_session) fn is_complete(&self) -> bool {
        self.inner.complete.is_cancelled()
    }
}
