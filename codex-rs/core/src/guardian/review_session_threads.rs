//! Adapts the existing reviewer lifecycle to the extension's ThreadManager.
//! Parent registration gates spawning; prompts, reuse and review outcomes stay shared.

use std::sync::Arc;
use std::sync::Weak;

use codex_async_utils::OrCancelExt;
use codex_history::InitialHistory;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianReviewContext;
use crate::StartThreadOptions;
use crate::ThreadManager;
use crate::codex_delegate::forward_session_io;
use crate::config::Config;
use crate::config::Constrained;
use crate::session::SessionIo;
use crate::session::emit_subagent_session_started;
use crate::session::session::Session;

pub(super) struct ManagedReviewerThreads {
    manager: Weak<ThreadManager>,
    ready: watch::Sender<bool>,
}

impl ManagedReviewerThreads {
    pub(super) fn new(manager: Weak<ThreadManager>) -> Self {
        Self {
            manager,
            ready: watch::channel(/*init*/ false).0,
        }
    }

    pub(super) fn mark_ready(&self) {
        self.ready.send_replace(true);
    }

    pub(super) async fn spawn(
        &self,
        parent: &Arc<Session>,
        context: &GuardianReviewContext,
        mut config: Config,
        cancel: CancellationToken,
        history: Option<InitialHistory>,
    ) -> anyhow::Result<(Arc<Session>, SessionIo)> {
        let mut ready = self.ready.subscribe();
        tokio::select! {
            result = ready.wait_for(|ready| *ready) => { result?; }
            _ = cancel.cancelled() => anyhow::bail!("guardian reviewer is shutting down"),
        }
        let manager = self
            .manager
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("thread manager is no longer available"))?;
        // Match the standalone delegate's settings before using the managed spawn path.
        if config.permissions.approval_policy.value() != AskForApproval::Never {
            anyhow::bail!("Codex delegates require approval policy `never`");
        }
        config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
        config.model_provider.supports_websockets &=
            parent.services.model_client.responses_websocket_enabled();
        let mut thread_extension_init = codex_extension_api::ExtensionDataInit::default();
        thread_extension_init.insert(codex_extension_api::SessionIsolation::Isolated);
        let options = StartThreadOptions {
            thread_extension_init,
            session_source: Some(SessionSource::Internal(InternalSessionSource::Guardian)),
            thread_source: Some(ThreadSource::GuardianReview),
            environments: Some(context.environments().to_selections()),
            inherited_environments: Some(context.environments().clone()),
            client_mcp_extensions: parent.services.client_mcp_extensions.clone(),
            ..StartThreadOptions::new(config)
        };
        let history = match history.unwrap_or(InitialHistory::New) {
            InitialHistory::Forked(history) => InitialHistory::Forked(history),
            InitialHistory::New | InitialHistory::Cleared => InitialHistory::New,
            InitialHistory::Resumed(_) => {
                anyhow::bail!("guardian review forks cannot resume an existing thread")
            }
        };
        // Generic internal sessions start isolated. Reviewers explicitly inherit
        // the parent's applied instructions, just like standalone delegates.
        let spawned = manager
            .spawn_internal_session_with_history(
                parent.thread_id(),
                options,
                history,
                Some(parent.inherited_instructions().await),
            )
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
