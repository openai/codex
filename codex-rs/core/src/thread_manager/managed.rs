//! Runs a thread for its owner's chosen lifetime, including cancellation during startup.
//! Cleanup remains tracked when the caller drops its startup future or stops waiting.

use std::future::Future;
use std::sync::Arc;

use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use super::NewThread;
use super::StartThreadOptions;
use super::ThreadManager;
use crate::session::startup::SessionStartup;

impl ThreadManager {
    /// Starts a new isolated thread and owns it until `until` completes or the thread exits.
    /// Dropping the startup future also cancels initialization. `tasks` tracks the
    /// entire lifetime, including partial startup cleanup and deregistration.
    /// Cleanup waits for any in-flight persistence acquisition before releasing its writer.
    /// Once the session runtime starts, cleanup follows normal shutdown history rules,
    /// even if the caller has not received the thread yet.
    /// Resumed threads are excluded because they can refer to another owner's live runtime.
    pub async fn start_thread_until(
        self: &Arc<Self>,
        options: StartThreadOptions,
        until: impl Future<Output = ()> + Send + 'static,
        tasks: &TaskTracker,
    ) -> CodexResult<NewThread> {
        if matches!(
            options.initial_history,
            codex_history::InitialHistory::Resumed(_)
        ) {
            return Err(CodexErr::InvalidRequest(
                "owned thread startup requires new or forked history".to_owned(),
            ));
        }
        if options
            .thread_extension_init
            .get::<codex_extension_api::SessionIsolation>()
            .as_deref()
            != Some(&codex_extension_api::SessionIsolation::Isolated)
        {
            return Err(CodexErr::InvalidRequest(
                "owned thread startup requires explicit session isolation".to_owned(),
            ));
        }
        let task = tasks.token();
        let manager = Arc::clone(self);
        let state = Arc::downgrade(&self.state);
        let (sender, receiver) = oneshot::channel();
        let abandoned = CancellationToken::new();
        let handoff = abandoned.clone().drop_guard();
        tokio::spawn(async move {
            let _task = task;
            let startup = Arc::new(SessionStartup::default());
            tokio::pin!(until);
            // Drop initialization before cleanup; startup still owns any unfinished acquisition.
            let result = {
                let start = manager.start_thread_inner(
                    options,
                    /*forked_from_thread_id*/ None,
                    Some(Arc::clone(&startup)),
                );
                tokio::select! {
                    biased;
                    _ = &mut until => Err(CodexErr::TurnAborted),
                    _ = abandoned.cancelled() => Err(CodexErr::TurnAborted),
                    result = start => result,
                }
            };
            // The lifetime task must not keep the manager and its parent threads alive.
            drop(manager);
            match result {
                Ok(thread) => {
                    let running = Arc::clone(&thread.thread);
                    if sender.send(Ok(thread)).is_ok() {
                        tokio::select! {
                            _ = abandoned.cancelled() => {}
                            _ = &mut until => {}
                            _ = running.wait_until_terminated() => {}
                        }
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                }
            }
            startup.cleanup().await;
            if let Some(state) = state.upgrade()
                && let Some(session) = startup.session.get()
            {
                let mut threads = state.threads.write().await;
                let thread_id = session.thread_id();
                if threads
                    .get(&thread_id)
                    .is_some_and(|thread| Arc::ptr_eq(&thread.session, session))
                {
                    threads.remove(&thread_id);
                }
            }
        });
        let thread = receiver.await.map_err(|_| CodexErr::InternalAgentDied)??;
        handoff.disarm();
        Ok(thread)
    }
}
