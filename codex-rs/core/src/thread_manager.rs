use crate::SkillsManager;
use crate::agent::AgentControl;
use crate::codex::Codex;
use crate::codex::CodexSpawnArgs;
use crate::codex::CodexSpawnOk;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex_thread::CodexThread;
use crate::config::Config;
use crate::file_watcher::FileWatcher;
use crate::mcp::McpManager;
use crate::plugins::PluginsManager;
use crate::rollout::RolloutRecorder;
use crate::rollout::truncation;
use crate::shell_snapshot::ShellSnapshot;
use crate::skills_watcher::SkillsWatcher;
use crate::skills_watcher::SkillsWatcherEvent;
use crate::tasks::interrupted_turn_history_marker;
use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::TurnStatus;
use codex_exec_server::EnvironmentManager;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
#[cfg(test)]
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::W3cTraceContext;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::runtime::RuntimeFlavor;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tracing::warn;

const THREAD_CREATED_CHANNEL_CAPACITY: usize = 1024;
/// Test-only override for enabling thread-manager behaviors used by integration
/// tests.
///
/// In production builds this value should remain at its default (`false`) and
/// must not be toggled.
static FORCE_TEST_THREAD_MANAGER_BEHAVIOR: AtomicBool = AtomicBool::new(false);

type CapturedOps = Vec<(ThreadId, Op)>;
type SharedCapturedOps = Arc<std::sync::Mutex<CapturedOps>>;

pub(crate) fn set_thread_manager_test_mode_for_tests(enabled: bool) {
    FORCE_TEST_THREAD_MANAGER_BEHAVIOR.store(enabled, Ordering::Relaxed);
}

fn should_use_test_thread_manager_behavior() -> bool {
    FORCE_TEST_THREAD_MANAGER_BEHAVIOR.load(Ordering::Relaxed)
}

struct TempCodexHomeGuard {
    path: PathBuf,
}

impl Drop for TempCodexHomeGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn build_skills_watcher(skills_manager: Arc<SkillsManager>) -> Arc<SkillsWatcher> {
    if should_use_test_thread_manager_behavior()
        && let Ok(handle) = Handle::try_current()
        && handle.runtime_flavor() == RuntimeFlavor::CurrentThread
    {
        // The real watcher spins background tasks that can starve the
        // current-thread test runtime and cause event waits to time out.
        warn!("using noop skills watcher under current-thread test runtime");
        return Arc::new(SkillsWatcher::noop());
    }

    let file_watcher = match FileWatcher::new() {
        Ok(file_watcher) => Arc::new(file_watcher),
        Err(err) => {
            warn!("failed to initialize file watcher: {err}");
            Arc::new(FileWatcher::noop())
        }
    };
    let skills_watcher = Arc::new(SkillsWatcher::new(&file_watcher));

    let mut rx = skills_watcher.subscribe();
    let skills_manager = Arc::clone(&skills_manager);
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(SkillsWatcherEvent::SkillsChanged { .. }) => {
                        skills_manager.clear_cache();
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    } else {
        warn!("skills watcher listener skipped: no Tokio runtime available");
    }

    skills_watcher
}

/// Represents a newly created Codex thread (formerly called a conversation), including the first event
/// (which is [`EventMsg::SessionConfigured`]).
pub struct NewThread {
    pub thread_id: ThreadId,
    pub thread: Arc<CodexThread>,
    pub session_configured: SessionConfiguredEvent,
}

// TODO(ccunningham): Add an explicit non-interrupting live-turn snapshot once
// core can represent sampling boundaries directly instead of relying on
// whichever items happened to be persisted mid-turn.
//
// Two likely future variants:
// - `TruncateToLastSamplingBoundary` for callers that want a coherent fork from
//   the last stable model boundary without synthesizing an interrupt.
// - `WaitUntilNextSamplingBoundary` (or similar) for callers that prefer to
//   fork after the next sampling boundary rather than interrupting immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkSnapshot {
    /// Fork a committed prefix ending strictly before the nth user message.
    ///
    /// When `n` is within range, this cuts before that 0-based user-message
    /// boundary. When `n` is out of range and the source thread is currently
    /// mid-turn, this instead cuts before the active turn's opening boundary
    /// so the fork drops the unfinished turn suffix. When `n` is out of range
    /// and the source thread is already at a turn boundary, this returns the
    /// full committed history unchanged.
    TruncateBeforeNthUserMessage(usize),

    /// Fork the current persisted history as if the source thread had been
    /// interrupted now.
    ///
    /// If the persisted snapshot ends mid-turn, this appends the same
    /// `<turn_aborted>` marker produced by a real interrupt. If the snapshot is
    /// already at a turn boundary, this returns the current persisted history
    /// unchanged.
    Interrupted,
}

