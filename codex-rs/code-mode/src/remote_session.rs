use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
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

use self::connection::Connection;
use crate::NoopCodeModeSessionDelegate;

mod connection;

const CODE_MODE_HOST_PATH_ENV: &str = "CODEX_CODE_MODE_HOST_PATH";

/// Creates code-mode sessions backed by one lazily spawned process host.
///
/// All sessions created by one provider share that host. Callers that want one
/// sidecar across multiple Codex threads must share the provider instance.
pub struct ProcessOwnedCodeModeSessionProvider {
    host_program: PathBuf,
    process_host: StdMutex<Option<Arc<OwnedProcessHost>>>,
}

impl ProcessOwnedCodeModeSessionProvider {
    pub fn with_host_program(host_program: PathBuf) -> Self {
        Self {
            host_program,
            process_host: StdMutex::new(None),
        }
    }

    fn process_host(&self) -> Arc<OwnedProcessHost> {
        let mut process_host = self
            .process_host
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(process_host) = process_host.as_ref() {
            return Arc::clone(process_host);
        }

        let new_process_host = Arc::new(OwnedProcessHost::new(self.host_program.clone()));
        *process_host = Some(Arc::clone(&new_process_host));
        new_process_host
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
        let session = ProcessOwnedCodeModeSession::with_process_host(delegate, self.process_host());
        Box::pin(async move {
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

    async fn connection(&self) -> Result<Arc<Connection>, String> {
        if let Some(connection) = self.live_connection() {
            return Ok(connection);
        }

        let _spawn_permit = self
            .spawn_permit
            .acquire()
            .await
            .map_err(|_| "code-mode host spawn coordinator closed".to_string())?;
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
    Open(Arc<Connection>),
    Shutdown,
}

/// A logical code-mode session assigned to a process-owned host.
pub struct ProcessOwnedCodeModeSession {
    process_host: Arc<OwnedProcessHost>,
    session_id: SessionId,
    next_cell_id: AtomicU64,
    delegate: Arc<dyn CodeModeSessionDelegate>,
    state: StdMutex<SessionState>,
    transition_permit: Semaphore,
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
        let session_id = process_host.allocate_session_id();
        Self {
            process_host,
            session_id,
            next_cell_id: AtomicU64::new(1),
            delegate,
            state: StdMutex::new(SessionState::New),
            transition_permit: Semaphore::new(/*permits*/ 1),
        }
    }

    async fn connection(&self) -> Result<Arc<Connection>, String> {
        if let Some(connection) = self.current_connection()? {
            return Ok(connection);
        }

        let _transition_permit = self
            .transition_permit
            .acquire()
            .await
            .map_err(|_| "code-mode session transition coordinator closed".to_string())?;
        if let Some(connection) = self.current_connection()? {
            return Ok(connection);
        }
        let connection = self.process_host.connection().await?;
        connection
            .open_session(self.session_id.clone(), Arc::clone(&self.delegate))
            .await?;
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            SessionState::Open(Arc::clone(&connection));
        Ok(connection)
    }

    fn current_connection(&self) -> Result<Option<Arc<Connection>>, String> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*state {
            SessionState::New => Ok(None),
            SessionState::Open(connection) if connection.is_alive() => {
                Ok(Some(Arc::clone(connection)))
            }
            SessionState::Open(_) => {
                *state = SessionState::New;
                Ok(None)
            }
            SessionState::Shutdown => Err("code mode session is shutting down".to_string()),
        }
    }

    pub async fn execute(&self, request: ExecuteRequest) -> Result<StartedCell, String> {
        let cell_id = self.allocate_cell_id();
        self.connection()
            .await?
            .execute(self.session_id.clone(), cell_id, request)
            .await
    }

    fn allocate_cell_id(&self) -> CellId {
        CellId::new(
            self.next_cell_id
                .fetch_add(1, Ordering::Relaxed)
                .to_string(),
        )
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<WaitOutcome, String> {
        self.connection()
            .await?
            .wait(self.session_id.clone(), request)
            .await
    }

    pub async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, String> {
        self.connection()
            .await?
            .terminate(self.session_id.clone(), cell_id)
            .await
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let transition_permit = self
            .transition_permit
            .acquire()
            .await
            .map_err(|_| "code-mode session transition coordinator closed".to_string())?;
        let connection = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match std::mem::replace(&mut *state, SessionState::Shutdown) {
                SessionState::Open(connection) => connection,
                SessionState::New | SessionState::Shutdown => return Ok(()),
            }
        };
        drop(transition_permit);
        connection.shutdown_session(self.session_id.clone()).await
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
    if let Some(path) = std::env::var_os(CODE_MODE_HOST_PATH_ENV) {
        return PathBuf::from(path);
    }
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        let sibling = parent.join(executable_name);
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from(executable_name)
}

#[cfg(test)]
#[path = "remote_session_tests.rs"]
mod tests;
