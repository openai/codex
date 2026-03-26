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
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxablePreference;
#[cfg(not(target_os = "windows"))]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;
#[cfg(not(target_os = "windows"))]
use std::fs::File;
#[cfg(not(target_os = "windows"))]
use std::io::BufRead;
#[cfg(not(target_os = "windows"))]
use std::io::BufReader;
#[cfg(not(target_os = "windows"))]
use std::path::Path;
use std::path::PathBuf;

#[cfg(not(target_os = "windows"))]
const APPLY_PATCH_SELF_EXEC_BIN_CANDIDATES: &[&str] = &[
    "codex",
    "codex-exec",
    "codex-app-server",
    "codex-mcp-server",
    "codex-tui",
    "codex-tui-app-server",
];

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

    #[cfg(target_os = "windows")]
    fn build_sandbox_command(
        req: &ApplyPatchRequest,
        codex_home: &std::path::Path,
    ) -> Result<SandboxCommand, ToolError> {
        Ok(Self::build_sandbox_command_with_program(
            req,
            codex_windows_sandbox::resolve_current_exe_for_launch(codex_home, "codex.exe"),
        ))
    }

    #[cfg(not(target_os = "windows"))]
    fn build_sandbox_command(
        req: &ApplyPatchRequest,
        configured_codex_exe: Option<&PathBuf>,
    ) -> Result<SandboxCommand, ToolError> {
        let exe = Self::resolve_apply_patch_program(configured_codex_exe)?;
        Ok(Self::build_sandbox_command_with_program(req, exe))
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_apply_patch_program(
        configured_codex_exe: Option<&PathBuf>,
    ) -> Result<PathBuf, ToolError> {
        if let Some(path) =
            configured_codex_exe.filter(|path| !Self::is_linux_sandbox_helper_path(path))
        {
            return Ok(path.clone());
        }

        if let Some(path) = Self::resolve_apply_patch_program_from_test_env() {
            return Ok(path);
        }

        std::env::current_exe()
            .map_err(|e| ToolError::Rejected(format!("failed to determine codex exe: {e}")))
    }

    #[cfg(not(target_os = "windows"))]
    fn is_linux_sandbox_helper_path(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some(CODEX_LINUX_SANDBOX_ARG0)
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_apply_patch_program_from_test_env() -> Option<PathBuf> {
        APPLY_PATCH_SELF_EXEC_BIN_CANDIDATES
            .iter()
            .find_map(|name| Self::resolve_test_binary(name))
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_test_binary(name: &str) -> Option<PathBuf> {
        Self::cargo_bin_env_keys(name)
            .into_iter()
            .filter_map(|key| std::env::var_os(key).map(PathBuf::from))
            .find_map(|path| Self::resolve_test_binary_path(path))
    }

    #[cfg(not(target_os = "windows"))]
    fn cargo_bin_env_keys(name: &str) -> Vec<String> {
        let mut env_keys = vec![format!("CARGO_BIN_EXE_{name}")];
        let underscored_name = name.replace('-', "_");
        if underscored_name != name {
            env_keys.push(format!("CARGO_BIN_EXE_{underscored_name}"));
        }

        env_keys
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_test_binary_path(path: PathBuf) -> Option<PathBuf> {
        if path.is_absolute() && path.exists() {
            return Some(path);
        }

        Self::resolve_runfile_path(&path)
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_runfile_path(path: &Path) -> Option<PathBuf> {
        if let Some(runfiles_dir) = std::env::var_os("RUNFILES_DIR") {
            let resolved = PathBuf::from(runfiles_dir).join(path);
            if resolved.exists() {
                return Some(resolved);
            }
        }

        let manifest = std::env::var_os("RUNFILES_MANIFEST_FILE")?;
        Self::lookup_runfiles_manifest(Path::new(&manifest), path)
    }

    #[cfg(not(target_os = "windows"))]
    fn lookup_runfiles_manifest(manifest: &Path, path: &Path) -> Option<PathBuf> {
        let path = path.to_string_lossy();
        let reader = BufReader::new(File::open(manifest).ok()?);
        for line in reader.lines() {
            let Ok(line) = line else {
                continue;
            };
            let Some((runfile, resolved_path)) = line.split_once(' ') else {
                continue;
            };
            if runfile == path {
                let resolved_path = PathBuf::from(resolved_path);
                if resolved_path.exists() {
                    return Some(resolved_path);
                }
            }
        }

        None
    }

    fn build_sandbox_command_with_program(req: &ApplyPatchRequest, exe: PathBuf) -> SandboxCommand {
        SandboxCommand {
            program: exe.to_string_lossy().to_string(),
            args: vec![
                CODEX_CORE_APPLY_PATCH_ARG1.to_string(),
                req.action.patch.clone(),
            ],
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
        #[cfg(target_os = "windows")]
        let command = Self::build_sandbox_command(req, &ctx.turn.config.codex_home)?;
        #[cfg(not(target_os = "windows"))]
        let command = Self::build_sandbox_command(req, ctx.turn.codex_linux_sandbox_exe.as_ref())?;
        let options = ExecOptions {
            expiration: req.timeout_ms.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let env = attempt
            .env_for(command, options, /*network*/ None)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
