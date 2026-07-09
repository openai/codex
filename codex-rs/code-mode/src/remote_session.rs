use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProviderFuture;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::host::SessionId;
use tokio::sync::Semaphore;
use tokio::sync::watch;

use self::connection::Connection;
use self::connection::ConnectionError;
use self::connection::RemoteSession;
use self::connection::SessionCleanup;
use crate::NoopCodeModeSessionDelegate;

mod connection;

const CODE_MODE_HOST_PATH_ENV: &str = "CODEX_CODE_MODE_HOST_PATH";

type ShutdownResultReceiver = watch::Receiver<Option<Result<(), String>>>;

/// Creates code-mode sessions backed by one lazily spawned process host.
pub struct ProcessOwnedCodeModeSessionProvider {
    state: StdMutex<ProviderState>,
}

enum ProviderState {
    OwnedProcess(Arc<OwnedProcessHost>),
    InProcess,
}

impl ProcessOwnedCodeModeSessionProvider {
    pub fn with_host_program(host_program: PathBuf) -> Self {
        Self {
            state: StdMutex::new(ProviderState::OwnedProcess(Arc::new(
                OwnedProcessHost::new(host_program),
            ))),
        }
    }

    fn process_host(&self) -> Option<Arc<OwnedProcessHost>> {
        match &*self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            ProviderState::OwnedProcess(process_host) => Some(Arc::clone(process_host)),
            ProviderState::InProcess => None,
        }
    }
}

impl Default for ProcessOwnedCodeModeSessionProvider {
    fn default() -> Self {
        Self::with_host_program(default_host_program())
    }
}

impl CodeModeSessionProvider for ProcessOwnedCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            let Some(process_host) = self.process_host() else {
                let session: Arc<dyn CodeModeSession> =
                    Arc::new(crate::InProcessCodeModeSession::with_delegate(delegate));
                return Ok(session);
            };

            match process_host.connection().await {
                Ok(_) => {}
                Err(error) if error.host_program_not_found() => {
                    *self
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        ProviderState::InProcess;
                    let session: Arc<dyn CodeModeSession> =
                        Arc::new(crate::InProcessCodeModeSession::with_delegate(delegate));
                    return Ok(session);
                }
                Err(error) => return Err(error.to_string()),
            }
            let session = ProcessOwnedCodeModeSession::with_process_host(delegate, process_host);
            session.connection().await?;
            let session: Arc<dyn CodeModeSession> = Arc::new(session);
            Ok(session)
        })
    }
}

struct OwnedProcessHost {
    host_program: PathBuf,
    connection: StdMutex<Option<Arc<Connection>>>,
    spawn_permit: Semaphore,
    next_session_id: AtomicU64,
}

impl OwnedProcessHost {
    fn new(host_program: PathBuf) -> Self {
        Self {
            host_program,
            connection: StdMutex::new(None),
            spawn_permit: Semaphore::new(/*permits*/ 1),
            next_session_id: AtomicU64::new(1),
        }
    }

    async fn connection(&self) -> Result<Arc<Connection>, ConnectionError> {
        if let Some(connection) = self.live_connection() {
            return Ok(connection);
        }

        let _spawn_permit = self.spawn_permit.acquire().await.map_err(|_| {
            ConnectionError::Other("code-mode host spawn coordinator closed".into())
        })?;
        if let Some(connection) = self.live_connection() {
            return Ok(connection);
        }
        let new_connection = Arc::new(Connection::spawn(&self.host_program).await?);
        *self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::clone(&new_connection));
        Ok(new_connection)
    }

    fn live_connection(&self) -> Option<Arc<Connection>> {
        self.connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|connection| connection.is_alive())
            .cloned()
    }

    fn allocate_session_id(&self) -> SessionId {
        let value = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        match SessionId::new(format!("session-{value}")) {
            Ok(session_id) => session_id,
            Err(_) => unreachable!("a generated code-mode session ID is nonempty"),
        }
    }
}

enum SessionState {
    New,
    Opening {
        remote: RemoteSession,
        result_rx: watch::Receiver<Option<Result<SessionBinding, String>>>,
    },
    Open(SessionBinding),
    Closing,
    Closed,
}

#[derive(Clone)]
struct SessionBinding {
    connection: Arc<Connection>,
    remote: RemoteSession,
    cleanup: SessionCleanup,
}

