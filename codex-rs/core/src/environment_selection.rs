use codex_exec_server::EnvironmentManager;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;

pub fn default_thread_environment_selections(
    environment_manager: &EnvironmentManager,
    cwd: &AbsolutePathBuf,
) -> Vec<TurnEnvironmentSelection> {
    if environment_manager.default_environment().is_none() {
        return Vec::new();
    }

    vec![TurnEnvironmentSelection {
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: cwd.clone(),
    }]
}

pub(crate) fn validate_environment_selections(
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    for selected_environment in environments {
        if environment_manager
            .get_environment(&selected_environment.environment_id)
            .is_none()
        {
            return Err(CodexErr::InvalidRequest(format!(
                "unknown turn environment id `{}`",
                selected_environment.environment_id
            )));
        }
    }

    Ok(())
}
