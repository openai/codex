//! Fences and signals one local agent tree, then exposes its teardown result.

use super::ThreadManager;
use crate::agent::control::AgentTreeShutdownState;
use codex_protocol::ThreadId;
use codex_protocol::error::Result as CodexResult;
use std::sync::Arc;

/// Completion handle for one exact local agent tree.
#[derive(Clone, Debug)]
#[must_use = "agent-tree shutdown is not complete until wait() finishes"]
pub struct AgentTreeShutdown {
    state: Arc<AgentTreeShutdownState>,
}

impl AgentTreeShutdown {
    /// Waits for every operation and session admitted before the shutdown fence to finish.
    /// Returns an error if cleanup or persistence writer termination failed. Registry removal and
    /// caller-owned persistence handoff remain with the caller. Dropping this future does not
    /// cancel shutdown.
    pub async fn wait(&self) -> CodexResult<()> {
        self.state.wait().await
    }
}

impl ThreadManager {
    /// Requests shutdown of a loaded thread and every session sharing its local runtime.
    ///
    /// Returns after fencing new starts and signalling admitted sessions. Callers can wait on the
    /// returned handle before completing registry cleanup or a persistence handoff. This fences
    /// only starts sharing the current runtime; callers must separately serialize top-level loads
    /// or resumes of the same thread ID through that handoff.
    pub async fn request_agent_tree_shutdown(
        &self,
        thread_id: ThreadId,
    ) -> CodexResult<AgentTreeShutdown> {
        let thread = self.get_thread(thread_id).await?;
        Ok(AgentTreeShutdown {
            state: thread
                .session
                .services
                .local_agent_runtime
                .request_shutdown(),
        })
    }
}