struct SessionInner {
    process_host: Arc<OwnedProcessHost>,
    delegate: Arc<dyn CodeModeSessionDelegate>,
    state: StdMutex<SessionState>,
    next_generation: AtomicU64,
    shutdown_requested: AtomicBool,
    shutdown_result: StdMutex<Option<ShutdownResultReceiver>>,
    retired_cleanups: StdMutex<Vec<SessionCleanup>>,
}

/// A logical code-mode session assigned to a process-owned host.
pub struct ProcessOwnedCodeModeSession {
    inner: Arc<SessionInner>,
}

impl ProcessOwnedCodeModeSession {
    pub fn new() -> Self {
        Self::with_process_host(
            Arc::new(NoopCodeModeSessionDelegate),
            Arc::new(OwnedProcessHost::new(default_host_program())),
        )
    }

    fn with_process_host(
        delegate: Arc<dyn CodeModeSessionDelegate>,
        process_host: Arc<OwnedProcessHost>,
    ) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                process_host,
                delegate,
                state: StdMutex::new(SessionState::New),
                next_generation: AtomicU64::new(1),
                shutdown_requested: AtomicBool::new(false),
                shutdown_result: StdMutex::new(None),
                retired_cleanups: StdMutex::new(Vec::new()),
            }),
        }
    }

    async fn connection(&self) -> Result<SessionBinding, String> {
        self.inner.connection().await
    }

    pub async fn execute(&self, request: ExecuteRequest) -> Result<StartedCell, String> {
        let binding = self.connection().await?;
        binding.connection.execute(binding.remote, request).await
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<WaitOutcome, String> {
        let binding = self.connection().await?;
        binding.connection.wait(binding.remote, request).await
    }

    pub async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, String> {
        let binding = self.connection().await?;
        binding.connection.terminate(binding.remote, cell_id).await
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        wait_for_watch(self.inner.request_shutdown()).await
    }
}

impl SessionInner {
    async fn connection(self: &Arc<Self>) -> Result<SessionBinding, String> {
        loop {
            if self.shutdown_requested.load(Ordering::Acquire) {
                return Err("code mode session is shutting down".to_string());
            }
            let (result_rx, start) = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &*state {
                    SessionState::New => {
                        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
                        let remote = RemoteSession {
                            id: self.process_host.allocate_session_id(),
                            generation,
                        };
                        let (result_tx, result_rx) = watch::channel(None);
                        *state = SessionState::Opening {
                            remote: remote.clone(),
                            result_rx: result_rx.clone(),
                        };
                        (result_rx, Some((remote, result_tx)))
                    }
                    SessionState::Opening { result_rx, .. } => (result_rx.clone(), None),
                    SessionState::Open(binding) if binding.connection.is_alive() => {
                        return Ok(binding.clone());
                    }
                    SessionState::Open(binding) => {
                        self.retain_cleanup(binding.cleanup.clone());
                        *state = SessionState::New;
                        continue;
                    }
                    SessionState::Closing | SessionState::Closed => {
                        return Err("code mode session is shutting down".to_string());
                    }
                }
            };
            if let Some((remote, result_tx)) = start {
                let inner = Arc::clone(self);
                tokio::spawn(async move {
                    inner.open(remote, result_tx).await;
                });
            }
            return wait_for_watch(result_rx).await;
        }
    }

    async fn open(
        self: Arc<Self>,
        remote: RemoteSession,
        result_tx: watch::Sender<Option<Result<SessionBinding, String>>>,
    ) {
        let result = match self.process_host.connection().await {
            Ok(connection) => {
                let cleanup = connection
                    .open_session(remote.clone(), Arc::clone(&self.delegate))
                    .await;
                cleanup.map(|cleanup| SessionBinding {
                    connection,
                    remote: remote.clone(),
                    cleanup,
                })
            }
            Err(err) => Err(err.to_string()),
        };
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if matches!(
                &*state,
                SessionState::Opening {
                    remote: opening_remote,
                    ..
                } if opening_remote == &remote
            ) {
                *state = match &result {
                    Ok(binding) => SessionState::Open(binding.clone()),
                    Err(_) => SessionState::New,
                };
            }
        }
        result_tx.send_replace(Some(result));
    }

    fn request_shutdown(self: &Arc<Self>) -> ShutdownResultReceiver {
        self.shutdown_requested.store(true, Ordering::Release);
        let mut shutdown_result = self
            .shutdown_result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(result_rx) = shutdown_result.as_ref() {
            return result_rx.clone();
        }
        let (result_tx, result_rx) = watch::channel(None);
        *shutdown_result = Some(result_rx.clone());
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            let result = inner.drive_shutdown().await;
            result_tx.send_replace(Some(result));
        });
        result_rx
    }

    async fn drive_shutdown(self: &Arc<Self>) -> Result<(), String> {
        loop {
            let action = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &*state {
                    SessionState::New => {
                        *state = SessionState::Closed;
                        ShutdownAction::Finish
                    }
                    SessionState::Opening { result_rx, .. } => {
                        ShutdownAction::WaitForOpen(result_rx.clone())
                    }
                    SessionState::Open(binding) if !binding.connection.is_alive() => {
                        let cleanup = binding.cleanup.clone();
                        *state = SessionState::Closing;
                        ShutdownAction::WaitForSessionCleanup(cleanup)
                    }
                    SessionState::Open(binding) => {
                        let binding = binding.clone();
                        *state = SessionState::Closing;
                        ShutdownAction::Close(binding)
                    }
                    SessionState::Closing => {
                        return Err("code-mode session shutdown driver entered twice".to_string());
                    }
                    SessionState::Closed => return Ok(()),
                }
            };
            match action {
                ShutdownAction::WaitForOpen(result_rx) => {
                    let _ = wait_for_watch(result_rx).await;
                }
                ShutdownAction::Finish => {
                    self.wait_for_retired_cleanups().await;
                    return Ok(());
                }
                ShutdownAction::WaitForSessionCleanup(cleanup) => {
                    cleanup.wait().await;
                    self.wait_for_retired_cleanups().await;
                    *self
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = SessionState::Closed;
                    return Ok(());
                }
                ShutdownAction::Close(binding) => {
                    let result = binding.connection.shutdown_session(binding.remote).await;
                    if result.is_err() && !binding.connection.is_alive() {
                        binding.cleanup.wait().await;
                    }
                    self.wait_for_retired_cleanups().await;
                    *self
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = SessionState::Closed;
                    return result;
                }
            }
        }
    }

    fn retain_cleanup(&self, cleanup: SessionCleanup) {
        let mut retired = self
            .retired_cleanups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        retired.retain(|cleanup| !cleanup.is_complete());
        if !cleanup.is_complete() {
            retired.push(cleanup);
        }
    }

    async fn wait_for_retired_cleanups(&self) {
        let retired = std::mem::take(
            &mut *self
                .retired_cleanups
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for cleanup in retired {
            cleanup.wait().await;
        }
    }
}

