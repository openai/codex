//! Thread MCP runtime projection and publication.
//!
//! This module owns the small correctness boundary between immutable session
//! inputs and the mutable [`codex_mcp::McpRuntime`]. Background scheduling
//! belongs elsewhere.

use super::session::SessionConfiguration;
use super::*;
use crate::mcp::McpRuntimeProjection;
use codex_mcp::ElicitationReviewerHandle;
use codex_protocol::capabilities::SelectedCapabilityRoot;

pub(super) struct McpDesiredState {
    pub(super) config: Arc<Config>,
    pub(super) submit_id: String,
    pub(super) originator: String,
    pub(super) environments: TurnEnvironmentSnapshot,
}

impl McpDesiredState {
    pub(super) fn local_stdio_fallback_cwd(&self) -> PathBuf {
        self.environments
            .primary()
            .and_then(|environment| environment.cwd().to_abs_path().ok())
            .map(|cwd| cwd.to_path_buf())
            .unwrap_or_else(|| self.config.cwd.to_path_buf())
    }
}

impl Session {
    pub(super) async fn latest_mcp_desired_state(&self) -> McpDesiredState {
        let session_configuration = {
            let state = self.state.lock().await;
            state.session_configuration.clone()
        };
        let environments = self.services.turn_environments.snapshot().await;
        let cwd = environments
            .primary()
            .and_then(|environment| environment.cwd().to_abs_path().ok())
            .unwrap_or_else(|| session_configuration.cwd().clone());
        let mut config = Self::build_per_turn_config(&session_configuration, cwd);
        config.permissions.approval_policy = session_configuration.approval_policy.clone();

        McpDesiredState {
            config: Arc::new(config),
            submit_id: self.next_internal_sub_id(),
            originator: session_configuration.originator.clone(),
            environments,
        }
    }

    pub(super) async fn install_initial_mcp_runtime(
        self: &Arc<Self>,
        session_configuration: &SessionConfiguration,
        mcp_projection: McpRuntimeProjection,
        resolved_environments: &TurnEnvironmentSnapshot,
        local_stdio_fallback_cwd: PathBuf,
    ) -> anyhow::Result<()> {
        let cwd = AbsolutePathBuf::from_absolute_path(local_stdio_fallback_cwd)
            .unwrap_or_else(|_| session_configuration.cwd().clone());
        let mut config = Self::build_per_turn_config(session_configuration, cwd);
        config.permissions.approval_policy = session_configuration.approval_policy.clone();
        let desired = McpDesiredState {
            config: Arc::new(config),
            submit_id: INITIAL_SUBMIT_ID.to_owned(),
            originator: session_configuration.originator.clone(),
            environments: resolved_environments.clone(),
        };
        self.publish_mcp_runtime(
            &desired,
            mcp_projection,
            /*ready_selected_capability_roots*/ &[],
            Some(self.mcp_elicitation_reviewer()),
        )
        .instrument(info_span!(
            "session_init.mcp_manager_init",
            otel.name = "session_init.mcp_manager_init",
        ))
        .await;

        self.services.mcp_runtime.validate_required_servers().await
    }

    #[tracing::instrument(name = "mcp.runtime.refresh", skip_all)]
    pub(super) async fn publish_mcp_runtime(
        &self,
        desired: &McpDesiredState,
        mcp_projection: McpRuntimeProjection,
        ready_selected_capability_roots: &[SelectedCapabilityRoot],
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let input = self
            .build_mcp_runtime_input(
                desired,
                mcp_projection,
                ready_selected_capability_roots,
                elicitation_reviewer,
            )
            .await;
        self.services.mcp_runtime.replace(input).await;
    }

    pub(super) async fn build_mcp_runtime_input(
        &self,
        desired: &McpDesiredState,
        mcp_projection: McpRuntimeProjection,
        ready_selected_capability_roots: &[SelectedCapabilityRoot],
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> McpRuntimeInput {
        let auth = self.services.auth_manager.auth().await;
        let supports_openai_form_elicitation = self
            .services
            .supports_openai_form_elicitation
            .load(std::sync::atomic::Ordering::Acquire);
        let McpRuntimeProjection {
            mut config,
            plugins_available,
        } = mcp_projection;
        config.approval_policy = desired.config.permissions.approval_policy.clone();
        config.permission_profile = desired.config.permissions.effective_permission_profile();
        config.approvals_reviewer = desired.config.approvals_reviewer;
        config.environment_cwds = desired
            .environments
            .turn_environments()
            .map(|environment| {
                (
                    environment.environment_id.clone(),
                    environment.cwd().clone(),
                )
            })
            .collect();
        config
            .environment_cwds
            .entry(codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string())
            .or_insert_with(|| PathUri::from_abs_path(&desired.config.cwd));
        let mcp_config = Arc::new(config);
        let mcp_servers = effective_mcp_servers(&mcp_config, auth.as_ref());
        let local_stdio_fallback_cwd = desired.local_stdio_fallback_cwd();
        let runtime_context = McpRuntimeContext::new(
            self.services.turn_environments.environment_manager(),
            local_stdio_fallback_cwd,
        );
        let codex_apps_auth_manager =
            codex_mcp::host_owned_codex_apps_enabled(&mcp_config, auth.as_ref())
                .then(|| Arc::clone(&self.services.auth_manager));

        McpRuntimeInput {
            config: mcp_config,
            plugins_available,
            ready_selected_capability_roots: ready_selected_capability_roots.to_vec(),
            mcp_servers,
            submit_id: desired.submit_id.clone(),
            tx_event: Some(self.get_tx_event()),
            startup_cancellation_token: CancellationToken::new(),
            runtime_context,
            codex_apps_tools_cache: self.services.mcp_manager.codex_apps_tools_cache(),
            tool_catalog_cache: self.services.mcp_manager.tool_catalog_cache(),
            codex_apps_tools_cache_key: connector_runtime_context_key(auth.as_ref()),
            supports_openai_form_elicitation,
            auth,
            codex_apps_auth_manager,
            elicitation_reviewer,
            elicitation_lifecycle: Some(self.mcp_elicitation_lifecycle()),
        }
    }
}
