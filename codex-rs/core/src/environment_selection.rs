use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::sync::OnceLock;

use arc_swap::ArcSwap;
use async_channel::Sender;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentConnectionState;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConnectionEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use tokio_util::task::AbortOnDropHandle;

use crate::session::turn_context::ShellSnapshotTask;
use crate::session::turn_context::TurnEnvironment;
use crate::session::turn_context::TurnEnvironmentConfig;
use crate::shell::Shell;
use crate::shell_snapshot::ShellSnapshot;

pub(crate) fn default_thread_environment_selections(
    environment_manager: &EnvironmentManager,
    cwd: &AbsolutePathBuf,
    workspace_roots: &[AbsolutePathBuf],
) -> Vec<TurnEnvironmentSelection> {
    environment_manager
        .default_environment_ids()
        .into_iter()
        .map(|environment_id| TurnEnvironmentSelection {
            environment_id,
            cwd: PathUri::from_abs_path(cwd),
            workspace_roots: workspace_roots.iter().map(PathUri::from_abs_path).collect(),
        })
        .collect()
}

type TurnEnvironmentResult = Result<ResolvedEnvironment, Arc<ExecServerError>>;
type TurnEnvironmentResolution = Shared<BoxFuture<'static, TurnEnvironmentResult>>;

// Shared startup result used to build each turn's environment with its own config
// without restarting the connection or shell resolution.
#[derive(Clone)]
struct ResolvedEnvironment {
    environment: Arc<Environment>,
    shell: Option<Shell>,
    shell_snapshot: ShellSnapshotTask,
}

#[derive(Clone)]
struct SelectedTurnEnvironment {
    selection: TurnEnvironmentSelection,
    config: TurnEnvironmentConfig,
    environment: Arc<Environment>,
    // Selection clones share one listener; the final handle drop aborts it.
    connection_events_task: Option<Arc<AbortOnDropHandle<()>>>,
    resolution: TurnEnvironmentResolution,
}

#[derive(Clone)]
pub(crate) struct StartingTurnEnvironment {
    pub(crate) selection: TurnEnvironmentSelection,
    config: TurnEnvironmentConfig,
    resolution: TurnEnvironmentResolution,
}

impl SelectedTurnEnvironment {
    fn has_installed_environment_config(&self) -> bool {
        self.config.selected_capability_roots.is_some()
    }

    // Thread settings still own permissions, but cannot overwrite shell policy
    // that was installed specifically for this environment attachment.
    fn update_thread_config(&mut self, config: &TurnEnvironmentConfig) {
        if !self.has_installed_environment_config() {
            self.config.allow_login_shell = config.allow_login_shell;
        }
        self.config.permission_profile = config.permission_profile.clone();
    }
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

impl StartingTurnEnvironment {
    pub(crate) async fn wait_until_ready(&self) -> Result<(), Arc<ExecServerError>> {
        self.resolution.clone().await.map(|_| ())
    }
}

pub(crate) struct ThreadEnvironments {
    environment_manager: Arc<EnvironmentManager>,
    local_shell: Shell,
    shell_snapshot: ShellSnapshot,
    non_blocking_snapshots: bool,
    environments: ArcSwap<Vec<SelectedTurnEnvironment>>,
    connection_event_tx: OnceLock<Sender<Event>>,
}

impl ThreadEnvironments {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_shell: Shell,
        thread_environment_config: TurnEnvironmentConfig,
        shell_snapshot: ShellSnapshot,
        current: TurnEnvironmentSnapshot,
        non_blocking_snapshots: bool,
    ) -> Self {
        // Reuse only attached environments from the supplied snapshot; drop starting entries.
        let environments = current
            .environments
            .into_iter()
            .filter_map(|environment| {
                let TurnEnvironmentState::Ready(environment) = environment else {
                    return None;
                };
                let selection = environment.selection();
                let selected_environment = Arc::clone(&environment.environment);
                let inherited_config = environment.config;
                let resolution: TurnEnvironmentResolution =
                    futures::future::ready(Ok(ResolvedEnvironment {
                        environment: environment.environment,
                        shell: environment.shell,
                        shell_snapshot: environment.shell_snapshot,
                    }))
                    .boxed()
                    .shared();
                let mut inherited_environment = SelectedTurnEnvironment {
                    selection,
                    config: inherited_config,
                    environment: selected_environment,
                    connection_events_task: None,
                    resolution,
                };
                // Child threads get their own settings, but inherit any
                // environment-owned policy that was already installed.
                inherited_environment.update_thread_config(&thread_environment_config);
                Some(inherited_environment)
            })
            .collect();
        Self {
            environment_manager,
            local_shell,
            shell_snapshot,
            non_blocking_snapshots,
            environments: ArcSwap::from_pointee(environments),
            connection_event_tx: OnceLock::new(),
        }
    }

