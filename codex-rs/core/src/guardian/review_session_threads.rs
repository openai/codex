//! Adapts the existing reviewer lifecycle to the extension's ThreadManager.
//! Captured parent inputs support inline delegates; prompts and review outcomes stay shared.

use std::sync::Arc;
use std::sync::Weak;

use codex_async_utils::OrCancelExt;
use codex_history::InitialHistory;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use tokio_util::sync::CancellationToken;

use super::GUARDIAN_REVIEWER_NAME;
use crate::StartThreadOptions;
use crate::ThreadManager;
use crate::codex_delegate::forward_session_io;
use crate::config::Constrained;
use crate::session::SessionIo;
use crate::session::emit_subagent_session_started;
use crate::session::session::Session;

pub(super) struct ManagedReviewerThreads {
    manager: Weak<ThreadManager>,
}

impl ManagedReviewerThreads {
    pub(super) fn new(manager: Weak<ThreadManager>) -> Self {
        Self { manager }
    }

    pub(super) async fn spawn(
        &self,
        parent: &Arc<Session>,
        mut options: StartThreadOptions,
        cancel: CancellationToken,
    ) -> anyhow::Result<(Arc<Session>, SessionIo)> {
        let manager = self
            .manager
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("thread manager is no longer available"))?;
        if options.config.permissions.approval_policy.value() != AskForApproval::Never {
            anyhow::bail!("Codex delegates require approval policy `never`");
        }
        options.config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
        options.session_source = Some(SessionSource::Internal(InternalSessionSource::Guardian));
        options.thread_source = Some(ThreadSource::GuardianReview);
        options
            .thread_extension_init
            .insert(codex_extension_api::SessionIsolation::Isolated);
        options.initial_history = match options.initial_history {
            InitialHistory::Forked(history) => InitialHistory::Forked(history),
            InitialHistory::New | InitialHistory::Cleared => InitialHistory::New,
            InitialHistory::Resumed(_) => {
                anyhow::bail!("guardian review forks cannot resume an existing thread")
            }
        };
        let spawned = manager
            .start_thread(options)
            .or_cancel(&cancel)
            .await
            .map_err(codex_protocol::error::CodexErr::from)??;
        let thread = spawned.thread;
        let session = Arc::clone(&thread.session);
        let io = forward_session_io(
            Arc::new(SessionIo {
                tx_sub: thread.io.tx_sub.clone(),
                rx_event: thread.io.rx_event.clone(),
                agent_status: thread.io.agent_status.clone(),
                session_loop_termination: thread.io.session_loop_termination.clone(),
            }),
            cancel,
        );
        let manager = Arc::downgrade(&manager);
        drop(tokio::spawn(async move {
            thread.wait_until_terminated().await;
            if let Some(manager) = manager.upgrade() {
                manager
                    .remove_thread_if_matches(&thread.session.thread_id(), &thread)
                    .await;
            }
        }));
        emit_subagent_session_started(
            &parent.services.analytics_events_client,
            parent.app_server_client_metadata().await,
            session.session_id(),
            session.thread_id(),
            Some(parent.thread_id()),
            session.thread_config_snapshot().await,
            SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_owned()),
        );
        Ok((session, io))
    }
}
