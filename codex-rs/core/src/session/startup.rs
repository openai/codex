//! Retains agent-tree membership and resources throughout managed startup.
//! Cleanup releases membership only after acquisition and partial session teardown finish;
//! abandoning that ownership marks tree shutdown as failed.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::sync::PoisonError;

use crate::agent::control::AgentTreeMembership;
use crate::agent::control::AgentTreeTeardownGuard;
use codex_protocol::protocol::Op;
use codex_thread_store::LiveThreadInitGuard;
use futures::future::BoxFuture;
use tokio::sync::Mutex;

use super::SessionIo;
use super::session::Session;

#[derive(Default)]
pub(crate) struct SessionStartup {
    teardown: StdMutex<Option<AgentTreeTeardownGuard>>,
    pub(crate) persistence: Mutex<LiveThreadInitGuard>,
    pub(crate) session: OnceLock<Arc<Session>>,
    pub(crate) io: OnceLock<SessionIo>,
}

impl SessionStartup {
    fn lock_teardown(&self) -> MutexGuard<'_, Option<AgentTreeTeardownGuard>> {
        self.teardown.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn hold_membership(&self, membership: AgentTreeMembership) {
        let previous = self
            .lock_teardown()
            .replace(membership.into_teardown_guard());
        assert!(previous.is_none(), "agent-tree membership already set");
    }

    pub(crate) fn session_teardown(&self) -> Option<AgentTreeTeardownGuard> {
        self.lock_teardown()
            .as_ref()
            .map(AgentTreeTeardownGuard::clone_for_teardown)
    }

    pub(crate) fn release_membership(&self) {
        if let Some(teardown) = self.lock_teardown().take() {
            teardown.complete();
        }
    }

    pub(crate) async fn cleanup(&self) {
        let teardown = self.lock_teardown().take();
        if let Some(io) = self.io.get() {
            // The session loop owns persistence now. Preserve its shutdown semantics even
            // if registration or the caller's handoff was interrupted after the loop started.
            self.persistence.lock().await.commit();
            let _ = io.submit(Op::Interrupt).await;
            if let Err(error) = io.shutdown_and_wait().await {
                if let Some(teardown) = teardown.as_ref() {
                    teardown.record_shutdown_failure();
                }
                tracing::warn!("failed to stop cancelled session init: {error}");
            }
        } else {
            if let Some(session) = self.session.get() {
                super::handlers::shutdown_session_runtime(session).await;
            }
            let mut persistence = std::mem::take(&mut *self.persistence.lock().await);
            if let Err(error) = persistence.discard().await {
                if let Some(teardown) = teardown.as_ref() {
                    teardown.record_shutdown_failure();
                }
                tracing::warn!(
                    "failed to discard thread persistence for failed session init: {error}"
                );
            }
        }
        if let Some(teardown) = teardown {
            teardown.complete();
        }
    }
}

/// Cleans up a cancelled session start before releasing its agent-tree membership.
pub(crate) struct SessionStartupGuard {
    startup: Option<Arc<SessionStartup>>,
}

impl SessionStartupGuard {
    pub(crate) fn new(startup: Arc<SessionStartup>) -> Self {
        Self {
            startup: Some(startup),
        }
    }

    pub(crate) fn disarm(mut self) {
        self.startup = None;
    }

    pub(crate) fn cleanup(mut self) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            if let Some(startup) = self.startup.take() {
                startup.cleanup().await;
            }
        })
    }
}

impl Drop for SessionStartupGuard {
    fn drop(&mut self) {
        let Some(startup) = self.startup.take() else {
            return;
        };
        drop(tokio::spawn(async move {
            startup.cleanup().await;
        }));
    }
}
