use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use crate::session::turn_context::TurnEnvironment;
use crate::shell::Shell;

pub(crate) fn default_thread_environment_selections(
    environment_manager: &EnvironmentManager,
    cwd: &AbsolutePathBuf,
) -> Vec<TurnEnvironmentSelection> {
    environment_manager
        .default_environment_ids()
        .into_iter()
        .map(|environment_id| TurnEnvironmentSelection {
            environment_id,
            cwd: PathUri::from_abs_path(cwd),
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
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

    pub(crate) fn primary(&self) -> Option<&TurnEnvironment> {
        self.turn_environments.first()
    }

    #[cfg(test)]
    pub(crate) fn primary_environment(&self) -> Option<Arc<codex_exec_server::Environment>> {
        self.primary()
            .map(|environment| Arc::clone(&environment.environment))
    }

    pub(crate) fn primary_filesystem(&self) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.primary()
            .map(|environment| environment.environment.get_filesystem())
    }

    pub(crate) fn single_local_environment_cwd(&self) -> Option<AbsolutePathBuf> {
        let [environment] = self.turn_environments.as_slice() else {
            return None;
        };

        (!environment.environment.is_remote())
            .then(|| environment.compatible_cwd())
            .flatten()
    }
}

pub(crate) async fn resolve_environment_selections(
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
        let info = environment.info().await.map_err(|err| {
            CodexErr::InvalidRequest(format!(
                "failed to get info for environment `{environment_id}`: {err}"
            ))
        })?;
        let path_convention = info.path_convention;
        let shell = Shell::from_environment_shell_info(info.shell).map_err(|err| {
            CodexErr::InvalidRequest(format!(
                "failed to resolve shell for environment `{environment_id}`: {err}"
            ))
        })?;
        turn_environments.push(TurnEnvironment::new_with_uri(
            environment_id,
            environment,
            selected_environment.cwd.clone(),
            path_convention,
            shell,
        )?);
    }
    Ok(ResolvedTurnEnvironments { turn_environments })
}

#[cfg(test)]
mod tests {
    use codex_exec_server::Environment;
    use codex_exec_server::ExecServerRuntimePaths;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
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
            Some(test_runtime_paths()),
        )
        .await;

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&cwd),
            }]
        );
    }

    #[tokio::test]
    async fn toml_default_thread_environment_selections_include_local_and_remote() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp_dir.path().join("environments.toml"),
            r#"
[[environments]]
id = "remote"
url = "ws://127.0.0.1:8765"
"#,
        )
        .expect("write environments.toml");
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager =
            EnvironmentManager::from_codex_home(temp_dir.path(), Some(test_runtime_paths()))
                .await
                .expect("environment manager");

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_abs_path(&cwd),
                },
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_abs_path(&cwd),
                },
            ]
        );
    }

    #[tokio::test]
    async fn default_thread_environment_selections_empty_when_default_disabled() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = EnvironmentManager::without_environments();

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
                    cwd: PathUri::from_abs_path(&cwd),
                },
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: PathUri::from_abs_path(&cwd.join("other")),
                },
            ],
        )
        .await
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
                cwd: PathUri::from_abs_path(&selected_cwd),
            }],
        )
        .await
        .expect("environment selections should resolve");

        assert_eq!(
            resolved
                .primary()
                .expect("primary environment")
                .environment_id,
            "local"
        );
        assert_eq!(
            resolved.primary().expect("primary environment").shell,
            Shell::from_environment_shell_info(
                manager
                    .get_environment("local")
                    .expect("local environment")
                    .info()
                    .await
                    .expect("local environment info")
                    .shell
            )
            .expect("resolved shell")
        );
    }

    #[tokio::test]
    async fn single_local_environment_cwd_requires_exactly_one_local_environment() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let local_manager = EnvironmentManager::default_for_tests();
        let local = resolve_environment_selections(
            &local_manager,
            &[TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&cwd),
            }],
        )
        .await
        .expect("local environment should resolve");
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let remote = ResolvedTurnEnvironments {
            turn_environments: vec![TurnEnvironment::new(
                REMOTE_ENVIRONMENT_ID.to_string(),
                remote_environment.clone(),
                cwd.clone(),
                crate::shell::default_user_shell(),
            )],
        };
        let multiple = ResolvedTurnEnvironments {
            turn_environments: vec![
                local.primary().expect("local environment").clone(),
                TurnEnvironment::new(
                    REMOTE_ENVIRONMENT_ID.to_string(),
                    remote_environment,
                    cwd.clone(),
                    crate::shell::default_user_shell(),
                ),
            ],
        };

        assert_eq!(local.single_local_environment_cwd(), Some(cwd.clone()));
        assert_eq!(remote.single_local_environment_cwd(), None);
        assert_eq!(multiple.single_local_environment_cwd(), None);
    }
}