/// Preserve legacy `fork_thread(usize, ...)` callsites by mapping them to the
/// existing truncate-before-nth-user-message snapshot mode.
impl From<usize> for ForkSnapshot {
    fn from(value: usize) -> Self {
        Self::TruncateBeforeNthUserMessage(value)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ThreadShutdownReport {
    pub completed: Vec<ThreadId>,
    pub submit_failed: Vec<ThreadId>,
    pub timed_out: Vec<ThreadId>,
}

enum ShutdownOutcome {
    Complete,
    SubmitFailed,
    TimedOut,
}

/// [`ThreadManager`] is responsible for creating threads and maintaining
/// them in memory.
pub struct ThreadManager {
    state: Arc<ThreadManagerState>,
    _test_codex_home_guard: Option<TempCodexHomeGuard>,
}

/// Shared, `Arc`-owned state for [`ThreadManager`]. This `Arc` is required to have a single
/// `Arc` reference that can be downgraded to by `AgentControl` while preventing every single
/// function to require an `Arc<&Self>`.
pub(crate) struct ThreadManagerState {
    threads: Arc<RwLock<HashMap<ThreadId, Arc<CodexThread>>>>,
    thread_created_tx: broadcast::Sender<ThreadId>,
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    environment_manager: Arc<EnvironmentManager>,
    skills_manager: Arc<SkillsManager>,
    plugins_manager: Arc<PluginsManager>,
    mcp_manager: Arc<McpManager>,
    skills_watcher: Arc<SkillsWatcher>,
    session_source: SessionSource,
    analytics_events_client: Option<AnalyticsEventsClient>,
    // Captures submitted ops for testing purpose when test mode is enabled.
    ops_log: Option<SharedCapturedOps>,
}

impl ThreadManager {
    pub fn new(
        config: &Config,
        auth_manager: Arc<AuthManager>,
        session_source: SessionSource,
        collaboration_modes_config: CollaborationModesConfig,
        environment_manager: Arc<EnvironmentManager>,
        analytics_events_client: Option<AnalyticsEventsClient>,
    ) -> Self {
        let codex_home = config.codex_home.clone();
        let restriction_product = session_source.restriction_product();
        let openai_models_provider = config
            .model_providers
            .get(OPENAI_PROVIDER_ID)
            .cloned()
            .unwrap_or_else(|| ModelProviderInfo::create_openai_provider(/*base_url*/ None));
        let (thread_created_tx, _) = broadcast::channel(THREAD_CREATED_CHANNEL_CAPACITY);
        let plugins_manager = Arc::new(PluginsManager::new_with_restriction_product(
            codex_home.to_path_buf(),
            restriction_product,
        ));
        let mcp_manager = Arc::new(McpManager::new(Arc::clone(&plugins_manager)));
        let skills_manager = Arc::new(SkillsManager::new_with_restriction_product(
            codex_home.clone(),
            config.bundled_skills_enabled(),
            restriction_product,
        ));
        let skills_watcher = build_skills_watcher(Arc::clone(&skills_manager));
        Self {
            state: Arc::new(ThreadManagerState {
                threads: Arc::new(RwLock::new(HashMap::new())),
                thread_created_tx,
                models_manager: Arc::new(ModelsManager::new_with_provider(
                    codex_home.to_path_buf(),
                    auth_manager.clone(),
                    config.model_catalog.clone(),
                    collaboration_modes_config,
                    openai_models_provider,
                )),
                environment_manager,
                skills_manager,
                plugins_manager,
                mcp_manager,
                skills_watcher,
                auth_manager,
                session_source,
                analytics_events_client,
                ops_log: should_use_test_thread_manager_behavior()
                    .then(|| Arc::new(std::sync::Mutex::new(Vec::new()))),
            }),
            _test_codex_home_guard: None,
        }
    }

    /// Construct with a dummy AuthManager containing the provided CodexAuth.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub(crate) fn with_models_provider_for_tests(
        auth: CodexAuth,
        provider: ModelProviderInfo,
    ) -> Self {
        set_thread_manager_test_mode_for_tests(/*enabled*/ true);
        let codex_home = std::env::temp_dir().join(format!(
            "codex-thread-manager-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&codex_home)
            .unwrap_or_else(|err| panic!("temp codex home dir create failed: {err}"));
        let mut manager = Self::with_models_provider_and_home_for_tests(
            auth,
            provider,
            codex_home.clone(),
            Arc::new(EnvironmentManager::new(/*exec_server_url*/ None)),
        );
        manager._test_codex_home_guard = Some(TempCodexHomeGuard { path: codex_home });
        manager
    }

    /// Construct with a dummy AuthManager containing the provided CodexAuth and codex home.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub(crate) fn with_models_provider_and_home_for_tests(
        auth: CodexAuth,
        provider: ModelProviderInfo,
        codex_home: PathBuf,
        environment_manager: Arc<EnvironmentManager>,
    ) -> Self {
        set_thread_manager_test_mode_for_tests(/*enabled*/ true);
        let auth_manager = AuthManager::from_auth_for_testing(auth);
        let skills_codex_home = match AbsolutePathBuf::from_absolute_path_checked(&codex_home) {
            Ok(codex_home) => codex_home,
            Err(err) => panic!("test codex_home should be absolute: {err}"),
        };
        let (thread_created_tx, _) = broadcast::channel(THREAD_CREATED_CHANNEL_CAPACITY);
        let restriction_product = SessionSource::Exec.restriction_product();
        let plugins_manager = Arc::new(PluginsManager::new_with_restriction_product(
            codex_home.clone(),
            restriction_product,
        ));
        let mcp_manager = Arc::new(McpManager::new(Arc::clone(&plugins_manager)));
        let skills_manager = Arc::new(SkillsManager::new_with_restriction_product(
            skills_codex_home,
            /*bundled_skills_enabled*/ true,
            restriction_product,
        ));
        let skills_watcher = build_skills_watcher(Arc::clone(&skills_manager));
        Self {
            state: Arc::new(ThreadManagerState {
                threads: Arc::new(RwLock::new(HashMap::new())),
                thread_created_tx,
                models_manager: Arc::new(ModelsManager::with_provider_for_tests(
                    codex_home,
                    auth_manager.clone(),
                    provider,
                )),
                environment_manager,
                skills_manager,
                plugins_manager,
                mcp_manager,
                skills_watcher,
                auth_manager,
                session_source: SessionSource::Exec,
                analytics_events_client: None,
                ops_log: should_use_test_thread_manager_behavior()
                    .then(|| Arc::new(std::sync::Mutex::new(Vec::new()))),
            }),
            _test_codex_home_guard: None,
        }
    }

    pub fn session_source(&self) -> SessionSource {
        self.state.session_source.clone()
    }

    pub fn auth_manager(&self) -> Arc<AuthManager> {
        self.state.auth_manager.clone()
    }

    pub fn skills_manager(&self) -> Arc<SkillsManager> {
        self.state.skills_manager.clone()
    }

    pub fn plugins_manager(&self) -> Arc<PluginsManager> {
        self.state.plugins_manager.clone()
    }

    pub fn mcp_manager(&self) -> Arc<McpManager> {
        self.state.mcp_manager.clone()
    }

    pub fn environment_manager(&self) -> Arc<EnvironmentManager> {
        self.state.environment_manager.clone()
    }

    pub fn get_models_manager(&self) -> Arc<ModelsManager> {
        self.state.models_manager.clone()
    }

    pub async fn list_models(&self, refresh_strategy: RefreshStrategy) -> Vec<ModelPreset> {
        self.state
            .models_manager
            .list_models(refresh_strategy)
            .await
    }

    pub fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.state.models_manager.list_collaboration_modes()
    }

    pub async fn list_thread_ids(&self) -> Vec<ThreadId> {
        self.state.list_thread_ids().await
    }

    pub async fn refresh_mcp_servers(&self, refresh_config: McpServerRefreshConfig) {
        let threads = self
            .state
            .threads
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for thread in threads {
            if let Err(err) = thread
                .submit(Op::RefreshMcpServers {
                    config: refresh_config.clone(),
                })
                .await
            {
                warn!("failed to request MCP server refresh: {err}");
            }
        }
    }

    pub fn subscribe_thread_created(&self) -> broadcast::Receiver<ThreadId> {
        self.state.thread_created_tx.subscribe()
    }

    pub async fn get_thread(&self, thread_id: ThreadId) -> CodexResult<Arc<CodexThread>> {
        self.state.get_thread(thread_id).await
    }

    /// List `thread_id` plus all known descendants in its spawn subtree.
    pub async fn list_agent_subtree_thread_ids(
        &self,
        thread_id: ThreadId,
    ) -> CodexResult<Vec<ThreadId>> {
        let thread = self.state.get_thread(thread_id).await?;

        let mut subtree_thread_ids = Vec::new();
        let mut seen_thread_ids = HashSet::new();
        subtree_thread_ids.push(thread_id);
        seen_thread_ids.insert(thread_id);

        if let Some(state_db_ctx) = thread.state_db() {
            for status in [
                DirectionalThreadSpawnEdgeStatus::Open,
                DirectionalThreadSpawnEdgeStatus::Closed,
            ] {
                for descendant_id in state_db_ctx
                    .list_thread_spawn_descendants_with_status(thread_id, status)
                    .await
                    .map_err(|err| {
                        CodexErr::Fatal(format!("failed to load thread-spawn descendants: {err}"))
                    })?
                {
                    if seen_thread_ids.insert(descendant_id) {
                        subtree_thread_ids.push(descendant_id);
                    }
                }
            }
        }

        for descendant_id in thread
            .codex
            .session
            .services
            .agent_control
            .list_live_agent_subtree_thread_ids(thread_id)
            .await?
        {
            if seen_thread_ids.insert(descendant_id) {
                subtree_thread_ids.push(descendant_id);
            }
        }

        Ok(subtree_thread_ids)
    }

    pub async fn start_thread(&self, config: Config) -> CodexResult<NewThread> {
        // Box delegated thread-spawn futures so these convenience wrappers do
        // not inline the full spawn path into every caller's async state.
        Box::pin(self.start_thread_with_tools(
            config,
            Vec::new(),
            /*persist_extended_history*/ false,
        ))
        .await
    }

    pub async fn start_thread_with_tools(
        &self,
        config: Config,
        dynamic_tools: Vec<codex_protocol::dynamic_tools::DynamicToolSpec>,
        persist_extended_history: bool,
    ) -> CodexResult<NewThread> {
        Box::pin(self.start_thread_with_tools_and_service_name(
            config,
            InitialHistory::New,
            dynamic_tools,
            persist_extended_history,
            /*metrics_service_name*/ None,
            /*parent_trace*/ None,
        ))
        .await
    }

    pub async fn start_thread_with_tools_and_service_name(
        &self,
        config: Config,
        initial_history: InitialHistory,
        dynamic_tools: Vec<codex_protocol::dynamic_tools::DynamicToolSpec>,
        persist_extended_history: bool,
        metrics_service_name: Option<String>,
        parent_trace: Option<W3cTraceContext>,
    ) -> CodexResult<NewThread> {
        Box::pin(self.state.spawn_thread(
            config,
            initial_history,
            Arc::clone(&self.state.auth_manager),
            self.agent_control(),
            dynamic_tools,
            persist_extended_history,
            metrics_service_name,
            parent_trace,
            /*user_shell_override*/ None,
        ))
        .await
    }

    pub async fn resume_thread_from_rollout(
        &self,
        config: Config,
        rollout_path: PathBuf,
        auth_manager: Arc<AuthManager>,
        parent_trace: Option<W3cTraceContext>,
    ) -> CodexResult<NewThread> {
        let initial_history = RolloutRecorder::get_rollout_history(&rollout_path).await?;
        Box::pin(self.resume_thread_with_history(
            config,
            initial_history,
            auth_manager,
            /*persist_extended_history*/ false,
            parent_trace,
        ))
        .await
    }

    pub async fn resume_thread_with_history(
        &self,
        config: Config,
        initial_history: InitialHistory,
        auth_manager: Arc<AuthManager>,
        persist_extended_history: bool,
        parent_trace: Option<W3cTraceContext>,
    ) -> CodexResult<NewThread> {
        Box::pin(self.state.spawn_thread(
            config,
            initial_history,
            auth_manager,
            self.agent_control(),
            Vec::new(),
            persist_extended_history,
            /*metrics_service_name*/ None,
            parent_trace,
            /*user_shell_override*/ None,
        ))
        .await
    }

    pub(crate) async fn start_thread_with_user_shell_override_for_tests(
        &self,
        config: Config,
        user_shell_override: crate::shell::Shell,
    ) -> CodexResult<NewThread> {
        Box::pin(self.state.spawn_thread(
            config,
            InitialHistory::New,
            Arc::clone(&self.state.auth_manager),
            self.agent_control(),
            Vec::new(),
            /*persist_extended_history*/ false,
            /*metrics_service_name*/ None,
            /*parent_trace*/ None,
            /*user_shell_override*/ Some(user_shell_override),
        ))
        .await
    }

    pub(crate) async fn resume_thread_from_rollout_with_user_shell_override_for_tests(
        &self,
        config: Config,
        rollout_path: PathBuf,
        auth_manager: Arc<AuthManager>,
        user_shell_override: crate::shell::Shell,
    ) -> CodexResult<NewThread> {
        let initial_history = RolloutRecorder::get_rollout_history(&rollout_path).await?;
        Box::pin(self.state.spawn_thread(
            config,
            initial_history,
            auth_manager,
            self.agent_control(),
            Vec::new(),
            /*persist_extended_history*/ false,
            /*metrics_service_name*/ None,
            /*parent_trace*/ None,
            /*user_shell_override*/ Some(user_shell_override),
        ))
        .await
    }

    /// Removes the thread from the manager's internal map, though the thread is stored
    /// as `Arc<CodexThread>`, it is possible that other references to it exist elsewhere.
    /// Returns the thread if the thread was found and removed.
    pub async fn remove_thread(&self, thread_id: &ThreadId) -> Option<Arc<CodexThread>> {
        self.state.threads.write().await.remove(thread_id)
    }

    /// Tries to shut down all tracked threads concurrently within the provided timeout.
    /// Threads that complete shutdown are removed from the manager; incomplete shutdowns
    /// remain tracked so callers can retry or inspect them later.
    pub async fn shutdown_all_threads_bounded(&self, timeout: Duration) -> ThreadShutdownReport {
        let threads = {
            let threads = self.state.threads.read().await;
            threads
                .iter()
                .map(|(thread_id, thread)| (*thread_id, Arc::clone(thread)))
                .collect::<Vec<_>>()
        };

        let mut shutdowns = threads
            .into_iter()
            .map(|(thread_id, thread)| async move {
                let outcome = match tokio::time::timeout(timeout, thread.shutdown_and_wait()).await
                {
                    Ok(Ok(())) => ShutdownOutcome::Complete,
                    Ok(Err(_)) => ShutdownOutcome::SubmitFailed,
                    Err(_) => ShutdownOutcome::TimedOut,
                };
                (thread_id, outcome)
            })
            .collect::<FuturesUnordered<_>>();
        let mut report = ThreadShutdownReport::default();

        while let Some((thread_id, outcome)) = shutdowns.next().await {
            match outcome {
                ShutdownOutcome::Complete => report.completed.push(thread_id),
                ShutdownOutcome::SubmitFailed => report.submit_failed.push(thread_id),
                ShutdownOutcome::TimedOut => report.timed_out.push(thread_id),
            }
        }

        let mut tracked_threads = self.state.threads.write().await;
        for thread_id in &report.completed {
            tracked_threads.remove(thread_id);
        }

        report
            .completed
            .sort_by_key(std::string::ToString::to_string);
        report
            .submit_failed
            .sort_by_key(std::string::ToString::to_string);
        report
            .timed_out
            .sort_by_key(std::string::ToString::to_string);
        report
    }

    /// Fork an existing thread by snapshotting rollout history according to
    /// `snapshot` and starting a new thread with identical configuration
    /// (unless overridden by the caller's `config`). The new thread will have
    /// a fresh id.
    pub async fn fork_thread<S>(
        &self,
        snapshot: S,
        config: Config,
        path: PathBuf,
        persist_extended_history: bool,
        parent_trace: Option<W3cTraceContext>,
    ) -> CodexResult<NewThread>
    where
        S: Into<ForkSnapshot>,
    {
        let snapshot = snapshot.into();
        let history = RolloutRecorder::get_rollout_history(&path).await?;
        let snapshot_state = snapshot_turn_state(&history);
        let history = match snapshot {
            ForkSnapshot::TruncateBeforeNthUserMessage(nth_user_message) => {
                truncate_before_nth_user_message(history, nth_user_message, &snapshot_state)
            }
            ForkSnapshot::Interrupted => {
                let history = match history {
                    InitialHistory::New => InitialHistory::New,
                    InitialHistory::Cleared => InitialHistory::Cleared,
                    InitialHistory::Forked(history) => InitialHistory::Forked(history),
                    InitialHistory::Resumed(resumed) => InitialHistory::Forked(resumed.history),
                };
                if snapshot_state.ends_mid_turn {
                    append_interrupted_boundary(history, snapshot_state.active_turn_id)
                } else {
                    history
                }
            }
        };
        Box::pin(self.state.spawn_thread(
            config,
            history,
            Arc::clone(&self.state.auth_manager),
            self.agent_control(),
            Vec::new(),
            persist_extended_history,
            /*metrics_service_name*/ None,
            parent_trace,
            /*user_shell_override*/ None,
        ))
        .await
    }

    pub(crate) fn agent_control(&self) -> AgentControl {
        AgentControl::new(Arc::downgrade(&self.state))
    }

    #[cfg(test)]
    pub(crate) fn captured_ops(&self) -> Vec<(ThreadId, Op)> {
        self.state
            .ops_log
            .as_ref()
            .and_then(|ops_log| ops_log.lock().ok().map(|log| log.clone()))
            .unwrap_or_default()
    }
}

impl ThreadManagerState {
    pub(crate) async fn list_thread_ids(&self) -> Vec<ThreadId> {
        self.threads.read().await.keys().copied().collect()
    }

