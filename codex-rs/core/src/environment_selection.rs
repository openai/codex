use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use async_channel::Receiver;
use async_channel::Sender;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentInfo;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerError;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::EnvironmentCapabilityRoots;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_skills_extension::EnvironmentSkillRequirements;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::FutureExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use futures::stream::BoxStream;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument;
use tracing::instrument::WithSubscriber;

use crate::session::ExecutorDiscovery;
use crate::session::ThreadEnvironmentDefaults;
use crate::session::turn_context::ShellSnapshotCache;
use crate::session::turn_context::ShellSnapshotTask;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::Shell;
use crate::shell_snapshot::ShellSnapshot;
use crate::shell_snapshot::SnapshotCredentialBrokerState;

mod attachment;
pub(crate) use attachment::EnvironmentAttachment;

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
/// A matching root from the current attachment refreshes its persisted location. Root IDs from
/// other environments and persisted roots absent from the attachment keep their existing location.
pub(crate) fn combine_selected_capability_roots(
    thread_roots: &[SelectedCapabilityRoot],
    sources: impl IntoIterator<Item = Vec<SelectedCapabilityRoot>>,
) -> Vec<SelectedCapabilityRoot> {
    let attachment_roots = sources.into_iter().flatten().collect::<Vec<_>>();
    let mut combined_roots = thread_roots
        .iter()
        .map(|thread_root| {
            let CapabilityRootLocation::Environment {
                environment_id: thread_environment_id,
                ..
            } = &thread_root.location;
            attachment_roots
                .iter()
                .find(|attachment_root| {
                    if attachment_root.id != thread_root.id {
                        return false;
                    }
                    let CapabilityRootLocation::Environment {
                        environment_id: attachment_environment_id,
                        ..
                    } = &attachment_root.location;
                    attachment_environment_id == thread_environment_id
                })
                .unwrap_or(thread_root)
                .clone()
        })
        .collect::<Vec<_>>();
    combined_roots.extend(attachment_roots);
    combined_roots
}

pub(crate) fn default_thread_environment_requests(
    environment_manager: &EnvironmentManager,
    cwd: &AbsolutePathBuf,
    workspace_roots: &[AbsolutePathBuf],
) -> Vec<TurnEnvironmentRequest> {
    environment_manager
        .default_environment_ids()
        .into_iter()
        .map(|environment_id| TurnEnvironmentRequest {
            environment_id,
            cwd: PathUri::from_abs_path(cwd),
            workspace_roots: workspace_roots.iter().map(PathUri::from_abs_path).collect(),
            config: EnvironmentConfigState::FromThread,
        })
        .collect()
}

/// Checks that environment IDs are registered and unique and that working directories are not too long.
pub fn validate_environment_ids_and_cwds<'a>(
    environment_manager: &EnvironmentManager,
    environments: impl IntoIterator<Item = (&'a str, &'a PathUri)>,
) -> CodexResult<()> {
    let mut environment_ids = HashSet::new();
    for (environment_id, cwd) in environments {
        if cwd.inferred_native_path_string().len() > MAX_TURN_ENVIRONMENT_CWD_BYTES {
            return Err(CodexErr::InvalidRequest(
                "turn environment working directory exceeds the maximum size".to_string(),
            ));
        }
        if !environment_ids.insert(environment_id) {
            return Err(CodexErr::InvalidRequest(format!(
                "duplicate turn environment id `{environment_id}`"
            )));
        }
        environment_manager
            .get_environment(environment_id)
            .ok_or_else(|| {
                CodexErr::InvalidRequest(format!("unknown turn environment id `{environment_id}`"))
            })?;
    }
    Ok(())
}

type TurnEnvironmentResult = Result<ResolvedEnvironment, Arc<ExecServerError>>;
type TurnEnvironmentResolution = Shared<BoxFuture<'static, TurnEnvironmentResult>>;
type PendingConfigurationResult = Result<EnvironmentConfig, String>;
pub(crate) type PendingConfiguration = Shared<BoxFuture<'static, PendingConfigurationResult>>;

