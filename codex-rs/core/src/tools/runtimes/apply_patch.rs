//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, builds the self-invocation command for
//! `codex --codex-run-as-apply-patch`, and runs under the current
//! `SandboxAttempt` with a minimal environment.
use crate::CODEX_APPLY_PATCH_ARG1;
use crate::exec::ExecToolCallOutput;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct ApplyPatchRequest {
    pub patch: String,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub user_explicitly_approved: bool,
    pub codex_exe: Option<PathBuf>,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    patch: String,
    cwd: PathBuf,
}

impl ApplyPatchRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_command_spec(req: &ApplyPatchRequest) -> Result<CommandSpec, ToolError> {
        use std::env;
        let exe = if let Some(path) = &req.codex_exe {
            path.clone()
        } else {
            env::current_exe()
                .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))?
        };
        let program = exe.to_string_lossy().to_string();
        Ok(CommandSpec {
            program,
            args: vec![CODEX_APPLY_PATCH_ARG1.to_string(), req.patch.clone()],
            cwd: req.cwd.clone(),
            expiration: req.timeout_ms.into(),
            // Run apply_patch with a minimal environment for determinism and to avoid leaks.
            env: HashMap::new(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            justification: None,
        })
    }

    fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for ApplyPatchRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<ApplyPatchRequest> for ApplyPatchRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_key(&self, req: &ApplyPatchRequest) -> Self::ApprovalKey {
        ApprovalKey {
            patch: req.patch.clone(),
            cwd: req.cwd.clone(),
        }
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ApplyPatchRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let key = self.approval_key(req);
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let cwd = req.cwd.clone();
        let retry_reason = ctx.retry_reason.clone();
        let user_explicitly_approved = req.user_explicitly_approved;
        Box::pin(async move {
            with_cached_approval(&session.services, key, move || async move {
                if let Some(reason) = retry_reason {
                    session
                        .request_command_approval(
                            turn,
                            call_id,
                            vec!["apply_patch".to_string()],
                            cwd,
                            Some(reason),
                            None,
                        )
                        .await
                } else if user_explicitly_approved {
                    ReviewDecision::ApprovedForSession
                } else {
                    ReviewDecision::Approved
                }
            })
            .await
        })
    }

    fn wants_no_sandbox_approval(&self, policy: AskForApproval) -> bool {
        !matches!(policy, AskForApproval::Never)
    }
}

impl ToolRuntime<ApplyPatchRequest, ExecToolCallOutput> for ApplyPatchRuntime {
    async fn run(
        &mut self,
        req: &ApplyPatchRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        // When there is no sandbox in play (DangerFullAccess / bypassed sandbox), apply the patch
        // in-process. This avoids relying on the current executable implementing the arg0/argv1
        // dispatch behavior (which is not true for unit/integration test binaries).
        if attempt.sandbox == crate::exec::SandboxType::None {
            let started = Instant::now();
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();

            // Apply the patch relative to `req.cwd` without mutating the process CWD.
            let exit_code = match codex_apply_patch::parse_patch(&req.patch) {
                Ok(args) => {
                    match codex_apply_patch::apply_hunks_in_dir(
                        &args.hunks,
                        &req.cwd,
                        &mut stdout,
                        &mut stderr,
                    ) {
                        Ok(()) => 0,
                        Err(_) => 1,
                    }
                }
                // Reuse codex-apply-patch's error formatting for parse errors.
                Err(_) => {
                    match codex_apply_patch::apply_patch(&req.patch, &mut stdout, &mut stderr) {
                        Ok(()) => 0,
                        Err(_) => 1,
                    }
                }
            };
            let stdout = String::from_utf8_lossy(&stdout).to_string();
            let stderr = String::from_utf8_lossy(&stderr).to_string();
            let aggregated_output = format!("{stdout}{stderr}");
            return Ok(ExecToolCallOutput {
                exit_code,
                stdout: crate::exec::StreamOutput::new(stdout),
                stderr: crate::exec::StreamOutput::new(stderr),
                aggregated_output: crate::exec::StreamOutput::new(aggregated_output),
                duration: started.elapsed(),
                timed_out: false,
            });
        }

        let spec = Self::build_command_spec(req)?;
        let env = attempt
            .env_for(spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}
