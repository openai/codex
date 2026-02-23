/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds a CommandSpec, and runs it under the current SandboxAttempt.
*/
use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::is_likely_sandbox_denied;
use crate::features::Feature;
use crate::powershell::prefix_powershell_script_with_utf8;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::shell::ShellType;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::build_command_spec;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::bash::parse_shell_lc_single_command_prefix;
#[cfg(unix)]
use codex_shell_escalation::unix::core_shell_escalation::ShellActionProvider;
#[cfg(unix)]
use codex_shell_escalation::unix::core_shell_escalation::ShellPolicyFactory;
#[cfg(unix)]
use codex_shell_escalation::unix::escalate_protocol::EscalateAction;
#[cfg(unix)]
use codex_shell_escalation::unix::escalate_server::ExecParams;
#[cfg(unix)]
use codex_shell_escalation::unix::escalate_server::ExecResult;
#[cfg(unix)]
use codex_shell_escalation::unix::escalate_server::SandboxState;
#[cfg(unix)]
use codex_shell_escalation::unix::escalate_server::ShellCommandExecutor;
#[cfg(unix)]
use codex_shell_escalation::unix::escalate_server::run_escalate_server;
#[cfg(unix)]
use codex_shell_escalation::unix::stopwatch::Stopwatch;
#[cfg(unix)]
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use shlex::try_join as shlex_try_join;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

#[derive(Default)]
pub struct ShellRuntime;

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: PathBuf,
    sandbox_permissions: SandboxPermissions,
}

impl ShellRuntime {
    pub fn new() -> Self {
        Self
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
        let reason = ctx
            .retry_reason
            .clone()
            .or_else(|| req.justification.clone());
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        Box::pin(async move {
            with_cached_approval(&session.services, "shell", keys, move || async move {
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        None,
                        command,
                        cwd,
                        reason,
                        ctx.network_approval_context.clone(),
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                    )
                    .await
            })
            .await
        })
    }

    fn exec_approval_requirement(&self, req: &ShellRequest) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn sandbox_mode_for_first_attempt(&self, req: &ShellRequest) -> SandboxOverride {
        if req.sandbox_permissions.requires_escalated_permissions()
            || matches!(
                req.exec_approval_requirement,
                ExecApprovalRequirement::Skip {
                    bypass_sandbox: true,
                    ..
                }
            )
        {
            SandboxOverride::BypassSandboxFirstAttempt
        } else {
            SandboxOverride::NoOverride
        }
    }
}

#[cfg(unix)]
struct CoreShellActionProvider {
    policy: Arc<RwLock<Policy>>,
    session: std::sync::Arc<crate::codex::Session>,
    turn: std::sync::Arc<crate::codex::TurnContext>,
    call_id: String,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
}

#[cfg(unix)]
impl CoreShellActionProvider {
    fn decision_driven_by_policy(matched_rules: &[RuleMatch], decision: Decision) -> bool {
        matched_rules.iter().any(|rule_match| {
            !matches!(rule_match, RuleMatch::HeuristicsRuleMatch { .. })
                && rule_match.decision() == decision
        })
    }

    async fn prompt(
        &self,
        command: &[String],
        workdir: &Path,
        stopwatch: &Stopwatch,
    ) -> anyhow::Result<ReviewDecision> {
        let command = command.to_vec();
        let workdir = workdir.to_path_buf();
        let session = self.session.clone();
        let turn = self.turn.clone();
        let call_id = self.call_id.clone();
        Ok(stopwatch
            .pause_for(async move {
                session
                    .request_command_approval(
                        &turn, call_id, None, command, workdir, None, None, None,
                    )
                    .await
            })
            .await)
    }
}