fn pending_configuration(
    completion: &watch::Sender<Option<PendingConfigurationResult>>,
) -> PendingConfiguration {
    let mut receiver = completion.subscribe();
    async move {
        receiver
            .wait_for(Option::is_some)
            .await
            .ok()
            .and_then(|result| result.clone())
            .unwrap_or_else(|| Err("environment configuration was canceled".to_string()))
    }
    .boxed()
    .shared()
}

// Shared startup result used to build each turn's environment with its own config
// without restarting the connection or shell resolution.
#[derive(Clone)]
struct ResolvedEnvironment {
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

impl ResolvedEnvironment {
    fn into_turn_environment(
        self,
        mut selection: TurnEnvironmentSelection,
        config_origin: EnvironmentConfigOrigin,
        attachment: Arc<EnvironmentAttachment>,
    ) -> Result<TurnEnvironment, String> {
        if matches!(selection.config, EnvironmentConfigState::Pending) {
            let config = self
                .installed_config
                .ok_or_else(|| "Environment configuration was not supplied.".to_string())?;
            selection.config = EnvironmentConfigState::Ready(config);
        }
        let mut environment =
            TurnEnvironment::from_attachment(selection, config_origin, attachment, self.shell);
        environment.executor_platform_os = self.executor_platform_os;
        environment.shell_snapshot = self.shell_snapshot;
        environment.shell_snapshot_builder = Some(Box::new(self.shell_snapshot_builder));
        environment.shell_snapshot_cache = self.shell_snapshot_cache;
        environment.shell_snapshot_v2_supported = self.shell_snapshot_v2_supported;
        environment.user_home_dir = self.user_home_dir;
        environment.temporary_directories = self.temporary_directories;
        Ok(environment)
    }
}

#[derive(Clone)]
struct SelectedTurnEnvironment {
    selection: TurnEnvironmentSelection,
    config_origin: EnvironmentConfigOrigin,
    attachment: Arc<EnvironmentAttachment>,
    // Only selections share this guard. A captured turn must not postpone detach.
    detach_on_drop: Arc<DropGuard>,
    resolution: TurnEnvironmentResolution,
    // Executor retries keep this session's wait for its first accepted configuration.
    pending_completion: Option<watch::Sender<Option<PendingConfigurationResult>>>,
    // Inherited owner only; otherwise snapshots expose this attachment's own completion.
    owner_config_result: Option<PendingConfiguration>,
}

#[derive(Clone)]
pub(crate) struct StartingTurnEnvironment {
    pub(crate) selection: TurnEnvironmentSelection,
    config_origin: EnvironmentConfigOrigin,
    resolution: TurnEnvironmentResolution,
    attachment: Arc<EnvironmentAttachment>,
    owner_config_result: Option<PendingConfiguration>,
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
    // Pending configuration belongs to each attachment set; completed startup can be shared.
    fn for_turn(
        &self,
        attachment: Arc<EnvironmentAttachment>,
        detach_on_drop: Arc<DropGuard>,
        local_shell: &Shell,
        shell_snapshot: &ShellSnapshot,
        executor_discovery: Option<&Arc<ExecutorDiscovery>>,
    ) -> Self {
        if matches!(self.selection.config, EnvironmentConfigState::Pending) {
            return ThreadEnvironments::start_environment(
                self.selection.clone(),
                self.config_origin,
                attachment,
                detach_on_drop,
                self.owner_config_result.clone(),
                /*pending_completion*/ None,
                local_shell,
                shell_snapshot,
                executor_discovery,
            );
        }
        Self {
            attachment,
            detach_on_drop,
            ..self.clone()
        }
    }

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
    pub(crate) fn owner_configuration(&self) -> Option<PendingConfiguration> {
        if matches!(self.selection.config, EnvironmentConfigState::Pending) {
            self.owner_config_result.clone()
        } else {
            None
        }
    }

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
    executor_discovery: Option<Arc<ExecutorDiscovery>>,
    state: Mutex<ThreadEnvironmentsState>,
    // Retain configured bindings independently of active-turn membership and cancellation.
    configured: Mutex<ThreadEnvironmentsState>,
}

struct ThreadEnvironmentsState {
    thread_defaults: ThreadEnvironmentDefaults,
    environments: Vec<SelectedTurnEnvironment>,
    connection_events: Option<ConnectionEventStreams>,
}

/// Forwards the active attachments' streams to their thread's event channel.
struct ConnectionEventStreams {
    streams: Sender<BoxStream<'static, Event>>,
    _task: AbortOnDropHandle<()>,
}

impl ConnectionEventStreams {
    fn new(tx_event: Sender<Event>) -> Self {
        let (streams, incoming) = async_channel::unbounded::<BoxStream<'static, Event>>();
        let task = tokio::spawn(async move {
            let events = incoming
                .flatten_unordered(/*limit*/ None)
                .take_until(tx_event.closed());
            tokio::pin!(events);
            while let Some(event) = events.next().await {
                if tx_event.send(event).await.is_err() {
                    break;
                }
            }
        });
        Self {
            streams,
            _task: AbortOnDropHandle::new(task),
        }
    }

