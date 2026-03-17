use crate::AuthManager;
use crate::CodexAuth;
use crate::ModelProviderInfo;
use crate::OPENAI_PROVIDER_ID;
use crate::agent::AgentControl;
use crate::codex::Codex;
use crate::codex::CodexSpawnArgs;
use crate::codex::CodexSpawnOk;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex_thread::CodexThread;
use crate::config::Config;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::file_watcher::FileWatcher;
use crate::file_watcher::FileWatcherEvent;
use crate::mcp::McpManager;
use crate::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use crate::models_manager::manager::ModelsManager;
use crate::plugins::PluginsManager;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::SessionConfiguredEvent;
use crate::rollout::RolloutRecorder;
use crate::rollout::truncation;
use crate::shell_snapshot::ShellSnapshot;
use crate::skills::SkillsManager;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::collections::HashMap;
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

fn build_file_watcher(codex_home: PathBuf, skills_manager: Arc<SkillsManager>) -> Arc<FileWatcher> {
    if should_use_test_thread_manager_behavior()
        && let Ok(handle) = Handle::try_current()
        && handle.runtime_flavor() == RuntimeFlavor::CurrentThread
    {
        // The real watcher spins background tasks that can starve the
        // current-thread test runtime and cause event waits to time out.
        warn!("using noop file watcher under current-thread test runtime");
        return Arc::new(FileWatcher::noop());
    }

    let file_watcher = match FileWatcher::new(codex_home) {
        Ok(file_watcher) => Arc::new(file_watcher),
        Err(err) => {
            warn!("failed to initialize file watcher: {err}");
            Arc::new(FileWatcher::noop())
        }
    };

    let mut rx = file_watcher.subscribe();
    let skills_manager = Arc::clone(&skills_manager);
    if let Ok(handle) = Handle::try_current() {
        handle.spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(FileWatcherEvent::SkillsChanged { .. }) => {
                        skills_manager.clear_cache();
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    } else {
        warn!("file watcher listener skipped: no Tokio runtime available");
    }

    file_watcher
}

/// Represents a newly created Codex thread (formerly called a conversation), including the first event
/// (which is [`EventMsg::SessionConfigured`]).
pub struct NewThread {
    pub thread_id: ThreadId,
    pub thread: Arc<CodexThread>,
    pub session_configured: SessionConfiguredEvent,
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
    skills_manager: Arc<SkillsManager>,
    plugins_manager: Arc<PluginsManager>,
    mcp_manager: Arc<McpManager>,
    file_watcher: Arc<FileWatcher>,
    session_source: SessionSource,
    // Captures submitted ops for testing purpose when test mode is enabled.
    ops_log: Option<SharedCapturedOps>,
}

impl ThreadManager {
    pub fn new(
        config: &Config,
        auth_manager: Arc<AuthManager>,
        session_source: SessionSource,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> Self {
        let codex_home = config.codex_home.clone();
        let openai_models_provider = config
            .model_providers
            .get(OPENAI_PROVIDER_ID)
            .cloned()
            .unwrap_or_else(|| ModelProviderInfo::create_openai_provider(/*base_url*/ None));
        let (thread_created_tx, _) = broadcast::channel(THREAD_CREATED_CHANNEL_CAPACITY);
        let plugins_manager = Arc::new(PluginsManager::new(codex_home.clone()));
        let mcp_manager = Arc::new(McpManager::new(Arc::clone(&plugins_manager)));
        let skills_manager = Arc::new(SkillsManager::new(
            codex_home.clone(),
            Arc::clone(&plugins_manager),
            config.bundled_skills_enabled(),
        ));
        let file_watcher = build_file_watcher(codex_home.clone(), Arc::clone(&skills_manager));
        Self {
            state: Arc::new(ThreadManagerState {
                threads: Arc::new(RwLock::new(HashMap::new())),
                thread_created_tx,
                models_manager: Arc::new(ModelsManager::new_with_provider(
                    codex_home,
                    auth_manager.clone(),
                    config.model_catalog.clone(),
                    collaboration_modes_config,
                    openai_models_provider,
                )),
                skills_manager,
                plugins_manager,
                mcp_manager,
                file_watcher,
                auth_manager,
                session_source,
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
        let mut manager =
            Self::with_models_provider_and_home_for_tests(auth, provider, codex_home.clone());
        manager._test_codex_home_guard = Some(TempCodexHomeGuard { path: codex_home });
        manager
    }

    /// Construct with a dummy AuthManager containing the provided CodexAuth and codex home.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub(crate) fn with_models_provider_and_home_for_tests(
        auth: CodexAuth,
        provider: ModelProviderInfo,
        codex_home: PathBuf,
    ) -> Self {
        set_thread_manager_test_mode_for_tests(/*enabled*/ true);
        let auth_manager = AuthManager::from_auth_for_testing(auth);
        let (thread_created_tx, _) = broadcast::channel(THREAD_CREATED_CHANNEL_CAPACITY);
        let plugins_manager = Arc::new(PluginsManager::new(codex_home.clone()));
        let mcp_manager = Arc::new(McpManager::new(Arc::clone(&plugins_manager)));
        let skills_manager = Arc::new(SkillsManager::new(
            codex_home.clone(),
            Arc::clone(&plugins_manager),
            /*bundled_skills_enabled*/ true,
        ));
        let file_watcher = build_file_watcher(codex_home.clone(), Arc::clone(&skills_manager));
        Self {
            state: Arc::new(ThreadManagerState {
                threads: Arc::new(RwLock::new(HashMap::new())),
                thread_created_tx,
                models_manager: Arc::new(ModelsManager::with_provider_for_tests(
                    codex_home,
                    auth_manager.clone(),
                    provider,
                )),
                skills_manager,
                plugins_manager,
                mcp_manager,
                file_watcher,
                auth_manager,
                session_source: SessionSource::Exec,
                ops_log: should_use_test_thread_manager_behavior()
                    .then(|| Arc::new(std::sync::Mutex::new(Vec::new()))),
            }),
            _test_codex_home_guard: None,
        }
    }

    pub fn session_source(&self) -> SessionSource {
        self.state.session_source.clone()
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

    pub fn subscribe_file_watcher(&self) -> broadcast::Receiver<FileWatcherEvent> {
        self.state.file_watcher.subscribe()
    }

    pub fn get_models_manager(&self) -> Arc<ModelsManager> {
        self.state.models_manager.clone()
    }

    pub async fn list_models(
        &self,
        refresh_strategy: crate::models_manager::manager::RefreshStrategy,
    ) -> Vec<ModelPreset> {
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
        dynamic_tools: Vec<codex_protocol::dynamic_tools::DynamicToolSpec>,
        persist_extended_history: bool,
        metrics_service_name: Option<String>,
        parent_trace: Option<W3cTraceContext>,
    ) -> CodexResult<NewThread> {
        Box::pin(self.state.spawn_thread(
            config,
            InitialHistory::New,
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

    /// Fork an existing thread by taking messages up to the given position (not including
    /// the message at the given position) and starting a new thread with identical
    /// configuration (unless overridden by the caller's `config`). The new thread will have
    /// a fresh id. Pass `usize::MAX` to keep the full rollout history.
    pub async fn fork_thread(
        &self,
        nth_user_message: usize,
        config: Config,
        path: PathBuf,
        persist_extended_history: bool,
        parent_trace: Option<W3cTraceContext>,
    ) -> CodexResult<NewThread> {
        let history = RolloutRecorder::get_rollout_history(&path).await?;
        let history = truncate_before_nth_user_message(history, nth_user_message);
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
        ))
        .await
    }

    pub(crate) async fn spawn_new_thread_with_source(
        &self,
        config: Config,
        agent_control: AgentControl,
        session_source: SessionSource,
        persist_extended_history: bool,
        metrics_service_name: Option<String>,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
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
            /*parent_trace*/ None,
            /*user_shell_override*/ None,
        ))
        .await
    }

    pub(crate) async fn fork_thread_with_source(
        &self,
        config: Config,
        initial_history: InitialHistory,
        agent_control: AgentControl,
        session_source: SessionSource,
        persist_extended_history: bool,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
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
        parent_trace: Option<W3cTraceContext>,
        user_shell_override: Option<crate::shell::Shell>,
    ) -> CodexResult<NewThread> {
        let watch_registration = self
            .file_watcher
            .register_config(&config, self.skills_manager.as_ref());
        let CodexSpawnOk {
            codex, thread_id, ..
        } = Codex::spawn(CodexSpawnArgs {
            config,
            auth_manager,
            models_manager: Arc::clone(&self.models_manager),
            skills_manager: Arc::clone(&self.skills_manager),
            plugins_manager: Arc::clone(&self.plugins_manager),
            mcp_manager: Arc::clone(&self.mcp_manager),
            file_watcher: Arc::clone(&self.file_watcher),
            conversation_history: initial_history,
            session_source,
            agent_control,
            dynamic_tools,
            persist_extended_history,
            metrics_service_name,
            inherited_shell_snapshot,
            user_shell_override,
            parent_trace,
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

/// Return a prefix of `items` obtained by cutting strictly before the nth user message
/// (0-based) and all items that follow it.
fn truncate_before_nth_user_message(history: InitialHistory, n: usize) -> InitialHistory {
    let items: Vec<RolloutItem> = history.get_rollout_items();
    let rolled = truncation::truncate_rollout_before_nth_user_message_from_start(&items, n);

    if rolled.is_empty() {
        InitialHistory::New
    } else {
        InitialHistory::Forked(rolled)
    }
}

#[cfg(test)]
#[path = "thread_manager_tests.rs"]
mod tests;
