use std::collections::HashSet;

use codex_exec_server::MAX_SELECTED_CAPABILITY_ROOTS;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelection;

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
    if config.selected_capability_roots.len() > MAX_SELECTED_CAPABILITY_ROOTS {
        return Err(CodexErr::InvalidRequest(format!(
            "environment readiness contains more than {MAX_SELECTED_CAPABILITY_ROOTS} selected capability roots"
        )));
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
        current.apply(updates, &self.services.turn_environments.selections())
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

        // Invalidate MCP before installed configuration can wake a waiting turn.
        self.mark_mcp_runtime_dirty();
        self.services.turn_environments.update_selections(
            &environments,
            &state.session_configuration.turn_environment_config(),
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
