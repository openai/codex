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

#[derive(Clone, Debug)]
pub(crate) struct TurnEnvironments {
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) turn_environments: Vec<TurnEnvironment>,
}

impl TurnEnvironments {
    pub(crate) async fn resolve(
        environment_manager: Arc<EnvironmentManager>,
        environments: &[TurnEnvironmentSelection],
    ) -> CodexResult<Self> {
        Self {
            environment_manager,
            turn_environments: Vec::new(),
        }
        .with_selections(environments)
        .await
    }

    pub(crate) async fn with_selections(
        &self,
        environments: &[TurnEnvironmentSelection],
    ) -> CodexResult<Self> {
        let mut seen_environment_ids = HashSet::with_capacity(environments.len());
        let mut turn_environments = Vec::with_capacity(environments.len());
        for selected_environment in environments {
            if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
                return Err(CodexErr::InvalidRequest(format!(
                    "duplicate turn environment id `{}`",
                    selected_environment.environment_id
                )));
            }
            let turn_environment = match self.turn_environments.iter().find(|environment| {
                environment.environment_id == selected_environment.environment_id
                    && environment.cwd_uri() == &selected_environment.cwd
            }) {
                Some(environment) => environment.clone(),
                None => self.resolve_selection(selected_environment).await?,
            };
            turn_environments.push(turn_environment);
        }
        Ok(Self {
            environment_manager: Arc::clone(&self.environment_manager),
            turn_environments,
        })
    }

    async fn resolve_selection(
        &self,
        selected_environment: &TurnEnvironmentSelection,
    ) -> CodexResult<TurnEnvironment> {
        let environment_id = selected_environment.environment_id.clone();
        let environment = self
            .environment_manager
            .get_environment(&environment_id)
            .ok_or_else(|| {
                CodexErr::InvalidRequest(format!("unknown turn environment id `{environment_id}`"))
            })?;
        let shell = match environment.info().await {
            Ok(info) => match Shell::from_environment_shell_info(info.shell) {
                Ok(shell) => Some(shell),
                Err(err) => {
                    tracing::warn!(
                        "failed to resolve shell for environment `{environment_id}`: {err}"
                    );
                    None
                }
            },
            Err(err) => {
                tracing::warn!("failed to get info for environment `{environment_id}`: {err}");
                None
            }
        };
        Ok(TurnEnvironment::new(
            environment_id,
            environment,
            selected_environment.cwd.to_abs_path().map_err(|err| {
                CodexErr::InvalidRequest(format!(
                    "turn environment cwd `{}` is not valid on this host: {err}",
                    selected_environment.cwd
                ))
            })?,
            shell,
        ))
    }

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

    pub(crate) fn single_local_environment_cwd(&self) -> Option<&AbsolutePathBuf> {
        let [environment] = self.turn_environments.as_slice() else {
            return None;
        };

        (!environment.environment.is_remote()).then_some(environment.cwd())
    }
}

#[cfg(test)]
mod tests {
    use codex_exec_server::Environment;
    use codex_exec_server::ExecServerRuntimePaths;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_exec_server::REMOTE_ENVIRONMENT_ID;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
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
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let manager = EnvironmentManager::create_for_tests(
            Some("ws://127.0.0.1:8765".to_string()),
            Some(test_runtime_paths()),
        )
        .await;

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri,
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
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let manager =
            EnvironmentManager::from_codex_home(temp_dir.path(), Some(test_runtime_paths()))
                .await
                .expect("environment manager");

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd),
            vec![
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.clone(),
                },
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri,
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
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let manager = Arc::new(EnvironmentManager::default_for_tests());

        let err = TurnEnvironments::resolve(
            manager,
            &[
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: cwd_uri.clone(),
                },
                TurnEnvironmentSelection {
                    environment_id: "local".to_string(),
                    cwd: cwd_uri.join("other").expect("other cwd URI"),
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
        let selected_cwd_uri = PathUri::from_abs_path(&selected_cwd);
        let manager = Arc::new(EnvironmentManager::default_for_tests());

        let resolved = TurnEnvironments::resolve(
            Arc::clone(&manager),
            &[TurnEnvironmentSelection {
                environment_id: "local".to_string(),
                cwd: selected_cwd_uri,
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
            Some(
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
            )
        );
    }

    #[tokio::test]
    async fn matching_environment_id_and_cwd_reuse_resolved_environment() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = Arc::new(
            EnvironmentManager::create_for_tests(
                Some("ws://127.0.0.1:8765".to_string()),
                Some(test_runtime_paths()),
            )
            .await,
        );
        let selection = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
        };
        let initial = TurnEnvironments::resolve(Arc::clone(&manager), &[selection.clone()])
            .await
            .expect("environment selection should resolve");
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                "ws://127.0.0.1:9876".to_string(),
            )
            .expect("replace environment");

        let reused = initial
            .with_selections(std::slice::from_ref(&selection))
            .await
            .expect("matching environment selection should resolve");
        let changed = reused
            .with_selections(&[TurnEnvironmentSelection {
                cwd: PathUri::from_abs_path(&cwd.join("changed")),
                ..selection
            }])
            .await
            .expect("changed environment selection should resolve");

        assert!(Arc::ptr_eq(
            &initial.primary().expect("initial environment").environment,
            &reused.primary().expect("reused environment").environment,
        ));
        assert!(!Arc::ptr_eq(
            &reused.primary().expect("reused environment").environment,
            &changed.primary().expect("changed environment").environment,
        ));
    }

    #[tokio::test]
    async fn single_local_environment_cwd_requires_exactly_one_local_environment() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let local_manager = Arc::new(EnvironmentManager::default_for_tests());
        let local = TurnEnvironments::resolve(
            Arc::clone(&local_manager),
            &[TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri,
            }],
        )
        .await
        .expect("local environment should resolve");
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let remote = TurnEnvironments {
            environment_manager: Arc::clone(&local_manager),
            turn_environments: vec![TurnEnvironment::new(
                REMOTE_ENVIRONMENT_ID.to_string(),
                remote_environment.clone(),
                cwd.clone(),
                /*shell*/ None,
            )],
        };
        let multiple = TurnEnvironments {
            environment_manager: local_manager,
            turn_environments: vec![
                local.primary().expect("local environment").clone(),
                TurnEnvironment::new(
                    REMOTE_ENVIRONMENT_ID.to_string(),
                    remote_environment,
                    cwd.clone(),
                    /*shell*/ None,
                ),
            ],
        };

        assert_eq!(local.single_local_environment_cwd(), Some(&cwd));
        assert_eq!(remote.single_local_environment_cwd(), None);
        assert_eq!(multiple.single_local_environment_cwd(), None);
    }
}
