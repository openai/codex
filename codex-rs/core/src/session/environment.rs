use std::collections::HashSet;

use codex_exec_server::MAX_SELECTED_CAPABILITY_ROOTS;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelection;

use crate::config::ConstraintError;
use crate::config::ConstraintResult;
use crate::session::session::Session;
use crate::session::session::SessionConfiguration;
use crate::session::session::SessionSettingsUpdate;

pub(super) fn validate_environment_selections(
    selections: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    for selection in selections {
        match &selection.config {
            EnvironmentConfigState::FromThread
            | EnvironmentConfigState::Pending
            | EnvironmentConfigState::Failed(_) => {}
            EnvironmentConfigState::Ready(config) => {
                validate_environment_config(selection, config)?;
            }
        }
    }
    Ok(())
}

fn validate_environment_config(
    selection: &TurnEnvironmentSelection,
    config: &EnvironmentConfig,
) -> CodexResult<()> {
    // The public type can be used by owners before runtime enforcement lands. Do not
    // accept restrictions here until the managed proxy can actually enforce them.
    if config.network_policy.is_some() {
        return Err(CodexErr::InvalidRequest(
            "attachment-owned network policy is not supported yet".to_string(),
        ));
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
        let current_environments = self.services.turn_environments.selections();
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

        current.apply(updates, &current_environments)
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

    async fn update_environment_configuration(
        &self,
        selection: &TurnEnvironmentSelection,
        config: EnvironmentConfigState,
    ) -> CodexResult<()> {
        // Serialize owner callbacks with ordinary thread settings updates.
        let state = self.state.lock().await;
        let mut environments = self.services.turn_environments.selections();
        let Some(environment) = environments.iter_mut().find(|environment| {
            environment.environment_id == selection.environment_id
                && environment.cwd == selection.cwd
                && environment.workspace_roots == selection.workspace_roots
        }) else {
            return Err(CodexErr::InvalidRequest(format!(
                "environment `{}` is not selected on this thread with the requested workspace",
                selection.environment_id
            )));
        };

        environment.config = config;
        if matches!(environment.config, EnvironmentConfigState::Ready(_)) {
            state
                .session_configuration
                .validate(&environments)
                .map_err(|error| CodexErr::InvalidRequest(error.to_string()))?;
        }

        // Invalidate MCP before installed configuration can wake a waiting turn.
        self.mark_mcp_runtime_dirty();
        self.services.turn_environments.update_selections(
            &environments,
            &state.session_configuration.inferred_environment_config(),
        );
        Ok(())
    }

    /// Combines this session's persisted roots with ready environment attachments.
    pub(crate) fn inspect_selected_capability_roots(&self) -> SelectedCapabilityRootsStatus {
        self.services
            .turn_environments
            .inspect_selected_capability_roots(&self.services.selected_capability_roots)
    }
}
