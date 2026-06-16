use std::collections::HashSet;
use std::sync::Arc;

use arc_swap::ArcSwap;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;

use crate::session::turn_context::TurnEnvironment;
use crate::shell::Shell;
use crate::shell_snapshot::ShellSnapshot;

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

type SnapshotTask = Shared<BoxFuture<'static, TurnEnvironmentSnapshot>>;

pub(crate) struct ThreadEnvironments {
    environment_manager: Arc<EnvironmentManager>,
    local_shell: Shell,
    shell_snapshot: ShellSnapshot,
    snapshot_task: ArcSwap<SnapshotTask>,
}

impl ThreadEnvironments {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_shell: Shell,
        shell_snapshot: ShellSnapshot,
        current: TurnEnvironmentSnapshot,
    ) -> Self {
        Self {
            environment_manager,
            local_shell,
            shell_snapshot,
            snapshot_task: ArcSwap::from_pointee(futures::future::ready(current).boxed().shared()),
        }
    }

    pub(crate) fn update_selections(&self, environments: &[TurnEnvironmentSelection]) {
        let previous = self
            .snapshot_task
            .load()
            .peek()
            .cloned()
            .unwrap_or_default();
        let environment_manager = Arc::clone(&self.environment_manager);
        let local_shell = self.local_shell.clone();
        let shell_snapshot = self.shell_snapshot.clone();
        let environments = environments.to_vec();
        let (snapshot_task, snapshot) = async move {
            Self::resolve_snapshot(
                environment_manager,
                local_shell,
                shell_snapshot,
                previous,
                environments,
            )
            .await
        }
        .remote_handle();
        self.snapshot_task
            .store(Arc::new(snapshot.boxed().shared()));
        drop(tokio::spawn(snapshot_task));
    }

    async fn resolve_snapshot(
        environment_manager: Arc<EnvironmentManager>,
        local_shell: Shell,
        shell_snapshot: ShellSnapshot,
        current: TurnEnvironmentSnapshot,
        environments: Vec<TurnEnvironmentSelection>,
    ) -> TurnEnvironmentSnapshot {
        let mut seen_environment_ids = HashSet::with_capacity(environments.len());
        let mut turn_environments = Vec::with_capacity(environments.len());
        for selected_environment in &environments {
            if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
                continue;
            }
            let turn_environment = match current.turn_environments.iter().find(|environment| {
                environment.environment_id == selected_environment.environment_id
                    && environment.cwd_uri() == &selected_environment.cwd
                    && (environment.environment.is_remote()
                        || environment.shell.as_ref() == Some(&local_shell))
            }) {
                Some(environment) => environment.clone(),
                None => match Self::resolve_selection(
                    &environment_manager,
                    &local_shell,
                    &shell_snapshot,
                    selected_environment,
                )
                .await
                {
                    Ok(environment) => environment,
                    Err(err) => {
                        tracing::warn!(
                            "skipping unresolved turn environment `{}`: {err}",
                            selected_environment.environment_id
                        );
                        continue;
                    }
                },
            };
            turn_environments.push(turn_environment);
        }
        TurnEnvironmentSnapshot { turn_environments }
    }

    async fn resolve_selection(
        environment_manager: &EnvironmentManager,
        local_shell: &Shell,
        shell_snapshot: &ShellSnapshot,
        selected_environment: &TurnEnvironmentSelection,
    ) -> CodexResult<TurnEnvironment> {
        let environment_id = selected_environment.environment_id.clone();
        let environment = environment_manager
            .get_environment(&environment_id)
            .ok_or_else(|| {
                CodexErr::InvalidRequest(format!("unknown turn environment id `{environment_id}`"))
            })?;
        let shell = if environment.is_remote() {
            match environment.info().await {
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
            }
        } else {
            Some(local_shell.clone())
        };
        let mut turn_environment = TurnEnvironment::new(
            environment_id,
            environment,
            selected_environment.cwd.to_abs_path().map_err(|err| {
                CodexErr::InvalidRequest(format!(
                    "turn environment cwd `{}` is not valid on this host: {err}",
                    selected_environment.cwd
                ))
            })?,
            shell,
        );
        let task = shell_snapshot
            .clone()
            .build(turn_environment.clone())
            .boxed()
            .shared();
        drop(tokio::spawn(task.clone()));
        turn_environment.shell_snapshot = task;
        Ok(turn_environment)
    }

    pub(crate) async fn snapshot(&self) -> TurnEnvironmentSnapshot {
        self.snapshot_task.load_full().as_ref().clone().await
    }

    pub(crate) fn environment_manager(&self) -> Arc<EnvironmentManager> {
        Arc::clone(&self.environment_manager)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TurnEnvironmentSnapshot {
    pub(crate) turn_environments: Vec<TurnEnvironment>,
}

impl TurnEnvironmentSnapshot {
    pub(crate) fn primary(&self) -> Option<&TurnEnvironment> {
        self.turn_environments.first()
    }

    #[cfg(test)]
    pub(crate) fn primary_environment(&self) -> Option<Arc<codex_exec_server::Environment>> {
        self.primary()
            .map(|environment| Arc::clone(&environment.environment))
    }

    pub(crate) fn to_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.turn_environments
            .iter()
            .map(TurnEnvironment::selection)
            .collect()
    }

    pub(crate) fn primary_filesystem(&self) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.primary()
            .map(|environment| environment.environment.get_filesystem())
    }

    pub(crate) fn single_local_environment(&self) -> Option<&TurnEnvironment> {
        let [environment] = self.turn_environments.as_slice() else {
            return None;
        };

        (!environment.environment.is_remote()).then_some(environment)
    }

    pub(crate) fn single_local_environment_cwd(&self) -> Option<&AbsolutePathBuf> {
        self.single_local_environment().map(TurnEnvironment::cwd)
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

    async fn resolve_turn_environments(
        environment_manager: Arc<EnvironmentManager>,
        selections: &[TurnEnvironmentSelection],
    ) -> Arc<ThreadEnvironments> {
        let turn_environments = Arc::new(ThreadEnvironments::new(
            environment_manager,
            crate::shell::default_user_shell(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
        ));
        turn_environments.update_selections(selections);
        turn_environments.snapshot().await;
        turn_environments
    }

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
    async fn local_environment_uses_configured_shell() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let local_shell = Shell {
            shell_type: crate::shell::ShellType::Zsh,
            shell_path: std::path::PathBuf::from("/configured/zsh"),
        };
        let turn_environments = ThreadEnvironments::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            local_shell.clone(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
        );
        turn_environments.update_selections(&[TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
        }]);

        let snapshot = turn_environments.snapshot().await;

        assert_eq!(
            snapshot
                .primary()
                .and_then(|environment| environment.shell.as_ref()),
            Some(&local_shell)
        );
    }

    #[tokio::test]
    async fn resolve_environment_selections_keeps_first_duplicate_id() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let manager = Arc::new(EnvironmentManager::default_for_tests());
        let first = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: cwd_uri.clone(),
        };

        let resolved = resolve_turn_environments(
            manager,
            &[
                first.clone(),
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.join("other").expect("other cwd URI"),
                },
            ],
        )
        .await;

        assert_eq!(resolved.snapshot().await.to_selections(), vec![first]);
    }

    #[tokio::test]
    async fn resolved_environment_selections_use_first_selection_as_primary() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selected_cwd = cwd.join("selected");
        let selected_cwd_uri = PathUri::from_abs_path(&selected_cwd);
        let manager = Arc::new(EnvironmentManager::default_for_tests());

        let resolved = resolve_turn_environments(
            Arc::clone(&manager),
            &[TurnEnvironmentSelection {
                environment_id: "local".to_string(),
                cwd: selected_cwd_uri,
            }],
        )
        .await;

        let resolved = resolved.snapshot().await;
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
    async fn unresolved_environment_selections_are_skipped() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let manager = Arc::new(EnvironmentManager::default_for_tests());
        let local = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: cwd_uri.clone(),
        };

        let resolved = resolve_turn_environments(
            manager,
            &[
                TurnEnvironmentSelection {
                    environment_id: "missing".to_string(),
                    cwd: cwd_uri,
                },
                local.clone(),
            ],
        )
        .await;

        assert_eq!(resolved.snapshot().await.to_selections(), vec![local]);
    }

    #[tokio::test]
    async fn latest_environment_update_wins_while_previous_resolution_is_pending() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind websocket listener");
        let manager = Arc::new(
            EnvironmentManager::create_for_tests_with_local(
                Some(format!(
                    "ws://{}",
                    listener.local_addr().expect("listener address")
                )),
                test_runtime_paths(),
            )
            .await,
        );
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let turn_environments = Arc::new(ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
        ));
        turn_environments.update_selections(&[TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
        }]);
        let (_connection, _) =
            tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                .await
                .expect("remote resolution should connect")
                .expect("accept remote resolution connection");
        let local = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
        };

        turn_environments.update_selections(std::slice::from_ref(&local));
        let snapshot = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            turn_environments.snapshot(),
        )
        .await
        .expect("latest environment resolution should complete");

        assert_eq!(snapshot.to_selections(), vec![local]);
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
        let initial =
            resolve_turn_environments(Arc::clone(&manager), std::slice::from_ref(&selection)).await;
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                "ws://127.0.0.1:9876".to_string(),
            )
            .expect("replace environment");

        let initial_snapshot = initial.snapshot().await;
        initial.update_selections(std::slice::from_ref(&selection));
        let reused_snapshot = initial.snapshot().await;
        initial.update_selections(&[TurnEnvironmentSelection {
            cwd: PathUri::from_abs_path(&cwd.join("changed")),
            ..selection
        }]);
        let changed_snapshot = initial.snapshot().await;

        assert!(Arc::ptr_eq(
            &initial_snapshot
                .primary()
                .expect("initial environment")
                .environment,
            &reused_snapshot
                .primary()
                .expect("reused environment")
                .environment,
        ));
        assert!(!Arc::ptr_eq(
            &reused_snapshot
                .primary()
                .expect("reused environment")
                .environment,
            &changed_snapshot
                .primary()
                .expect("changed environment")
                .environment,
        ));
    }

    #[tokio::test]
    async fn single_local_environment_cwd_requires_exactly_one_local_environment() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let local_manager = Arc::new(EnvironmentManager::default_for_tests());
        let local = resolve_turn_environments(
            Arc::clone(&local_manager),
            &[TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri,
            }],
        )
        .await;
        let local = local.snapshot().await;
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let remote = TurnEnvironmentSnapshot {
            turn_environments: vec![TurnEnvironment::new(
                REMOTE_ENVIRONMENT_ID.to_string(),
                remote_environment.clone(),
                cwd.clone(),
                /*shell*/ None,
            )],
        };
        let multiple = TurnEnvironmentSnapshot {
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
