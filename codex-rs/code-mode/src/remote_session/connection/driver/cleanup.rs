use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::host::SessionId;
use tokio_util::sync::CancellationToken;

use super::notify_cell_closed;
use super::session_registry::CellOwner;

#[derive(Clone, Eq, Hash, PartialEq)]
struct CellKey {
    session_id: SessionId,
    cell_id: CellId,
}

impl CellKey {
    fn for_owner(owner: &CellOwner) -> Self {
        Self {
            session_id: owner.session_id.clone(),
            cell_id: owner.cell_id.clone(),
        }
    }
}

#[derive(Default)]
struct CleanupState {
    outstanding_delegates: usize,
    failure_started: bool,
    completed: bool,
    pending_cells: HashMap<CellKey, CellOwner>,
}

struct CleanupInner {
    state: Mutex<CleanupState>,
    complete: CancellationToken,
}

#[derive(Clone)]
pub(in crate::remote_session::connection) struct ConnectionCleanup {
    inner: Arc<CleanupInner>,
}

impl ConnectionCleanup {
    pub(in crate::remote_session::connection) fn new() -> Self {
        Self {
            inner: Arc::new(CleanupInner {
                state: Mutex::new(CleanupState::default()),
                complete: CancellationToken::new(),
            }),
        }
    }

    pub(super) fn delegate_guard(&self) -> DelegateGuard {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .outstanding_delegates += 1;
        DelegateGuard {
            cleanup: self.clone(),
        }
    }

    pub(super) fn fail(&self, cells: Vec<CellOwner>) {
        let ready = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.failure_started = true;
            for owner in cells {
                state
                    .pending_cells
                    .insert(CellKey::for_owner(&owner), owner);
            }
            take_ready_cells(&mut state)
        };
        self.finish(ready);
    }

    pub(in crate::remote_session::connection) async fn wait(&self) {
        self.inner.complete.cancelled().await;
    }

    pub(in crate::remote_session::connection) fn is_complete(&self) -> bool {
        self.inner.complete.is_cancelled()
    }

    fn delegate_finished(&self) {
        let ready = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.outstanding_delegates -= 1;
            take_ready_cells(&mut state)
        };
        self.finish(ready);
    }

    fn finish(&self, ready: Option<Vec<CellOwner>>) {
        let Some(cells) = ready else {
            return;
        };
        for owner in cells {
            notify_cell_closed(&owner.delegate, &owner.cell_id);
        }
        self.inner.complete.cancel();
    }
}

pub(super) struct DelegateGuard {
    cleanup: ConnectionCleanup,
}

impl Drop for DelegateGuard {
    fn drop(&mut self) {
        self.cleanup.delegate_finished();
    }
}

fn take_ready_cells(state: &mut CleanupState) -> Option<Vec<CellOwner>> {
    if state.completed || !state.failure_started || state.outstanding_delegates != 0 {
        return None;
    }
    state.completed = true;
    Some(
        std::mem::take(&mut state.pending_cells)
            .into_values()
            .collect(),
    )
}
