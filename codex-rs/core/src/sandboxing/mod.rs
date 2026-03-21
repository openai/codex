use crate::error::Result;
use crate::exec::ExecToolCallOutput;
use crate::exec::StdoutStream;

pub(crate) use codex_sandbox::CommandSpec;
pub(crate) use codex_sandbox::ExecRequest;
pub(crate) use codex_sandbox::SandboxManager;
pub(crate) use codex_sandbox::SandboxPermissions;
pub(crate) use codex_sandbox::SandboxTransformError;
pub(crate) use codex_sandbox::effective_file_system_sandbox_policy;
pub(crate) use codex_sandbox::intersect_permission_profiles;
pub(crate) use codex_sandbox::merge_permission_profiles;
pub(crate) use codex_sandbox::normalize_additional_permissions;
pub(crate) use codex_sandbox::sandboxing::SandboxTransformRequest;

pub(crate) async fn execute_env(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    codex_sandbox::execute_env(exec_request, stdout_stream)
        .await
        .map_err(Into::into)
}

pub(crate) async fn execute_exec_request_with_after_spawn(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> Result<ExecToolCallOutput> {
    codex_sandbox::execute_exec_request_with_after_spawn(exec_request, stdout_stream, after_spawn)
        .await
        .map_err(Into::into)
}
