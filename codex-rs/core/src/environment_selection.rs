use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;

use async_channel::Sender;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentConnectionState;
use codex_exec_server::EnvironmentInfo;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EnvironmentConnectionEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;

use crate::session::ThreadEnvironmentDefaults;
use crate::session::turn_context::ShellSnapshotCache;
use crate::session::turn_context::ShellSnapshotTask;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::Shell;
use crate::shell_snapshot::ShellSnapshot;
use crate::shell_snapshot::SnapshotCredentialBrokerState;

// Reject pathological selected cwd values at the environment-selection boundary.
const MAX_TURN_ENVIRONMENT_CWD_BYTES: usize = 8 * 1024;

/// Records whether a normalized config should follow later thread setting updates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EnvironmentConfigOrigin {
    Thread,
    Owner,
}

impl EnvironmentConfigOrigin {
    /// Reconstructs the input form so another attachment boundary preserves config ownership.
    pub(crate) fn into_input_selection(
        self,
        mut selection: TurnEnvironmentSelection,
    ) -> TurnEnvironmentSelection {
        if self == Self::Thread {
            selection.config = EnvironmentConfigState::FromThread;
        }
        selection
    }

    pub(crate) fn selected_capability_roots(
        self,
        environment: &Environment,
        config: &EnvironmentConfig,
    ) -> Vec<SelectedCapabilityRoot> {
        match self {
            Self::Thread => environment.selected_capability_roots(),
            Self::Owner => config.selected_capability_roots.clone(),
        }
    }
}

/// Combines persisted thread roots with attachment roots in selection order.
///
/// Thread-owned attachments still rely on roots reported by legacy executors. A matching live root
/// refreshes the persisted location while explicit owner configuration retains the existing
/// thread-first collision behavior.
pub(crate) fn combine_selected_capability_roots(
    thread_roots: &[SelectedCapabilityRoot],
    sources: impl IntoIterator<Item = (EnvironmentConfigOrigin, Vec<SelectedCapabilityRoot>)>,
) -> Vec<SelectedCapabilityRoot> {
    let attachment_roots = sources
        .into_iter()
        .flat_map(|(config_origin, roots)| roots.into_iter().map(move |root| (config_origin, root)))
        .collect::<Vec<_>>();
    let mut combined_roots = thread_roots
        .iter()
        .map(|thread_root| {
            let CapabilityRootLocation::Environment {
                environment_id: thread_environment_id,
                ..
            } = &thread_root.location;
            attachment_roots
                .iter()
                .find_map(|(origin, attachment_root)| {
                    if *origin != EnvironmentConfigOrigin::Thread
                        || attachment_root.id != thread_root.id
                    {
                        return None;
                    }
                    let CapabilityRootLocation::Environment {
                        environment_id: attachment_environment_id,
                        ..
                    } = &attachment_root.location;
                    (attachment_environment_id == thread_environment_id).then_some(attachment_root)
                })
                .unwrap_or(thread_root)
                .clone()
        })
        .collect::<Vec<_>>();
    combined_roots.extend(attachment_roots.into_iter().map(|(_, root)| root));
    combined_roots
}

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
            config: EnvironmentConfigState::FromThread,
        })
        .collect()
}

/// Checks that environment IDs are registered and unique and that working directories are not too long.
pub fn validate_environment_ids_and_cwds(
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    let mut environment_ids = HashSet::with_capacity(environments.len());
    for environment in environments {
        if environment.cwd.inferred_native_path_string().len() > MAX_TURN_ENVIRONMENT_CWD_BYTES {
            return Err(CodexErr::InvalidRequest(
                "turn environment working directory exceeds the maximum size".to_string(),
            ));
        }
        if !environment_ids.insert(environment.environment_id.as_str()) {
            return Err(CodexErr::InvalidRequest(format!(
                "duplicate turn environment id `{}`",
                environment.environment_id
            )));
        }
        environment_manager
            .get_environment(&environment.environment_id)
            .ok_or_else(|| {
                CodexErr::InvalidRequest(format!(
                    "unknown turn environment id `{}`",
                    environment.environment_id
                ))
            })?;
    }
    Ok(())
}

type TurnEnvironmentResult = Result<ResolvedEnvironment, Arc<ExecServerError>>;
type TurnEnvironmentResolution = Shared<BoxFuture<'static, TurnEnvironmentResult>>;
type PendingConfigurationResult = Result<EnvironmentConfig, String>;

// Shared startup result used to build each turn's environment with its own config
// without restarting the connection or shell resolution.
#[derive(Clone)]
struct ResolvedEnvironment {
    environment: Arc<Environment>,
    shell: Option<Shell>,
    user_home_dir: Option<PathUri>,
    executor_platform_os: Option<String>,
    temporary_directories: Option<Vec<PathUri>>,
    shell_snapshot: ShellSnapshotTask,
    shell_snapshot_builder: ShellSnapshot,
    shell_snapshot_cache: ShellSnapshotCache,
    shell_snapshot_v2_supported: bool,
    installed_config: Option<EnvironmentConfig>,
}

#[derive(Clone)]
struct SelectedTurnEnvironment {
    selection: TurnEnvironmentSelection,
    config_origin: EnvironmentConfigOrigin,
    environment: Arc<Environment>,
    // Selection clones share one listener; the final handle drop aborts it.
    connection_events_task: Option<Arc<AbortOnDropHandle<()>>>,
    resolution: TurnEnvironmentResolution,
    pending_completion: Option<mpsc::Sender<PendingConfigurationResult>>,
}

#[derive(Clone)]
pub(crate) struct StartingTurnEnvironment {
    pub(crate) selection: TurnEnvironmentSelection,
    config_origin: EnvironmentConfigOrigin,
    resolution: TurnEnvironmentResolution,
}

/// Uses the current thread defaults when requested and records who owns later config updates.
fn resolve_selection_config(
    mut selection: TurnEnvironmentSelection,
    defaults: &ThreadEnvironmentDefaults,
) -> (TurnEnvironmentSelection, EnvironmentConfigOrigin) {
    let (config, origin) = match selection.config {
        EnvironmentConfigState::FromThread => (
            EnvironmentConfigState::Ready(defaults.for_selection(&selection)),
            EnvironmentConfigOrigin::Thread,
        ),
        config @ (EnvironmentConfigState::Ready(_)
        | EnvironmentConfigState::Pending
        | EnvironmentConfigState::Failed(_)) => (config, EnvironmentConfigOrigin::Owner),
    };
    selection.config = config;
    (selection, origin)
}

