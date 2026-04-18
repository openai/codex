//! Launch MCP stdio servers and return the transport rmcp should use.
//!
//! This module owns the "where does the server process run?" boundary for
//! stdio MCP servers. In this PR there is only the local launcher, which keeps
//! the existing behavior: the orchestrator starts the configured command and
//! rmcp talks to the child process through local stdin/stdout pipes.
//!
//! Later stack entries add an executor-backed launcher without changing
//! `RmcpClient`'s MCP lifecycle code.

use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
#[cfg(unix)]
use std::thread::sleep;
#[cfg(unix)]
use std::thread::spawn;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use codex_utils_pty::process_group::kill_process_group;
#[cfg(unix)]
use codex_utils_pty::process_group::terminate_process_group;
use futures::FutureExt;
use futures::future::BoxFuture;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use rmcp::transport::child_process::TokioChildProcess;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tracing::info;
use tracing::warn;

use crate::program_resolver;
use crate::utils::create_env_for_mcp_server;

// General purpose public code.

/// Launches an MCP stdio server and returns the transport for rmcp.
///
/// This trait is the boundary between MCP lifecycle code and process placement.
/// `RmcpClient` owns MCP operations such as `initialize` and `tools/list`; the
/// launcher owns starting the configured command and producing an rmcp
/// [`Transport`] over the server's stdin/stdout bytes.
pub trait StdioServerLauncher: private::Sealed + Send + Sync {
    /// Start the configured stdio server and return its rmcp-facing transport.
    fn launch(
        &self,
        command: StdioServerCommand,
    ) -> BoxFuture<'static, io::Result<StdioServerTransport>>;
}

/// Command-line process shape shared by stdio server launchers.
#[derive(Clone)]
pub struct StdioServerCommand {
    program: OsString,
    args: Vec<OsString>,
    env: Option<HashMap<OsString, OsString>>,
    env_vars: Vec<String>,
    cwd: Option<PathBuf>,
}

/// Client-side rmcp transport for a launched MCP stdio server.
///
/// The concrete process placement stays private to this module. `RmcpClient`
/// only sees the standard rmcp transport abstraction and can pass this value
/// directly to `rmcp::service::serve_client`.
pub struct StdioServerTransport {
    inner: StdioServerTransportInner,
    _process_group_guard: Option<ProcessGroupGuard>,
}

enum StdioServerTransportInner {
    Local(TokioChildProcess),
}

impl Transport<RoleClient> for StdioServerTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send + 'static {
        match &mut self.inner {
            StdioServerTransportInner::Local(transport) => transport.send(item).boxed(),
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        match &mut self.inner {
            StdioServerTransportInner::Local(transport) => transport.receive().boxed(),
        }
    }

    async fn close(&mut self) -> std::result::Result<(), Self::Error> {
        match &mut self.inner {
            StdioServerTransportInner::Local(transport) => transport.close().await,
        }
    }
}

impl StdioServerCommand {
    /// Build the stdio process parameters before choosing where the process
    /// runs.
    pub(super) fn new(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<OsString, OsString>>,
        env_vars: Vec<String>,
        cwd: Option<PathBuf>,
    ) -> Self {
        Self {
            program,
            args,
            env,
            env_vars,
            cwd,
        }
    }
}

// Local public implementation.

/// Starts MCP stdio servers as local child processes.
///
/// This is the existing behavior for local MCP servers: the orchestrator
/// process spawns the configured command and rmcp talks to the child's local
/// stdin/stdout pipes directly.
#[derive(Clone)]
pub struct LocalStdioServerLauncher;

impl StdioServerLauncher for LocalStdioServerLauncher {
    fn launch(
        &self,
        command: StdioServerCommand,
    ) -> BoxFuture<'static, io::Result<StdioServerTransport>> {
        async move { Self::launch_server(command) }.boxed()
    }
}

// Local private implementation.

#[cfg(unix)]
const PROCESS_GROUP_TERM_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[cfg(unix)]
struct ProcessGroupGuard {
    process_group_id: u32,
}

#[cfg(not(unix))]
struct ProcessGroupGuard;

mod private {
    pub trait Sealed {}
}

impl private::Sealed for LocalStdioServerLauncher {}

impl LocalStdioServerLauncher {
    fn launch_server(command: StdioServerCommand) -> io::Result<StdioServerTransport> {
        let StdioServerCommand {
            program,
            args,
            env,
            env_vars,
            cwd,
        } = command;
        let program_name = program.to_string_lossy().into_owned();
        let envs = create_env_for_mcp_server(env, &env_vars);
        let resolved_program =
            program_resolver::resolve(program, &envs).map_err(io::Error::other)?;

        let mut command = Command::new(resolved_program);
        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env_clear()
            .envs(envs)
            .args(args);
        #[cfg(unix)]
        command.process_group(0);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }

        let (transport, stderr) = TokioChildProcess::builder(command)
            .stderr(Stdio::piped())
            .spawn()?;
        let process_group_guard = transport.id().map(ProcessGroupGuard::new);

        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                loop {
                    match reader.next_line().await {
                        Ok(Some(line)) => {
                            info!("MCP server stderr ({program_name}): {line}");
                        }
                        Ok(None) => break,
                        Err(error) => {
                            warn!("Failed to read MCP server stderr ({program_name}): {error}");
                            break;
                        }
                    }
                }
            });
        }

        Ok(StdioServerTransport {
            inner: StdioServerTransportInner::Local(transport),
            _process_group_guard: process_group_guard,
        })
    }
}

impl ProcessGroupGuard {
    fn new(process_group_id: u32) -> Self {
        #[cfg(unix)]
        {
            Self { process_group_id }
        }
        #[cfg(not(unix))]
        {
            let _ = process_group_id;
            Self
        }
    }

    #[cfg(unix)]
    fn maybe_terminate_process_group(&self) {
        let process_group_id = self.process_group_id;
        let should_escalate = match terminate_process_group(process_group_id) {
            Ok(exists) => exists,
            Err(error) => {
                warn!("Failed to terminate MCP process group {process_group_id}: {error}");
                false
            }
        };
        if should_escalate {
            spawn(move || {
                sleep(PROCESS_GROUP_TERM_GRACE_PERIOD);
                if let Err(error) = kill_process_group(process_group_id) {
                    warn!("Failed to kill MCP process group {process_group_id}: {error}");
                }
            });
        }
    }

    #[cfg(not(unix))]
    fn maybe_terminate_process_group(&self) {}
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        if cfg!(unix) {
            self.maybe_terminate_process_group();
        }
    }
}