#[cfg(unix)]
#[async_trait::async_trait]
impl ShellActionProvider for CoreShellActionProvider {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
        stopwatch: &Stopwatch,
    ) -> anyhow::Result<EscalateAction> {
        let command = std::iter::once(file.to_string_lossy().to_string())
            .chain(argv.iter().cloned())
            .collect::<Vec<_>>();
        let (commands, used_complex_parsing) =
            if let Some(commands) = parse_shell_lc_plain_commands(&command) {
                (commands, false)
            } else if let Some(single_command) = parse_shell_lc_single_command_prefix(&command) {
                (vec![single_command], true)
            } else {
                (vec![command.clone()], false)
            };

        let policy = self.policy.read().await;
        let fallback = |cmd: &[String]| {
            crate::exec_policy::render_decision_for_unmatched_command(
                self.approval_policy,
                &self.sandbox_policy,
                cmd,
                self.sandbox_permissions,
                used_complex_parsing,
            )
        };
        let evaluation = policy.check_multiple(commands.iter(), &fallback);
        let decision_driven_by_policy =
            Self::decision_driven_by_policy(&evaluation.matched_rules, evaluation.decision);
        let needs_escalation =
            self.sandbox_permissions.requires_escalated_permissions() || decision_driven_by_policy;

        Ok(match evaluation.decision {
            Decision::Forbidden => EscalateAction::Deny {
                reason: Some("Execution forbidden by policy".to_string()),
            },
            Decision::Prompt => {
                if self.approval_policy == AskForApproval::Never {
                    EscalateAction::Deny {
                        reason: Some("Execution forbidden by policy".to_string()),
                    }
                } else if decision_driven_by_policy {
                    EscalateAction::Escalate
                } else {
                    match self.prompt(&command, workdir, stopwatch).await? {
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                        | ReviewDecision::ApprovedForSession => {
                            if needs_escalation {
                                EscalateAction::Escalate
                            } else {
                                EscalateAction::Run
                            }
                        }
                        ReviewDecision::Denied => EscalateAction::Deny {
                            reason: Some("User denied execution".to_string()),
                        },
                        ReviewDecision::Abort => EscalateAction::Deny {
                            reason: Some("User cancelled execution".to_string()),
                        },
                    }
                }
            }
            Decision::Allow => EscalateAction::Run,
        })
    }
}

#[cfg(unix)]
struct CoreShellCommandExecutor;

#[cfg(unix)]
#[async_trait::async_trait]
impl ShellCommandExecutor for CoreShellCommandExecutor {
    async fn run(
        &self,
        command: Vec<String>,
        cwd: PathBuf,
        env: HashMap<String, String>,
        cancel_rx: CancellationToken,
        sandbox_state: &SandboxState,
    ) -> anyhow::Result<ExecResult> {
        let result = crate::exec::process_exec_tool_call(
            crate::exec::ExecParams {
                command,
                cwd,
                expiration: crate::exec::ExecExpiration::Cancellation(cancel_rx),
                env,
                network: None,
                sandbox_permissions: SandboxPermissions::UseDefault,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                justification: None,
                arg0: None,
            },
            &sandbox_state.sandbox_policy,
            &sandbox_state.sandbox_cwd,
            &sandbox_state.codex_linux_sandbox_exe,
            sandbox_state.use_linux_sandbox_bwrap,
            None,
        )
        .await?;

        Ok(ExecResult {
            exit_code: result.exit_code,
            output: result.aggregated_output.text,
            duration: result.duration,
            timed_out: result.timed_out,
        })
    }
}

#[cfg(unix)]
fn shell_execve_wrapper() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(|parent| parent.join("codex-execve-wrapper"))
        .ok_or_else(|| anyhow::anyhow!("failed to determine codex-execve-wrapper path"))
}

#[cfg(unix)]
fn shell_exec_zsh_path(path: &AbsolutePathBuf) -> PathBuf {
    path.to_path_buf()
}

