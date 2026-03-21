use crate::error::Result;
use crate::protocol::SandboxPolicy;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use std::path::Path;
use std::path::PathBuf;

pub(crate) use codex_sandbox::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
pub(crate) use codex_sandbox::ExecCapturePolicy;
pub(crate) use codex_sandbox::ExecExpiration;
pub(crate) use codex_sandbox::ExecParams;
pub(crate) use codex_sandbox::ExecToolCallOutput;
pub(crate) use codex_sandbox::MAX_EXEC_OUTPUT_DELTAS_PER_CALL;
pub(crate) use codex_sandbox::SandboxType;
pub(crate) use codex_sandbox::StdoutStream;
pub(crate) use codex_sandbox::StreamOutput;
pub(crate) use codex_sandbox::is_likely_sandbox_denied;

pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_cwd: &Path,
    codex_linux_sandbox_exe: &Option<PathBuf>,
    use_legacy_landlock: bool,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    codex_sandbox::process_exec_tool_call(
        params,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox_cwd,
        codex_linux_sandbox_exe,
        use_legacy_landlock,
        stdout_stream,
    )
    .await
    .map_err(Into::into)
}

pub fn build_exec_request(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_cwd: &Path,
    codex_linux_sandbox_exe: &Option<PathBuf>,
    use_legacy_landlock: bool,
) -> Result<crate::sandboxing::ExecRequest> {
    codex_sandbox::build_exec_request(
        params,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox_cwd,
        codex_linux_sandbox_exe,
        use_legacy_landlock,
    )
    .map_err(Into::into)
}

pub(crate) async fn execute_exec_request(
    exec_request: crate::sandboxing::ExecRequest,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> Result<ExecToolCallOutput> {
    codex_sandbox::execute_exec_request(exec_request, sandbox_policy, stdout_stream, after_spawn)
        .await
        .map_err(Into::into)
}