    /// Fetch a thread by ID or return ThreadNotFound.
    pub(crate) async fn get_thread(&self, thread_id: ThreadId) -> CodexResult<Arc<CodexThread>> {
        let threads = self.threads.read().await;
        threads
            .get(&thread_id)
            .cloned()
            .ok_or_else(|| CodexErr::ThreadNotFound(thread_id))
    }

    /// Send an operation to a thread by ID.
    pub(crate) async fn send_op(&self, thread_id: ThreadId, op: Op) -> CodexResult<String> {
        let thread = self.get_thread(thread_id).await?;
        if let Some(ops_log) = &self.ops_log
            && let Ok(mut log) = ops_log.lock()
        {
            log.push((thread_id, op.clone()));
        }
        thread.submit(op).await
    }

    #[cfg(test)]
    /// Append a prebuilt message to a thread by ID outside the normal user-input path.
    pub(crate) async fn append_message(
        &self,
        thread_id: ThreadId,
        message: ResponseItem,
    ) -> CodexResult<String> {
        let thread = self.get_thread(thread_id).await?;
        thread.append_message(message).await
    }

    /// Remove a thread from the manager by ID, returning it when present.
    pub(crate) async fn remove_thread(&self, thread_id: &ThreadId) -> Option<Arc<CodexThread>> {
        self.threads.write().await.remove(thread_id)
    }

