/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds sandbox transform inputs, and runs them under the current SandboxAttempt.
*/
#[cfg(unix)]
pub(crate) mod unix_escalation;
pub(crate) mod zsh_fork_backend;

use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecCapturePolicy;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::guardian::review_approval_request;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::shell::ShellType;
use crate::tools::flat_tool_name;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::RuntimePathPrepends;
#[cfg(unix)]
use crate::tools::runtimes::apply_zsh_fork_path_prepend;
use crate::tools::runtimes::build_sandbox_command;
use crate::tools::runtimes::disable_powershell_profile_for_elevated_windows_sandbox;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::managed_network_for_sandbox_permissions;
use crate::tools::sandboxing::sandbox_permissions_preserving_denied_reads;
use crate::tools::sandboxing::with_cached_approval;
use codex_network_proxy::NetworkProxy;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxablePreference;
use codex_shell_command::powershell::parse_powershell_command_into_plain_commands;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub shell_type: Option<ShellType>,
    pub hook_command: String,
    pub cwd: AbsolutePathBuf,
    pub timeout_ms: Option<u64>,
    pub cancellation_token: CancellationToken,
    pub env: HashMap<String, String>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Selects `ShellRuntime` behavior for different callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellRuntimeBackend {
    /// Legacy backend for the `shell_command` tool.
    ///
    /// Keeps `shell_command` on the standard shell runtime flow without the
    /// zsh-fork shell-escalation adapter.
    ShellCommandClassic,
    /// zsh-fork backend for the `shell_command` tool.
    ///
    /// On Unix, attempts to run via the zsh-fork + `codex-shell-escalation`
    /// adapter, with fallback to the standard shell runtime flow if
    /// prerequisites are not met.
    ShellCommandZshFork,
}

pub struct ShellRuntime {
    backend: ShellRuntimeBackend,
}

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: AbsolutePathBuf,
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<AdditionalPermissionProfile>,
}

impl ShellRuntime {
    pub(crate) fn for_shell_command(backend: ShellRuntimeBackend) -> Self {
        Self { backend }
    }

    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for ShellRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<ShellRequest> for ShellRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_keys(&self, req: &ShellRequest) -> Vec<Self::ApprovalKey> {
        vec![ApprovalKey {
            command: canonicalize_command_for_approval(&req.command),
            cwd: req.cwd.clone(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
        }]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let keys = self.approval_keys(req);
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let retry_reason = ctx.retry_reason.clone();
        let reason = retry_reason.clone().or_else(|| req.justification.clone());
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            if let Some(review_id) = guardian_review_id {
                return review_approval_request(
                    session,
                    turn,
                    review_id,
                    GuardianApprovalRequest::Shell {
                        id: call_id,
                        command,
                        cwd: cwd.clone(),
                        sandbox_permissions: req.sandbox_permissions,
                        additional_permissions: req.additional_permissions.clone(),
                        justification: req.justification.clone(),
                    },
                    retry_reason,
                )
                .await;
            }
            with_cached_approval(&session.services, "shell", keys, move || async move {
                let available_decisions = None;
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        /*approval_id*/ None,
                        command,
                        cwd,
                        reason,
                        ctx.network_approval_context.clone(),
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                        req.additional_permissions.clone(),
                        available_decisions,
                    )
                    .await
            })
            .await
        })
    }

    fn exec_approval_requirement(&self, req: &ShellRequest) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn permission_request_payload(&self, req: &ShellRequest) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload::bash(
            req.hook_command.clone(),
            req.justification.clone(),
        ))
    }

    fn sandbox_permissions(&self, req: &ShellRequest) -> SandboxPermissions {
        req.sandbox_permissions
    }
}

fn maybe_add_windows_powershell_path_hint(
    output: &mut ExecToolCallOutput,
    command: &[String],
    cwd: &AbsolutePathBuf,
    env: &HashMap<String, String>,
) {
    if !looks_like_powershell_command_not_found(output) {
        return;
    }

    let Some((program, resolved_path)) = find_powershell_path_resolved_command(command, cwd, env)
    else {
        return;
    };

    let note = format!(
        "Codex note: `{program}` resolves on PATH to `{}`, but PowerShell reported it as not found. In the Windows Sandbox this usually means execution was blocked by sandbox policy rather than the binary being missing.",
        resolved_path.display()
    );
    append_note(&mut output.stderr.text, &note);
    append_note(&mut output.aggregated_output.text, &note);
}

