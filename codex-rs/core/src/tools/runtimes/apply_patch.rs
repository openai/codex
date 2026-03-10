//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, builds the self-invocation command for
//! `codex --codex-run-as-apply-patch`, and runs under the current
//! `SandboxAttempt` with a minimal environment.
use crate::exec::ExecToolCallOutput;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct ApplyPatchRequest {
    pub action: ApplyPatchAction,
    pub file_paths: Vec<AbsolutePathBuf>,
    pub changes: std::collections::HashMap<PathBuf, FileChange>,
    pub exec_approval_requirement: ExecApprovalRequirement,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub permissions_preapproved: bool,
    pub timeout_ms: Option<u64>,
    pub codex_exe: Option<PathBuf>,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

impl ApplyPatchRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_guardian_review_request(req: &ApplyPatchRequest) -> GuardianApprovalRequest {
        GuardianApprovalRequest::ApplyPatch {
            cwd: req.action.cwd.clone(),
            files: req.file_paths.clone(),
            change_count: req.changes.len(),
            patch: req.action.patch.clone(),
        }
    }

    fn build_command_spec(
        req: &ApplyPatchRequest,
        _codex_home: &std::path::Path,
    ) -> Result<CommandSpec, ToolError> {
        let exe = if let Some(path) = &req.codex_exe {
            path.clone()
        } else {
            #[cfg(target_os = "windows")]
            {
                codex_windows_sandbox::resolve_current_exe_for_launch(_codex_home, "codex.exe")
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::env::current_exe().map_err(|e| {
                    ToolError::Rejected(format!("failed to determine codex exe: {e}"))
                })?
            }
        };
        let program = exe.to_string_lossy().to_string();
        Ok(CommandSpec {
            program,
            args: vec![
                CODEX_CORE_APPLY_PATCH_ARG1.to_string(),
                req.action.patch.clone(),
            ],
            cwd: req.action.cwd.clone(),
            expiration: req.timeout_ms.into(),
            // Run apply_patch with a minimal environment for determinism and to avoid leaks.
            env: HashMap::new(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
            justification: None,
        })
    }

    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
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
            if routes_approval_to_guardian(turn) {
                let action = ApplyPatchRuntime::build_guardian_review_request(req);
                return review_approval_request(session, turn, action, retry_reason).await;
            }
            if req.permissions_preapproved && retry_reason.is_none() {
                return ReviewDecision::Approved;
            }
            if let Some(reason) = retry_reason {
                let rx_approve = session
                    .request_patch_approval(turn, call_id, changes.clone(), Some(reason), None)
                    .await;
                return rx_approve.await.unwrap_or_default();
            }

            with_cached_approval(
                &session.services,
                "apply_patch",
                approval_keys,
                || async move {
                    let rx_approve = session
                        .request_patch_approval(turn, call_id, changes, None, None)
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
            AskForApproval::Reject(reject_config) => !reject_config.rejects_sandbox_approval(),
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
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = Self::build_command_spec(req, &ctx.turn.config.codex_home)?;
        let env = attempt
            .env_for(spec, None)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::RejectConfig;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    #[test]
    fn wants_no_sandbox_approval_reject_respects_sandbox_flag() {
        let runtime = ApplyPatchRuntime::new();
        assert!(runtime.wants_no_sandbox_approval(AskForApproval::OnRequest));
        assert!(
            !runtime.wants_no_sandbox_approval(AskForApproval::Reject(RejectConfig {
                sandbox_approval: true,
                rules: false,
                mcp_elicitations: false,
            }))
        );
        assert!(
            runtime.wants_no_sandbox_approval(AskForApproval::Reject(RejectConfig {
                sandbox_approval: false,
                rules: false,
                mcp_elicitations: false,
            }))
        );
    }

    #[test]
    fn guardian_review_request_includes_full_patch_without_duplicate_changes() {
        let path = std::env::temp_dir().join("guardian-apply-patch-test.txt");
        let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
        let expected_cwd = action.cwd.clone();
        let expected_patch = action.patch.clone();
        let request = ApplyPatchRequest {
            action,
            file_paths: vec![
                AbsolutePathBuf::from_absolute_path(&path).expect("temp path should be absolute"),
            ],
            changes: HashMap::from([(
                path,
                FileChange::Add {
                    content: "hello".to_string(),
                },
            )]),
            exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: None,
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            permissions_preapproved: false,
            timeout_ms: None,
            codex_exe: None,
        };

        let guardian_request = ApplyPatchRuntime::build_guardian_review_request(&request);

        assert_eq!(
            guardian_request,
            GuardianApprovalRequest::ApplyPatch {
                cwd: expected_cwd,
                files: request.file_paths,
                change_count: 1usize,
                patch: expected_patch,
            }
        );
    }
}