#[cfg(unix)]
fn map_exec_result(
    sandbox: SandboxType,
    result: ExecResult,
) -> Result<ExecToolCallOutput, ToolError> {
    let output = ExecToolCallOutput {
        exit_code: result.exit_code,
        stdout: crate::exec::StreamOutput::new(result.output.clone()),
        stderr: crate::exec::StreamOutput::new(String::new()),
        aggregated_output: crate::exec::StreamOutput::new(result.output.clone()),
        duration: result.duration,
        timed_out: result.timed_out,
    };

    if result.timed_out {
        return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout {
            output: Box::new(output),
        })));
    }

    if is_likely_sandbox_denied(sandbox, &output) {
        return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
            output: Box::new(output),
            network_policy_decision: None,
        })));
    }

    Ok(output)
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    fn network_approval_spec(
        &self,
        req: &ShellRequest,
        _ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        req.network.as_ref()?;
        Some(NetworkApprovalSpec {
            network: req.network.clone(),
            mode: NetworkApprovalMode::Immediate,
        })
    }

    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let command = maybe_wrap_shell_lc_with_snapshot(
            &req.command,
            ctx.session.user_shell().as_ref(),
            &req.cwd,
            &req.explicit_env_overrides,
        );
        let command = if matches!(ctx.session.user_shell().shell_type, ShellType::PowerShell)
            && ctx.session.features().enabled(Feature::PowershellUtf8)
        {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        #[cfg(unix)]
        if let Some(shell_zsh_path) = ctx.session.services.shell_zsh_path.as_ref()
            && ctx.session.features().enabled(Feature::ShellZshFork)
            && matches!(ctx.session.user_shell().shell_type, ShellType::Zsh)
        {
            let spec = build_command_spec(
                &command,
                &req.cwd,
                &req.env,
                req.timeout_ms.into(),
                req.sandbox_permissions,
                req.justification.clone(),
            )?;
            let env = attempt
                .env_for(spec, req.network.as_ref())
                .map_err(|err| ToolError::Codex(err.into()))?;
            let (_, args) = env
                .command
                .split_first()
                .ok_or_else(|| ToolError::Rejected("command args are empty".to_string()))?;
            let script = shlex_try_join(args.iter().map(String::as_str))
                .map_err(|err| ToolError::Rejected(format!("serialize shell script: {err}")))?;
            let effective_timeout = Duration::from_millis(
                req.timeout_ms
                    .unwrap_or(crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
            );
            let exec_policy = Arc::new(RwLock::new(
                ctx.session.services.exec_policy.current().as_ref().clone(),
            ));
            let sandbox_state = SandboxState {
                sandbox_policy: ctx.turn.sandbox_policy.get().clone(),
                codex_linux_sandbox_exe: attempt.codex_linux_sandbox_exe.cloned(),
                sandbox_cwd: req.cwd.clone(),
                use_linux_sandbox_bwrap: attempt.use_linux_sandbox_bwrap,
            };
            let exec_result = run_escalate_server(
                ExecParams {
                    command: script,
                    workdir: req.cwd.to_string_lossy().to_string(),
                    timeout_ms: Some(effective_timeout.as_millis() as u64),
                    login: Some(false),
                },
                &sandbox_state,
                shell_exec_zsh_path(shell_zsh_path),
                shell_execve_wrapper().map_err(|err| ToolError::Rejected(format!("{err}")))?,
                exec_policy.clone(),
                ShellPolicyFactory::new(CoreShellActionProvider {
                    policy: Arc::clone(&exec_policy),
                    session: Arc::clone(&ctx.session),
                    turn: Arc::clone(&ctx.turn),
                    call_id: ctx.call_id.clone(),
                    approval_policy: ctx.turn.approval_policy.value(),
                    sandbox_policy: attempt.policy.clone(),
                    sandbox_permissions: req.sandbox_permissions,
                }),
                effective_timeout,
                &CoreShellCommandExecutor,
            )
            .await
            .map_err(|err| ToolError::Rejected(err.to_string()))?;

            return map_exec_result(attempt.sandbox, exec_result);
        }

        let spec = build_command_spec(
            &command,
            &req.cwd,
            &req.env,
            req.timeout_ms.into(),
            req.sandbox_permissions,
            req.justification.clone(),
        )?;
        let env = attempt
            .env_for(spec, req.network.as_ref())
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}
