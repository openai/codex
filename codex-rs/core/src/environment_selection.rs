use std::sync::Arc;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) fn default_thread_environment_selections(
    environment_manager: &EnvironmentManager,
    cwd: &AbsolutePathBuf,
) -> Vec<TurnEnvironmentSelection> {
    environment_manager
        .default_environment_id()
        .map(|environment_id| TurnEnvironmentSelection {
            environment_id: environment_id.to_string(),
            cwd: cwd.clone(),
        })
        .into_iter()
        .collect()
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

pub(crate) fn selected_primary_environment(
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<Option<Arc<Environment>>> {
    environments
        .first()
        .map(|selected_environment| {
            environment_manager
                .get_environment(&selected_environment.environment_id)
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(format!(
                        "unknown turn environment id `{}`",
                        selected_environment.environment_id
                    ))
                })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use codex_exec_server::ExecServerRuntimePaths;
    use codex_exec_server::REMOTE_ENVIRONMENT_ID;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use super::*;

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    #[tokio::test]
    async fn default_thread_environment_selections_use_manager_default_id() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::create_for_tests(
            Some("ws://127.0.0.1:8765".to_string()),
            test_runtime_paths(),
        )
        .await;

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd,
            }]
        );
    }

    #[tokio::test]
    async fn default_thread_environment_selections_empty_when_default_disabled() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            Vec::<TurnEnvironmentSelection>::new()
        );
    }
}