    pub(crate) fn update_selections(
        &self,
        environments: &[TurnEnvironmentSelection],
        thread_environment_config: &TurnEnvironmentConfig,
    ) {
        let previous = self.environments.load();
        let mut seen_environment_ids = HashSet::with_capacity(environments.len());
        let mut next = Vec::with_capacity(environments.len());
        for selected_environment in environments {
            if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
                continue;
            }
            if let Some(environment) = previous
                .iter()
                .find(|environment| environment.selection == *selected_environment)
                && !matches!(environment.resolution.clone().now_or_never(), Some(Err(_)))
            {
                let mut environment = environment.clone();
                environment.update_thread_config(thread_environment_config);
                next.push(environment);
                continue;
            }

            let environment_id = &selected_environment.environment_id;
            let Some(environment) = self.environment_manager.get_environment(environment_id) else {
                tracing::warn!("skipping unknown turn environment `{environment_id}`");
                continue;
            };
            // Connection state belongs to the environment instance, not its cwd or roots.
            let connection_events_task = previous
                .iter()
                .find(|previous| {
                    previous.selection.environment_id.as_str() == environment_id.as_str()
                        && Arc::ptr_eq(&previous.environment, &environment)
                })
                .and_then(|previous| previous.connection_events_task.clone())
                .or_else(|| {
                    self.connection_event_tx.get().and_then(|tx_event| {
                        Self::spawn_connection_event_listener(
                            environment.as_ref(),
                            environment_id.clone(),
                            tx_event.clone(),
                        )
                    })
                });
            let (resolution_task, resolution) = Self::resolve_environment(
                selected_environment.clone(),
                Arc::clone(&environment),
                self.local_shell.clone(),
                self.shell_snapshot.clone(),
            )
            .remote_handle();
            drop(tokio::spawn(resolution_task));
            let resolution = resolution.boxed().shared();
            next.push(SelectedTurnEnvironment {
                selection: selected_environment.clone(),
                config: thread_environment_config.clone(),
                environment,
                connection_events_task,
                resolution,
            });
        }
        let removed_connection_tasks = previous
            .iter()
            .filter_map(|previous| {
                let task = previous.connection_events_task.as_ref()?;
                (!next.iter().any(|next| {
                    next.connection_events_task
                        .as_ref()
                        .is_some_and(|next_task| Arc::ptr_eq(task, next_task))
                }))
                .then(|| Arc::clone(task))
            })
            .collect::<Vec<_>>();
        self.environments.store(Arc::new(next));
        // ArcSwap readers may retain removed selections, so abort at logical removal.
        for task in removed_connection_tasks {
            task.abort();
        }
    }

    pub(crate) fn selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.environments
            .load()
            .iter()
            .map(|environment| environment.selection.clone())
            .collect()
    }

    pub(crate) fn primary_workspace_roots(&self) -> Vec<AbsolutePathBuf> {
        self.environments
            .load()
            .first()
            .map_or_else(Vec::new, |environment| {
                Self::primary_workspace_roots_for(std::slice::from_ref(&environment.selection))
            })
    }

    pub(crate) fn primary_workspace_roots_for(
        selections: &[TurnEnvironmentSelection],
    ) -> Vec<AbsolutePathBuf> {
        selections
            .first()
            .map(|selection| {
                selection
                    .workspace_roots
                    .iter()
                    .filter_map(|workspace_root| workspace_root.to_abs_path().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn update_environment_configs(&self, config: &TurnEnvironmentConfig) {
        let environments = self
            .environments
            .load()
            .iter()
            .map(|environment| {
                let mut environment = environment.clone();
                environment.update_thread_config(config);
                environment
            })
            .collect();
        self.environments.store(Arc::new(environments));
    }

    /// Installs owner-provided config and roots on their exact thread attachment.
    /// Additional environment-owned settings should be applied to its config here.
    pub(crate) fn environment_ready(
        &self,
        selection: &TurnEnvironmentSelection,
        config: EnvironmentConfig,
    ) -> CodexResult<()> {
        let mut environments = Vec::clone(&self.environments.load());
        let Some(environment) = environments.iter_mut().find(|environment| {
            environment.selection.environment_id == selection.environment_id
                && environment.selection.cwd == selection.cwd
        }) else {
            return Err(CodexErr::InvalidRequest(format!(
                "environment `{}` is not selected on this thread with the requested cwd",
                selection.environment_id
            )));
        };

        environment.config.allow_login_shell = config.allow_login_shell;
        environment.config.selected_capability_roots = Some(config.selected_capability_roots);
        self.environments.store(Arc::new(environments));
        Ok(())
    }

    /// Combines persisted thread roots with installed attachment roots, keeping
    /// thread roots first and hiding attachments that are not ready yet.
    pub(crate) fn inspect_selected_capability_roots(
        &self,
        thread_selected_capability_roots: &[SelectedCapabilityRoot],
    ) -> SelectedCapabilityRootsStatus {
        let environments = self.environments.load();
        let mut selected_capability_roots = thread_selected_capability_roots.to_vec();
        for environment in environments.iter() {
            if let Some(roots) = &environment.config.selected_capability_roots {
                selected_capability_roots.extend(roots.iter().cloned());
            }
        }
        let mut seen_root_ids = HashSet::with_capacity(selected_capability_roots.len());
        selected_capability_roots.retain(|root| seen_root_ids.insert(root.id.clone()));

        let mut status = self
            .environment_manager
            .inspect_selected_capability_roots(&selected_capability_roots);
        status.ready_roots.retain(|root| {
            let CapabilityRootLocation::Environment { environment_id, .. } = &root.location;
            environments
                .iter()
                .find(|environment| &environment.selection.environment_id == environment_id)
                .is_none_or(|environment| {
                    matches!(environment.resolution.clone().now_or_never(), Some(Ok(_)))
                })
        });
        status
    }

    fn spawn_connection_event_listener(
        environment: &Environment,
        environment_id: String,
        tx_event: Sender<Event>,
    ) -> Option<Arc<AbortOnDropHandle<()>>> {
        let mut connection_state = environment.subscribe_connection_state()?;
        let task = tokio::spawn(async move {
            loop {
                let state = tokio::select! {
                    _ = tx_event.closed() => return,
                    changed = connection_state.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        *connection_state.borrow_and_update()
                    }
                };
                let msg = match state {
                    EnvironmentConnectionState::Connected => {
                        EventMsg::EnvironmentConnected(EnvironmentConnectionEvent {
                            environment_id: environment_id.clone(),
                        })
                    }
                    EnvironmentConnectionState::Disconnected => {
                        EventMsg::EnvironmentDisconnected(EnvironmentConnectionEvent {
                            environment_id: environment_id.clone(),
                        })
                    }
                };
                if tx_event
                    .send(Event {
                        id: String::new(),
                        msg,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }
        });
        Some(Arc::new(AbortOnDropHandle::new(task)))
    }

    pub(crate) fn start_connection_event_forwarding(&self, tx_event: Sender<Event>) {
        let tx_event = self.connection_event_tx.get_or_init(|| tx_event);
        let current = self.environments.load_full();
        let environments = current
            .iter()
            .map(|selected| {
                let mut selected = selected.clone();
                if selected.connection_events_task.is_none() {
                    selected.connection_events_task = Self::spawn_connection_event_listener(
                        selected.environment.as_ref(),
                        selected.selection.environment_id.clone(),
                        tx_event.clone(),
                    );
                }
                selected
            })
            .collect();
        self.environments.store(Arc::new(environments));
    }

    fn resolve_environment(
        selection: TurnEnvironmentSelection,
        environment: Arc<Environment>,
        local_shell: Shell,
        shell_snapshot: ShellSnapshot,
    ) -> BoxFuture<'static, TurnEnvironmentResult> {
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
            let task = shell_snapshot
                .build(Arc::clone(&environment), selection.cwd, shell.clone())
                .boxed()
                .shared();
            drop(tokio::spawn(task.clone()));
            Ok(ResolvedEnvironment {
                environment,
                shell,
                shell_snapshot: task,
            })
        }
        .boxed()
    }

    #[tracing::instrument(name = "environments.snapshot", skip_all)]
    pub(crate) async fn snapshot(&self) -> TurnEnvironmentSnapshot {
        let selected = self.environments.load_full();
        let mut environments = Vec::with_capacity(selected.len());
        for environment in selected.iter() {
            let resolved = if self.non_blocking_snapshots {
                environment.resolution.clone().now_or_never()
            } else {
                Some(environment.resolution.clone().await)
            };
            if let Some(environment) = TurnEnvironmentState::from_resolution(
                StartingTurnEnvironment {
                    selection: environment.selection.clone(),
                    config: environment.config.clone(),
                    resolution: environment.resolution.clone(),
                },
                resolved,
            ) {
                environments.push(environment);
            }
        }
        TurnEnvironmentSnapshot { environments }
    }

    pub(crate) fn environment_manager(&self) -> Arc<EnvironmentManager> {
        Arc::clone(&self.environment_manager)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TurnEnvironmentState {
    Ready(TurnEnvironment),
    Starting(StartingTurnEnvironment),
}

impl TurnEnvironmentState {
    fn from_resolution(
        starting: StartingTurnEnvironment,
        resolved: Option<TurnEnvironmentResult>,
    ) -> Option<Self> {
        match resolved {
            Some(Ok(environment)) => {
                let mut turn_environment = TurnEnvironment::new(
                    starting.selection,
                    environment.environment,
                    environment.shell,
                    starting.config,
                );
                turn_environment.shell_snapshot = environment.shell_snapshot;
                Some(Self::Ready(turn_environment))
            }
            Some(Err(err)) => {
                tracing::debug!(
                    environment_id = %starting.selection.environment_id,
                    "skipping failed turn environment: {err}"
                );
                None
            }
            None => Some(Self::Starting(starting)),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TurnEnvironmentSnapshot {
    // Keep ready and starting environments in their original selection order.
    pub(crate) environments: Vec<TurnEnvironmentState>,
}

impl TurnEnvironmentSnapshot {
    /// Promotes completed startup work without adopting newer thread selections.
    pub(crate) fn refresh_readiness(&self) -> Self {
        let environments = self
            .environments
            .iter()
            .filter_map(|environment| match environment {
                TurnEnvironmentState::Ready(environment) => {
                    Some(TurnEnvironmentState::Ready(environment.clone()))
                }
                TurnEnvironmentState::Starting(environment) => {
                    TurnEnvironmentState::from_resolution(
                        environment.clone(),
                        environment.resolution.clone().now_or_never(),
                    )
                }
            })
            .collect();
        Self { environments }
    }

    pub(crate) fn turn_environments(&self) -> impl Iterator<Item = &TurnEnvironment> {
        self.environments.iter().filter_map(|environment| {
            let TurnEnvironmentState::Ready(environment) = environment else {
                return None;
            };
            Some(environment)
        })
    }

    pub(crate) fn starting(&self) -> impl Iterator<Item = &StartingTurnEnvironment> {
        self.environments.iter().filter_map(|environment| {
            let TurnEnvironmentState::Starting(environment) = environment else {
                return None;
            };
            Some(environment)
        })
    }

    /// Maps each captured environment to its exact ready handle, or `None` when it was starting.
    pub(crate) fn captured_environments(&self) -> HashMap<String, Option<Arc<Environment>>> {
        self.turn_environments()
            .map(|environment| {
                (
                    environment.selection.environment_id.clone(),
                    Some(Arc::clone(&environment.environment)),
                )
            })
            .chain(
                self.starting()
                    .map(|environment| (environment.selection.environment_id.clone(), None)),
            )
            .collect()
    }

    pub(crate) fn primary(&self) -> Option<&TurnEnvironment> {
        self.turn_environments().next()
    }

    pub(crate) fn local(&self) -> Option<&TurnEnvironment> {
        self.turn_environments()
            .find(|environment| !environment.environment.is_remote())
    }

    pub(crate) fn local_environment_cwd(&self) -> Option<AbsolutePathBuf> {
        self.environments
            .iter()
            .find_map(|environment| match environment {
                TurnEnvironmentState::Ready(environment)
                    if !environment.environment.is_remote() =>
                {
                    environment.cwd().to_abs_path().ok()
                }
                TurnEnvironmentState::Ready(_) => None,
                TurnEnvironmentState::Starting(environment)
                    if environment.selection.environment_id
                        == codex_exec_server::LOCAL_ENVIRONMENT_ID =>
                {
                    environment.selection.cwd.to_abs_path().ok()
                }
                TurnEnvironmentState::Starting(_) => None,
            })
    }

    #[cfg(test)]
    pub(crate) fn primary_environment(&self) -> Option<Arc<codex_exec_server::Environment>> {
        self.primary()
            .map(|environment| Arc::clone(&environment.environment))
    }

    pub(crate) fn to_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.turn_environments()
            .map(TurnEnvironment::selection)
            .collect()
    }

    pub(crate) fn primary_filesystem(&self) -> Option<Arc<dyn ExecutorFileSystem>> {
        self.primary()
            .map(|environment| environment.environment.get_filesystem())
    }

    pub(crate) fn single_local_environment(&self) -> Option<&TurnEnvironment> {
        if self.starting().next().is_some() {
            return None;
        }
        let mut environments = self.turn_environments();
        let environment = environments.next()?;
        if environments.next().is_some() {
            return None;
        }

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
    use std::time::Duration;

    use crate::config::PermissionProfileSnapshot;
    use codex_exec_server::Environment;
    use codex_exec_server::ExecServerRuntimePaths;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_exec_server::REMOTE_ENVIRONMENT_ID;
    use codex_exec_server_test_support::environment_manager_without_environments;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use codex_protocol::models::ActivePermissionProfile;
    use codex_protocol::models::PermissionProfile;
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

    fn test_environment_config() -> TurnEnvironmentConfig {
        TurnEnvironmentConfig {
            allow_login_shell: true,
            permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::read_only()),
            selected_capability_roots: None,
        }
    }

    async fn resolve_turn_environments(
        environment_manager: Arc<EnvironmentManager>,
        selections: &[TurnEnvironmentSelection],
    ) -> Arc<ThreadEnvironments> {
        let turn_environments = Arc::new(ThreadEnvironments::new(
            environment_manager,
            crate::shell::default_user_shell(),
            test_environment_config(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
        ));
        turn_environments.update_selections(selections, &test_environment_config());
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
            default_thread_environment_selections(&manager, &cwd, std::slice::from_ref(&cwd)),
            vec![TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri.clone(),
                workspace_roots: vec![cwd_uri],
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
        let manager = EnvironmentManager::from_codex_home(
            temp_dir.path(),
            Some(test_runtime_paths()),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
        .expect("environment manager");

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd, std::slice::from_ref(&cwd)),
            vec![
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.clone(),
                    workspace_roots: vec![cwd_uri.clone()],
                },
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.clone(),
                    workspace_roots: vec![cwd_uri],
                },
            ]
        );
    }

    #[tokio::test]
    async fn default_thread_environment_selections_empty_when_default_disabled() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let manager = environment_manager_without_environments();

        assert_eq!(
            default_thread_environment_selections(&manager, &cwd, std::slice::from_ref(&cwd)),
            Vec::<TurnEnvironmentSelection>::new()
        );
    }

    #[tokio::test]
    async fn local_environment_uses_configured_shell_and_login_policy() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let local_shell = Shell {
            shell_type: crate::shell::ShellType::Zsh,
            shell_path: std::path::PathBuf::from("/configured/zsh"),
        };
        let expected_config = TurnEnvironmentConfig {
            allow_login_shell: false,
            permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::read_only(),
                vec![cwd.join("profile-root")],
            ),
            selected_capability_roots: None,
        };
        let turn_environments = ThreadEnvironments::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            local_shell.clone(),
            expected_config.clone(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
        );
        turn_environments.update_selections(
            &[TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&cwd),
                workspace_roots: Vec::new(),
            }],
            &expected_config,
        );

        let snapshot = turn_environments.snapshot().await;
        let environment = snapshot.primary().expect("local environment");

        assert_eq!(environment.shell.as_ref(), Some(&local_shell));
        assert_eq!(environment.config, expected_config);
    }

    #[tokio::test]
    async fn resolve_environment_selections_keeps_first_duplicate_id() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let manager = Arc::new(EnvironmentManager::default_for_tests());
        let first = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: cwd_uri.clone(),
            workspace_roots: Vec::new(),
        };

        let resolved = resolve_turn_environments(
            manager,
            &[
                first.clone(),
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.join("other").expect("other cwd URI"),
                    workspace_roots: Vec::new(),
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
                workspace_roots: Vec::new(),
            }],
        )
        .await;

        let resolved = resolved.snapshot().await;
        assert_eq!(
            resolved
                .primary()
                .expect("primary environment")
                .selection
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
            workspace_roots: Vec::new(),
        };

        let resolved = resolve_turn_environments(
            manager,
            &[
                TurnEnvironmentSelection {
                    environment_id: "missing".to_string(),
                    cwd: cwd_uri,
                    workspace_roots: Vec::new(),
                },
                local.clone(),
            ],
        )
        .await;

        assert_eq!(resolved.snapshot().await.to_selections(), vec![local]);
    }

    #[tokio::test]
    async fn blocking_snapshot_waits_for_starting_environment() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind websocket listener");
        let manager = Arc::new(
            EnvironmentManager::create_for_tests(
                Some(format!(
                    "ws://{}",
                    listener.local_addr().expect("listener address")
                )),
                Some(test_runtime_paths()),
            )
            .await,
        );
        let selection = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd")),
            workspace_roots: Vec::new(),
        };
        let environments = Arc::new(ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            test_environment_config(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
        ));
        environments
            .update_selections(std::slice::from_ref(&selection), &test_environment_config());
        let snapshot_task = tokio::spawn({
            let environments = Arc::clone(&environments);
            async move { environments.snapshot().await }
        });
        tokio::task::yield_now().await;
        assert!(!snapshot_task.is_finished());

        let server = tokio::spawn(serve_environment_info(listener));
        let snapshot = timeout(Duration::from_secs(5), snapshot_task)
            .await
            .expect("snapshot should finish after the environment starts")
            .expect("snapshot task");

        assert!(snapshot.starting().next().is_none());
        assert_eq!(snapshot.to_selections(), vec![selection]);
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn snapshot_refreshes_readiness_in_selection_order() {
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
        let expected_config = TurnEnvironmentConfig {
            allow_login_shell: false,
            permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::read_only(),
                vec![cwd.join("profile-root")],
            ),
            selected_capability_roots: None,
        };
        let cwd = PathUri::from_abs_path(&cwd);
        let remote = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: cwd.clone(),
            workspace_roots: Vec::new(),
        };
        let local = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd,
            workspace_roots: Vec::new(),
        };
        let turn_environments = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            test_environment_config(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ true,
        );
        turn_environments
            .update_selections(std::slice::from_ref(&local), &test_environment_config());
        turn_environments.environments.load()[0]
            .resolution
            .clone()
            .await
            .expect("local environment should resolve");
        turn_environments.update_selections(&[remote.clone(), local.clone()], &expected_config);

        let starting = turn_environments.snapshot().await;
        assert_eq!(
            starting
                .turn_environments()
                .map(TurnEnvironment::selection)
                .collect::<Vec<_>>(),
            vec![local.clone()]
        );
        assert_eq!(
            starting
                .turn_environments()
                .map(|environment| environment.config.clone())
                .collect::<Vec<_>>(),
            vec![expected_config.clone()]
        );
        assert_eq!(
            starting
                .starting()
                .map(|environment| environment.selection.clone())
                .collect::<Vec<_>>(),
            vec![remote.clone()]
        );
        assert_eq!(starting.to_selections(), vec![local.clone()]);
        assert!(starting.single_local_environment().is_none());

        let next_config = test_environment_config();
        turn_environments.update_environment_configs(&next_config);
        let next_starting = turn_environments.snapshot().await;

        let server = tokio::spawn(serve_environment_info(listener));
        timeout(
            std::time::Duration::from_secs(5),
            starting
                .starting()
                .next()
                .expect("starting environment")
                .resolution
                .clone(),
        )
        .await
        .expect("environment resolution should finish")
        .expect("environment resolution should succeed");
        let attached = starting.refresh_readiness();

        assert!(attached.starting().next().is_none());
        assert_eq!(
            attached
                .turn_environments()
                .map(TurnEnvironment::selection)
                .collect::<Vec<_>>(),
            vec![remote.clone(), local.clone()]
        );
        assert_eq!(
            attached
                .turn_environments()
                .map(|environment| environment.config.clone())
                .collect::<Vec<_>>(),
            vec![expected_config.clone(), expected_config]
        );
        assert_eq!(attached.to_selections(), vec![remote, local]);
        assert_eq!(
            next_starting
                .refresh_readiness()
                .turn_environments()
                .map(|environment| environment.config.clone())
                .collect::<Vec<_>>(),
            vec![next_config.clone(), next_config]
        );
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn failed_resolution_is_replaced_from_the_environment_manager() {
        let manager = Arc::new(
            EnvironmentManager::create_for_tests(
                Some("http://example.com".to_string()),
                Some(test_runtime_paths()),
            )
            .await,
        );
        let selection = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd")),
            workspace_roots: Vec::new(),
        };
        let environments = ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            test_environment_config(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ true,
        );
        environments
            .update_selections(std::slice::from_ref(&selection), &test_environment_config());
        let failed_resolution = environments.environments.load()[0].resolution.clone();
        let error = failed_resolution
            .clone()
            .await
            .err()
            .expect("environment should fail to start");
        let selected_root = SelectedCapabilityRoot {
            id: "failed-root".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: selection.environment_id.clone(),
                path: selection.cwd.clone(),
            },
        };
        assert_eq!(
            environments.inspect_selected_capability_roots(&[selected_root]),
            SelectedCapabilityRootsStatus {
                ready_roots: Vec::new(),
                warnings: vec![format!(
                    "selected capability environment `{}` is unavailable: {error}",
                    selection.environment_id
                )],
            }
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind replacement listener");
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                format!("ws://{}", listener.local_addr().expect("listener address")),
                /*connect_timeout*/ None,
            )
            .expect("replacement environment");
        let next_config = TurnEnvironmentConfig {
            allow_login_shell: false,
            ..test_environment_config()
        };
        environments.update_environment_configs(&next_config);
        assert!(
            failed_resolution.ptr_eq(&environments.environments.load()[0].resolution),
            "updating environment config must not retry a failed environment"
        );
        environments.update_selections(std::slice::from_ref(&selection), &next_config);

        let replacement = environments.snapshot().await;
        let replacement = replacement
            .starting()
            .next()
            .expect("expected the replacement environment to be starting");
        assert_eq!(replacement.selection, selection);
        assert!(!failed_resolution.ptr_eq(&replacement.resolution));
    }

    #[tokio::test]
    async fn replacement_environment_events_follow_selected_environment() {
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
            workspace_roots: Vec::new(),
        };
        let (tx_event, rx_event) = async_channel::unbounded();
        let environments = Arc::new(ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            test_environment_config(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ true,
        ));
        environments.start_connection_event_forwarding(tx_event);
        environments
            .update_selections(std::slice::from_ref(&selection), &test_environment_config());
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
                /*connect_timeout*/ None,
            )
            .expect("replace environment");

        environments
            .update_selections(std::slice::from_ref(&selection), &test_environment_config());
        let reused_snapshot = environments.snapshot().await;
        environments.update_selections(
            &[TurnEnvironmentSelection {
                cwd: PathUri::from_abs_path(&cwd.join("changed")),
                ..selection
            }],
            &test_environment_config(),
        );
        let changed_snapshot = environments.snapshot().await;

        let initial = initial_snapshot
            .starting()
            .next()
            .expect("initial environment");
        let reused = reused_snapshot
            .starting()
            .next()
            .expect("reused environment");
        let changed = changed_snapshot
            .starting()
            .next()
            .expect("changed environment");
        assert!(initial.resolution.ptr_eq(&reused.resolution));
        assert!(!reused.resolution.ptr_eq(&changed.resolution));

        serve_environment_info(first_listener).await;
        assert!(
            timeout(Duration::from_millis(250), rx_event.recv())
                .await
                .is_err(),
            "old environment event should not be forwarded"
        );

        serve_environment_info(second_listener).await;
        let event = timeout(Duration::from_secs(5), rx_event.recv())
            .await
            .expect("replacement environment event")
            .expect("event channel");
        let event = match event.msg {
            EventMsg::EnvironmentConnected(event) => event,
            other => panic!("expected connected event, got {other:?}"),
        };
        assert_eq!(
            event,
            EnvironmentConnectionEvent {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            }
        );
    }

    #[tokio::test]
    async fn inherited_environment_reuses_parent_handle_and_uses_child_config() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selection = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
            workspace_roots: Vec::new(),
        };
        let inherited_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("inherited environment"),
        );
        let inherited = TurnEnvironment::new(
            selection.clone(),
            Arc::clone(&inherited_environment),
            /*shell*/ None,
            test_environment_config(),
        );
        let manager = Arc::new(environment_manager_without_environments());
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                "ws://127.0.0.1:9876".to_string(),
                /*connect_timeout*/ None,
            )
            .expect("replacement environment");
        let child_config = TurnEnvironmentConfig {
            allow_login_shell: false,
            permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::read_only(),
                vec![cwd.join("child-profile-root")],
            ),
            selected_capability_roots: None,
        };
        let environments = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            child_config.clone(),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot {
                environments: vec![TurnEnvironmentState::Ready(inherited)],
            },
            /*non_blocking_snapshots*/ false,
        );

        environments.update_selections(std::slice::from_ref(&selection), &child_config);
        let snapshot = environments.snapshot().await;

        let inherited = snapshot.primary().expect("inherited environment");
        assert!(Arc::ptr_eq(&inherited.environment, &inherited_environment));
        assert_eq!(inherited.config, child_config);
    }

    #[tokio::test]
    async fn installed_environment_config_is_inherited_and_reset_for_new_cwd() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selection = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
            workspace_roots: Vec::new(),
        };
        let root = |id: &str| SelectedCapabilityRoot {
            id: id.to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: selection.environment_id.clone(),
                path: selection.cwd.clone(),
            },
        };
        let manager = Arc::new(EnvironmentManager::default_for_tests());
        let parent =
            resolve_turn_environments(Arc::clone(&manager), std::slice::from_ref(&selection)).await;
        parent
            .environment_ready(
                &selection,
                EnvironmentConfig {
                    allow_login_shell: false,
                    selected_capability_roots: vec![root("parent-root")],
                },
            )
            .expect("install environment config");

        let child_thread_config = TurnEnvironmentConfig {
            permission_profile: PermissionProfileSnapshot::legacy(
                PermissionProfile::workspace_write(),
            ),
            ..test_environment_config()
        };
        let child = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            child_thread_config.clone(),
            ShellSnapshot::disabled(),
            parent.snapshot().await,
            /*non_blocking_snapshots*/ false,
        );
        let child_snapshot = child.snapshot().await;
        let child_environment = child_snapshot.primary().expect("child environment");

        parent
            .environment_ready(
                &selection,
                EnvironmentConfig {
                    allow_login_shell: false,
                    selected_capability_roots: Vec::new(),
                },
            )
            .expect("clear parent roots");
        let cleared_snapshot = parent.snapshot().await;
        assert_eq!(
            cleared_snapshot
                .primary()
                .expect("environment with cleared roots")
                .config,
            TurnEnvironmentConfig {
                allow_login_shell: false,
                selected_capability_roots: Some(Vec::new()),
                ..test_environment_config()
            }
        );
        assert_eq!(
            child_environment.config,
            TurnEnvironmentConfig {
                allow_login_shell: false,
                selected_capability_roots: Some(vec![root("parent-root")]),
                ..child_thread_config
            }
        );

        let changed_selection = TurnEnvironmentSelection {
            cwd: PathUri::from_abs_path(&cwd.join("changed")),
            ..selection
        };
        let new_thread_config = test_environment_config();
        parent.update_selections(std::slice::from_ref(&changed_selection), &new_thread_config);
        let changed_snapshot = parent.snapshot().await;
        let changed_environment = changed_snapshot.primary().expect("changed environment");
        assert_eq!(changed_environment.config, new_thread_config);
    }

    #[tokio::test]
    async fn single_local_environment_cwd_requires_exactly_one_local_environment() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let remote_cwd_uri = PathUri::from_abs_path(&cwd.join("remote-cwd"));
        let local_manager = Arc::new(EnvironmentManager::default_for_tests());
        let local = resolve_turn_environments(
            Arc::clone(&local_manager),
            &[TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri.clone(),
                workspace_roots: Vec::new(),
            }],
        )
        .await;
        let local = local.snapshot().await;
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let remote = TurnEnvironmentSnapshot {
            environments: vec![TurnEnvironmentState::Ready(TurnEnvironment::new(
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: remote_cwd_uri.clone(),
                    workspace_roots: Vec::new(),
                },
                remote_environment.clone(),
                /*shell*/ None,
                test_environment_config(),
            ))],
        };
        let multiple = TurnEnvironmentSnapshot {
            environments: vec![
                TurnEnvironmentState::Ready(TurnEnvironment::new(
                    TurnEnvironmentSelection {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: remote_cwd_uri,
                        workspace_roots: Vec::new(),
                    },
                    remote_environment,
                    /*shell*/ None,
                    test_environment_config(),
                )),
                TurnEnvironmentState::Ready(local.primary().expect("local environment").clone()),
            ],
        };

        assert_eq!(local.single_local_environment_cwd(), Some(cwd.clone()));
        assert_eq!(remote.single_local_environment_cwd(), None);
        assert_eq!(multiple.single_local_environment_cwd(), None);
        assert_eq!(multiple.local_environment_cwd(), Some(cwd));
    }

    #[test]
    fn local_environment_cwd_uses_starting_local_selection() {
        let cwd = AbsolutePathBuf::current_dir()
            .expect("cwd")
            .join("starting-local");
        let snapshot = TurnEnvironmentSnapshot {
            environments: vec![TurnEnvironmentState::Starting(StartingTurnEnvironment {
                selection: TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::from_abs_path(&cwd),
                    workspace_roots: Vec::new(),
                },
                config: test_environment_config(),
                resolution: futures::future::pending().boxed().shared(),
            })],
        };

        assert_eq!(snapshot.local_environment_cwd(), Some(cwd));
    }
}
