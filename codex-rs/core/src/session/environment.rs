use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::MAX_SELECTED_CAPABILITY_ROOTS;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_execpolicy::Policy;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::sandbox::SandboxType;

use crate::config::ConstraintError;
use crate::config::ConstraintResult;
use crate::config::NetworkProxySpec;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::session::Session;
use crate::session::session::SessionConfiguration;
use crate::session::session::SessionSettingsUpdate;

/// Defaults for environments that inherit their configuration from the running turn.
pub(crate) struct ThreadEnvironmentDefaults {
    pub(crate) common: EnvironmentConfig,
    // Chosen once when the session starts; it does not apply to remote environments.
    local_windows_sandbox_type: SandboxType,
}

impl ThreadEnvironmentDefaults {
    pub(crate) fn new(common: EnvironmentConfig, local_windows_sandbox_type: SandboxType) -> Self {
        Self {
            common,
            local_windows_sandbox_type,
        }
    }

    pub(crate) fn for_selection(&self, selection: &TurnEnvironmentSelection) -> EnvironmentConfig {
        let mut config = self.common.clone();
        config.workspace_roots = selection.workspace_roots.clone();
        if selection.environment_id == LOCAL_ENVIRONMENT_ID {
            config.windows_sandbox_type = self.local_windows_sandbox_type;
        }
        config
    }
}

pub(super) fn validate_environment_configs(
    selections: &[TurnEnvironmentSelection],
) -> ConstraintResult<()> {
    for selection in selections {
        match &selection.config {
            EnvironmentConfigState::FromThread
            | EnvironmentConfigState::Pending
            | EnvironmentConfigState::Failed(_) => {}
            EnvironmentConfigState::Ready(config) => {
                validate_environment_config(selection, config).map_err(|error| {
                    ConstraintError::InvalidValue {
                        field_name: "environments",
                        candidate: "environment configuration".to_string(),
                        allowed: format!("valid environment configuration ({error})"),
                        requirement_source: codex_config::RequirementSource::Unknown,
                    }
                })?;
            }
        }
    }
    Ok(())
}

pub(super) fn ensure_configs_stay_owner_provided(
    current: &[TurnEnvironmentSelection],
    proposed: &[TurnEnvironmentSelection],
) -> ConstraintResult<()> {
    if let Some(environment) = proposed.iter().find(|environment| {
        environment.config == EnvironmentConfigState::FromThread
            && current.iter().any(|current| {
                current.environment_id == environment.environment_id
                    && current.config != EnvironmentConfigState::FromThread
            })
    }) {
        return Err(ConstraintError::InvalidValue {
            field_name: "environments",
            candidate: environment.environment_id.clone(),
            allowed: "owner-provided environment configuration".to_string(),
            requirement_source: codex_config::RequirementSource::Unknown,
        });
    }
    Ok(())
}

fn validate_environment_config(
    selection: &TurnEnvironmentSelection,
    config: &EnvironmentConfig,
) -> CodexResult<()> {
    if let Some(policy) = config.network_policy.as_ref() {
        if selection.environment_id == LOCAL_ENVIRONMENT_ID {
            return Err(CodexErr::InvalidRequest(
                "attachment-owned network policy requires a remote executor".to_string(),
            ));
        }
        if config
            .exec_policy
            .as_ref()
            .is_some_and(|policy| !policy.as_ref().network_rules().is_empty())
        {
            return Err(CodexErr::InvalidRequest(
                "environment network restrictions must use network_policy".to_string(),
            ));
        }
        // Validate owner policy on its own; controller compatibility is checked at execution.
        NetworkProxySpec::for_environment(
            /*controller*/ None,
            policy,
            config.permission_profile.permission_profile(),
            &Policy::empty(),
            codex_network_proxy::LocalBindingPolicy::DefaultFalse,
        )
        .map_err(|error| {
            CodexErr::InvalidRequest(format!("invalid environment network policy: {error}"))
        })?;
    }
    if config.selected_capability_roots.len() > MAX_SELECTED_CAPABILITY_ROOTS {
        return Err(CodexErr::InvalidRequest(format!(
            "environment readiness contains more than {MAX_SELECTED_CAPABILITY_ROOTS} selected capability roots"
        )));
    }
    if config
        .exec_policy
        .as_ref()
        .is_some_and(|policy| !policy.as_ref().get_allowed_prefixes().is_empty())
    {
        return Err(CodexErr::InvalidRequest(
            "environment command policy cannot contain allow rules".to_string(),
        ));
    }

    let mut root_ids = HashSet::with_capacity(config.selected_capability_roots.len());
    for root in &config.selected_capability_roots {
        let CapabilityRootLocation::Environment { environment_id, .. } = &root.location;
        if root.id.trim().is_empty()
            || environment_id != &selection.environment_id
            || !root_ids.insert(root.id.as_str())
        {
            return Err(CodexErr::InvalidRequest(format!(
                "selected capability roots must have unique non-empty IDs and belong to environment `{}`",
                selection.environment_id
            )));
        }
    }
    Ok(())
}

