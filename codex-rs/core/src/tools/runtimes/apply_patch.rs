//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, then applies the verified action directly
//! through the turn environment's filesystem with the effective sandbox policy.
use crate::apply_patch::EnvironmentApplyPatchFileSystem;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_apply_patch::ApplyPatchAction;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxablePreference;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub struct ApplyPatchRequest {
    pub action: ApplyPatchAction,
    pub file_paths: Vec<AbsolutePathBuf>,
    pub changes: std::collections::HashMap<PathBuf, FileChange>,
    pub sandbox_policy: SandboxPolicy,
    pub exec_approval_requirement: ExecApprovalRequirement,
    pub permissions_preapproved: bool,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

impl ApplyPatchRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_guardian_review_request(
        req: &ApplyPatchRequest,
        call_id: &str,
    ) -> GuardianApprovalRequest {
        GuardianApprovalRequest::ApplyPatch {
            id: call_id.to_string(),
            cwd: req.action.cwd.clone(),
            files: req.file_paths.clone(),
            patch: req.action.patch.clone(),
        }
    }

    async fn run_with_environment_fs(
        req: &ApplyPatchRequest,
        fs: EnvironmentApplyPatchFileSystem,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let affected: codex_apply_patch::AffectedPaths =
            codex_apply_patch::apply_action_with_fs(&req.action, &fs)
                .await
                .map_err(|err| ToolError::Rejected(err.to_string()))?;
        let affected = relativize_affected_paths(&affected, &req.action.cwd);
        let mut stdout = Vec::new();
        codex_apply_patch::print_summary(&affected, &mut stdout)
            .map_err(|err| ToolError::Rejected(err.to_string()))?;
        let stdout = String::from_utf8(stdout).map_err(|err| {
            ToolError::Rejected(format!("apply_patch wrote non-UTF-8 output: {err}"))
        })?;
        Ok(ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new(stdout.clone()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(stdout),
            duration: Duration::ZERO,
            timed_out: false,
        })
    }
}

fn relativize_affected_paths(
    affected: &codex_apply_patch::AffectedPaths,
    cwd: &Path,
) -> codex_apply_patch::AffectedPaths {
    codex_apply_patch::AffectedPaths {
        added: affected
            .added
            .iter()
            .map(|path| summary_path(path, cwd))
            .collect(),
        modified: affected
            .modified
            .iter()
            .map(|path| summary_path(path, cwd))
            .collect(),
        deleted: affected
            .deleted
            .iter()
            .map(|path| summary_path(path, cwd))
            .collect(),
    }
}

fn summary_path(path: &Path, cwd: &Path) -> PathBuf {
    match path.strip_prefix(cwd) {
        Ok(relative) if !relative.as_os_str().is_empty() => relative.to_path_buf(),
        _ => path.to_path_buf(),
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
    type ApprovalKey = AbsolutePathBuf;

    fn approval_keys(&self, req: &ApplyPatchRequest) -> Vec<Self::ApprovalKey> {
        req.file_paths.clone()
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ApplyPatchRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let retry_reason = ctx.retry_reason.clone();
        let approval_keys = self.approval_keys(req);
        let changes = req.changes.clone();
        Box::pin(async move {
            if req.permissions_preapproved && retry_reason.is_none() {
                return ReviewDecision::Approved;
            }
            if routes_approval_to_guardian(turn) {
                let action = ApplyPatchRuntime::build_guardian_review_request(req, ctx.call_id);
                return review_approval_request(session, turn, action, retry_reason).await;
            }
            if let Some(reason) = retry_reason {
                let rx_approve = session
                    .request_patch_approval(
                        turn,
                        call_id,
                        changes.clone(),
                        Some(reason),
                        /*grant_root*/ None,
                    )
                    .await;
                return rx_approve.await.unwrap_or_default();
            }

            with_cached_approval(
                &session.services,
                "apply_patch",
                approval_keys,
                || async move {
                    let rx_approve = session
                        .request_patch_approval(
                            turn, call_id, changes, /*reason*/ None, /*grant_root*/ None,
                        )
                        .await;
                    rx_approve.await.unwrap_or_default()
                },
            )
            .await
        })
    }

    fn wants_no_sandbox_approval(&self, policy: AskForApproval) -> bool {
        match policy {
            AskForApproval::Never => false,
            AskForApproval::Granular(granular_config) => granular_config.allows_sandbox_approval(),
            AskForApproval::OnFailure => true,
            AskForApproval::OnRequest => true,
            AskForApproval::UnlessTrusted => true,
        }
    }

    // apply_patch approvals are decided upstream by assess_patch_safety.
    //
    // This override ensures the orchestrator runs the patch approval flow when required instead
    // of falling back to the global exec approval policy.
    fn exec_approval_requirement(
        &self,
        req: &ApplyPatchRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }
}

impl ToolRuntime<ApplyPatchRequest, ExecToolCallOutput> for ApplyPatchRuntime {
    async fn run(
        &mut self,
        req: &ApplyPatchRequest,
        _attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let fs = EnvironmentApplyPatchFileSystem::for_apply(
            ctx.turn.environment.get_filesystem(),
            req.action.cwd.clone(),
            req.sandbox_policy.clone(),
        );
        Self::run_with_environment_fs(req, fs).await
    }
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
