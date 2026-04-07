/*
Module: sandboxing

Core-owned adapter types for exec/runtime plumbing. Policy selection and
command transformation live in the codex-sandboxing crate; this module keeps
the exec-only metadata and translates transformed sandbox commands back into
ExecRequest for execution.
*/

use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::exec::StdoutStream;
use crate::exec::WindowsRestrictedTokenFilesystemOverlay;
use crate::exec::execute_exec_request;
#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::exec_output::ExecToolCallOutput;
pub use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxType;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct ExecOptions {
    pub(crate) expiration: ExecExpiration,
    pub(crate) capture_policy: ExecCapturePolicy,
}

#[derive(Debug)]
pub struct ExecRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub expiration: ExecExpiration,
    pub capture_policy: ExecCapturePolicy,
    pub sandbox: SandboxType,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub windows_sandbox_private_desktop: bool,
    pub sandbox_policy: SandboxPolicy,
    pub file_system_sandbox_policy: FileSystemSandboxPolicy,
    pub network_sandbox_policy: NetworkSandboxPolicy,
    pub(crate) windows_restricted_token_filesystem_overlay:
        Option<WindowsRestrictedTokenFilesystemOverlay>,
    pub arg0: Option<String>,
}

impl ExecRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command: Vec<String>,
        cwd: PathBuf,
        env: HashMap<String, String>,
        network: Option<NetworkProxy>,
        expiration: ExecExpiration,
        capture_policy: ExecCapturePolicy,
        sandbox: SandboxType,
        windows_sandbox_level: WindowsSandboxLevel,
        windows_sandbox_private_desktop: bool,
        sandbox_policy: SandboxPolicy,
        file_system_sandbox_policy: FileSystemSandboxPolicy,
        network_sandbox_policy: NetworkSandboxPolicy,
        arg0: Option<String>,
    ) -> Self {
        Self {
            command,
            cwd,
            env,
            network,
            expiration,
            capture_policy,
            sandbox,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            windows_restricted_token_filesystem_overlay: None,
            arg0,
        }
    }

    pub(crate) fn from_sandbox_exec_request(
        request: SandboxExecRequest,
        options: ExecOptions,
    ) -> Self {
        let SandboxExecRequest {
            command,
            cwd,
            mut env,
            network,
            sandbox,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            arg0,
        } = request;
        let ExecOptions {
            expiration,
            capture_policy,
        } = options;
        if !network_sandbox_policy.is_enabled() {
            env.insert(
                CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }
        #[cfg(target_os = "macos")]
        if sandbox == SandboxType::MacosSeatbelt {
            env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
        }
        Self {
            command,
            cwd,
            env,
            network,
            expiration,
            capture_policy,
            sandbox,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            windows_restricted_token_filesystem_overlay: None,
            arg0,
        }
    }
}

pub async fn execute_env(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
) -> codex_protocol::error::Result<ExecToolCallOutput> {
    execute_exec_request(exec_request, stdout_stream, /*after_spawn*/ None).await
}

pub async fn execute_exec_request_with_after_spawn(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> codex_protocol::error::Result<ExecToolCallOutput> {
    execute_exec_request(exec_request, stdout_stream, after_spawn).await
}
