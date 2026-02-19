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
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::PRESERVE_CRLF_FLAG;
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
    pub preserve_crlf: bool,
    pub file_paths: Vec<AbsolutePathBuf>,
    pub changes: std::collections::HashMap<PathBuf, FileChange>,
    pub exec_approval_requirement: ExecApprovalRequirement,
    pub timeout_ms: Option<u64>,
    pub codex_exe: Option<PathBuf>,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

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
        let mut args = vec![CODEX_APPLY_PATCH_ARG1.to_string()];
        if req.preserve_crlf {
            args.push(PRESERVE_CRLF_FLAG.to_string());
        }
        args.push(req.action.patch.clone());
        Ok(CommandSpec {
            program,
            args,
            cwd: req.action.cwd.clone(),
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
        !matches!(policy, AskForApproval::Never)
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
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = Self::build_command_spec(req)?;
        let env = attempt
            .env_for(spec, None)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CODEX_APPLY_PATCH_ARG1;
    use codex_apply_patch::PRESERVE_CRLF_FLAG;
    use codex_protocol::protocol::FileChange;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn build_command_spec_omits_crlf_flag_by_default() {
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("a.txt");
        let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
        let req = ApplyPatchRequest {
            action,
            preserve_crlf: false,
            file_paths: vec![AbsolutePathBuf::try_from(path.clone()).expect("abs path")],
            changes: HashMap::from([(
                path.clone(),
                FileChange::Add {
                    content: "hello".to_string(),
                },
            )]),
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            timeout_ms: None,
            codex_exe: Some(path),
        };

        let spec = ApplyPatchRuntime::build_command_spec(&req).expect("command spec");
        assert_eq!(
            spec.args.first().map(String::as_str),
            Some(CODEX_APPLY_PATCH_ARG1)
        );
        assert_eq!(spec.args.len(), 2);
    }

    #[test]
    fn build_command_spec_includes_crlf_flag_when_requested() {
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("a.txt");
        let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
        let req = ApplyPatchRequest {
            action,
            preserve_crlf: true,
            file_paths: vec![AbsolutePathBuf::try_from(path.clone()).expect("abs path")],
            changes: HashMap::from([(
                path.clone(),
                FileChange::Add {
                    content: "hello".to_string(),
                },
            )]),
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
            timeout_ms: None,
            codex_exe: Some(path),
        };

        let spec = ApplyPatchRuntime::build_command_spec(&req).expect("command spec");
        assert_eq!(
            spec.args,
            vec![
                CODEX_APPLY_PATCH_ARG1.to_string(),
                PRESERVE_CRLF_FLAG.to_string(),
                req.action.patch
            ]
        );
    }
}
