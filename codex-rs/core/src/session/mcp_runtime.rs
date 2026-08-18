//! Thread MCP runtime projection and publication.
//!
//! This module owns the small correctness boundary between immutable session
//! inputs and the mutable [`codex_mcp::McpRuntime`]. Background scheduling
//! belongs elsewhere.

use super::session::SessionConfiguration;
use super::*;
use crate::mcp::McpRuntimeProjection;
use codex_mcp::ElicitationReviewerHandle;
use codex_mcp::McpStartupPolicy;
use codex_mcp::PreparedMcpCall;
use codex_protocol::capabilities::SelectedCapabilityRoot;

pub(super) struct McpDesiredState {
    pub(super) config: Arc<Config>,
    pub(super) auth: Option<CodexAuth>,
    pub(super) submit_id: String,
    pub(super) originator: String,
    pub(super) session_source: SessionSource,
    pub(super) environments: TurnEnvironmentSnapshot,
    pub(super) local_process_cwd: PathBuf,
    pub(super) windows_sandbox_level: WindowsSandboxLevel,
}

impl Session {
    pub(super) fn mcp_inputs_differ(
        &self,
        current: &SessionConfiguration,
        next: &SessionConfiguration,
        updates: &SessionSettingsUpdate,
    ) -> bool {
        current.cwd() != next.cwd()
            || current.approval_policy.value() != next.approval_policy.value()
            || current.approvals_reviewer != next.approvals_reviewer
            || current.permission_profile() != next.permission_profile()
            || updates.environments.as_ref().is_some_and(|environments| {
                environments.environments != self.services.turn_environments.selections()
            })
    }

    /// Waits on this session's refreshed server before tool execution is admitted.
    pub(crate) async fn wait_for_mcp_server(self: &Arc<Self>, server: &str) {
        self.refresh_mcp_if_dirty().await;
        self.services
            .mcp_runtime
            .wait_for_server_startup(server)
            .await;
    }

    /// Captures this session's current MCP client and catalog for one tool call.
    pub(crate) async fn prepare_mcp_call(
        self: &Arc<Self>,
        server: &str,
        tool: &str,
    ) -> Option<PreparedMcpCall> {
        self.refresh_mcp_if_dirty().await;
        self.services
            .mcp_runtime
            .current_binding_for_call(server)
            .await?
            .prepare_call(server, tool)
    }

    pub(super) async fn latest_mcp_desired_state(
        &self,
        auth: Option<CodexAuth>,
    ) -> McpDesiredState {
        let session_configuration = {
            let state = self.state.lock().await;
            state.session_configuration.clone()
        };
        let environments = self.services.turn_environments.snapshot().await;
        let cwd = environments
            .primary()
            .and_then(|environment| environment.cwd().to_abs_path().ok())
            .unwrap_or_else(|| session_configuration.cwd().clone());
        let config = self.build_per_turn_config(&session_configuration, cwd);
        let local_process_cwd = environments
            .local_environment_cwd()
            .unwrap_or_else(|| session_configuration.cwd().clone())
            .to_path_buf();

        McpDesiredState {
            config: Arc::new(config),
            auth,
            submit_id: self.next_internal_sub_id(),
            originator: session_configuration.originator.clone(),
            session_source: session_configuration.session_source.clone(),
            environments,
            local_process_cwd,
            windows_sandbox_level: session_configuration.windows_sandbox_level,
        }
    }

    pub(super) async fn install_initial_mcp_runtime(
        self: &Arc<Self>,
        session_configuration: &SessionConfiguration,
        auth: Option<CodexAuth>,
        mcp_projection: McpRuntimeProjection,
        resolved_environments: &TurnEnvironmentSnapshot,
        mcp_runtime_cwd: PathBuf,
    ) -> anyhow::Result<()> {
        let cwd = AbsolutePathBuf::from_absolute_path(mcp_runtime_cwd)
            .unwrap_or_else(|_| session_configuration.cwd().clone());
        let config = self.build_per_turn_config(session_configuration, cwd);
        let local_process_cwd = resolved_environments
            .local_environment_cwd()
            .unwrap_or_else(|| session_configuration.cwd().clone())
            .to_path_buf();
        let desired = McpDesiredState {
            config: Arc::new(config),
            auth,
            submit_id: INITIAL_SUBMIT_ID.to_owned(),
            originator: session_configuration.originator.clone(),
            session_source: session_configuration.session_source.clone(),
            environments: resolved_environments.clone(),
            local_process_cwd,
            windows_sandbox_level: session_configuration.windows_sandbox_level,
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
        let selected_plugins = mcp_projection.selected_plugins.clone();
        let input = self.build_mcp_runtime_input(
            desired,
            mcp_projection,
            ready_selected_capability_roots,
            elicitation_reviewer,
        );
        self.services.mcp_runtime.replace(input).await;
        self.services.thread_extension_data.insert(selected_plugins);
    }

    pub(super) fn build_mcp_runtime_input(
        &self,
        desired: &McpDesiredState,
        mcp_projection: McpRuntimeProjection,
        ready_selected_capability_roots: &[SelectedCapabilityRoot],
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> McpRuntimeInput {
        let auth = desired.auth.clone();
        let McpRuntimeProjection {
            mut config,
            plugins_available,
            selected_plugins: _,
        } = mcp_projection;
        config.approval_policy = desired.config.permissions.approval_policy.clone();
        config.permission_profile = desired.config.permissions.effective_permission_profile();
        config.approvals_reviewer = desired.config.approvals_reviewer;
        config.environment_cwds = desired
            .environments
            .turn_environments()
            .map(|environment| {
                (
                    environment.selection.environment_id.clone(),
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
        let runtime_context = McpRuntimeContext::new(
            self.services.turn_environments.environment_manager(),
            desired.local_process_cwd.clone(),
        );
        let codex_apps_auth_manager =
            codex_mcp::host_owned_codex_apps_enabled(&mcp_config, auth.as_ref())
                .then(|| Arc::clone(&self.services.auth_manager));

        McpRuntimeInput {
            startup_policy: if matches!(desired.session_source, SessionSource::SubAgent(_)) {
                McpStartupPolicy::LazyWhenCached
            } else {
                McpStartupPolicy::Eager
            },
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
            client_mcp_extensions: self.services.client_mcp_extensions.for_mcp_servers(),
            auth,
            codex_apps_auth_manager,
            elicitation_reviewer,
            elicitation_lifecycle: Some(self.mcp_elicitation_lifecycle()),
        }
    }
}
