use codex_core_plugins::SelectedCapabilityActivation;
use codex_core_plugins::SelectedCapabilityBindings;
use codex_mcp::ElicitationReviewerHandle;
use codex_mcp::McpConnectionManager;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::Session;
use super::TurnContext;

struct PendingMcpManager<'a> {
    slot: &'a arc_swap::ArcSwapOption<McpConnectionManager>,
    manager: Arc<McpConnectionManager>,
    cancellation_token: CancellationToken,
    committed: bool,
}

impl<'a> PendingMcpManager<'a> {
    fn new(
        slot: &'a arc_swap::ArcSwapOption<McpConnectionManager>,
        manager: McpConnectionManager,
        cancellation_token: CancellationToken,
    ) -> Self {
        let manager = Arc::new(manager);
        slot.store(Some(Arc::clone(&manager)));
        Self {
            slot,
            manager,
            cancellation_token,
            committed: false,
        }
    }

    fn commit(&mut self) -> Arc<McpConnectionManager> {
        self.committed = true;
        Arc::clone(&self.manager)
    }
}

impl Drop for PendingMcpManager<'_> {
    fn drop(&mut self) {
        self.slot.store(None);
        if !self.committed {
            self.cancellation_token.cancel();
        }
    }
}

impl Session {
    /// Publishes the latest ready selected roots at a model sampling boundary.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "one activation candidate must build, validate, and commit at a time"
    )]
    pub(crate) async fn prepare_runtime_snapshot(
        &self,
        turn_context: &TurnContext,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> CodexResult<()> {
        let _runtime_snapshot_guard = self.runtime_snapshot_lock.lock().await;
        let Some(bindings) = self
            .services
            .mcp_thread_init
            .get::<SelectedCapabilityBindings>()
        else {
            return Ok(());
        };
        let Some(active) = self
            .services
            .mcp_thread_init
            .get::<SelectedCapabilityActivation>()
        else {
            return Ok(());
        };
        let selected_capabilities = bindings.snapshot();
        if selected_capabilities.generation() <= active.snapshot().generation() {
            return Ok(());
        }

        let mut candidate = self.services.mcp_thread_init.clone();
        candidate.insert(SelectedCapabilityActivation::new(selected_capabilities));
        self.services
            .extensions
            .prepare_runtime_snapshot(&mut candidate)
            .await;
        let Some(candidate_activation) = candidate.get::<SelectedCapabilityActivation>() else {
            return Ok(());
        };
        let candidate_snapshot = candidate_activation.snapshot();
        let config = turn_context.config.as_ref();
        let mcp_config = self
            .services
            .mcp_manager
            .runtime_config_for_thread(config, &candidate)
            .await;
        let candidate_cancellation_token = CancellationToken::new();
        let refreshed_manager = self
            .build_mcp_connection_manager(
                turn_context,
                codex_mcp::configured_mcp_servers(&mcp_config),
                config.mcp_oauth_credentials_store_mode,
                config.auth_keyring_backend_kind(),
                elicitation_reviewer,
                config,
                mcp_config,
                candidate_cancellation_token.clone(),
            )
            .await;
        let current_manager = self.services.mcp_connection_manager.load_full();
        refreshed_manager.set_elicitations_auto_deny(current_manager.elicitations_auto_deny());
        let mut pending_manager = PendingMcpManager::new(
            &self.services.pending_mcp_connection_manager,
            refreshed_manager,
            candidate_cancellation_token.clone(),
        );
        if let Err(err) = pending_manager.manager.validate_required_servers().await {
            tracing::warn!(error = %err, "selected capability MCP generation failed validation");
            let failed_manager = Arc::clone(&pending_manager.manager);
            drop(pending_manager);
            failed_manager.shutdown().await;
            return Err(CodexErr::InvalidRequest(err.to_string()));
        }
        let superseded_manager = {
            let mut startup_token = self.services.mcp_startup_cancellation_token.lock().await;
            let _runtime_snapshot_view = self.runtime_snapshot_view_lock.write().await;
            startup_token.cancel();
            *startup_token = candidate_cancellation_token;
            pending_manager.manager.set_elicitations_auto_deny(
                self.services
                    .mcp_connection_manager
                    .load_full()
                    .elicitations_auto_deny(),
            );
            let refreshed_manager = pending_manager.commit();
            let superseded_manager = self.services.mcp_connection_manager.swap(refreshed_manager);
            drop(pending_manager);
            active.publish(candidate_snapshot.selected_capabilities().clone());
            self.services
                .extensions
                .commit_runtime_snapshot(&candidate, &self.services.mcp_thread_init);
            superseded_manager
        };
        superseded_manager.shutdown().await;
        Ok(())
    }
}