enum ConfigUpdateSource {
    Direct,
    Inherited,
}

impl Session {
    pub(super) fn apply_session_settings(
        &self,
        current: &SessionConfiguration,
        updates: &SessionSettingsUpdate,
    ) -> ConstraintResult<SessionConfiguration> {
        let current_environments = &current.environments;
        if let Some(environments) = &updates.environments {
            ensure_configs_stay_owner_provided(current_environments, &environments.environments)?;
        }

        current.apply(updates, current_environments)
    }

    /// Activates the environments accepted for a new task. Configuration may have arrived for
    /// those same environments while its settings were being prepared.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "do not change environments while another task is running"
    )]
    pub(super) async fn activate_turn_environments(
        &self,
        configuration: &SessionConfiguration,
    ) -> TurnEnvironmentSnapshot {
        let snapshot = {
            let active = self.active_turn.lock().await;
            let state = self.state.lock().await;
            // Another task may have started while this one was preparing its settings.
            if active.as_ref().is_none_or(|turn| turn.task.is_none()) {
                let mut environments = configuration.environments.clone();
                let latest_environments = &state.session_configuration.environments;
                for environment in &mut environments {
                    if let Some(latest) = latest_environments.iter().find(|latest| {
                        latest.environment_id == environment.environment_id
                            && latest.cwd == environment.cwd
                            && latest.workspace_roots == environment.workspace_roots
                    }) {
                        environment.config = latest.config.clone();
                    }
                }
                // Apply the saved defaults even if this turn selects the same environments.
                self.services
                    .turn_environments
                    .set_active_thread_defaults(configuration.inferred_environment_config());
                if environments != self.services.turn_environments.selections() {
                    self.services
                        .turn_environments
                        .update_selections(&environments);
                }
            }
            self.services.turn_environments.snapshot()
        };
        snapshot.await
    }

    pub(crate) async fn environment_ready(
        &self,
        selection: &TurnEnvironmentSelection,
        config: EnvironmentConfig,
    ) -> CodexResult<()> {
        self.update_environment_configuration(selection, Ok(config), ConfigUpdateSource::Direct)
            .await
    }

    pub(crate) async fn environment_failed(
        &self,
        selection: &TurnEnvironmentSelection,
        error: String,
    ) -> CodexResult<()> {
        self.update_environment_configuration(selection, Err(error), ConfigUpdateSource::Direct)
            .await
    }

    /// Installs the original owner's first result through this session's usual configuration checks.
    pub(super) fn follow_inherited_environment_configurations(
        self: &Arc<Self>,
        inherited: &TurnEnvironmentSnapshot,
        selected: &[TurnEnvironmentSelection],
    ) {
        for starting in inherited
            .starting()
            .filter(|starting| selected.contains(&starting.selection))
        {
            let Some(configuration) = starting.owner_configuration() else {
                continue;
            };
            let selection = starting.selection.clone();
            let session = Arc::downgrade(self);
            let mut status = self.agent_status.subscribe();
            drop(tokio::spawn(async move {
                let result = tokio::select! {
                    result = configuration => result,
                    _ = status.wait_for(|status| matches!(status, AgentStatus::Shutdown)) => return,
                };
                let Some(session) = session.upgrade() else {
                    return;
                };
                let apply = |result| {
                    session.update_environment_configuration(
                        &selection,
                        result,
                        ConfigUpdateSource::Inherited,
                    )
                };
                if let Err(error) = apply(result).await {
                    // Unlike the external owner, no one will retry this child's one-time update.
                    let _ = apply(Err(error.to_string())).await;
                }
            }));
        }
    }

    /// Records configuration, or a failure to obtain it, for this environment and workspace.
    /// The running turn and future turns may use different environments. Update the matching
    /// selection or selections so work already waiting can still finish, and check successful
    /// configuration before saving either list.
    async fn update_environment_configuration(
        &self,
        selection: &TurnEnvironmentSelection,
        result: Result<EnvironmentConfig, String>,
        source: ConfigUpdateSource,
    ) -> CodexResult<()> {
        let mut state = self.state.lock().await;
        let mut current = self.services.turn_environments.selections();
        let mut future = state.session_configuration.environments.clone();
        let inherited = matches!(source, ConfigUpdateSource::Inherited);
        let matches = |environment: &TurnEnvironmentSelection| {
            environment.environment_id == selection.environment_id
                && environment.cwd == selection.cwd
                && environment.workspace_roots == selection.workspace_roots
                && (!inherited || matches!(environment.config, EnvironmentConfigState::Pending))
        };
        let environments = (
            current.iter_mut().find(|environment| matches(environment)),
            future.iter_mut().find(|environment| matches(environment)),
        );
        // Ignore the parent's result if neither the running turn nor future turns are still
        // waiting for this environment's configuration.
        if inherited && matches!(environments, (None, None)) {
            return Ok(());
        }
        if let Ok(config) = &result {
            validate_environment_config(selection, config)?;
        }
        let (current_environment, future_environment) = match environments {
            (None, None) => {
                return Err(CodexErr::InvalidRequest(format!(
                    "environment `{}` is not selected on this thread with the requested workspace",
                    selection.environment_id
                )));
            }
            (Some(current), Some(future)) if current.config != future.config => {
                // Update the selection still waiting for configuration. If neither is waiting,
                // keep the existing behavior of updating the current one.
                if matches!(future.config, EnvironmentConfigState::Pending) {
                    (None, Some(future))
                } else {
                    (Some(current), None)
                }
            }
            (Some(current), future) => (Some(current), future),
            (None, Some(future)) => (None, Some(future)),
        };
        let successful = result.is_ok();
        let config = match result {
            Ok(config) => EnvironmentConfigState::Ready(config),
            Err(error) => EnvironmentConfigState::Failed(error),
        };
        let update_current = current_environment.is_some();
        let update_future = future_environment.is_some();

        if let Some(environment) = current_environment {
            environment.config = config.clone();
        }
        if let Some(environment) = future_environment {
            environment.config = config;
        }
        if successful {
            let validate = |environments: &[TurnEnvironmentSelection]| {
                state
                    .session_configuration
                    .validate(environments)
                    .map_err(|error| CodexErr::InvalidRequest(error.to_string()))
            };
            if update_current {
                validate(&current)?;
            }
            if update_future && (!update_current || current != future) {
                validate(&future)?;
            }
        }

        state.session_configuration.environments = future;
        if update_current {
            // Invalidate MCP before installed configuration can wake a waiting turn.
            self.mark_mcp_runtime_dirty();
            self.services.turn_environments.update_selections(&current);
        }
        Ok(())
    }

    /// Combines this session's persisted roots with ready environment attachments.
    pub(crate) fn inspect_selected_capability_roots(&self) -> SelectedCapabilityRootsStatus {
        self.services
            .turn_environments
            .inspect_selected_capability_roots(&self.services.selected_capability_roots)
    }
}
