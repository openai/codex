//! Tracks conversation ownership and the live request handle for each backend.
//! A thread ID keeps its first observed owner across connection replacement.

use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerStartedThread;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerRequestHandle;
use codex_protocol::ThreadId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct BackendRegistry {
    owners: HashMap<ThreadId, BackendId>,
    connections: HashMap<BackendId, AppServerRequestHandle>,
}

impl BackendRegistry {
    pub(super) fn owner(&self, thread: ThreadId) -> Option<BackendId> {
        self.owners.get(&thread).copied()
    }

    pub(super) fn register_started(
        &mut self,
        owner: BackendId,
        started: &AppServerStartedThread,
    ) -> color_eyre::Result<()> {
        color_eyre::eyre::ensure!(
            self.observe(owner, started.session.thread_id),
            "conversation belongs to a different backend"
        );
        Ok(())
    }

    pub(super) fn observe(&mut self, owner: BackendId, thread: ThreadId) -> bool {
        if *self.owners.entry(thread).or_insert(owner) != owner {
            tracing::warn!(%thread, "ignoring conflicting thread ownership");
            return false;
        }
        true
    }

    pub(super) fn connect(&mut self, session: &AppServerSession) {
        self.connections
            .insert(session.backend_id(), session.request_handle());
    }

    pub(super) fn disconnect(&mut self, owner: BackendId) {
        self.connections.remove(&owner);
    }

    pub(super) fn request_handle(&self, owner: BackendId) -> Option<AppServerRequestHandle> {
        self.connections.get(&owner).cloned()
    }
}