impl SelectedTurnEnvironment {
    fn refresh_thread_config(&mut self, defaults: &ThreadEnvironmentDefaults) {
        if self.config_origin == EnvironmentConfigOrigin::Thread {
            self.selection.config =
                EnvironmentConfigState::Ready(defaults.for_selection(&self.selection));
        }
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
    #[tracing::instrument(
        name = "environments.wait_until_ready",
        skip_all,
        fields(environment_id = %self.selection.environment_id)
    )]
    pub(crate) async fn wait_until_ready(&self) -> Result<(), Arc<ExecServerError>> {
        self.resolution.clone().await.map(|_| ())
    }
}

pub(crate) struct ThreadEnvironments {
    environment_manager: Arc<EnvironmentManager>,
    local_shell: Shell,
    shell_snapshot: ShellSnapshot,
    non_blocking_snapshots: bool,
    state: Mutex<ThreadEnvironmentsState>,
    connection_event_tx: OnceLock<Sender<Event>>,
}

struct ThreadEnvironmentsState {
    thread_defaults: ThreadEnvironmentDefaults,
    environments: Vec<SelectedTurnEnvironment>,
}

impl ThreadEnvironments {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_shell: Shell,
        thread_defaults: ThreadEnvironmentDefaults,
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
                let selection = environment.selection;
                let config_origin = environment.config_origin;
                let selected_environment = Arc::clone(&environment.environment);
                let inherited_snapshot = if !selected_environment.is_remote()
                    && shell_snapshot.should_rebuild_inherited()
                {
                    futures::future::ready(None).boxed().shared()
                } else {
                    environment.shell_snapshot
                };
                let resolution: TurnEnvironmentResolution =
                    futures::future::ready(Ok(ResolvedEnvironment {
                        environment: environment.environment,
                        shell: environment.shell,
                        user_home_dir: environment.user_home_dir,
                        executor_platform_os: environment.executor_platform_os,
                        temporary_directories: environment.temporary_directories,
                        shell_snapshot: inherited_snapshot,
                        shell_snapshot_builder: shell_snapshot.clone(),
                        shell_snapshot_cache: Arc::default(),
                        shell_snapshot_v2_supported: environment.shell_snapshot_v2_supported
                            && (selected_environment.is_remote()
                                || !shell_snapshot.should_rebuild_inherited()),
                        installed_config: None,
                    }))
                    .boxed()
                    .shared();
                let mut inherited_environment = SelectedTurnEnvironment {
                    selection,
                    config_origin,
                    environment: selected_environment,
                    connection_events_task: None,
                    resolution,
                    pending_completion: None,
                };
                // Child threads re-infer thread-owned config while preserving owner config.
                inherited_environment.refresh_thread_config(&thread_defaults);
                Some(inherited_environment)
            })
            .collect();
        Self {
            environment_manager,
            local_shell,
            shell_snapshot,
            non_blocking_snapshots,
            state: Mutex::new(ThreadEnvironmentsState {
                thread_defaults,
                environments,
            }),
            connection_event_tx: OnceLock::new(),
        }
    }

    fn start_shell_snapshot_task(
        shell_snapshot: ShellSnapshot,
        environment: Arc<Environment>,
        cwd: PathUri,
        shell: Option<Shell>,
        config: &EnvironmentConfigState,
    ) -> ShellSnapshotTask {
        // Protected snapshots require the command's sandbox and are captured lazily.
        if shell_snapshot.should_rebuild_inherited() {
            return futures::future::ready(None).boxed().shared();
        }
        let EnvironmentConfigState::Ready(config) = config else {
            return futures::future::ready(None).boxed().shared();
        };
        let shell_snapshot = shell_snapshot
            .build(
                environment,
                cwd,
                shell,
                /*allow_login_shell*/ true,
                config.shell_environment_policy.clone(),
                /*sandbox*/ None,
            )
            .boxed()
            .shared();
        drop(tokio::spawn(
            shell_snapshot
                .clone()
                .in_current_span()
                .with_current_subscriber(),
        ));
        shell_snapshot
    }

    /// Updates the selected list before waking work that was waiting on the previous selection.
    pub(crate) fn update_selections(&self, environments: &[TurnEnvironmentSelection]) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let ThreadEnvironmentsState {
            thread_defaults,
            environments: current,
        } = &mut *state;
        let mut seen_environment_ids = HashSet::with_capacity(environments.len());
        let mut next = Vec::with_capacity(environments.len());
        for selected_environment in environments {
            if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
                continue;
            }
            if let Some(environment) = current.iter().find(|environment| {
                let previous = &environment.selection;
                previous.environment_id == selected_environment.environment_id
                    && previous.cwd == selected_environment.cwd
                    && previous.workspace_roots == selected_environment.workspace_roots
            }) {
                let failed =
                    matches!(
                        environment.selection.config,
                        EnvironmentConfigState::Failed(_)
                    ) || matches!(environment.resolution.clone().now_or_never(), Some(Err(_)));
                let restarting_as_pending =
                    matches!(selected_environment.config, EnvironmentConfigState::Pending)
                        && !matches!(
                            environment.selection.config,
                            EnvironmentConfigState::Pending
                        );

                if !failed && !restarting_as_pending {
                    let (selected_environment, config_origin) =
                        resolve_selection_config(selected_environment.clone(), thread_defaults);
                    let shell_settings_changed = matches!(
                        (&environment.selection.config, &selected_environment.config),
                        (EnvironmentConfigState::Ready(previous), EnvironmentConfigState::Ready(current))
                            if previous.allow_login_shell != current.allow_login_shell
                                || previous.shell_environment_policy != current.shell_environment_policy
                    );
                    let mut environment = environment.clone();
                    environment.selection = selected_environment;
                    environment.config_origin = config_origin;
                    if shell_settings_changed
                        && !environment.environment.is_remote()
                        && self.shell_snapshot.should_rebuild_inherited()
                        && let Some(Ok(resolved)) = environment.resolution.clone().now_or_never()
                    {
                        self.restart_shell_snapshot(&mut environment, resolved);
                    }
                    next.push(environment);
                    continue;
                }
            }

            let environment_id = &selected_environment.environment_id;
            let Some(environment) = self.environment_manager.get_environment(environment_id) else {
                tracing::warn!("skipping unknown turn environment `{environment_id}`");
                continue;
            };
            let (selected_environment, config_origin) =
                resolve_selection_config(selected_environment.clone(), thread_defaults);
            // Connection state belongs to the environment instance, not its cwd or roots.
            let connection_events_task = current
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
            let (pending_completion, configuration_ready) =
                if matches!(selected_environment.config, EnvironmentConfigState::Pending) {
                    let (sender, receiver) = mpsc::channel(/*buffer*/ 1);
                    (Some(sender), Some(receiver))
                } else {
                    (None, None)
                };
            let (resolution_task, resolution) = Self::resolve_environment(
                selected_environment.clone(),
                Arc::clone(&environment),
                self.local_shell.clone(),
                self.shell_snapshot.clone(),
                configuration_ready,
            )
            .remote_handle();
            drop(tokio::spawn(
                resolution_task.in_current_span().with_current_subscriber(),
            ));
            let resolution = resolution.boxed().shared();
            let selected = SelectedTurnEnvironment {
                selection: selected_environment,
                config_origin,
                environment,
                connection_events_task,
                resolution,
                pending_completion,
            };
            next.push(selected);
        }
        let removed_connection_tasks = current
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
        let previous = std::mem::replace(current, next);

        // Publish owner configuration before waking turns waiting on this attachment.
        for environment in current.iter() {
            let Some(completion) = &environment.pending_completion else {
                continue;
            };
            let result = match &environment.selection.config {
                EnvironmentConfigState::Ready(config) => Ok(config.clone()),
                EnvironmentConfigState::Failed(error) => Err(error.clone()),
                EnvironmentConfigState::FromThread | EnvironmentConfigState::Pending => continue,
            };
            let _ = completion.try_send(result);
        }

        for task in removed_connection_tasks {
            task.abort();
        }

        drop(previous);
    }

    /// Projects canonical selections back to caller input form without losing config ownership.
    pub(crate) fn selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .environments
            .iter()
            .map(|environment| {
                environment
                    .config_origin
                    .into_input_selection(environment.selection.clone())
            })
            .collect()
    }

    /// Returns installed owner configuration without treating pending attachments as ready.
    pub(crate) fn primary_config_for(
        selections: &[TurnEnvironmentSelection],
    ) -> Option<&EnvironmentConfig> {
        match &selections.first()?.config {
            EnvironmentConfigState::Ready(config) => Some(config),
            EnvironmentConfigState::FromThread
            | EnvironmentConfigState::Pending
            | EnvironmentConfigState::Failed(_) => None,
        }
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

    /// Changes the defaults inherited by all selected and later-added environments. A step that
    /// already captured its environments keeps the previous values.
    pub(crate) fn set_active_thread_defaults(&self, defaults: EnvironmentConfig) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let ThreadEnvironmentsState {
            thread_defaults,
            environments,
        } = &mut *state;
        if thread_defaults.common == defaults {
            return;
        }
        thread_defaults.common = defaults;
        for environment in environments.iter_mut() {
            let shell_settings_changed = environment.config_origin
                == EnvironmentConfigOrigin::Thread
                && matches!(
                    &environment.selection.config,
                    EnvironmentConfigState::Ready(previous)
                        if previous.allow_login_shell != thread_defaults.common.allow_login_shell
                            || previous.shell_environment_policy != thread_defaults.common.shell_environment_policy
                );
            environment.refresh_thread_config(thread_defaults);
            if shell_settings_changed
                && !environment.environment.is_remote()
                && self.shell_snapshot.should_rebuild_inherited()
                && let Some(Ok(resolved)) = environment.resolution.clone().now_or_never()
            {
                self.restart_shell_snapshot(environment, resolved);
            }
        }
    }

    pub(crate) fn set_snapshot_credential_broker(&self, broker: SnapshotCredentialBrokerState) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !self.shell_snapshot.set_credential_broker(broker) {
            return;
        }

        for selected in &mut state.environments {
            if !selected.environment.is_remote()
                && let Some(Ok(resolved)) = selected.resolution.clone().now_or_never()
            {
                self.restart_shell_snapshot(selected, resolved);
            }
        }
    }

    fn restart_shell_snapshot(
        &self,
        selected: &mut SelectedTurnEnvironment,
        resolved: ResolvedEnvironment,
    ) {
        let shell_snapshot = Self::start_shell_snapshot_task(
            self.shell_snapshot.clone(),
            Arc::clone(&resolved.environment),
            selected.selection.cwd.clone(),
            resolved.shell.clone(),
            &selected.selection.config,
        );
        selected.resolution = futures::future::ready(Ok(ResolvedEnvironment {
            shell_snapshot,
            shell_snapshot_cache: Arc::default(),
            shell_snapshot_v2_supported: cfg!(unix)
                && !self.shell_snapshot.should_rebuild_inherited(),
            ..resolved
        }))
        .boxed()
        .shared();
    }

    /// Combines persisted thread roots with installed attachment roots, keeping
    /// thread roots first and hiding attachments that are not ready yet.
    pub(crate) fn inspect_selected_capability_roots(
        &self,
        thread_selected_capability_roots: &[SelectedCapabilityRoot],
    ) -> SelectedCapabilityRootsStatus {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let environments = &state.environments;
        let mut selected_capability_roots = combine_selected_capability_roots(
            thread_selected_capability_roots,
            environments.iter().filter_map(|environment| {
                let EnvironmentConfigState::Ready(config) = &environment.selection.config else {
                    return None;
                };
                Some((
                    environment.config_origin,
                    environment
                        .config_origin
                        .selected_capability_roots(&environment.environment, config),
                ))
            }),
        );
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
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let tx_event = self.connection_event_tx.get_or_init(|| tx_event);
        for selected in &mut state.environments {
            if selected.connection_events_task.is_none() {
                selected.connection_events_task = Self::spawn_connection_event_listener(
                    selected.environment.as_ref(),
                    selected.selection.environment_id.clone(),
                    tx_event.clone(),
                );
            }
        }
    }

    #[tracing::instrument(
        name = "environments.resolve",
        skip_all,
        fields(
            environment_id = %selection.environment_id,
            remote = environment.is_remote(),
            configuration_pending = configuration_ready.is_some(),
        )
    )]
    async fn resolve_environment(
        selection: TurnEnvironmentSelection,
        environment: Arc<Environment>,
        local_shell: Shell,
        shell_snapshot: ShellSnapshot,
        configuration_ready: Option<mpsc::Receiver<PendingConfigurationResult>>,
    ) -> TurnEnvironmentResult {
        let environment_id = &selection.environment_id;
        if let EnvironmentConfigState::Failed(error) = &selection.config {
            return Err(Arc::new(ExecServerError::Protocol(error.clone())));
        }

        // Wait for the shared executor connection.
        let connection_ready = async {
            environment.wait_until_ready().await.map_err(|error| {
                tracing::warn!("turn environment `{environment_id}` failed to start: {error}");
                Arc::new(error)
            })
        };
        // Wait for this thread attachment's owner configuration, if pending.
        let configuration_ready = async move {
            let Some(mut configuration_ready) = configuration_ready else {
                return Ok(None);
            };
            match configuration_ready.recv().await {
                Some(Ok(config)) => Ok(Some(config)),
                Some(Err(error)) => Err(Arc::new(ExecServerError::Protocol(error))),
                None => Err(Arc::new(ExecServerError::Protocol(
                    "environment configuration was canceled".to_string(),
                ))),
            }
        };
        // Resolve the attachment only after both prerequisites are ready.
        let ((), installed_config) = tokio::try_join!(connection_ready, configuration_ready)?;
        let executor_platform_os;
        let (shell, user_home_dir, temporary_dirs, snapshot_v2) = if environment.is_remote() {
            match environment.info().await {
                Ok(info) => {
                    executor_platform_os = info.platform_os;
                    let user_home_dir = info.user_home_dir;
                    let temporary_directories = info.temporary_directories;
                    let shell_snapshot_v2_supported = info.capabilities.shell_snapshot_v2;
                    let shell = match Shell::from_environment_shell_info(info.shell) {
                        Ok(shell) => Some(shell),
                        Err(err) => {
                            tracing::warn!(
                                "failed to resolve shell for environment `{environment_id}`: {err}"
                            );
                            None
                        }
                    };
                    (
                        shell,
                        user_home_dir,
                        temporary_directories,
                        shell_snapshot_v2_supported,
                    )
                }
                Err(err) => {
                    executor_platform_os = None;
                    tracing::warn!("failed to get info for environment `{environment_id}`: {err}");
                    (None, None, None, false)
                }
            }
        } else {
            executor_platform_os = Some(std::env::consts::OS.to_string());
            (
                Some(local_shell),
                PathUri::from_host_native_path("~").ok(),
                Some(EnvironmentInfo::local_temporary_directories()),
                cfg!(unix),
            )
        };
        let shell_snapshot_builder = shell_snapshot.clone();
        let snapshot_config = installed_config
            .clone()
            .map(EnvironmentConfigState::Ready)
            .unwrap_or(selection.config);
        let task = Self::start_shell_snapshot_task(
            shell_snapshot,
            Arc::clone(&environment),
            selection.cwd,
            shell.clone(),
            &snapshot_config,
        );
        let shell_snapshot_v2_supported = snapshot_v2
            && (environment.is_remote() || !shell_snapshot_builder.should_rebuild_inherited());
        Ok(ResolvedEnvironment {
            environment,
            shell,
            user_home_dir,
            executor_platform_os,
            temporary_directories: temporary_dirs,
            shell_snapshot: task,
            shell_snapshot_builder,
            shell_snapshot_cache: Arc::default(),
            shell_snapshot_v2_supported,
            installed_config,
        })
    }

    /// Captures the selected list immediately, then returns a future that may wait for setup.
    /// This lets callers release their locks before waiting without accidentally using a newer
    /// selection. The trace still covers the setup wait itself.
    pub(crate) fn snapshot(
        &self,
    ) -> impl Future<Output = TurnEnvironmentSnapshot> + Send + 'static {
        self.resolve_snapshot(self.snapshot_now())
    }

    /// Copies the current selection without waiting for any executor to connect.
    pub(crate) fn snapshot_now(&self) -> TurnEnvironmentSnapshot {
        // Keep the startup results, but not the senders that would keep removed Pending work alive.
        let environments = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .environments
            .iter()
            .map(|environment| {
                let starting = StartingTurnEnvironment {
                    selection: environment.selection.clone(),
                    config_origin: environment.config_origin,
                    resolution: environment.resolution.clone(),
                };
                let resolved = starting.resolution.clone().now_or_never();
                TurnEnvironmentState::from_resolution(starting, resolved)
            })
            .collect();
        TurnEnvironmentSnapshot { environments }
    }

    /// Applies the same startup policy to an already captured selection without adopting newer ones.
    #[tracing::instrument(
        name = "environments.snapshot",
        skip_all,
        fields(
            environment_count = snapshot.environments.len(),
            non_blocking = non_blocking_snapshots,
        )
    )]
    fn resolve_snapshot(
        &self,
        snapshot: TurnEnvironmentSnapshot,
    ) -> impl Future<Output = TurnEnvironmentSnapshot> + Send + 'static {
        let non_blocking_snapshots = self.non_blocking_snapshots;
        async move {
            if !non_blocking_snapshots {
                for starting in snapshot.starting() {
                    if !matches!(starting.selection.config, EnvironmentConfigState::Pending) {
                        let _ = starting.wait_until_ready().await;
                    }
                }
            }
            // An earlier environment may have become ready while we waited for a later one.
            snapshot.refresh_readiness()
        }
    }

    pub(crate) fn environment_manager(&self) -> Arc<EnvironmentManager> {
        Arc::clone(&self.environment_manager)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TurnEnvironmentState {
    Ready(TurnEnvironment),
    Starting(StartingTurnEnvironment),
    /// Unavailable for execution, but still selected when evaluating permissions.
    Failed {
        // Keep the input form so connection failure doesn't hide whether config comes from the thread.
        selection: TurnEnvironmentSelection,
        error: String,
    },
}

impl TurnEnvironmentState {
    fn from_resolution(
        starting: StartingTurnEnvironment,
        resolved: Option<TurnEnvironmentResult>,
    ) -> Self {
        if let EnvironmentConfigState::Failed(error) = &starting.selection.config {
            let error = error.clone();
            return Self::Failed {
                selection: starting
                    .config_origin
                    .into_input_selection(starting.selection),
                error,
            };
        }
        match resolved {
            Some(Ok(environment)) => {
                let mut selection = starting.selection;
                if matches!(selection.config, EnvironmentConfigState::Pending) {
                    let Some(config) = environment.installed_config else {
                        return Self::Failed {
                            selection: starting.config_origin.into_input_selection(selection),
                            error: "Environment configuration was not supplied.".to_string(),
                        };
                    };
                    selection.config = EnvironmentConfigState::Ready(config);
                }
                let mut turn_environment = TurnEnvironment::new(
                    selection,
                    starting.config_origin,
                    environment.environment,
                    environment.shell,
                );
                turn_environment.executor_platform_os = environment.executor_platform_os;
                turn_environment.shell_snapshot = environment.shell_snapshot;
                turn_environment.shell_snapshot_builder =
                    Some(Box::new(environment.shell_snapshot_builder));
                turn_environment.shell_snapshot_cache = environment.shell_snapshot_cache;
                turn_environment.shell_snapshot_v2_supported =
                    environment.shell_snapshot_v2_supported;
                turn_environment.user_home_dir = environment.user_home_dir;
                turn_environment.temporary_directories = environment.temporary_directories;
                Self::Ready(turn_environment)
            }
            Some(Err(err)) => {
                tracing::debug!(
                    environment_id = %starting.selection.environment_id,
                    "skipping failed turn environment: {err}"
                );
                Self::Failed {
                    selection: starting
                        .config_origin
                        .into_input_selection(starting.selection),
                    error: err.to_string(),
                }
            }
            None => Self::Starting(starting),
        }
    }
}

/// Existing environment bindings captured for a turn. Internal child threads can
/// retain these bindings without resolving a different set of environments.
#[derive(Clone, Debug, Default)]
pub struct TurnEnvironmentSnapshot {
    // Keep every selected environment, including failures, in its original order.
    pub(crate) environments: Vec<TurnEnvironmentState>,
}

impl TurnEnvironmentSnapshot {
    pub(crate) fn has_full_access(
        &self,
        approval_policy: AskForApproval,
        thread_profile: &PermissionProfile,
    ) -> bool {
        codex_protocol::protocol::has_full_access(
            approval_policy,
            thread_profile,
            self.refresh_readiness()
                .environments
                .iter()
                .map(|environment| match environment {
                    TurnEnvironmentState::Ready(environment) => &environment.selection.config,
                    TurnEnvironmentState::Starting(_) | TurnEnvironmentState::Failed { .. } => {
                        &EnvironmentConfigState::Pending
                    }
                }),
        )
    }

    /// Promotes completed startup work without adopting newer thread selections.
    pub(crate) fn refresh_readiness(&self) -> Self {
        let environments = self
            .environments
            .iter()
            .map(|environment| match environment {
                TurnEnvironmentState::Ready(environment) => {
                    TurnEnvironmentState::Ready(environment.clone())
                }
                TurnEnvironmentState::Starting(environment) => {
                    TurnEnvironmentState::from_resolution(
                        environment.clone(),
                        environment.resolution.clone().now_or_never(),
                    )
                }
                TurnEnvironmentState::Failed { .. } => environment.clone(),
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

    pub(crate) fn ready_environment_handles(&self) -> HashMap<String, Arc<Environment>> {
        self.turn_environments()
            .map(|environment| {
                (
                    environment.selection.environment_id.clone(),
                    Arc::clone(&environment.environment),
                )
            })
            .collect()
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

    /// Returns the first selected environment's host folders, even if setup is not ready yet.
    pub(crate) fn primary_workspace_roots(&self) -> Vec<AbsolutePathBuf> {
        self.primary_workspace_root_uris()
            .iter()
            .filter_map(|root| root.to_abs_path().ok())
            .collect()
    }

    /// Returns the first selection's executor paths, even when setup is not ready yet.
    pub(crate) fn primary_workspace_root_uris(&self) -> &[PathUri] {
        let selection = match self.environments.first() {
            Some(TurnEnvironmentState::Ready(environment)) => &environment.selection,
            Some(TurnEnvironmentState::Starting(environment)) => &environment.selection,
            Some(TurnEnvironmentState::Failed { selection, .. }) => selection,
            None => return &[],
        };
        &selection.workspace_roots
    }

    /// Returns the primary environment's resolved permissions, or the provided fallback.
    pub(crate) fn permission_profile_or_else(
        &self,
        fallback: impl FnOnce() -> PermissionProfile,
    ) -> PermissionProfile {
        self.primary()
            .map(TurnEnvironment::permission_profile_with_workspace_roots)
            .unwrap_or_else(fallback)
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
                TurnEnvironmentState::Starting(_) | TurnEnvironmentState::Failed { .. } => None,
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

    /// Returns every captured selection, including those still starting or unable to connect.
    pub(crate) fn all_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.environments
            .iter()
            .map(|environment| match environment {
                TurnEnvironmentState::Ready(environment) => environment.selection(),
                TurnEnvironmentState::Starting(environment) => environment
                    .config_origin
                    .into_input_selection(environment.selection.clone()),
                TurnEnvironmentState::Failed { selection, .. } => selection.clone(),
            })
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
#[path = "environment_selection_snapshot_tests.rs"]
mod snapshot_tests;

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
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::ActivePermissionProfile;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemSandboxPolicyContext;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_protocol::sandbox::SandboxType;
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
    use tracing::Level;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_test::internal::MockWriter;

    use super::*;

    pub(super) fn test_environment_config() -> EnvironmentConfig {
        EnvironmentConfig {
            allow_login_shell: true,
            workspace_roots: Vec::new(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
            use_legacy_landlock: false,
            permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::read_only()),
            shell_environment_policy: Default::default(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: Vec::new(),
        }
    }

    async fn resolve_turn_environments(
        environment_manager: Arc<EnvironmentManager>,
        selections: &[TurnEnvironmentSelection],
    ) -> Arc<ThreadEnvironments> {
        let turn_environments = Arc::new(ThreadEnvironments::new(
            environment_manager,
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
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
                    "result": {
                        "shell": { "name": "zsh", "path": "/bin/zsh" },
                        "userHomeDir": "file:///home/remote",
                        "platformOs": "windows",
                        "temporaryDirectories": ["file:///tmp/remote"],
                        "capabilities": {"windowsMxc": true},
                    }
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
                config: EnvironmentConfigState::FromThread,
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
                    config: EnvironmentConfigState::FromThread,
                },
                TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.clone(),
                    workspace_roots: vec![cwd_uri],
                    config: EnvironmentConfigState::FromThread,
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
        let expected_config = EnvironmentConfig {
            allow_login_shell: false,
            workspace_roots: Vec::new(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
            use_legacy_landlock: false,
            permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::read_only(),
                vec![cwd.join("profile-root").into()],
            ),
            shell_environment_policy: Default::default(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: Vec::new(),
        };
        let turn_environments = ThreadEnvironments::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            local_shell.clone(),
            ThreadEnvironmentDefaults::new(
                expected_config.clone(),
                expected_config.windows_sandbox_type,
            ),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
        );
        turn_environments.update_selections(&[TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        }]);

        let snapshot = turn_environments.snapshot().await;
        let environment = snapshot.primary().expect("local environment");
        let expected_user_home_dir = PathUri::from_host_native_path("~").ok();
        let expected_temporary_directories = EnvironmentInfo::local_temporary_directories();

        assert_eq!(environment.shell.as_ref(), Some(&local_shell));
        assert_eq!(environment.config(), &expected_config);
        assert_eq!(
            environment
                .sandbox_context(/*additional_permissions*/ None)
                .policy_context(),
            FileSystemSandboxPolicyContext {
                cwd: environment.cwd(),
                workspace_roots: &[],
                user_home_dir: expected_user_home_dir.as_ref(),
                temporary_directories: Some(expected_temporary_directories.as_slice()),
            }
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
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        };

        let resolved = resolve_turn_environments(
            manager,
            &[
                first.clone(),
                TurnEnvironmentSelection {
                    environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    cwd: cwd_uri.join("other").expect("other cwd URI"),
                    workspace_roots: Vec::new(),
                    config: EnvironmentConfigState::FromThread,
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
                config: EnvironmentConfigState::FromThread,
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
            config: EnvironmentConfigState::FromThread,
        };

        let resolved = resolve_turn_environments(
            manager,
            &[
                TurnEnvironmentSelection {
                    environment_id: "missing".to_string(),
                    cwd: cwd_uri,
                    workspace_roots: Vec::new(),
                    config: EnvironmentConfigState::FromThread,
                },
                local.clone(),
            ],
        )
        .await;

        assert_eq!(resolved.snapshot().await.to_selections(), vec![local]);
    }

    #[tokio::test]
    async fn blocking_snapshot_waits_for_starting_environment() {
        let buffer: &'static std::sync::Mutex<Vec<u8>> =
            Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(Level::TRACE)
            .with_span_events(FmtSpan::NEW)
            .with_writer(MockWriter::new(buffer))
            .finish();
        // Avoid tracing-core's single-dispatch path caching no interest when another
        // test first reaches these shared callsites without a subscriber.
        let _interest_cache_guard =
            tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

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
            config: EnvironmentConfigState::FromThread,
        };
        let environments = Arc::new(ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
        ));
        environments.update_selections(std::slice::from_ref(&selection));
        // Exercise first-use callsite registration from a thread without a subscriber.
        std::thread::spawn({
            let environments = Arc::clone(&environments);
            move || assert!(environments.snapshot().now_or_never().is_none())
        })
        .join()
        .expect("unsubscribed snapshot");
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

        let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
            .expect("UTF-8 tracing output");
        assert!(
            logs.contains(
                "environments.snapshot{environment_count=1 non_blocking=false}:environments.wait_until_ready{environment_id=remote}"
            ),
            "blocking snapshot should contain its environment wait span: {logs}"
        );
        assert!(
            logs.contains(
                "environments.resolve{environment_id=remote remote=true configuration_pending=false}:exec_server.environment.wait_until_ready{remote=true}"
            ),
            "environment resolution should contain the executor connection wait span: {logs}"
        );
        assert!(
            logs.contains(
                "environments.resolve{environment_id=remote remote=true configuration_pending=false}:exec_server.environment.info{remote=true}"
            ),
            "environment resolution should contain the remote environment-info span: {logs}"
        );
    }

    #[test_case::test_case(SandboxType::None, SandboxType::None, WindowsSandboxLevel::Disabled; "legacy")]
    #[test_case::test_case(SandboxType::WindowsRestrictedToken, SandboxType::WindowsMxc, WindowsSandboxLevel::Elevated; "local preference")]
    #[test_case::test_case(SandboxType::WindowsMxc, SandboxType::WindowsMxc, WindowsSandboxLevel::Disabled; "explicit MXC")]
    #[tokio::test]
    async fn snapshot_refreshes_readiness_in_selection_order(
        configured_sandbox: SandboxType,
        local_sandbox: SandboxType,
        legacy_level: WindowsSandboxLevel,
    ) {
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
        let expected_config = EnvironmentConfig {
            allow_login_shell: false,
            workspace_roots: Vec::new(),
            windows_sandbox_level: legacy_level,
            windows_sandbox_type: configured_sandbox,
            use_legacy_landlock: false,
            permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::read_only(),
                vec![cwd.join("profile-root").into()],
            ),
            shell_environment_policy: Default::default(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: Vec::new(),
        };
        let expected_local_config = EnvironmentConfig {
            windows_sandbox_type: local_sandbox,
            ..expected_config.clone()
        };
        let cwd = PathUri::from_abs_path(&cwd);
        let remote = TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: cwd.clone(),
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        };
        let local = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd,
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        };
        let turn_environments = ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(test_environment_config(), local_sandbox),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ true,
        );
        turn_environments.update_selections(std::slice::from_ref(&local));
        let resolution = turn_environments.state.lock().unwrap().environments[0]
            .resolution
            .clone();
        resolution.await.expect("local environment should resolve");
        turn_environments.update_selections(&[remote.clone(), local.clone()]);
        let initial = turn_environments.snapshot().await;
        assert_eq!(
            initial.primary().expect("local environment").config(),
            &EnvironmentConfig {
                windows_sandbox_type: local_sandbox,
                ..test_environment_config()
            },
        );
        assert_eq!(
            initial.starting().next().unwrap().selection.config,
            EnvironmentConfigState::Ready(test_environment_config()),
        );
        turn_environments.set_active_thread_defaults(expected_config.clone());

        let starting = turn_environments.snapshot().await;
        let resolved_remote = TurnEnvironmentSelection {
            config: EnvironmentConfigState::Ready(expected_config.clone()),
            ..remote.clone()
        };
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
                .map(|environment| environment.config().clone())
                .collect::<Vec<_>>(),
            vec![expected_local_config.clone()]
        );
        assert_eq!(
            starting
                .starting()
                .map(|environment| environment.selection.clone())
                .collect::<Vec<_>>(),
            vec![resolved_remote.clone()]
        );
        assert_eq!(starting.to_selections(), vec![local.clone()]);
        assert_eq!(
            starting.all_selections(),
            vec![remote.clone(), local.clone()]
        );
        assert!(starting.single_local_environment().is_none());

        let next_config = EnvironmentConfig {
            permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::Disabled),
            ..test_environment_config()
        };
        turn_environments.set_active_thread_defaults(next_config.clone());
        let next_starting = turn_environments.snapshot().await;
        assert!(
            !next_starting.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled)
        );

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
        assert!(next_starting.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));
        assert!(!starting.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));
        let attached = starting.refresh_readiness();

        assert!(attached.starting().next().is_none());
        let child = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(next_config.clone(), local_sandbox),
            ShellSnapshot::disabled(),
            attached.clone(),
            /*non_blocking_snapshots*/ true,
        );
        assert_eq!(
            child
                .snapshot()
                .await
                .turn_environments()
                .map(|environment| environment.config().clone())
                .collect::<Vec<_>>(),
            next_starting
                .refresh_readiness()
                .turn_environments()
                .map(|environment| environment.config().clone())
                .collect::<Vec<_>>(),
        );
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
                .map(|environment| environment.config().clone())
                .collect::<Vec<_>>(),
            vec![expected_config, expected_local_config]
        );
        assert_eq!(
            attached
                .turn_environments()
                .map(|environment| environment.executor_platform_os.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("windows"), Some(std::env::consts::OS)]
        );
        assert_eq!(attached.to_selections(), vec![remote, local]);
        let environment = attached.primary().expect("remote environment");
        let expected_user_home_dir = PathUri::parse("file:///home/remote").expect("remote home");
        let expected_temporary_directories =
            [PathUri::parse("file:///tmp/remote").expect("remote temporary directory")];
        assert_eq!(
            environment
                .sandbox_context(/*additional_permissions*/ None)
                .policy_context(),
            FileSystemSandboxPolicyContext {
                cwd: environment.cwd(),
                workspace_roots: &[],
                user_home_dir: Some(&expected_user_home_dir),
                temporary_directories: Some(expected_temporary_directories.as_slice()),
            }
        );
        assert_eq!(
            next_starting
                .refresh_readiness()
                .turn_environments()
                .map(|environment| environment.config().clone())
                .collect::<Vec<_>>(),
            vec![
                next_config.clone(),
                EnvironmentConfig {
                    windows_sandbox_type: local_sandbox,
                    ..next_config
                }
            ]
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
            config: EnvironmentConfigState::FromThread,
        };
        let environments = ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ true,
        );
        environments.update_selections(std::slice::from_ref(&selection));
        let starting = environments.snapshot().await;
        let failed_resolution = environments.state.lock().unwrap().environments[0]
            .resolution
            .clone();
        let error = failed_resolution
            .clone()
            .await
            .err()
            .expect("environment should fail to start");
        // Failed selections must not turn an empty executable snapshot into Full Access.
        for snapshot in [starting.refresh_readiness(), environments.snapshot().await] {
            assert!(!snapshot.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));
            assert_eq!(snapshot.all_selections(), vec![selection.clone()]);
            assert!(snapshot.ready_environment_handles().is_empty());
        }
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
        let next_config = EnvironmentConfig {
            allow_login_shell: false,
            ..test_environment_config()
        };
        environments.set_active_thread_defaults(next_config.clone());
        assert!(
            failed_resolution
                .ptr_eq(&environments.state.lock().unwrap().environments[0].resolution),
            "updating environment config must not retry a failed environment"
        );
        environments.update_selections(std::slice::from_ref(&selection));

        let replacement = environments.snapshot().await;
        let replacement = replacement
            .starting()
            .next()
            .expect("expected the replacement environment to be starting");
        assert_eq!(
            replacement.selection,
            TurnEnvironmentSelection {
                config: EnvironmentConfigState::Ready(next_config),
                ..selection.clone()
            }
        );
        assert_eq!(environments.selections(), vec![selection]);
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
            config: EnvironmentConfigState::FromThread,
        };
        let (tx_event, rx_event) = async_channel::unbounded();
        let environments = Arc::new(ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ true,
        ));
        environments.start_connection_event_forwarding(tx_event);
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
                /*connect_timeout*/ None,
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
            config: EnvironmentConfigState::FromThread,
        };
        let inherited_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("inherited environment"),
        );
        let mut inherited = TurnEnvironment::new(
            TurnEnvironmentSelection {
                config: EnvironmentConfigState::Ready(test_environment_config()),
                ..selection.clone()
            },
            EnvironmentConfigOrigin::Thread,
            Arc::clone(&inherited_environment),
            /*shell*/ None,
        );
        inherited.user_home_dir =
            Some(PathUri::parse("file:///home/inherited").expect("home directory"));
        inherited.temporary_directories = Some(vec![
            PathUri::parse("file:///tmp/inherited").expect("temporary directory"),
        ]);
        let manager = Arc::new(environment_manager_without_environments());
        manager
            .upsert_environment(
                REMOTE_ENVIRONMENT_ID.to_string(),
                "ws://127.0.0.1:9876".to_string(),
                /*connect_timeout*/ None,
            )
            .expect("replacement environment");
        let child_config = EnvironmentConfig {
            allow_login_shell: false,
            workspace_roots: Vec::new(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
            use_legacy_landlock: false,
            permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::read_only(),
                vec![cwd.join("child-profile-root").into()],
            ),
            shell_environment_policy: Default::default(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: Vec::new(),
        };
        let environments = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(child_config.clone(), child_config.windows_sandbox_type),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot {
                environments: vec![TurnEnvironmentState::Ready(inherited)],
            },
            /*non_blocking_snapshots*/ false,
        );

        environments.update_selections(std::slice::from_ref(&selection));
        let snapshot = environments.snapshot().await;

        let inherited = snapshot.primary().expect("inherited environment");
        assert!(Arc::ptr_eq(&inherited.environment, &inherited_environment));
        assert_eq!(inherited.config(), &child_config);
        assert_eq!(
            inherited
                .sandbox_context(/*additional_permissions*/ None)
                .user_home_dir,
            Some(PathUri::parse("file:///home/inherited").expect("home directory")),
        );
        assert_eq!(
            inherited
                .sandbox_context(/*additional_permissions*/ None)
                .temporary_directories
                .as_deref(),
            Some(
                [PathUri::parse("file:///tmp/inherited").expect("temporary directory")].as_slice()
            )
        );
    }

    #[tokio::test]
    async fn owner_environment_config_is_inherited_and_reset_for_new_cwd() {
        let cwd = AbsolutePathBuf::current_dir().expect("cwd");
        let selection = TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&cwd),
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        };
        let root = |id: &str| SelectedCapabilityRoot {
            id: id.to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: selection.environment_id.clone(),
                path: selection.cwd.clone(),
            },
        };
        let manager = Arc::new(EnvironmentManager::default_for_tests());
        let parent = ThreadEnvironments::new(
            Arc::clone(&manager),
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::WindowsMxc),
            ShellSnapshot::disabled(),
            TurnEnvironmentSnapshot::default(),
            /*non_blocking_snapshots*/ false,
        );
        parent.update_selections(std::slice::from_ref(&selection));
        parent.snapshot().await;
        let parent_owner_config = EnvironmentConfig {
            allow_login_shell: false,
            workspace_roots: selection.workspace_roots.clone(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
            use_legacy_landlock: false,
            permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::read_only()),
            shell_environment_policy: Default::default(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: vec![root("parent-root")],
        };
        let mut owner_selection = selection.clone();
        owner_selection.config = EnvironmentConfigState::Ready(parent_owner_config.clone());
        parent.update_selections(std::slice::from_ref(&owner_selection));

        let child_thread_config = EnvironmentConfig {
            windows_sandbox_type: SandboxType::WindowsMxc,
            permission_profile: PermissionProfileSnapshot::legacy(
                PermissionProfile::workspace_write(),
            ),
            ..test_environment_config()
        };
        parent.set_active_thread_defaults(child_thread_config.clone());
        assert_eq!(
            parent
                .snapshot()
                .await
                .primary()
                .expect("owner-configured environment")
                .config(),
            &parent_owner_config
        );
        let child = ThreadEnvironments::new(
            manager,
            crate::shell::default_user_shell(),
            ThreadEnvironmentDefaults::new(child_thread_config.clone(), SandboxType::WindowsMxc),
            ShellSnapshot::disabled(),
            parent.snapshot().await,
            /*non_blocking_snapshots*/ false,
        );
        let child_snapshot = child.snapshot().await;
        let child_environment = child_snapshot.primary().expect("child environment");

        let cleared_parent_owner_config = EnvironmentConfig {
            selected_capability_roots: Vec::new(),
            ..parent_owner_config.clone()
        };
        owner_selection.config = EnvironmentConfigState::Ready(cleared_parent_owner_config.clone());
        parent.update_selections(std::slice::from_ref(&owner_selection));
        let cleared_snapshot = parent.snapshot().await;
        assert_eq!(
            cleared_snapshot
                .primary()
                .expect("environment with cleared roots")
                .config(),
            &cleared_parent_owner_config
        );
        assert_eq!(child_environment.config(), &parent_owner_config);

        let changed_selection = TurnEnvironmentSelection {
            cwd: PathUri::from_abs_path(&cwd.join("changed")),
            ..selection
        };
        parent.update_selections(std::slice::from_ref(&changed_selection));
        let changed_snapshot = parent.snapshot().await;
        let changed_environment = changed_snapshot.primary().expect("changed environment");
        assert_eq!(changed_environment.config(), &child_thread_config);
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
                config: EnvironmentConfigState::FromThread,
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
                    config: EnvironmentConfigState::Ready(test_environment_config()),
                },
                EnvironmentConfigOrigin::Thread,
                remote_environment.clone(),
                /*shell*/ None,
            ))],
        };
        let multiple = TurnEnvironmentSnapshot {
            environments: vec![
                TurnEnvironmentState::Ready(TurnEnvironment::new(
                    TurnEnvironmentSelection {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: remote_cwd_uri,
                        workspace_roots: Vec::new(),
                        config: EnvironmentConfigState::Ready(test_environment_config()),
                    },
                    EnvironmentConfigOrigin::Thread,
                    remote_environment,
                    /*shell*/ None,
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
                    config: EnvironmentConfigState::Ready(test_environment_config()),
                },
                config_origin: EnvironmentConfigOrigin::Thread,
                resolution: futures::future::pending().boxed().shared(),
            })],
        };

        assert_eq!(snapshot.local_environment_cwd(), Some(cwd));
    }
}
