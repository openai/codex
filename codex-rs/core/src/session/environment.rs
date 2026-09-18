use std::collections::HashSet;

use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::MAX_SELECTED_CAPABILITY_ROOTS;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_execpolicy::Policy;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelection;

use crate::config::ConstraintError;
use crate::config::ConstraintResult;
use crate::config::NetworkProxySpec;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::session::Session;
use crate::session::session::SessionConfiguration;
use crate::session::session::SessionSettingsUpdate;

pub(super) fn validate_environment_selections(
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

impl Session {
    pub(super) fn apply_session_settings(
        &self,
        current: &SessionConfiguration,
        updates: &SessionSettingsUpdate,
    ) -> ConstraintResult<SessionConfiguration> {
        let current_environments = &current.environments;
        if let Some(environments) = &updates.environments
            && let Some(environment) = environments.environments.iter().find(|environment| {
                environment.config == EnvironmentConfigState::FromThread
                    && current_environments.iter().any(|current| {
                        current.environment_id == environment.environment_id
                            && current.config != EnvironmentConfigState::FromThread
                    })
            })
        {
            return Err(ConstraintError::InvalidValue {
                field_name: "environments",
                candidate: environment.environment_id.clone(),
                allowed: "owner-provided environment configuration".to_string(),
                requirement_source: codex_config::RequirementSource::Unknown,
            });
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
                if environments != self.services.turn_environments.selections() {
                    self.services.turn_environments.update_selections(
                        &environments,
                        &configuration.inferred_environment_config(),
                    );
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
        validate_environment_config(selection, &config)?;
        self.update_environment_configuration(selection, EnvironmentConfigState::Ready(config))
            .await
    }

    pub(crate) async fn environment_failed(
        &self,
        selection: &TurnEnvironmentSelection,
        error: String,
    ) -> CodexResult<()> {
        self.update_environment_configuration(selection, EnvironmentConfigState::Failed(error))
            .await
    }

    /// Records configuration, or a failure to obtain it, for this environment and workspace.
    /// The running turn and future turns may use different environments. Update the matching
    /// selection or selections so work already waiting can still finish, and check successful
    /// configuration before saving either list.
    async fn update_environment_configuration(
        &self,
        selection: &TurnEnvironmentSelection,
        config: EnvironmentConfigState,
    ) -> CodexResult<()> {
        // Serialize owner callbacks with ordinary thread settings updates.
        let mut state = self.state.lock().await;
        let mut current = self.services.turn_environments.selections();
        let mut future = state.session_configuration.environments.clone();
        let matches = |environment: &TurnEnvironmentSelection| {
            environment.environment_id == selection.environment_id
                && environment.cwd == selection.cwd
                && environment.workspace_roots == selection.workspace_roots
        };
        let (current_environment, future_environment) = match (
            current.iter_mut().find(|environment| matches(environment)),
            future.iter_mut().find(|environment| matches(environment)),
        ) {
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
        let update_current = current_environment.is_some();
        let update_future = future_environment.is_some();

        if let Some(environment) = current_environment {
            environment.config = config.clone();
        }
        if let Some(environment) = future_environment {
            environment.config = config.clone();
        }
        if matches!(config, EnvironmentConfigState::Ready(_)) {
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
            self.services.turn_environments.update_selections(
                &current,
                &state.session_configuration.inferred_environment_config(),
            );
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