    fn add(&self, events: Option<Receiver<Event>>, detached: &CancellationToken) {
        if let Some(events) = events {
            // A closed receiver can still yield events queued before detach.
            let events = events.take_until(detached.clone().cancelled_owned());
            let _ = self.streams.try_send(events.boxed());
        }
    }
}

#[derive(Clone, Copy)]
enum StartupRetry<'a> {
    AfterSelectionUpdate,
    OnNewTurn(&'a [TurnEnvironmentSelection]),
}

impl StartupRetry<'_> {
    fn should_restart(self, environment: &SelectedTurnEnvironment) -> bool {
        if matches!(
            environment.selection.config,
            EnvironmentConfigState::Failed(_)
        ) {
            return matches!(self, Self::AfterSelectionUpdate);
        }
        // Avoid cloning a resolved environment on every healthy turn.
        if environment.resolution.peek().is_some_and(Result::is_ok) {
            return false;
        }
        match environment.resolution.clone().now_or_never() {
            Some(Err(error)) => match self {
                Self::AfterSelectionUpdate => true,
                Self::OnNewTurn(selected) => {
                    error.is_retryable_preparation_error()
                        && selected
                            .iter()
                            .any(|s| s.has_same_workspace(&environment.selection))
                }
            },
            Some(Ok(_)) | None => false,
        }
    }
}

