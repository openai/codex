use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

#[cfg(unix)]
use codex_utils_pty::process_group::kill_process_group;
#[cfg(unix)]
use codex_utils_pty::process_group::terminate_process_group;
use futures::FutureExt;
use futures::future::BoxFuture;
use rmcp::transport::child_process::TokioChildProcess;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tracing::info;
use tracing::warn;

use crate::program_resolver;
use crate::utils::create_env_for_mcp_server;

/// Runtime responsible for creating the byte transport for an MCP stdio server.
///
/// This trait hides where the stdio process runs. `RmcpClient` owns MCP
/// semantics and asks this trait for a transport. The local implementation
/// preserves the existing child-process behavior.
pub trait StdioTransportRuntime: private::Sealed + Send + Sync {
    /// Create the transport that rmcp will use for one MCP stdio server.
    fn create_transport(
        &self,
        params: StdioTransportParams,
    ) -> BoxFuture<'static, io::Result<StdioTransport>>;
}

/// Runtime that starts MCP stdio servers as local child processes.
///
/// This is the existing behavior for local MCP servers: the orchestrator
/// process spawns the configured command and rmcp talks to the child's local
/// stdin/stdout pipes directly.
#[derive(Clone)]
pub struct LocalStdioTransportRuntime;

/// Command-line process shape shared by stdio runtimes.
#[derive(Clone)]
pub struct StdioTransportParams {
    program: OsString,
    args: Vec<OsString>,
    env: Option<HashMap<OsString, OsString>>,
    env_vars: Vec<String>,
    cwd: Option<PathBuf>,
}

/// Opaque stdio transport produced by a [`StdioTransportRuntime`].
pub struct StdioTransport {
    pub(super) inner: StdioTransportInner,
}

pub(super) enum StdioTransportInner {
    Local {
        transport: TokioChildProcess,
        process_group_guard: Option<ProcessGroupGuard>,
    },
}

#[cfg(unix)]
const PROCESS_GROUP_TERM_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[cfg(unix)]
pub(super) struct ProcessGroupGuard {
    process_group_id: u32,
}

#[cfg(not(unix))]
pub(super) struct ProcessGroupGuard;

mod private {
    pub trait Sealed {}
}

impl private::Sealed for LocalStdioTransportRuntime {}

impl StdioTransportParams {
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

impl StdioTransportRuntime for LocalStdioTransportRuntime {
    fn create_transport(
        &self,
        params: StdioTransportParams,
    ) -> BoxFuture<'static, io::Result<StdioTransport>> {
        async move { create_local_stdio_transport(params) }.boxed()
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
            std::thread::spawn(move || {
                std::thread::sleep(PROCESS_GROUP_TERM_GRACE_PERIOD);
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

fn create_local_stdio_transport(params: StdioTransportParams) -> io::Result<StdioTransport> {
    let StdioTransportParams {
        program,
        args,
        env,
        env_vars,
        cwd,
    } = params;
    let program_name = program.to_string_lossy().into_owned();
    let envs = create_env_for_mcp_server(env, &env_vars);
    let resolved_program = program_resolver::resolve(program, &envs).map_err(io::Error::other)?;

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

    Ok(StdioTransport {
        inner: StdioTransportInner::Local {
            transport,
            process_group_guard,
        },
    })
}
