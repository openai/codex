//! Retains persistence acquisition and resources throughout managed startup.
//! The thread manager drops initialization, then joins acquisition and resource cleanup here.

use std::sync::Arc;
use std::sync::OnceLock;

use codex_protocol::protocol::Op;
use codex_thread_store::LiveThreadInitGuard;
use tokio::sync::Mutex;

use super::SessionIo;
use super::session::Session;

#[derive(Default)]
pub(crate) struct SessionStartup {
    pub(crate) persistence: Mutex<LiveThreadInitGuard>,
    pub(crate) session: OnceLock<Arc<Session>>,
    pub(crate) io: OnceLock<SessionIo>,
}

impl SessionStartup {
    pub(crate) async fn cleanup(&self) {
        if let Some(io) = self.io.get() {
            // The session loop owns persistence now. Preserve its shutdown semantics even
            // if registration or the caller's handoff was interrupted after the loop started.
            self.persistence.lock().await.commit();
            let _ = io.submit(Op::Interrupt).await;
            let _ = io.shutdown_and_wait().await;
        } else {
            if let Some(session) = self.session.get() {
                super::handlers::shutdown_session_runtime(session).await;
            }
            let mut persistence = std::mem::take(&mut *self.persistence.lock().await);
            persistence.discard().await;
        }
    }
}