impl ThreadEnvironments {
    pub(crate) fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_shell: Shell,
        thread_defaults: ThreadEnvironmentDefaults,
        shell_snapshot: ShellSnapshot,
        current: TurnEnvironmentSnapshot,
        non_blocking_snapshots: bool,
        executor_discovery: Option<Arc<ExecutorDiscovery>>,
    ) -> Self {
        if let Some(discovery) = &executor_discovery {
            discovery.ensure_capacity(current.environments.len());
        }
        let environments: Vec<_> = current
            .environments
            .into_iter()
            .filter_map(|inherited| {
                // Reuse completed startup without resolving a Pending owner's configuration.
                let inherited = if let TurnEnvironmentState::Starting(starting) = &inherited
                    && let EnvironmentConfigState::Ready(_) = &starting.selection.config
                    && let Some(Ok(resolved)) = starting.resolution.clone().now_or_never()
                {
                    TurnEnvironmentState::from_resolution(starting.clone(), Some(Ok(resolved)))
                } else {
                    inherited
                };
                let environment = match inherited {
                    TurnEnvironmentState::Ready(environment) => environment,
                    TurnEnvironmentState::Starting(inherited) => {
                        let (selection, config_origin) = resolve_selection_config(
                            inherited
                                .config_origin
                                .into_input_selection(inherited.selection.clone()),
                            &thread_defaults,
                        );
                        let (attachment, detach_on_drop, _) = EnvironmentAttachment::new(
                            &selection.environment_id,
                            Arc::clone(&inherited.attachment.environment),
                        );
                        return Some(Self::start_environment(
                            selection,
                            config_origin,
                            attachment,
                            Arc::new(detach_on_drop),
                            inherited.owner_configuration(),
                            /*pending_completion*/ None,
                            &local_shell,
                            &shell_snapshot,
                            executor_discovery.as_ref(),
                        ));
                    }
                    TurnEnvironmentState::Failed { .. } => return None,
                };
                let selection = environment.selection;
                let config_origin = environment.config_origin;
                let selected_environment = &environment.attachment.environment;
                let inherited_snapshot = if !selected_environment.is_remote()
                    && shell_snapshot.should_rebuild_inherited()
                {
                    futures::future::ready(None).boxed().shared()
                } else {
                    environment.shell_snapshot
                };
                let resolution: TurnEnvironmentResolution =
                    futures::future::ready(Ok(ResolvedEnvironment {
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
                let (attachment, detach_on_drop, _) = EnvironmentAttachment::new(
                    &selection.environment_id,
                    Arc::clone(selected_environment),
                );
                let mut inherited_environment = SelectedTurnEnvironment {
                    selection,
                    config_origin,
                    attachment,
                    detach_on_drop: Arc::new(detach_on_drop),
                    resolution,
                    pending_completion: None,
                    owner_config_result: None,
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
            executor_discovery,
            state: Mutex::new(ThreadEnvironmentsState {
                thread_defaults: thread_defaults.clone(),
                environments: Vec::new(),
                connection_events: None,
            }),
            configured: Mutex::new(ThreadEnvironmentsState {
                thread_defaults,
                environments,
                connection_events: None,
            }),
        }
    }

    /// Attach the initial active selection after `SessionConfigured` so clients see only later
    /// connection changes. Startup discovery uses the configured bindings until this point.
    pub(crate) fn activate_configured(&self, tx_event: Sender<Event>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let configured = self
            .configured
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let connection_events = ConnectionEventStreams::new(tx_event);
        state.environments = configured
            .environments
            .iter()
            .map(|environment| {
                let (attachment, detach_on_drop, events) = EnvironmentAttachment::new(
                    &environment.selection.environment_id,
                    Arc::clone(&environment.attachment.environment),
                );
                connection_events.add(events, &attachment.detached);
                environment.for_turn(
                    attachment,
                    Arc::new(detach_on_drop),
                    &self.local_shell,
                    &self.shell_snapshot,
                    self.executor_discovery.as_ref(),
                )
            })
            .collect();
        state.connection_events = Some(connection_events);
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
        self.apply_selections(environments, StartupRetry::AfterSelectionUpdate);
    }

    /// A new turn can retry a failed connection even when it keeps the same workspace.
    pub(crate) fn activate_selections(&self, environments: &[TurnEnvironmentSelection]) {
        let retry = StartupRetry::OnNewTurn(environments);
        if environments != self.selections() {
            self.update_selections(environments);
        } else if self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .environments
            .iter()
            .any(|environment| retry.should_restart(environment))
        {
            self.apply_selections(environments, retry);
        }
    }

    fn apply_selections(&self, environments: &[TurnEnvironmentSelection], retry: StartupRetry<'_>) {
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let configured = self
                .configured
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            self.update_selections_with_bindings(
                &mut state,
                environments,
                &configured.environments,
                retry,
            );
        }
        // Retry configured bindings with their own settings, keeping turn-only changes temporary.
        self.apply_configured_selections(&self.configured_snapshot_now().all_selections(), retry);
    }

    /// Retains configured bindings without changing the active turn's attachments.
    pub(crate) fn update_configured_selections(&self, environments: &[TurnEnvironmentSelection]) {
        self.apply_configured_selections(environments, StartupRetry::AfterSelectionUpdate);
    }

    fn apply_configured_selections(
        &self,
        environments: &[TurnEnvironmentSelection],
        retry: StartupRetry<'_>,
    ) {
        let mut configured = self
            .configured
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.update_selections_with_bindings(&mut configured, environments, &[], retry);
    }

    fn update_selections_with_bindings(
        &self,
        state: &mut ThreadEnvironmentsState,
        environments: &[TurnEnvironmentSelection],
        bindings: &[SelectedTurnEnvironment],
        retry: StartupRetry<'_>,
    ) {
        if let Some(discovery) = &self.executor_discovery {
            discovery.ensure_capacity(environments.len());
        }
        let ThreadEnvironmentsState {
            thread_defaults,
            environments: current,
            connection_events,
        } = state;
        let connection_events = connection_events.as_ref();
        let mut seen_environment_ids = HashSet::with_capacity(environments.len());
        let mut next = Vec::with_capacity(environments.len());
        let mut pending_completions = Vec::new();
        // Workspace and configuration changes preserve the attachment to the same executor.
        let attachment_for =
            |environment_id: &str,
             environment: &Arc<Environment>,
             connection_events: Option<&ConnectionEventStreams>| {
                current
                    .iter()
                    .find(|previous| {
                        previous.selection.environment_id == environment_id
                            && Arc::ptr_eq(&previous.attachment.environment, environment)
                    })
                    .map(|previous| {
                        (
                            Arc::clone(&previous.attachment),
                            Arc::clone(&previous.detach_on_drop),
                        )
                    })
                    .unwrap_or_else(|| {
                        let (attachment, guard, events) =
                            EnvironmentAttachment::new(environment_id, Arc::clone(environment));
                        if let Some(connection_events) = connection_events {
                            connection_events.add(events, &attachment.detached);
                        }
                        (attachment, Arc::new(guard))
                    })
            };
        for selected_environment in environments {
            if !seen_environment_ids.insert(selected_environment.environment_id.as_str()) {
                continue;
            }
            let current_environment = current.iter().find(|environment| {
                environment
                    .selection
                    .has_same_workspace(selected_environment)
            });
            let previous = current_environment.or_else(|| {
                bindings.iter().find(|environment| {
                    environment
                        .selection
                        .has_same_workspace(selected_environment)
                })
            });
            let (selected_environment, config_origin) =
                resolve_selection_config(selected_environment.clone(), thread_defaults);
            if let Some(environment) = previous {
                let failed = retry.should_restart(environment);
                let restarting_as_pending =
                    matches!(selected_environment.config, EnvironmentConfigState::Pending)
                        && !matches!(
                            environment.selection.config,
                            EnvironmentConfigState::Pending
                        );
                let reuse_startup = !failed && !restarting_as_pending;
                let mut environment = if current_environment.is_some() {
                    environment.clone()
                } else {
                    let (attachment, detach_on_drop) = attachment_for(
                        &selected_environment.environment_id,
                        &environment.attachment.environment,
                        connection_events.filter(|_| reuse_startup),
                    );
                    environment.for_turn(
                        attachment,
                        detach_on_drop,
                        &self.local_shell,
                        &self.shell_snapshot,
                        self.executor_discovery.as_ref(),
                    )
                };
                if let Some(completion) = environment.pending_completion.take() {
                    match &selected_environment.config {
                        EnvironmentConfigState::Ready(config) => {
                            pending_completions.push((completion, Ok(config.clone())));
                        }
                        EnvironmentConfigState::Failed(error) => {
                            pending_completions.push((completion, Err(error.clone())));
                        }
                        EnvironmentConfigState::FromThread | EnvironmentConfigState::Pending => {
                            environment.pending_completion = Some(completion);
                        }
                    }
                }
                if reuse_startup {
                    let shell_settings_changed = matches!(
                        (&environment.selection.config, &selected_environment.config),
                        (EnvironmentConfigState::Ready(previous), EnvironmentConfigState::Ready(current))
                            if previous.allow_login_shell != current.allow_login_shell
                                || previous.shell_environment_policy != current.shell_environment_policy
                    );
                    environment.selection = selected_environment;
                    environment.config_origin = config_origin;
                    if shell_settings_changed
                        && !environment.attachment.environment.is_remote()
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
            let (attachment, detach_on_drop) =
                attachment_for(environment_id, &environment, connection_events);
            let pending = previous.filter(|previous| {
                matches!(selected_environment.config, EnvironmentConfigState::Pending)
                    && previous.pending_completion.is_some()
            });
            let selected = Self::start_environment(
                selected_environment,
                config_origin,
                attachment,
                detach_on_drop,
                pending.and_then(|previous| previous.owner_config_result.clone()),
                current_environment.and_then(|previous| previous.pending_completion.clone()),
                &self.local_shell,
                &self.shell_snapshot,
                self.executor_discovery.as_ref(),
            );
            next.push(selected);
        }
        // Install the new selection before dropping the guards for removed attachments.
        drop(std::mem::replace(current, next));

        // Publish after installing the new list, including pending bindings replaced by retries.
        for (completion, result) in pending_completions {
            completion.send_if_modified(|current| {
                if current.is_some() {
                    return false;
                }
                *current = Some(result);
                true
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start_environment(
        selection: TurnEnvironmentSelection,
        config_origin: EnvironmentConfigOrigin,
        attachment: Arc<EnvironmentAttachment>,
        detach_on_drop: Arc<DropGuard>,
        owner_config_result: Option<PendingConfiguration>,
        pending_completion: Option<watch::Sender<Option<PendingConfigurationResult>>>,
        local_shell: &Shell,
        shell_snapshot: &ShellSnapshot,
        executor_discovery: Option<&Arc<ExecutorDiscovery>>,
    ) -> SelectedTurnEnvironment {
        let (pending_completion, configuration_ready) =
            if matches!(selection.config, EnvironmentConfigState::Pending) {
                let sender =
                    pending_completion.unwrap_or_else(|| watch::channel(/*init*/ None).0);
                let result = pending_configuration(&sender);
                (Some(sender), Some(result))
            } else {
                (None, None)
            };
        let executor_discovery = executor_discovery.filter(|_| attachment.environment.is_remote());
        // Deferred turns can sample while this attachment is preparing its inventory.
        let (task, resolution) = {
            let selection = selection.clone();
            let attachment = Arc::clone(&attachment);
            let local_shell = local_shell.clone();
            let shell_snapshot = shell_snapshot.clone();
            let executor_discovery = executor_discovery.cloned();
            async move {
                let resolved = Self::resolve_environment(
                    selection.clone(),
                    Arc::clone(&attachment.environment),
                    local_shell,
                    shell_snapshot,
                    configuration_ready,
                )
                .await?;
                if let Some(discovery) = executor_discovery {
                    let selected = resolved
                        .clone()
                        .into_turn_environment(selection, config_origin, Arc::clone(&attachment))
                        .map_err(|error| Arc::new(ExecServerError::Protocol(error)))?;
                    discovery.prewarm(&selected).await;
                }
                Ok(resolved)
            }
            .remote_handle()
        };
        drop(tokio::spawn(
            task.in_current_span().with_current_subscriber(),
        ));
        let resolution = resolution.boxed().shared();
        SelectedTurnEnvironment {
            selection,
            config_origin,
            attachment,
            detach_on_drop,
            resolution,
            pending_completion,
            owner_config_result,
        }
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
        self.set_thread_defaults(&self.state, defaults);
    }

    pub(crate) fn set_configured_thread_defaults(&self, defaults: EnvironmentConfig) {
        self.set_thread_defaults(&self.configured, defaults);
    }

    fn set_thread_defaults(
        &self,
        state: &Mutex<ThreadEnvironmentsState>,
        defaults: EnvironmentConfig,
    ) {
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        let ThreadEnvironmentsState {
            thread_defaults,
            environments,
            ..
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
                && !environment.attachment.environment.is_remote()
                && self.shell_snapshot.should_rebuild_inherited()
                && let Some(Ok(resolved)) = environment.resolution.clone().now_or_never()
            {
                self.restart_shell_snapshot(environment, resolved);
            }
        }
    }

    pub(crate) fn set_snapshot_credential_broker(&self, broker: SnapshotCredentialBrokerState) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut configured = self
            .configured
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !self.shell_snapshot.set_credential_broker(broker) {
            return;
        }

        for selected in state
            .environments
            .iter_mut()
            .chain(&mut configured.environments)
        {
            if !selected.attachment.environment.is_remote()
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
            Arc::clone(&selected.attachment.environment),
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
                Some(
                    environment
                        .config_origin
                        .selected_capability_roots(&environment.attachment.environment, config),
                )
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
        configuration_ready: Option<PendingConfiguration>,
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
        // Wait for this thread to accept the configuration, if pending.
        let configuration_ready = async move {
            let Some(configuration_ready) = configuration_ready else {
                return Ok(None);
            };
            configuration_ready
                .await
                .map(Some)
                .map_err(|error| Arc::new(ExecServerError::Protocol(error)))
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
        Self::snapshot_state(&self.state).refresh_readiness()
    }

    /// Captures configured bindings without resolving readiness or losing config ownership.
    /// Inheritance and eviction retain these bindings; execution and validation resolve a view.
    pub(crate) fn configured_snapshot_now(&self) -> TurnEnvironmentSnapshot {
        Self::snapshot_state(&self.configured)
    }

    fn snapshot_state(state: &Mutex<ThreadEnvironmentsState>) -> TurnEnvironmentSnapshot {
        // Keep the startup results, but not the senders that would keep removed Pending work alive.
        let environments = state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .environments
            .iter()
            .map(|environment| {
                TurnEnvironmentState::Starting(StartingTurnEnvironment {
                    selection: environment.selection.clone(),
                    config_origin: environment.config_origin,
                    resolution: environment.resolution.clone(),
                    attachment: Arc::clone(&environment.attachment),
                    owner_config_result: environment.owner_config_result.clone().or_else(|| {
                        environment
                            .pending_completion
                            .as_ref()
                            .map(pending_configuration)
                    }),
                })
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
    pub(crate) fn resolve_snapshot(
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
        required_skills: Vec<String>,
        error: String,
    },
}

impl TurnEnvironmentState {
    fn from_resolution(
        starting: StartingTurnEnvironment,
        resolved: Option<TurnEnvironmentResult>,
    ) -> Self {
        let required_skills = starting.attachment.environment.required_skills().to_vec();
        if let EnvironmentConfigState::Failed(error) = &starting.selection.config {
            let error = error.clone();
            return Self::Failed {
                required_skills,
                selection: starting
                    .config_origin
                    .into_input_selection(starting.selection),
                error,
            };
        }
        match resolved {
            Some(Ok(environment)) => match environment.into_turn_environment(
                starting.selection.clone(),
                starting.config_origin,
                starting.attachment,
            ) {
                Ok(environment) => Self::Ready(environment),
                Err(error) => Self::Failed {
                    required_skills,
                    selection: starting
                        .config_origin
                        .into_input_selection(starting.selection),
                    error,
                },
            },
            Some(Err(err)) => {
                tracing::debug!(
                    environment_id = %starting.selection.environment_id,
                    "skipping failed turn environment: {err}"
                );
                Self::Failed {
                    required_skills,
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
    /// Keeps the original root order for child threads and saved history.
    pub(crate) fn selected_capability_roots(&self) -> Vec<SelectedCapabilityRoot> {
        EnvironmentCapabilityRoots::collect(self.environments.iter().map(|environment| {
            let selection = match environment {
                TurnEnvironmentState::Ready(environment) => &environment.selection,
                TurnEnvironmentState::Starting(environment) => &environment.selection,
                TurnEnvironmentState::Failed { selection, .. } => selection,
            };
            &selection.selected_capability_roots
        }))
    }

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
                    Arc::clone(environment.environment()),
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
                    Some(Arc::clone(environment.environment())),
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
            .find(|environment| !environment.environment().is_remote())
    }

    pub(crate) fn local_environment_cwd(&self) -> Option<AbsolutePathBuf> {
        self.local_selection()
            .and_then(|selection| selection.cwd.to_abs_path().ok())
    }

    /// The local workspace remains host-owned while its executor is still starting.
    pub(crate) fn local_selection(&self) -> Option<&TurnEnvironmentSelection> {
        self.environments
            .iter()
            .find_map(|environment| match environment {
                TurnEnvironmentState::Ready(environment)
                    if !environment.environment().is_remote() =>
                {
                    Some(&environment.selection)
                }
                TurnEnvironmentState::Ready(_) => None,
                TurnEnvironmentState::Starting(environment)
                    if environment.selection.environment_id
                        == codex_exec_server::LOCAL_ENVIRONMENT_ID =>
                {
                    Some(&environment.selection)
                }
                TurnEnvironmentState::Starting(_) | TurnEnvironmentState::Failed { .. } => None,
            })
    }

    #[cfg(test)]
    pub(crate) fn primary_environment(&self) -> Option<Arc<codex_exec_server::Environment>> {
        self.primary()
            .map(|environment| Arc::clone(environment.environment()))
    }

    pub(crate) fn to_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.turn_environments()
            .map(TurnEnvironment::selection)
            .collect()
    }

    /// Captures requirements from the selected registration even when connection setup fails.
    pub(crate) fn required_skills(&self) -> impl Iterator<Item = EnvironmentSkillRequirements<'_>> {
        self.environments.iter().filter_map(|environment| {
            let (selection, required) = match environment {
                TurnEnvironmentState::Ready(environment) => (
                    &environment.selection,
                    environment.environment().required_skills(),
                ),
                TurnEnvironmentState::Starting(environment) => (
                    &environment.selection,
                    environment.attachment.environment.required_skills(),
                ),
                TurnEnvironmentState::Failed {
                    selection,
                    required_skills,
                    ..
                } => (selection, required_skills.as_slice()),
            };
            if required.is_empty() {
                None
            } else {
                Some(EnvironmentSkillRequirements {
                    environment_id: &selection.environment_id,
                    skill_names: required,
                })
            }
        })
    }

    /// Requests for native children retain ready and starting attachments from their spawning step.
    pub(crate) fn inheritable_requests(&self) -> Vec<TurnEnvironmentRequest> {
        self.environments
            .iter()
            .filter_map(|environment| match environment {
                TurnEnvironmentState::Ready(environment) => Some(environment.selection()),
                TurnEnvironmentState::Starting(environment) => Some(
                    environment
                        .config_origin
                        .into_input_selection(environment.selection.clone()),
                ),
                TurnEnvironmentState::Failed { .. } => None,
            })
            .map(TurnEnvironmentSelection::into_request)
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

    pub(crate) fn single_local_environment(&self) -> Option<&TurnEnvironment> {
        if self.starting().next().is_some() {
            return None;
        }
        let mut environments = self.turn_environments();
        let environment = environments.next()?;
        if environments.next().is_some() {
            return None;
        }

        (!environment.environment().is_remote()).then_some(environment)
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
#[path = "environment_connection_tests.rs"]
mod connection_tests;

#[cfg(test)]
#[path = "environment_selection_tests.rs"]
mod tests;
