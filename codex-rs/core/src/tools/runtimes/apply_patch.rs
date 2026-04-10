//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses that
//! decision to avoid re-prompting, applies through the remote filesystem when
//! the turn uses a remote environment, or builds the self-invocation command
//! for `codex --codex-run-as-apply-patch` and runs it under the current
//! `SandboxAttempt` with a minimal environment for local turns.
use crate::exec::ExecCapturePolicy;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::execute_env;
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
#[cfg(not(target_os = "windows"))]
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
#[cfg(target_os = "windows")]
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_FILE_ARG1;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxablePreference;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
#[cfg(target_os = "windows")]
use std::time::SystemTime;
#[cfg(target_os = "windows")]
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub struct ApplyPatchRequest {
    pub action: ApplyPatchAction,
    pub file_paths: Vec<AbsolutePathBuf>,
    pub changes: std::collections::HashMap<PathBuf, FileChange>,
    pub exec_approval_requirement: ExecApprovalRequirement,
    pub additional_permissions: Option<PermissionProfile>,
    pub permissions_preapproved: bool,
    pub timeout_ms: Option<u64>,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

struct BuiltApplyPatchCommand {
    command: SandboxCommand,
    temp_patch_path: Option<PathBuf>,
}

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
            cwd: req.action.cwd.to_path_buf(),
            files: req.file_paths.clone(),
            patch: req.action.patch.clone(),
        }
    }

    #[cfg(target_os = "windows")]
    fn build_sandbox_command(
        req: &ApplyPatchRequest,
        codex_home: &std::path::Path,
    ) -> Result<BuiltApplyPatchCommand, ToolError> {
        let patch_file = Self::write_patch_to_temp_file(codex_home, &req.action.patch)?;
        let patch_file_abs = AbsolutePathBuf::from_absolute_path(&patch_file).map_err(|err| {
            ToolError::Rejected(format!("failed to resolve patch temp file path: {err}"))
        })?;
        let mut command = Self::build_sandbox_command_with_program(
            req,
            codex_windows_sandbox::resolve_current_exe_for_launch(codex_home, "codex.exe"),
            vec![
                CODEX_CORE_APPLY_PATCH_FILE_ARG1.to_string(),
                patch_file.to_string_lossy().to_string(),
            ],
        );
        command.additional_permissions = Some(Self::with_patch_file_read_permission(
            req.additional_permissions.clone(),
            patch_file_abs,
        ));
        Ok(BuiltApplyPatchCommand {
            command,
            temp_patch_path: Some(patch_file),
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn build_sandbox_command(
        req: &ApplyPatchRequest,
        codex_self_exe: Option<&PathBuf>,
    ) -> Result<BuiltApplyPatchCommand, ToolError> {
        let exe = Self::resolve_apply_patch_program(codex_self_exe)?;
        Ok(BuiltApplyPatchCommand {
            command: Self::build_sandbox_command_with_program(
                req,
                exe,
                vec![
                    CODEX_CORE_APPLY_PATCH_ARG1.to_string(),
                    req.action.patch.clone(),
                ],
            ),
            temp_patch_path: None,
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_apply_patch_program(codex_self_exe: Option<&PathBuf>) -> Result<PathBuf, ToolError> {
        if let Some(path) = codex_self_exe {
            return Ok(path.clone());
        }

        std::env::current_exe()
            .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))
    }

    #[cfg(target_os = "windows")]
    fn write_patch_to_temp_file(
        codex_home: &std::path::Path,
        patch: &str,
    ) -> Result<PathBuf, ToolError> {
        let dir = codex_home.join("tmp").join("apply-patch");
        std::fs::create_dir_all(&dir).map_err(|err| {
            ToolError::Rejected(format!("failed to create apply_patch temp dir: {err}"))
        })?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = dir.join(format!("patch-{}-{nanos}.txt", std::process::id()));
        std::fs::write(&path, patch).map_err(|err| {
            ToolError::Rejected(format!("failed to write apply_patch temp file: {err}"))
        })?;
        Ok(path)
    }

    #[cfg(target_os = "windows")]
    fn with_patch_file_read_permission(
        mut profile: Option<PermissionProfile>,
        patch_file: AbsolutePathBuf,
    ) -> PermissionProfile {
        let profile = profile.get_or_insert_with(PermissionProfile::default);
        let file_system = profile.file_system.get_or_insert_with(Default::default);
        file_system
            .read
            .get_or_insert_with(Vec::new)
            .push(patch_file);
        profile.clone()
    }

    fn build_sandbox_command_with_program(
        req: &ApplyPatchRequest,
        exe: PathBuf,
        args: Vec<String>,
    ) -> SandboxCommand {
        SandboxCommand {
            program: exe.into_os_string(),
            args,
            cwd: req.action.cwd.clone(),
            // Run apply_patch with a minimal environment for determinism and to avoid leaks.
            env: HashMap::new(),
            additional_permissions: req.additional_permissions.clone(),
        }
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
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        if let Some(environment) = ctx.turn.environment.as_ref().filter(|env| env.is_remote()) {
            let started_at = Instant::now();
            let fs = environment.get_filesystem();
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let result = codex_apply_patch::apply_patch(
                &req.action.patch,
                &req.action.cwd,
                &mut stdout,
                &mut stderr,
                fs.as_ref(),
            )
            .await;
            let stdout = String::from_utf8_lossy(&stdout).into_owned();
            let stderr = String::from_utf8_lossy(&stderr).into_owned();
            let exit_code = if result.is_ok() { 0 } else { 1 };
            return Ok(ExecToolCallOutput {
                exit_code,
                stdout: StreamOutput::new(stdout.clone()),
                stderr: StreamOutput::new(stderr.clone()),
                aggregated_output: StreamOutput::new(format!("{stdout}{stderr}")),
                duration: started_at.elapsed(),
                timed_out: false,
            });
        }

        #[cfg(target_os = "windows")]
        let built_command = Self::build_sandbox_command(req, &ctx.turn.config.codex_home)?;
        #[cfg(not(target_os = "windows"))]
        let built_command = Self::build_sandbox_command(req, ctx.turn.codex_self_exe.as_ref())?;
        let BuiltApplyPatchCommand {
            command,
            temp_patch_path,
        } = built_command;
        let options = ExecOptions {
            expiration: req.timeout_ms.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let env = match attempt.env_for(command, options, /*network*/ None) {
            Ok(env) => env,
            Err(err) => {
                if let Some(temp_patch_path) = temp_patch_path {
                    let _ = std::fs::remove_file(temp_patch_path);
                }
                return Err(ToolError::Codex(err.into()));
            }
        };
        let out = execute_env(env, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex);
        if let Some(temp_patch_path) = temp_patch_path {
            let _ = std::fs::remove_file(temp_patch_path);
        }
        let out = out?;
        Ok(out)
    }
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
