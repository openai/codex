use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::session::turn_context::TurnEnvironment;

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

#[derive(Clone, Debug)]
pub(crate) struct ResolvedTurnEnvironments {
    pub(crate) turn_environments: Vec<TurnEnvironment>,
}

impl ResolvedTurnEnvironments {
    pub(crate) fn to_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.turn_environments
            .iter()
            .map(TurnEnvironment::selection)
            .collect()
    }

    pub(crate) fn primary_turn_environment(&self) -> Option<&TurnEnvironment> {
        self.turn_environments.first()
    }

    pub(crate) fn primary_environment(&self) -> Option<Arc<codex_exec_server::Environment>> {
        self.primary_turn_environment()
            .map(|environment| Arc::clone(&environment.environment))
    }

    pub(crate) fn primary_filesystem(&self) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.primary_turn_environment()
            .map(|environment| environment.environment.get_filesystem())
    }
}

pub(crate) fn resolve_environment_selections(
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<ResolvedTurnEnvironments> {
    let mut seen_environment_ids = HashSet::with_capacity(environments.len());
    let mut turn_environments = Vec::with_capacity(environments.len());
    for selected_environment in environments {
        if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
            return Err(CodexErr::InvalidRequest(format!(
                "duplicate turn environment id `{}`",
                selected_environment.environment_id
            )));
        }
        let environment_id = selected_environment.environment_id.clone();
        let environment = environment_manager
            .get_environment(&environment_id)
            .ok_or_else(|| {
                CodexErr::InvalidRequest(format!("unknown turn environment id `{environment_id}`"))
            })?;
        turn_environments.push(TurnEnvironment {
            environment_id,
            environment,
            cwd: selected_environment.cwd.clone(),
            // TODO(starr): Resolve shell metadata per environment instead of
            // hardcoding bash.
            shell: "bash".to_string(),
        });
    }

    Ok(ResolvedTurnEnvironments { turn_environments })
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

    #[tokio::test]
    async fn resolve_environment_selections_rejects_duplicate_ids() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::default_for_tests();

        let err = resolve_environment_selections(
            &manager,
            &[
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: cwd.clone(),
                },
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: cwd.join("other"),
                },
            ],
        )
        .expect_err("duplicate environment id should fail");

        assert!(err.to_string().contains("duplicate"));
    }

    #[tokio::test]
    async fn resolved_environment_selections_use_first_selection_as_primary() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selected_cwd = cwd.join("selected");
        let manager = EnvironmentManager::default_for_tests();

        let resolved = resolve_environment_selections(
            &manager,
            &[TurnEnvironmentSelection {
                environment_id: "local".to_string(),
                cwd: selected_cwd,
            }],
        )
        .expect("environment selections should resolve");

        assert_eq!(
            resolved
                .primary_turn_environment()
                .expect("primary environment")
                .environment_id,
            "local"
        );
    }
}
