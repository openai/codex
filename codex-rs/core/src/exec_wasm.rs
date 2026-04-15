use async_channel::Sender;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxTransformError;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::error::CodexErr;
use crate::error::Result;
use crate::protocol::Event;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;

pub const DEFAULT_EXEC_COMMAND_TIMEOUT_MS: u64 = 10_000;
pub const IO_DRAIN_TIMEOUT_MS: u64 = 2_000;
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Debug)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub expiration: ExecExpiration,
    pub capture_policy: ExecCapturePolicy,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub windows_sandbox_private_desktop: bool,
    pub justification: Option<String>,
    pub arg0: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowsRestrictedTokenFilesystemOverlay {
    pub(crate) additional_deny_write_paths: Vec<AbsolutePathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecCapturePolicy {
    #[default]
    ShellTool,
    FullBuffer,
}

#[derive(Clone, Debug)]
pub enum ExecExpiration {
    Timeout(Duration),
    DefaultTimeout,
    Cancellation(CancellationToken),
}

impl From<Option<u64>> for ExecExpiration {
    fn from(timeout_ms: Option<u64>) -> Self {
        timeout_ms.map_or(ExecExpiration::DefaultTimeout, |timeout_ms| {
            ExecExpiration::Timeout(Duration::from_millis(timeout_ms))
        })
    }
}

impl From<u64> for ExecExpiration {
    fn from(timeout_ms: u64) -> Self {
        ExecExpiration::Timeout(Duration::from_millis(timeout_ms))
    }
}

impl ExecExpiration {
    pub(crate) async fn wait(self) {
        match self {
            ExecExpiration::Timeout(duration) => tokio::time::sleep(duration).await,
            ExecExpiration::DefaultTimeout => {
                tokio::time::sleep(Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)).await
            }
            ExecExpiration::Cancellation(cancel) => cancel.cancelled().await,
        }
    }

    pub(crate) fn timeout_ms(&self) -> Option<u64> {
        match self {
            ExecExpiration::Timeout(duration) => Some(duration.as_millis() as u64),
            ExecExpiration::DefaultTimeout => Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
            ExecExpiration::Cancellation(_) => None,
        }
    }
}

#[derive(Clone)]
pub struct StdoutStream {
    pub sub_id: String,
    pub call_id: String,
    pub tx_event: Sender<Event>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamOutput<T: Clone> {
    pub text: T,
    pub truncated_after_lines: Option<u32>,
}

impl<T: Clone> StreamOutput<T> {
    pub fn new(text: T) -> Self {
        Self {
            text,
            truncated_after_lines: None,
        }
    }
}

impl StreamOutput<Vec<u8>> {
    pub fn from_utf8_lossy(&self) -> StreamOutput<String> {
        StreamOutput {
            text: String::from_utf8_lossy(&self.text).into_owned(),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
    pub timed_out: bool,
}

impl Default for ExecToolCallOutput {
    fn default() -> Self {
        Self {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::ZERO,
            timed_out: false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn process_exec_tool_call(
    _params: ExecParams,
    _sandbox_policy: &SandboxPolicy,
    _file_system_sandbox_policy: &FileSystemSandboxPolicy,
    _network_sandbox_policy: NetworkSandboxPolicy,
    _sandbox_cwd: &Path,
    _codex_linux_sandbox_exe: &Option<PathBuf>,
    _use_legacy_landlock: bool,
    _stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    Err(CodexErr::UnsupportedOperation(
        "process execution is unavailable in wasm".to_string(),
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn build_exec_request(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    _sandbox_cwd: &Path,
    _codex_linux_sandbox_exe: &Option<PathBuf>,
    _use_legacy_landlock: bool,
) -> Result<ExecRequest> {
    Ok(ExecRequest::new(
        params.command,
        params.cwd,
        params.env,
        params.network,
        params.expiration,
        params.capture_policy,
        SandboxType::None,
        params.windows_sandbox_level,
        params.windows_sandbox_private_desktop,
        sandbox_policy.clone(),
        file_system_sandbox_policy.clone(),
        network_sandbox_policy,
        params.arg0,
    ))
}

pub(crate) async fn execute_exec_request(
    _exec_request: ExecRequest,
    _effective_policy: &SandboxPolicy,
    _stdout_stream: Option<StdoutStream>,
    _after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> Result<ExecToolCallOutput> {
    Err(CodexErr::UnsupportedOperation(
        "process execution is unavailable in wasm".to_string(),
    ))
}

pub(crate) fn is_likely_sandbox_denied(
    _sandbox_type: SandboxType,
    _exec_output: &ExecToolCallOutput,
) -> bool {
    false
}

pub(crate) fn unsupported_windows_restricted_token_sandbox_reason(
    _file_system_sandbox_policy: &FileSystemSandboxPolicy,
    _sandbox_permissions: SandboxPermissions,
) -> Option<String> {
    None
}

pub(crate) fn resolve_windows_restricted_token_filesystem_overlay(
    _file_system_sandbox_policy: &FileSystemSandboxPolicy,
    _sandbox_permissions: SandboxPermissions,
) -> std::result::Result<Option<WindowsRestrictedTokenFilesystemOverlay>, String> {
    Ok(None)
}

impl From<SandboxTransformError> for CodexErr {
    fn from(err: SandboxTransformError) -> Self {
        match err {
            SandboxTransformError::MissingLinuxSandboxExecutable => {
                CodexErr::LandlockSandboxExecutableNotProvided
            }
            SandboxTransformError::SeatbeltUnavailable => {
                CodexErr::UnsupportedOperation("seatbelt sandbox is unavailable".to_string())
            }
            other => CodexErr::UnsupportedOperation(other.to_string()),
        }
    }
}