enum ShutdownAction {
    WaitForOpen(watch::Receiver<Option<Result<SessionBinding, String>>>),
    Finish,
    WaitForSessionCleanup(SessionCleanup),
    Close(SessionBinding),
}

async fn wait_for_watch<T>(
    mut result_rx: watch::Receiver<Option<Result<T, String>>>,
) -> Result<T, String>
where
    T: Clone,
{
    loop {
        if let Some(result) = result_rx.borrow().clone() {
            return result;
        }
        result_rx
            .changed()
            .await
            .map_err(|_| "code-mode session transition stopped".to_string())?;
    }
}

impl Drop for ProcessOwnedCodeModeSession {
    fn drop(&mut self) {
        if tokio::runtime::Handle::try_current().is_ok() {
            self.inner.request_shutdown();
        }
    }
}

impl Default for ProcessOwnedCodeModeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeSession for ProcessOwnedCodeModeSession {
    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(ProcessOwnedCodeModeSession::execute(self, request))
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(ProcessOwnedCodeModeSession::wait(self, request))
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(ProcessOwnedCodeModeSession::terminate(self, cell_id))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(ProcessOwnedCodeModeSession::shutdown(self))
    }
}

fn default_host_program() -> PathBuf {
    resolve_host_program(
        std::env::var_os(CODE_MODE_HOST_PATH_ENV),
        std::env::current_exe(),
    )
}

fn resolve_host_program(
    override_path: Option<OsString>,
    current_exe: io::Result<PathBuf>,
) -> PathBuf {
    if let Some(path) = override_path {
        return PathBuf::from(path);
    }
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };
    if let Ok(current_exe) = current_exe
        && let Some(parent) = current_exe.parent()
    {
        return parent.join(executable_name);
    }
    PathBuf::from(executable_name)
}

#[cfg(test)]
#[path = "remote_session_tests.rs"]
mod tests;
