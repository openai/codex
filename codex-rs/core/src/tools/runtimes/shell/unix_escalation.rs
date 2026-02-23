use super::ShellRequest;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::is_likely_sandbox_denied;
use crate::features::Feature;
use crate::sandboxing::SandboxPermissions;
use crate::shell::ShellType;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::bash::parse_shell_lc_single_command_prefix;
use codex_shell_escalation::unix::core_shell_escalation::ShellActionProvider;
use codex_shell_escalation::unix::core_shell_escalation::ShellPolicyFactory;
use codex_shell_escalation::unix::escalate_protocol::EscalateAction;
use codex_shell_escalation::unix::escalate_server::ExecParams;
use codex_shell_escalation::unix::escalate_server::ExecResult;
use codex_shell_escalation::unix::escalate_server::SandboxState;
use codex_shell_escalation::unix::escalate_server::ShellCommandExecutor;
use codex_shell_escalation::unix::escalate_server::run_escalate_server;
use codex_shell_escalation::unix::stopwatch::Stopwatch;
use shlex::try_join as shlex_try_join;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub(super) async fn try_run_zsh_fork(
    req: &ShellRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    command: &[String],
) -> Result<Option<ExecToolCallOutput>, ToolError> {
    let Some(shell_zsh_path) = ctx.session.services.shell_zsh_path.as_ref() else {
        return Ok(None);
    };
    if !ctx.session.features().enabled(Feature::ShellZshFork) {
        return Ok(None);
    }
    if !matches!(ctx.session.user_shell().shell_type, ShellType::Zsh) {
        return Ok(None);
    }

    let spec = build_command_spec(
        command,
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
        shell_zsh_path.to_path_buf(),
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

    map_exec_result(attempt.sandbox, exec_result).map(Some)
}

struct CoreShellActionProvider {
    policy: Arc<RwLock<Policy>>,
    session: Arc<crate::codex::Session>,
    turn: Arc<crate::codex::TurnContext>,
    call_id: String,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
}

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

struct CoreShellCommandExecutor;

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

fn shell_execve_wrapper() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(|parent| parent.join("codex-execve-wrapper"))
        .ok_or_else(|| anyhow::anyhow!("failed to determine codex-execve-wrapper path"))
}

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
