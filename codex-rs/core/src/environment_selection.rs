use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use arc_swap::ArcSwap;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecutorFileSystem;
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

type TurnEnvironmentResolution =
    Shared<BoxFuture<'static, Result<TurnEnvironment, Arc<ExecServerError>>>>;

#[derive(Clone)]
struct SelectedTurnEnvironment {
    selection: TurnEnvironmentSelection,
    // Starting environments resolve in the background without blocking snapshots.
    delayed_start: bool,
    resolution: TurnEnvironmentResolution,
}

#[derive(Clone)]
pub(crate) struct StartingTurnEnvironment {
    pub(crate) selection: TurnEnvironmentSelection,
    resolution: TurnEnvironmentResolution,
}

impl fmt::Debug for StartingTurnEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StartingTurnEnvironment")
            .field("selection", &self.selection)
            .field("resolved", &self.resolution.peek().is_some())
            .finish_non_exhaustive()
    }
}

pub(crate) struct ThreadEnvironments {
    environment_manager: Arc<EnvironmentManager>,
    local_shell: Shell,
    shell_snapshot: ShellSnapshot,
    environments: ArcSwap<Vec<SelectedTurnEnvironment>>,
}

impl ThreadEnvironments {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_shell: Shell,
        shell_snapshot: ShellSnapshot,
        current: TurnEnvironmentSnapshot,
    ) -> Self {
        // Reuse only attached environments from the supplied snapshot; drop starting entries.
        let environments = current
            .turn_environments
            .into_iter()
            .map(|environment| {
                let selection = environment.selection();
                let resolution: TurnEnvironmentResolution =
                    futures::future::ready(Ok(environment)).boxed().shared();
                SelectedTurnEnvironment {
                    selection,
                    delayed_start: false,
                    resolution,
                }
            })
            .collect();
        Self {
            environment_manager,
            local_shell,
            shell_snapshot,
            environments: ArcSwap::from_pointee(environments),
        }
    }

    pub(crate) fn update_selections(&self, environments: &[TurnEnvironmentSelection]) {
        let previous = self.environments.load();
        let mut seen_environment_ids = HashSet::with_capacity(environments.len());
        let mut next = Vec::with_capacity(environments.len());
        let mut new_resolutions = Vec::new();
        for selected_environment in environments {
            if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
                continue;
            }
            if let Some(environment) = previous
                .iter()
                .find(|environment| environment.selection == *selected_environment)
            {
                next.push(environment.clone());
                continue;
            }

            let environment_id = &selected_environment.environment_id;
            let Some(environment) = self.environment_manager.get_environment(environment_id) else {
                tracing::warn!("skipping unknown turn environment `{environment_id}`");
                continue;
            };
            let delayed_start = environment.is_remote() && !environment.startup_finished();
            let resolution = Self::resolve_environment(
                selected_environment.clone(),
                environment,
                self.local_shell.clone(),
                self.shell_snapshot.clone(),
            );
            new_resolutions.push(resolution.clone());
            next.push(SelectedTurnEnvironment {
                selection: selected_environment.clone(),
                delayed_start,
                resolution,
            });
        }
        self.environments.store(Arc::new(next));
        for resolution in new_resolutions {
            drop(tokio::spawn(resolution));
        }
    }

    fn resolve_environment(
        selection: TurnEnvironmentSelection,
        environment: Arc<Environment>,
        local_shell: Shell,
        shell_snapshot: ShellSnapshot,
    ) -> TurnEnvironmentResolution {
        async move {
            let environment_id = &selection.environment_id;
            if let Err(err) = environment.wait_until_ready().await {
                tracing::warn!("turn environment `{environment_id}` failed to start: {err}");
                return Err(Arc::new(err));
            }
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
                        tracing::warn!(
                            "failed to get info for environment `{environment_id}`: {err}"
                        );
                        None
                    }
                }
            } else {
                Some(local_shell)
            };
            let mut turn_environment =
                TurnEnvironment::new(selection.environment_id, environment, selection.cwd, shell);
            let task = shell_snapshot
                .build(turn_environment.clone())
                .boxed()
                .shared();
            drop(tokio::spawn(task.clone()));
            turn_environment.shell_snapshot = task;
            Ok(turn_environment)
        }
        .boxed()
        .shared()
    }

    pub(crate) async fn snapshot(&self) -> TurnEnvironmentSnapshot {
        let current = self.environments.load_full();
        let mut turn_environments = Vec::with_capacity(current.len());
        let mut starting = Vec::new();
        for environment in current.iter() {
            let resolved = if environment.delayed_start {
                environment.resolution.peek().cloned()
            } else {
                Some(environment.resolution.clone().await)
            };
            match resolved {
                Some(Ok(turn_environment)) => turn_environments.push(turn_environment),
                Some(Err(_)) => {}
                None => starting.push(StartingTurnEnvironment {
                    selection: environment.selection.clone(),
                    resolution: environment.resolution.clone(),
                }),
            }
        }
        TurnEnvironmentSnapshot {
            turn_environments,
            starting,
        }
    }

    pub(crate) fn environment_manager(&self) -> Arc<EnvironmentManager> {
        Arc::clone(&self.environment_manager)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TurnEnvironmentSnapshot {
    pub(crate) turn_environments: Vec<TurnEnvironment>,
    pub(crate) starting: Vec<StartingTurnEnvironment>,
}

impl TurnEnvironmentSnapshot {
    #[cfg(test)]
    pub(crate) fn from_turn_environments(turn_environments: Vec<TurnEnvironment>) -> Self {
        Self {
            turn_environments,
            starting: Vec::new(),
        }
    }

    pub(crate) fn primary(&self) -> Option<&TurnEnvironment> {
        self.turn_environments.first()
    }

    pub(crate) fn local(&self) -> Option<&TurnEnvironment> {
        self.turn_environments
            .iter()
            .find(|environment| !environment.environment.is_remote())
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

    pub(crate) fn single_local_environment_cwd(&self) -> Option<AbsolutePathBuf> {
        // TODO(anp): Migrate local-environment consumers to PathUri so this compatibility
        // conversion can be removed.
        self.single_local_environment()?.cwd().to_abs_path().ok()
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
    use futures::SinkExt;
    use futures::StreamExt;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

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

    async fn read_websocket_json(websocket: &mut WebSocketStream<TcpStream>) -> Value {
        loop {
            match timeout(std::time::Duration::from_secs(5), websocket.next())
                .await
                .expect("websocket read should not time out")
                .expect("websocket should stay open")
                .expect("websocket frame should read")
            {
                Message::Text(text) => {
                    return serde_json::from_str(text.as_ref()).expect("valid JSON-RPC message");
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(bytes.as_ref()).expect("valid JSON-RPC message");
                }
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("expected JSON-RPC message, got {other:?}"),
            }
        }
    }

    async fn serve_environment_info(listener: TcpListener) {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut websocket = accept_async(stream).await.expect("websocket handshake");

        let initialize = read_websocket_json(&mut websocket).await;
        assert_eq!(initialize["method"], "initialize");
        websocket
            .send(Message::Text(
                serde_json::json!({
                    "id": initialize["id"],
                    "result": { "sessionId": "test-session" }
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("initialize response");
        let initialized = read_websocket_json(&mut websocket).await;
        assert_eq!(initialized["method"], "initialized");

        let info = read_websocket_json(&mut websocket).await;
        assert_eq!(info["method"], "environment/info");
        websocket
            .send(Message::Text(
                serde_json::json!({
                    "id": info["id"],
                    "result": { "shell": { "name": "zsh", "path": "/bin/zsh" } }
                })
                .to_string()
                .into(),
            ))
            .await
            .expect("environment info response");
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
    async fn snapshot_keeps_starting_environment_until_it_can_be_attached() {
        let listener = TcpListener::bind("127.0.0.1:0")
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
        let cwd = PathUri::from_abs_path(&cwd);
        let remote = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: cwd.clone(),
        };
        let local = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd,
        };
        let turn_environments = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
        );
        turn_environments.update_selections(&[remote.clone(), local.clone()]);

        let starting = turn_environments.snapshot().await;
        assert_eq!(
            starting
                .turn_environments
                .iter()
                .map(TurnEnvironment::selection)
                .collect::<Vec<_>>(),
            vec![local.clone()]
        );
        assert_eq!(
            starting
                .starting
                .iter()
                .map(|environment| environment.selection.clone())
                .collect::<Vec<_>>(),
            vec![remote.clone()]
        );
        assert_eq!(starting.to_selections(), vec![local.clone()]);

        let server = tokio::spawn(serve_environment_info(listener));
        timeout(
            std::time::Duration::from_secs(5),
            starting.starting[0].resolution.clone(),
        )
        .await
        .expect("environment resolution should finish")
        .expect("environment resolution should succeed");
        let attached = turn_environments.snapshot().await;

        assert!(attached.starting.is_empty());
        assert_eq!(
            attached
                .turn_environments
                .iter()
                .map(TurnEnvironment::selection)
                .collect::<Vec<_>>(),
            vec![remote.clone(), local.clone()]
        );
        assert_eq!(attached.to_selections(), vec![remote, local]);
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn latest_environment_update_replaces_pending_resolution() {
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
    async fn matching_environment_id_and_cwd_reuse_resolution() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let first_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind first listener");
        let manager = Arc::new(
            EnvironmentManager::create_for_tests(
                Some(format!(
                    "ws://{}",
                    first_listener.local_addr().expect("first listener address")
                )),
                Some(test_runtime_paths()),
            )
            .await,
        );
        let selection = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
        };
        let environments = ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
        );
        environments.update_selections(std::slice::from_ref(&selection));
        let initial_snapshot = environments.snapshot().await;
        let second_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind second listener");
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                format!(
                    "ws://{}",
                    second_listener
                        .local_addr()
                        .expect("second listener address")
                ),
            )
            .expect("replace environment");

        environments.update_selections(std::slice::from_ref(&selection));
        let reused_snapshot = environments.snapshot().await;
        environments.update_selections(&[TurnEnvironmentSelection {
            cwd: PathUri::from_abs_path(&cwd.join("changed")),
            ..selection
        }]);
        let changed_snapshot = environments.snapshot().await;

        let initial = initial_snapshot
            .starting
            .first()
            .expect("initial environment");
        let reused = reused_snapshot
            .starting
            .first()
            .expect("reused environment");
        let changed = changed_snapshot
            .starting
            .first()
            .expect("changed environment");
        assert!(initial.resolution.ptr_eq(&reused.resolution));
        assert!(!reused.resolution.ptr_eq(&changed.resolution));
    }

    #[tokio::test]
    async fn inherited_environment_reuses_parent_handle() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selection = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
        };
        let inherited_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("inherited environment"),
        );
        let inherited = TurnEnvironment::new(
            selection.environment_id.clone(),
            Arc::clone(&inherited_environment),
            selection.cwd.clone(),
            /*shell*/ None,
        );
        let manager = Arc::new(EnvironmentManager::without_environments());
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                "ws://127.0.0.1:9876".to_string(),
            )
            .expect("replacement environment");
        let environments = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::from_turn_environments(vec![inherited]),
        );

        environments.update_selections(std::slice::from_ref(&selection));
        let snapshot = environments.snapshot().await;

        assert!(Arc::ptr_eq(
            &snapshot
                .primary()
                .expect("inherited environment")
                .environment,
            &inherited_environment,
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
                cwd: cwd_uri.clone(),
            }],
        )
        .await;
        let local = local.snapshot().await;
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let remote = TurnEnvironmentSnapshot::from_turn_environments(vec![TurnEnvironment::new(
            REMOTE_ENVIRONMENT_ID.to_string(),
            remote_environment.clone(),
            cwd_uri.clone(),
            /*shell*/ None,
        )]);
        let multiple = TurnEnvironmentSnapshot::from_turn_environments(vec![
            local.primary().expect("local environment").clone(),
            TurnEnvironment::new(
                REMOTE_ENVIRONMENT_ID.to_string(),
                remote_environment,
                cwd_uri,
                /*shell*/ None,
            ),
        ]);

        assert_eq!(local.single_local_environment_cwd(), Some(cwd));
        assert_eq!(remote.single_local_environment_cwd(), None);
        assert_eq!(multiple.single_local_environment_cwd(), None);
    }
}
