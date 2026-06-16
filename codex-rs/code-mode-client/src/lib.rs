use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CellOutcome;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProviderFuture;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::CreateCellRequest;
use codex_code_mode_protocol::ObserveRequest;
use codex_code_mode_protocol::wire;
use tokio::sync::Semaphore;

mod connection;
mod convert;

use connection::Connection;
use connection::RequestError;

const CODE_MODE_HOST_PATH_ENV: &str = "CODEX_CODE_MODE_HOST_PATH";

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

pub struct StdioCodeModeSessionProvider {
    command: CodeModeHostCommand,
    connection: std::sync::Mutex<Option<Arc<Connection>>>,
    spawn_permit: Semaphore,
}

impl StdioCodeModeSessionProvider {
    pub fn new(command: CodeModeHostCommand) -> Self {
        Self {
            command,
            connection: std::sync::Mutex::new(None),
            spawn_permit: Semaphore::new(/*permits*/ 1),
        }
    }

    async fn connection(&self) -> Result<Arc<Connection>, String> {
        if let Some(connection) = self.live_connection()? {
            return Ok(connection);
        }

        let _spawn_permit = self
            .spawn_permit
            .acquire()
            .await
            .map_err(|_| "code-mode host spawn coordinator closed".to_string())?;
        if let Some(connection) = self.live_connection()? {
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

    fn live_connection(&self) -> Result<Option<Arc<Connection>>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "code-mode host connection lock poisoned".to_string())?;
        Ok(connection
            .as_ref()
            .filter(|connection| connection.is_alive())
            .cloned())
    }
}

impl Default for StdioCodeModeSessionProvider {
    fn default() -> Self {
        Self::new(CodeModeHostCommand::default())
    }
}

impl CodeModeSessionProvider for StdioCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            let connection = self.connection().await?;
            let response = connection
                .request(wire::HostRequest::CreateSession)
                .await
                .map_err(RequestError::into_message)?;
            let wire::HostResponse::SessionCreated { session_id } = response else {
                return Err(
                    "code-mode host returned an invalid create-session response".to_string()
                );
            };
            connection.register_delegate(session_id, delegate).await;
            let session: Arc<dyn CodeModeSession> = Arc::new(StdioCodeModeSession {
                connection,
                session_id,
                shutdown: AtomicBool::new(false),
            });
            Ok(session)
        })
    }
}

struct StdioCodeModeSession {
    connection: Arc<Connection>,
    session_id: wire::SessionId,
    shutdown: AtomicBool,
}

impl CodeModeSession for StdioCodeModeSession {
    fn is_alive(&self) -> bool {
        !self.shutdown.load(Ordering::Acquire) && self.connection.is_alive()
    }

    fn create_cell<'a>(
        &'a self,
        request: CreateCellRequest,
    ) -> CodeModeSessionResultFuture<'a, CellId> {
        Box::pin(async move {
            self.ensure_active()?;
            let response = self
                .connection
                .request(wire::HostRequest::CreateCell {
                    session_id: self.session_id,
                    request: convert::create_cell_request(request),
                })
                .await
                .map_err(RequestError::into_message)?;
            match response {
                wire::HostResponse::CellCreated { cell_id } => {
                    Ok(convert::protocol_cell_id(&cell_id))
                }
                _ => Err("code-mode host returned an invalid create-cell response".to_string()),
            }
        })
    }

    fn observe<'a>(
        &'a self,
        request: ObserveRequest,
    ) -> CodeModeSessionResultFuture<'a, CellOutcome> {
        Box::pin(async move {
            self.ensure_active()?;
            let cell_id = request.cell_id;
            let response = self
                .connection
                .request(wire::HostRequest::Observe {
                    session_id: self.session_id,
                    cell_id: convert::wire_cell_id(&cell_id),
                    mode: wire::ObserveMode::YieldAfter {
                        duration_ms: request.yield_time_ms,
                    },
                })
                .await;
            match response {
                Ok(wire::HostResponse::Observed { event }) => Ok(CellOutcome::LiveCell(
                    convert::runtime_response(&cell_id, event)?,
                )),
                Ok(_) => Err("code-mode host returned an invalid observe response".to_string()),
                Err(RequestError::Host(wire::Error::MissingCell { .. })) => Ok(
                    CellOutcome::MissingCell(convert::missing_cell_response(cell_id)),
                ),
                Err(error) => Err(error.into_message()),
            }
        })
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, CellOutcome> {
        Box::pin(async move {
            self.ensure_active()?;
            let response = self
                .connection
                .request(wire::HostRequest::Terminate {
                    session_id: self.session_id,
                    cell_id: convert::wire_cell_id(&cell_id),
                })
                .await;
            match response {
                Ok(wire::HostResponse::Observed { event }) => Ok(CellOutcome::LiveCell(
                    convert::runtime_response(&cell_id, event)?,
                )),
                Ok(_) => Err("code-mode host returned an invalid terminate response".to_string()),
                Err(RequestError::Host(wire::Error::MissingCell { .. })) => Ok(
                    CellOutcome::MissingCell(convert::missing_cell_response(cell_id)),
                ),
                Err(error) => Err(error.into_message()),
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
                .request(wire::HostRequest::ShutdownSession {
                    session_id: self.session_id,
                })
                .await;
            self.connection.remove_delegate(self.session_id).await;
            match result.map_err(RequestError::into_message)? {
                wire::HostResponse::SessionShutdown => Ok(()),
                _ => Err("code-mode host returned an invalid shutdown response".to_string()),
            }
        })
    }
}

impl StdioCodeModeSession {
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
