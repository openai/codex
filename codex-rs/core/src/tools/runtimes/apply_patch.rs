//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, builds the self-invocation command for
//! `codex --codex-run-as-apply-patch`, and runs under the current
//! `SandboxAttempt` with a minimal environment.
use crate::CODEX_APPLY_PATCH_ARG1;
use crate::exec::ExecToolCallOutput;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::execute_env;
use crate::tools::sandboxing::{
    Approvable,
    ApprovalCtx,
    ApprovalDecision,
    SandboxAttempt,
    Sandboxable,
    SandboxablePreference,
    ToolCtx,
    ToolError,
    ToolRuntime,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ApplyPatchRequest {
    pub patch: String,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub user_explicitly_approved: bool,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

impl ApplyPatchRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_command_spec(req: &ApplyPatchRequest) -> Result<CommandSpec, ToolError> {
        use std::env;
        let exe = env::current_exe()
            .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))?;
        let program = exe.to_string_lossy().to_string();
        Ok(CommandSpec {
            program,
            args: vec![CODEX_APPLY_PATCH_ARG1.to_string(), req.patch.clone()],
            cwd: req.cwd.clone(),
            timeout_ms: req.timeout_ms,
            // Run apply_patch with a minimal environment for determinism and to avoid leaks.
            env: HashMap::new(),
            with_escalated_permissions: None,
            justification: None,
        })
    }

    fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.sub_id.clone(),
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

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    patch: String,
    cwd: PathBuf,
}

impl Approvable<ApplyPatchRequest> for ApplyPatchRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_key(&self, req: &ApplyPatchRequest) -> Self::ApprovalKey {
        ApprovalKey {
            patch: req.patch.clone(),
            cwd: req.cwd.clone(),
        }
    }

    fn reset_cache(&mut self) {}

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ApplyPatchRequest,
        ctx: ApprovalCtx<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ApprovalDecision> + Send + 'a>> {
        if let Some(reason) = ctx.retry_reason.clone() {
            return Box::pin(async move {
                ctx.session
                    .request_command_approval(
                        ctx.sub_id.to_string(),
                        ctx.call_id.to_string(),
                        vec!["apply_patch".to_string()],
                        req.cwd.clone(),
                        Some(reason),
                    )
                    .await
                    .into()
            });
        }

        // `apply_patch` verification/approval was already handled upstream in tools::mod
        // via `apply_patch::apply_patch`, which requests explicit approval when needed.
        // Avoid re-prompting here; map explicit approval to ApprovedForSession, else Approved.
        let decision = if req.user_explicitly_approved {
            ApprovalDecision::ApprovedForSession
        } else {
            ApprovalDecision::Approved
        };
        Box::pin(async move { decision })
    }
}

impl ToolRuntime<ApplyPatchRequest, ExecToolCallOutput> for ApplyPatchRuntime {
    async fn run(
        &mut self,
        req: &ApplyPatchRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = Self::build_command_spec(req)?;
        let env = attempt
            .env_for(&spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(&env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}
