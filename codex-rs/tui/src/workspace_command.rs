//! App-server-backed workspace command execution for TUI-owned background lookups.
//!
//! This module is the TUI boundary for short, non-interactive commands that need to run wherever
//! the active workspace lives. Callers describe a command in terms of argv, cwd, environment
//! overrides, timeout, and output cap; the runner translates that request to app-server
//! `command/exec`. Keeping this as a TUI-local abstraction lets status surfaces avoid knowing
//! whether the current app-server is embedded or remote.
//!
//! Commands sent through this path are best-effort metadata probes. They should not prompt for
//! stdin, should tolerate failure by omitting optional UI, and should keep output bounded so a
//! status-line refresh cannot grow into an unbounded background process.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::CommandExecResponse;
use codex_app_server_protocol::RequestId;
use uuid::Uuid;

/// Shared handle for running workspace commands from TUI components.
pub(crate) type WorkspaceCommandRunner = Arc<dyn WorkspaceCommandExecutor>;

/// Describes a bounded non-interactive command to execute in the active workspace.
///
/// The command is intentionally argv-based rather than shell-based so callers do not need to quote
/// user or repository data. `cwd` is interpreted by app-server relative to the workspace rules for
/// the active session, which is what makes the same request shape work for embedded and remote
/// app-server instances.
#[derive(Clone, Debug)]
pub(crate) struct WorkspaceCommand {
    /// Program and arguments to execute without shell interpolation.
    pub(crate) argv: Vec<String>,
    /// Working directory for the command, if different from app-server's session cwd.
    pub(crate) cwd: Option<PathBuf>,
    /// Environment overrides where `None` removes a variable.
    pub(crate) env: HashMap<String, Option<String>>,
    /// Maximum wall-clock duration before app-server cancels the command.
    pub(crate) timeout: Duration,
    /// Maximum captured stdout/stderr bytes returned by app-server.
    pub(crate) output_bytes_cap: usize,
}

impl WorkspaceCommand {
    /// Creates a workspace command with conservative defaults for status-style metadata probes.
    pub(crate) fn new(argv: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            cwd: None,
            env: HashMap::new(),
            timeout: Duration::from_secs(5),
            output_bytes_cap: 64 * 1024,
        }
    }

    /// Sets the command working directory.
    pub(crate) fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Adds or replaces one environment variable override.
    pub(crate) fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), Some(value.into()));
        self
    }
}

/// Captured result from a completed workspace command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCommandOutput {
    /// Process exit status code reported by app-server.
    pub(crate) exit_code: i32,
    /// Captured stdout after app-server output capping.
    pub(crate) stdout: String,
    /// Captured stderr after app-server output capping.
    pub(crate) stderr: String,
}

impl WorkspaceCommandOutput {
    /// Returns whether the process exited successfully.
    pub(crate) fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Transport or protocol failure before a command result was available.
///
/// Non-zero process exits are represented as `WorkspaceCommandOutput` so callers can distinguish
/// a normal probe miss from an app-server request failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCommandError {
    message: String,
}

impl WorkspaceCommandError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WorkspaceCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WorkspaceCommandError {}

/// Executes non-interactive workspace commands through the active TUI app-server session.
///
/// Implementations decide where the workspace lives. Callers provide argv/cwd/env and should not
/// branch on local versus remote execution.
pub(crate) trait WorkspaceCommandExecutor: Send + Sync {
    /// Runs a workspace command and returns captured output or an app-server request error.
    ///
    /// Callers should treat errors as infrastructure failures and should treat successful output
    /// with a non-zero exit code as ordinary command failure. Returning a future instead of using
    /// `async_trait` keeps the trait object-safe while matching the repo's native async trait
    /// conventions.
    fn run(
        &self,
        command: WorkspaceCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<WorkspaceCommandOutput, WorkspaceCommandError>> + Send + '_>,
    >;
}

/// Workspace command runner that forwards every request to the active app-server.
#[derive(Clone)]
pub(crate) struct AppServerWorkspaceCommandRunner {
    request_handle: AppServerRequestHandle,
}

impl AppServerWorkspaceCommandRunner {
    /// Creates a runner from an app-server request handle owned by the current TUI session.
    pub(crate) fn new(request_handle: AppServerRequestHandle) -> Self {
        Self { request_handle }
    }
}

impl WorkspaceCommandExecutor for AppServerWorkspaceCommandRunner {
    /// Sends the command as a one-off app-server `command/exec` request.
    ///
    /// The request is non-tty, does not stream stdin/stdout/stderr, and uses the caller's timeout
    /// and output cap. It leaves sandbox and permission profile selection to app-server so the same
    /// runner follows the active session's embedded or remote execution policy.
    fn run(
        &self,
        command: WorkspaceCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<WorkspaceCommandOutput, WorkspaceCommandError>> + Send + '_>,
    > {
        Box::pin(async move {
            let timeout_ms = i64::try_from(command.timeout.as_millis()).unwrap_or(i64::MAX);
            let env = if command.env.is_empty() {
                None
            } else {
                Some(command.env)
            };
            let response: CommandExecResponse = self
                .request_handle
                .request_typed(ClientRequest::OneOffCommandExec {
                    request_id: RequestId::String(format!("workspace-command-{}", Uuid::new_v4())),
                    params: CommandExecParams {
                        command: command.argv,
                        process_id: None,
                        tty: false,
                        stream_stdin: false,
                        stream_stdout_stderr: false,
                        output_bytes_cap: Some(command.output_bytes_cap),
                        disable_output_cap: false,
                        disable_timeout: false,
                        timeout_ms: Some(timeout_ms),
                        cwd: command.cwd,
                        env,
                        size: None,
                        sandbox_policy: None,
                        permission_profile: None,
                    },
                })
                .await
                .map_err(|err| WorkspaceCommandError::new(err.to_string()))?;

            Ok(WorkspaceCommandOutput {
                exit_code: response.exit_code,
                stdout: response.stdout,
                stderr: response.stderr,
            })
        })
    }
}
