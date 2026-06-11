use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
use codex_code_mode_protocol::wire::HostRequest;
use codex_code_mode_protocol::wire::HostResponse;
use codex_code_mode_protocol::wire::SessionId;
use tokio::sync::Semaphore;

const CODE_MODE_HOST_PATH_ENV: &str = "CODEX_CODE_MODE_HOST_PATH";

mod connection;

use connection::Connection;

#[derive(Clone, Debug)]
pub struct CodeModeHostCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl Default for CodeModeHostCommand {
    fn default() -> Self {
        Self {
            program: default_host_program(),
            args: Vec::new(),
        }
    }
}

pub struct IpcCodeModeSessionProvider {
    command: CodeModeHostCommand,
    connection: std::sync::Mutex<Option<Arc<Connection>>>,
    spawn_permit: Semaphore,
}

impl Default for IpcCodeModeSessionProvider {
    fn default() -> Self {
        Self::new(CodeModeHostCommand::default())
    }
}

impl IpcCodeModeSessionProvider {
    pub fn new(command: CodeModeHostCommand) -> Self {
        Self {
            command,
            connection: std::sync::Mutex::new(None),
            spawn_permit: Semaphore::new(/*permits*/ 1),
        }
    }

    async fn connection(&self) -> Result<Arc<Connection>, String> {
        if let Some(connection) = {
            let current = self
                .connection
                .lock()
                .map_err(|_| "code-mode host connection lock poisoned".to_string())?;
            current
                .as_ref()
                .filter(|connection| connection.is_alive())
                .cloned()
        } {
            return Ok(connection);
        }

        let _spawn_permit = self
            .spawn_permit
            .acquire()
            .await
            .map_err(|_| "code-mode host spawn coordinator closed".to_string())?;
        if let Some(connection) = {
            let current = self
                .connection
                .lock()
                .map_err(|_| "code-mode host connection lock poisoned".to_string())?;
            current
                .as_ref()
                .filter(|connection| connection.is_alive())
                .cloned()
        } {
            return Ok(connection);
        }
        let connection = Arc::new(Connection::spawn(&self.command).await?);
        *self
            .connection
            .lock()
            .map_err(|_| "code-mode host connection lock poisoned".to_string())? =
            Some(Arc::clone(&connection));
        Ok(connection)
    }
}

impl CodeModeSessionProvider for IpcCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            let connection = self.connection().await?;
            let response = connection.request(HostRequest::CreateSession).await?;
            let HostResponse::SessionCreated { session_id } = response else {
                return Err(
                    "code-mode host returned an invalid create-session response".to_string()
                );
            };
            connection.register_delegate(session_id, delegate).await;
            let session: Arc<dyn CodeModeSession> = Arc::new(IpcCodeModeSession {
                connection,
                session_id,
                shutdown: AtomicBool::new(false),
            });
            Ok(session)
        })
    }
}

struct IpcCodeModeSession {
    connection: Arc<Connection>,
    session_id: SessionId,
    shutdown: AtomicBool,
}

impl CodeModeSession for IpcCodeModeSession {
    fn is_alive(&self) -> bool {
        !self.shutdown.load(Ordering::Acquire) && self.connection.is_alive()
    }

    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(async move {
            self.ensure_active()?;
            self.connection.execute(self.session_id, request).await
        })
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(async move {
            self.ensure_active()?;
            let response = self
                .connection
                .request(HostRequest::Wait {
                    session_id: self.session_id,
                    request,
                })
                .await?;
            match response {
                HostResponse::WaitCompleted { outcome } => Ok(outcome),
                _ => Err("code-mode host returned an invalid wait response".to_string()),
            }
        })
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(async move {
            self.ensure_active()?;
            let response = self
                .connection
                .request(HostRequest::Terminate {
                    session_id: self.session_id,
                    cell_id,
                })
                .await?;
            match response {
                HostResponse::WaitCompleted { outcome } => Ok(outcome),
                _ => Err("code-mode host returned an invalid terminate response".to_string()),
            }
        })
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(async move {
            if self.shutdown.swap(true, Ordering::AcqRel) {
                return Ok(());
            }
            let result = self
                .connection
                .request(HostRequest::ShutdownSession {
                    session_id: self.session_id,
                })
                .await;
            self.connection.remove_delegate(self.session_id).await;
            match result? {
                HostResponse::SessionShutdown => Ok(()),
                _ => Err("code-mode host returned an invalid shutdown response".to_string()),
            }
        })
    }
}

impl IpcCodeModeSession {
    fn ensure_active(&self) -> Result<(), String> {
        if self.shutdown.load(Ordering::Acquire) {
            Err("code mode session is shutting down".to_string())
        } else {
            Ok(())
        }
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
#[path = "tests.rs"]
mod tests;
