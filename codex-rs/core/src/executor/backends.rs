use std::collections::HashMap;
use std::env;

use async_trait::async_trait;

use crate::CODEX_APPLY_PATCH_ARG1;
use crate::apply_patch::ApplyPatchExec;
use crate::exec::ExecParams;
use crate::function_tool::FunctionCallError;

pub(crate) enum ExecutionMode {
    Shell,
    ApplyPatch(ApplyPatchExec),
}

#[async_trait]
/// Backend-specific hooks that prepare and post-process execution requests for a
/// given [`ExecutionMode`].
pub(crate) trait ExecutionBackend: Send + Sync {
    fn prepare(
        &self,
        params: ExecParams,
        // Required for downcasting the apply_patch.
        mode: &ExecutionMode,
    ) -> Result<ExecParams, FunctionCallError>;

    fn stream_stdout(&self, _mode: &ExecutionMode) -> bool {
        true
    }
}

static SHELL_BACKEND: ShellBackend = ShellBackend;
static APPLY_PATCH_BACKEND: ApplyPatchBackend = ApplyPatchBackend;

pub(crate) fn backend_for_mode(mode: &ExecutionMode) -> &'static dyn ExecutionBackend {
    match mode {
        ExecutionMode::Shell => &SHELL_BACKEND,
        ExecutionMode::ApplyPatch(_) => &APPLY_PATCH_BACKEND,
    }
}

struct ShellBackend;

#[async_trait]
impl ExecutionBackend for ShellBackend {
    fn prepare(
        &self,
        params: ExecParams,
        mode: &ExecutionMode,
    ) -> Result<ExecParams, FunctionCallError> {
        match mode {
            ExecutionMode::Shell => Ok(params),
            _ => Err(FunctionCallError::RespondToModel(
                "shell backend invoked with non-shell mode".to_string(),
            )),
        }
    }
}

struct ApplyPatchBackend;

#[async_trait]
impl ExecutionBackend for ApplyPatchBackend {
    fn prepare(
        &self,
        params: ExecParams,
        mode: &ExecutionMode,
    ) -> Result<ExecParams, FunctionCallError> {
        match mode {
            ExecutionMode::ApplyPatch(exec) => {
                let path_to_codex = env::current_exe()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
                    .ok_or_else(|| {
                        FunctionCallError::RespondToModel(
                            "failed to determine path to codex executable".to_string(),
                        )
                    })?;

                let patch = exec.action.patch.clone();
                Ok(ExecParams {
                    command: vec![path_to_codex, CODEX_APPLY_PATCH_ARG1.to_string(), patch],
                    cwd: exec.action.cwd.clone(),
                    timeout_ms: params.timeout_ms,
                    // Run apply_patch with a minimal environment for determinism and to
                    // avoid leaking host environment variables into the patch process.
                    env: HashMap::new(),
                    with_escalated_permissions: params.with_escalated_permissions,
                    justification: params.justification,
                })
            }
            ExecutionMode::Shell => Err(FunctionCallError::RespondToModel(
                "apply_patch backend invoked without patch context".to_string(),
            )),
        }
    }

    fn stream_stdout(&self, _mode: &ExecutionMode) -> bool {
        false
    }
}