fn looks_like_powershell_command_not_found(output: &ExecToolCallOutput) -> bool {
    if output.exit_code == 0 {
        return false;
    }

    let combined = format!(
        "{}\n{}\n{}",
        output.stderr.text, output.stdout.text, output.aggregated_output.text
    )
    .to_ascii_lowercase();

    [
        "commandnotfoundexception",
        "is not recognized as the name of a cmdlet",
        "was not found, but does exist in the current location",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
}

fn find_powershell_path_resolved_command(
    command: &[String],
    cwd: &AbsolutePathBuf,
    env: &HashMap<String, String>,
) -> Option<(String, std::path::PathBuf)> {
    let parsed_commands = parse_powershell_command_into_plain_commands(command)?;
    let search_path = env
        .iter()
        .find_map(|(key, value)| key.eq_ignore_ascii_case("PATH").then_some(value.as_str()));

    parsed_commands.into_iter().find_map(|plain_command| {
        let executable = plain_command.first()?.trim();
        if !should_probe_path_lookup(executable) {
            return None;
        }

        let resolved = which::which_in(executable, search_path, cwd.as_path()).ok()?;
        Some((executable.to_string(), resolved))
    })
}

fn should_probe_path_lookup(executable: &str) -> bool {
    !executable.is_empty() && !executable.starts_with('.') && !executable.contains(['\\', '/', ':'])
}

fn append_note(target: &mut String, note: &str) {
    if target.contains(note) {
        return;
    }
    if !target.is_empty() && !target.ends_with('\n') {
        target.push('\n');
    }
    target.push_str(note);
    target.push('\n');
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    fn network_approval_spec(
        &self,
        req: &ShellRequest,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let file_system_sandbox_policy = ctx.turn.file_system_sandbox_policy();
        let sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), sandbox_permissions)?;
        Some(NetworkApprovalSpec {
            network: Some(network.clone()),
            mode: NetworkApprovalMode::Immediate,
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: flat_tool_name(&ctx.tool_name).into_owned(),
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                sandbox_permissions: req.sandbox_permissions,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: None,
            },
            command: req.hook_command.clone(),
        })
    }

    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let session_shell = ctx.session.user_shell();
        let (file_system_sandbox_policy, _) = attempt.permissions.to_runtime_permissions();
        let sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let managed_network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), sandbox_permissions);
        let env = exec_env_for_sandbox_permissions(&req.env, sandbox_permissions);
        let explicit_env_overrides = req.explicit_env_overrides.clone();
        #[cfg(unix)]
        let (env, runtime_path_prepends) = {
            let mut env = env;
            let mut runtime_path_prepends = RuntimePathPrepends::default();
            crate::tools::runtimes::apply_package_path_prepend(
                &mut env,
                &mut runtime_path_prepends,
            );
            if self.backend == ShellRuntimeBackend::ShellCommandZshFork
                && let Some(shell_zsh_path) = ctx.session.services.shell_zsh_path.as_deref()
            {
                apply_zsh_fork_path_prepend(&mut env, &mut runtime_path_prepends, shell_zsh_path);
            }
            (env, runtime_path_prepends)
        };
        #[cfg(not(unix))]
        let runtime_path_prepends = RuntimePathPrepends::default();
        let command = maybe_wrap_shell_lc_with_snapshot(
            &req.command,
            session_shell.as_ref(),
            &req.cwd,
            &explicit_env_overrides,
            &env,
            &runtime_path_prepends,
        );
        let command = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            req.shell_type.as_ref(),
            attempt.sandbox,
            attempt.windows_sandbox_level,
        );
        let command = if matches!(session_shell.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        if self.backend == ShellRuntimeBackend::ShellCommandZshFork {
            match zsh_fork_backend::maybe_run_shell_command(req, attempt, ctx, &command).await? {
                Some(out) => return Ok(out),
                None => {
                    tracing::warn!(
                        "ZshFork backend specified, but conditions for using it were not met, falling back to normal execution",
                    );
                }
            }
        }

        let command =
            build_sandbox_command(&command, &req.cwd, &env, req.additional_permissions.clone())?;
        let mut expiration: crate::exec::ExecExpiration = req.timeout_ms.into();
        expiration = expiration.with_cancellation(req.cancellation_token.clone());
        if let Some(cancellation) = attempt.network_denial_cancellation_token.clone() {
            expiration = expiration.with_cancellation(cancellation);
        }
        let options = ExecOptions {
            expiration,
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let env = attempt
            .env_for(command, options, managed_network)
            .map_err(ToolError::Codex)?;
        let diagnostic_env = env.env.clone();
        let mut out = execute_env(env, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        if matches!(session_shell.shell_type, ShellType::PowerShell) {
            maybe_add_windows_powershell_path_hint(
                &mut out,
                &req.command,
                &req.cwd,
                &diagnostic_env,
            );
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::append_note;
    use super::find_powershell_path_resolved_command;
    use super::looks_like_powershell_command_not_found;
    use super::maybe_add_windows_powershell_path_hint;
    use codex_protocol::exec_output::ExecToolCallOutput;
    use codex_protocol::exec_output::StreamOutput;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::collections::HashMap;
    #[cfg(windows)]
    use std::fs;
    #[cfg(windows)]
    use tempfile::tempdir;

    #[test]
    fn detects_powershell_command_not_found_output() {
        let output = ExecToolCallOutput {
            exit_code: 1,
            stderr: StreamOutput::new(
                "go : The term 'go' is not recognized as the name of a cmdlet".to_string(),
            ),
            ..ExecToolCallOutput::default()
        };

        assert!(looks_like_powershell_command_not_found(&output));
    }

    #[test]
    fn append_note_adds_single_copy() {
        let mut text = String::from("prefix");
        append_note(&mut text, "Codex note: hello");
        append_note(&mut text, "Codex note: hello");
        assert_eq!(text, "prefix\nCodex note: hello\n");
    }

    #[cfg(windows)]
    #[test]
    fn finds_path_resolved_powershell_command() {
        let dir = tempdir().expect("tempdir");
        let command_path = dir.path().join("go.cmd");
        fs::write(&command_path, "@echo off\r\nexit /b 0\r\n").expect("write stub");

        let cwd = AbsolutePathBuf::from_absolute_path(dir.path()).expect("absolute cwd");
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), dir.path().display().to_string());

        let resolved = find_powershell_path_resolved_command(
            &[
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "go version".to_string(),
            ],
            &cwd,
            &env,
        )
        .expect("resolved command");

        assert_eq!(resolved.0, "go");
        assert_eq!(resolved.1, command_path);
    }

    #[cfg(windows)]
    #[test]
    fn appends_windows_sandbox_hint_when_path_command_resolves() {
        let dir = tempdir().expect("tempdir");
        let command_path = dir.path().join("go.cmd");
        fs::write(&command_path, "@echo off\r\nexit /b 0\r\n").expect("write stub");

        let cwd = AbsolutePathBuf::from_absolute_path(dir.path()).expect("absolute cwd");
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), dir.path().display().to_string());

        let mut output = ExecToolCallOutput {
            exit_code: 1,
            stderr: StreamOutput::new(
                "go : The term 'go' is not recognized as the name of a cmdlet".to_string(),
            ),
            aggregated_output: StreamOutput::new(
                "go : The term 'go' is not recognized as the name of a cmdlet".to_string(),
            ),
            ..ExecToolCallOutput::default()
        };

        maybe_add_windows_powershell_path_hint(
            &mut output,
            &[
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "go version".to_string(),
            ],
            &cwd,
            &env,
        );

        assert!(
            output
                .stderr
                .text
                .contains("Codex note: `go` resolves on PATH")
        );
        assert!(
            output
                .stderr
                .text
                .contains(command_path.to_string_lossy().as_ref())
        );
        assert!(
            output
                .aggregated_output
                .text
                .contains("execution was blocked by sandbox policy")
        );
    }
}
