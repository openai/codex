//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, builds the self-invocation command for
//! `codex --codex-run-as-apply-patch`, and runs under the current
//! `SandboxAttempt` with a minimal environment.
use crate::exec::ExecCapturePolicy;
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
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

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

    fn build_guardian_review_request(
        req: &ApplyPatchRequest,
        call_id: &str,
    ) -> GuardianApprovalRequest {
        GuardianApprovalRequest::ApplyPatch {
            id: call_id.to_string(),
            cwd: req.action.cwd.clone(),
            files: req.file_paths.clone(),
            change_count: req.changes.len(),
            patch: req.action.patch.clone(),
        }
    }

    fn build_command_spec(
        req: &ApplyPatchRequest,
        _codex_home: &std::path::Path,
    ) -> Result<Option<CommandSpec>, ToolError> {
        let exe = req
            .codex_exe
            .clone()
            .filter(is_codex_cli)
            .or_else(|| {
                req.codex_exe
                    .as_ref()
                    .and_then(resolve_adjacent_codex_cli)
                    .filter(|path| path.is_file())
            })
            .or_else(|| resolve_current_codex_cli(_codex_home).ok().flatten());
        let Some(exe) = exe else {
            return Ok(None);
        };
        let program = exe.to_string_lossy().to_string();
        Ok(Some(CommandSpec {
            program,
            args: vec![
                CODEX_CORE_APPLY_PATCH_ARG1.to_string(),
                req.action.patch.clone(),
            ],
            cwd: req.action.cwd.clone(),
            expiration: req.timeout_ms.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
            // Run apply_patch with a minimal environment for determinism and to avoid leaks.
            env: HashMap::new(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
            justification: None,
        }))
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
                let action = ApplyPatchRuntime::build_guardian_review_request(req, ctx.call_id);
                return review_approval_request(session, turn, action, retry_reason).await;
            }
            if req.permissions_preapproved && retry_reason.is_none() {
                return ReviewDecision::Approved;
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
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        if let Some(spec) = Self::build_command_spec(req, &ctx.turn.config.codex_home)? {
            let env = attempt
                .env_for(spec, /*network*/ None)
                .map_err(|err| ToolError::Codex(err.into()))?;
            let out = execute_env(env, Self::stdout_stream(ctx))
                .await
                .map_err(ToolError::Codex)?;
            return Ok(out);
        }

        let started = Instant::now();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit_code = match apply_patch_in_process(req, &mut stdout, &mut stderr) {
            Ok(()) => 0,
            Err(error) => {
                let _ = writeln!(&mut stderr, "{error}");
                1
            }
        };
        let stdout = String::from_utf8_lossy(&stdout).into_owned();
        let stderr = String::from_utf8_lossy(&stderr).into_owned();
        Ok(ExecToolCallOutput {
            exit_code,
            stdout: crate::exec::StreamOutput::new(stdout.clone()),
            stderr: crate::exec::StreamOutput::new(stderr.clone()),
            aggregated_output: crate::exec::StreamOutput::new(format!("{stdout}{stderr}")),
            duration: started.elapsed(),
            timed_out: false,
        })
    }
}

fn is_codex_cli(path: &PathBuf) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("codex") | Some("codex.exe")
    )
}

fn resolve_adjacent_codex_cli(path: &PathBuf) -> Option<PathBuf> {
    let sibling = if cfg!(windows) {
        path.with_file_name("codex.exe")
    } else {
        path.with_file_name("codex")
    };
    sibling.is_file().then_some(sibling)
}

fn resolve_current_codex_cli(codex_home: &std::path::Path) -> Result<Option<PathBuf>, ToolError> {
    #[cfg(target_os = "windows")]
    {
        let exe = codex_windows_sandbox::resolve_current_exe_for_launch(codex_home, "codex.exe");
        return Ok(is_codex_cli(&exe).then_some(exe));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let current_exe = std::env::current_exe()
            .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))?;
        if is_codex_cli(&current_exe) {
            return Ok(Some(current_exe));
        }
        let _ = codex_home;
        Ok(resolve_adjacent_codex_cli(&current_exe))
    }
}

fn apply_patch_in_process(
    req: &ApplyPatchRequest,
    stdout: &mut impl std::io::Write,
    _stderr: &mut impl std::io::Write,
) -> anyhow::Result<()> {
    let mut affected = codex_apply_patch::AffectedPaths {
        added: Vec::new(),
        modified: Vec::new(),
        deleted: Vec::new(),
    };

    let mut changes = req.action.changes().iter().collect::<Vec<_>>();
    changes.sort_by(|(left_path, _), (right_path, _)| left_path.cmp(right_path));

    for (path, change) in changes {
        match change {
            codex_apply_patch::ApplyPatchFileChange::Add { content } => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, content)?;
                affected
                    .added
                    .push(display_path_relative_to_cwd(path, &req.action.cwd));
            }
            codex_apply_patch::ApplyPatchFileChange::Delete { .. } => {
                std::fs::remove_file(path)?;
                affected
                    .deleted
                    .push(display_path_relative_to_cwd(path, &req.action.cwd));
            }
            codex_apply_patch::ApplyPatchFileChange::Update {
                move_path,
                new_content,
                ..
            } => {
                let destination = move_path.as_ref().unwrap_or(path);
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(destination, new_content)?;
                if let Some(move_path) = move_path
                    && move_path != path
                {
                    std::fs::remove_file(path)?;
                }
                affected
                    .modified
                    .push(display_path_relative_to_cwd(destination, &req.action.cwd));
            }
        }
    }

    affected.added.sort();
    affected.modified.sort();
    affected.deleted.sort();
    codex_apply_patch::print_summary(&affected, stdout)?;
    Ok(())
}

fn display_path_relative_to_cwd(path: &std::path::Path, cwd: &std::path::Path) -> PathBuf {
    path.strip_prefix(cwd).unwrap_or(path).to_path_buf()
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