    /// Spawn a new thread with no history using a provided config.
    pub(crate) async fn spawn_new_thread(
        &self,
        config: Config,
        agent_control: AgentControl,
    ) -> CodexResult<NewThread> {
        Box::pin(self.spawn_new_thread_with_source(
            config,
            agent_control,
            self.session_source.clone(),
            /*persist_extended_history*/ false,
            /*metrics_service_name*/ None,
            /*inherited_shell_snapshot*/ None,
            /*inherited_exec_policy*/ None,
        ))
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn spawn_new_thread_with_source(
        &self,
        config: Config,
        agent_control: AgentControl,
        session_source: SessionSource,
        persist_extended_history: bool,
        metrics_service_name: Option<String>,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
        inherited_exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
    ) -> CodexResult<NewThread> {
        Box::pin(self.spawn_thread_with_source(
            config,
            InitialHistory::New,
            Arc::clone(&self.auth_manager),
            agent_control,
            session_source,
            Vec::new(),
            persist_extended_history,
            metrics_service_name,
            inherited_shell_snapshot,
            inherited_exec_policy,
            /*parent_trace*/ None,
            /*user_shell_override*/ None,
        ))
        .await
    }

    pub(crate) async fn resume_thread_from_rollout_with_source(
        &self,
        config: Config,
        rollout_path: PathBuf,
        agent_control: AgentControl,
        session_source: SessionSource,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
        inherited_exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
    ) -> CodexResult<NewThread> {
        let initial_history = RolloutRecorder::get_rollout_history(&rollout_path).await?;
        Box::pin(self.spawn_thread_with_source(
            config,
            initial_history,
            Arc::clone(&self.auth_manager),
            agent_control,
            session_source,
            Vec::new(),
            /*persist_extended_history*/ false,
            /*metrics_service_name*/ None,
            inherited_shell_snapshot,
            inherited_exec_policy,
            /*parent_trace*/ None,
            /*user_shell_override*/ None,
        ))
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn fork_thread_with_source(
        &self,
        config: Config,
        initial_history: InitialHistory,
        agent_control: AgentControl,
        session_source: SessionSource,
        persist_extended_history: bool,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
        inherited_exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
    ) -> CodexResult<NewThread> {
        Box::pin(self.spawn_thread_with_source(
            config,
            initial_history,
            Arc::clone(&self.auth_manager),
            agent_control,
            session_source,
            Vec::new(),
            persist_extended_history,
            /*metrics_service_name*/ None,
            inherited_shell_snapshot,
            inherited_exec_policy,
            /*parent_trace*/ None,
            /*user_shell_override*/ None,
        ))
        .await
    }

    /// Spawn a new thread with optional history and register it with the manager.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn spawn_thread(
        &self,
        config: Config,
        initial_history: InitialHistory,
        auth_manager: Arc<AuthManager>,
        agent_control: AgentControl,
        dynamic_tools: Vec<codex_protocol::dynamic_tools::DynamicToolSpec>,
        persist_extended_history: bool,
        metrics_service_name: Option<String>,
        parent_trace: Option<W3cTraceContext>,
        user_shell_override: Option<crate::shell::Shell>,
    ) -> CodexResult<NewThread> {
        Box::pin(self.spawn_thread_with_source(
            config,
            initial_history,
            auth_manager,
            agent_control,
            self.session_source.clone(),
            dynamic_tools,
            persist_extended_history,
            metrics_service_name,
            /*inherited_shell_snapshot*/ None,
            /*inherited_exec_policy*/ None,
            parent_trace,
            user_shell_override,
        ))
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn spawn_thread_with_source(
        &self,
        config: Config,
        initial_history: InitialHistory,
        auth_manager: Arc<AuthManager>,
        agent_control: AgentControl,
        session_source: SessionSource,
        dynamic_tools: Vec<codex_protocol::dynamic_tools::DynamicToolSpec>,
        persist_extended_history: bool,
        metrics_service_name: Option<String>,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
        inherited_exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
        parent_trace: Option<W3cTraceContext>,
        user_shell_override: Option<crate::shell::Shell>,
    ) -> CodexResult<NewThread> {
        let environment = self
            .environment_manager
            .current()
            .await
            .map_err(|err| CodexErr::Fatal(format!("failed to create environment: {err}")))?;
        let watch_registration = match environment.as_ref() {
            Some(environment) if !environment.is_remote() => {
                self.skills_watcher
                    .register_config(
                        &config,
                        self.skills_manager.as_ref(),
                        self.plugins_manager.as_ref(),
                        Some(environment.get_filesystem()),
                    )
                    .await
            }
            Some(_) | None => crate::file_watcher::WatchRegistration::default(),
        };
        let CodexSpawnOk {
            codex,
            thread_id,
            ..
        } = Codex::spawn(CodexSpawnArgs {
            config,
            auth_manager,
            models_manager: Arc::clone(&self.models_manager),
            environment_manager: Arc::clone(&self.environment_manager),
            skills_manager: Arc::clone(&self.skills_manager),
            plugins_manager: Arc::clone(&self.plugins_manager),
            mcp_manager: Arc::clone(&self.mcp_manager),
            skills_watcher: Arc::clone(&self.skills_watcher),
            conversation_history: initial_history,
            session_source,
            agent_control,
            dynamic_tools,
            persist_extended_history,
            metrics_service_name,
            inherited_shell_snapshot,
            inherited_exec_policy,
            user_shell_override,
            parent_trace,
            analytics_events_client: self.analytics_events_client.clone(),
        })
        .await?;
        self.finalize_thread_spawn(codex, thread_id, watch_registration)
            .await
    }

    async fn finalize_thread_spawn(
        &self,
        codex: Codex,
        thread_id: ThreadId,
        watch_registration: crate::file_watcher::WatchRegistration,
    ) -> CodexResult<NewThread> {
        let event = codex.next_event().await?;
        let session_configured = match event {
            Event {
                id,
                msg: EventMsg::SessionConfigured(session_configured),
            } if id == INITIAL_SUBMIT_ID => session_configured,
            _ => {
                return Err(CodexErr::SessionConfiguredNotFirstEvent);
            }
        };

        let thread = Arc::new(CodexThread::new(
            codex,
            session_configured.rollout_path.clone(),
            watch_registration,
        ));
        let mut threads = self.threads.write().await;
        threads.insert(thread_id, thread.clone());

        Ok(NewThread {
            thread_id,
            thread,
            session_configured,
        })
    }

    pub(crate) fn notify_thread_created(&self, thread_id: ThreadId) {
        let _ = self.thread_created_tx.send(thread_id);
    }
}

/// Return a fork snapshot cut strictly before the nth user message (0-based).
///
/// Out-of-range values keep the full committed history at a turn boundary, but
/// when the source thread is currently mid-turn they fall back to cutting
/// before the active turn's opening boundary so the fork omits the unfinished
/// suffix entirely.
fn truncate_before_nth_user_message(
    history: InitialHistory,
    n: usize,
    snapshot_state: &SnapshotTurnState,
) -> InitialHistory {
    let items: Vec<RolloutItem> = history.get_rollout_items();
    let user_positions = truncation::user_message_positions_in_rollout(&items);
    let rolled = if snapshot_state.ends_mid_turn && n >= user_positions.len() {
        if let Some(cut_idx) = snapshot_state
            .active_turn_start_index
            .or_else(|| user_positions.last().copied())
        {
            items[..cut_idx].to_vec()
        } else {
            items
        }
    } else {
        truncation::truncate_rollout_before_nth_user_message_from_start(&items, n)
    };

    if rolled.is_empty() {
        InitialHistory::New
    } else {
        InitialHistory::Forked(rolled)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SnapshotTurnState {
    ends_mid_turn: bool,
    active_turn_id: Option<String>,
    active_turn_start_index: Option<usize>,
}

fn snapshot_turn_state(history: &InitialHistory) -> SnapshotTurnState {
    let rollout_items = history.get_rollout_items();
    let mut builder = ThreadHistoryBuilder::new();
    for item in &rollout_items {
        builder.handle_rollout_item(item);
    }
    let active_turn_id = builder.active_turn_id_if_explicit();
    if builder.has_active_turn() && active_turn_id.is_some() {
        let active_turn_snapshot = builder.active_turn_snapshot();
        if active_turn_snapshot
            .as_ref()
            .is_some_and(|turn| turn.status != TurnStatus::InProgress)
        {
            return SnapshotTurnState {
                ends_mid_turn: false,
                active_turn_id: None,
                active_turn_start_index: None,
            };
        }

        return SnapshotTurnState {
            ends_mid_turn: true,
            active_turn_id,
            active_turn_start_index: builder.active_turn_start_index(),
        };
    }

    let Some(last_user_position) = truncation::user_message_positions_in_rollout(&rollout_items)
        .last()
        .copied()
    else {
        return SnapshotTurnState {
            ends_mid_turn: false,
            active_turn_id: None,
            active_turn_start_index: None,
        };
    };

    // Synthetic fork/resume histories can contain user/assistant response items
    // without explicit turn lifecycle events. If the persisted snapshot has no
    // terminating boundary after its last user message, treat it as mid-turn.
    SnapshotTurnState {
        ends_mid_turn: !rollout_items[last_user_position + 1..].iter().any(|item| {
            matches!(
                item,
                RolloutItem::EventMsg(EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_))
            )
        }),
        active_turn_id: None,
        active_turn_start_index: None,
    }
}

/// Append the same persisted interrupt boundary used by the live interrupt path
/// to an existing fork snapshot after the source thread has been confirmed to
/// be mid-turn.
fn append_interrupted_boundary(history: InitialHistory, turn_id: Option<String>) -> InitialHistory {
    let aborted_event = RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
        turn_id,
        reason: TurnAbortReason::Interrupted,
        completed_at: None,
        duration_ms: None,
    }));

    match history {
        InitialHistory::New | InitialHistory::Cleared => InitialHistory::Forked(vec![
            RolloutItem::ResponseItem(interrupted_turn_history_marker()),
            aborted_event,
        ]),
        InitialHistory::Forked(mut history) => {
            history.push(RolloutItem::ResponseItem(interrupted_turn_history_marker()));
            history.push(aborted_event);
            InitialHistory::Forked(history)
        }
        InitialHistory::Resumed(mut resumed) => {
            resumed
                .history
                .push(RolloutItem::ResponseItem(interrupted_turn_history_marker()));
            resumed.history.push(aborted_event);
            InitialHistory::Forked(resumed.history)
        }
    }
}

#[cfg(test)]
#[path = "thread_manager_tests.rs"]
mod tests;
