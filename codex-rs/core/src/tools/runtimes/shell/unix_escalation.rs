use super::ShellRequest;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::is_likely_sandbox_denied;
use crate::features::Feature;
use crate::sandboxing::SandboxPermissions;
use crate::shell::ShellType;
use crate::skills::SkillMetadata;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::RejectConfig;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::bash::parse_shell_lc_single_command_prefix;
use codex_shell_escalation::EscalateAction;
use codex_shell_escalation::EscalateServer;
use codex_shell_escalation::EscalationPolicy;
use codex_shell_escalation::ExecParams;
use codex_shell_escalation::ExecResult;
use codex_shell_escalation::ShellCommandExecutor;
use codex_shell_escalation::Stopwatch;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) async fn try_run_zsh_fork(
    req: &ShellRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    command: &[String],
) -> Result<Option<ExecToolCallOutput>, ToolError> {
    let Some(shell_zsh_path) = ctx.session.services.shell_zsh_path.as_ref() else {
        tracing::warn!("ZshFork backend specified, but shell_zsh_path is not configured.");
        return Ok(None);
    };
    if !ctx.session.features().enabled(Feature::ShellZshFork) {
        tracing::warn!("ZshFork backend specified, but ShellZshFork feature is not enabled.");
        return Ok(None);
    }
    if !matches!(ctx.session.user_shell().shell_type, ShellType::Zsh) {
        tracing::warn!("ZshFork backend specified, but user shell is not Zsh.");
        return Ok(None);
    }

    let spec = build_command_spec(
        command,
        &req.cwd,
        &req.env,
        req.timeout_ms.into(),
        req.sandbox_permissions,
        req.additional_permissions.clone(),
        req.justification.clone(),
    )?;
    let sandbox_exec_request = attempt
        .env_for(spec, req.network.as_ref())
        .map_err(|err| ToolError::Codex(err.into()))?;
    // Keep env/network/sandbox metadata from `attempt.env_for()`, but build the
    // script from the original shell argv. `attempt.env_for()` may wrap the
    // command with `sandbox-exec` on macOS, and passing those wrapper flags
    // (`-p`, `-D...`) through zsh breaks the zsh-fork path before subcommand
    // approval runs.
    let crate::sandboxing::ExecRequest {
        command: _sandbox_command,
        cwd: _sandbox_cwd,
        env: sandbox_env,
        network: sandbox_network,
        expiration: _sandbox_expiration,
        sandbox,
        windows_sandbox_level,
        sandbox_permissions,
        sandbox_policy,
        justification,
        arg0,
    } = sandbox_exec_request;
    let ParsedShellCommand { script, login } = extract_shell_script(command)?;
    let effective_timeout = Duration::from_millis(
        req.timeout_ms
            .unwrap_or(crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
    );
    let exec_policy = Arc::new(RwLock::new(
        ctx.session.services.exec_policy.current().as_ref().clone(),
    ));
    let command_executor = CoreShellCommandExecutor {
        sandbox_policy,
        sandbox,
        env: sandbox_env,
        network: sandbox_network,
        windows_sandbox_level,
        sandbox_permissions,
        justification,
        arg0,
    };
    let main_execve_wrapper_exe = ctx
        .session
        .services
        .main_execve_wrapper_exe
        .clone()
        .ok_or_else(|| {
            ToolError::Rejected(
                "zsh fork feature enabled, but execve wrapper is not configured".to_string(),
            )
        })?;
    let exec_params = ExecParams {
        command: script,
        workdir: req.cwd.to_string_lossy().to_string(),
        timeout_ms: Some(effective_timeout.as_millis() as u64),
        login: Some(login),
    };

    // Note that Stopwatch starts immediately upon creation, so currently we try
    // to minimize the time between creating the Stopwatch and starting the
    // escalation server.
    let stopwatch = Stopwatch::new(effective_timeout);
    let cancel_token = stopwatch.cancellation_token();
    let escalation_policy = CoreShellActionProvider {
        policy: Arc::clone(&exec_policy),
        session: Arc::clone(&ctx.session),
        turn: Arc::clone(&ctx.turn),
        call_id: ctx.call_id.clone(),
        approval_policy: ctx.turn.approval_policy.value(),
        sandbox_policy: attempt.policy.clone(),
        sandbox_permissions: req.sandbox_permissions,
        stopwatch: stopwatch.clone(),
    };

    let escalate_server = EscalateServer::new(
        shell_zsh_path.clone(),
        main_execve_wrapper_exe,
        escalation_policy,
    );

    let exec_result = escalate_server
        .exec(exec_params, cancel_token, &command_executor)
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
    stopwatch: Stopwatch,
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
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        stopwatch: &Stopwatch,
        additional_permissions: Option<PermissionProfile>,
    ) -> anyhow::Result<ReviewDecision> {
        let command = join_program_and_argv(program, argv);
        let workdir = workdir.to_path_buf();
        let session = self.session.clone();
        let turn = self.turn.clone();
        let call_id = self.call_id.clone();
        let approval_id = Some(Uuid::new_v4().to_string());
        Ok(stopwatch
            .pause_for(async move {
                session
                    .request_command_approval(
                        &turn,
                        call_id,
                        approval_id,
                        command,
                        workdir,
                        None,
                        None,
                        None,
                        additional_permissions,
                    )
                    .await
            })
            .await)
    }

    /// Because we should be intercepting execve(2) calls, `program` should be
    /// an absolute path. The idea is that we check to see whether it matches
    /// any skills.
    async fn find_skill(&self, program: &AbsolutePathBuf) -> Option<SkillMetadata> {
        let force_reload = false;
        let skills_outcome = self
            .session
            .services
            .skills_manager
            .skills_for_cwd(&self.turn.cwd, force_reload)
            .await;

        let program_path = program.as_path();
        for skill in skills_outcome.skills {
            // We intentionally ignore "enabled" status here for now.
            let Some(skill_root) = skill.path_to_skills_md.parent() else {
                continue;
            };
            if program_path.starts_with(skill_root.join("scripts")) {
                return Some(skill);
            }
        }

        None
    }

    async fn process_decision(
        &self,
        decision: Decision,
        needs_escalation: bool,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        additional_permissions: Option<PermissionProfile>,
    ) -> anyhow::Result<EscalateAction> {
        let action = match decision {
            Decision::Forbidden => EscalateAction::Deny {
                reason: Some("Execution forbidden by policy".to_string()),
            },
            Decision::Prompt => {
                if matches!(
                    self.approval_policy,
                    AskForApproval::Never
                        | AskForApproval::Reject(RejectConfig { rules: true, .. })
                ) {
                    EscalateAction::Deny {
                        reason: Some("Execution forbidden by policy".to_string()),
                    }
                } else {
                    match self
                        .prompt(
                            program,
                            argv,
                            workdir,
                            &self.stopwatch,
                            additional_permissions,
                        )
                        .await?
                    {
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                        | ReviewDecision::ApprovedForSession => {
                            if needs_escalation {
                                EscalateAction::Escalate
                            } else {
                                EscalateAction::Run
                            }
                        }
                        ReviewDecision::NetworkPolicyAmendment {
                            network_policy_amendment,
                        } => match network_policy_amendment.action {
                            NetworkPolicyRuleAction::Allow => {
                                if needs_escalation {
                                    EscalateAction::Escalate
                                } else {
                                    EscalateAction::Run
                                }
                            }
                            NetworkPolicyRuleAction::Deny => EscalateAction::Deny {
                                reason: Some("User denied execution".to_string()),
                            },
                        },
                        ReviewDecision::Denied => EscalateAction::Deny {
                            reason: Some("User denied execution".to_string()),
                        },
                        ReviewDecision::Abort => EscalateAction::Deny {
                            reason: Some("User cancelled execution".to_string()),
                        },
                    }
                }
            }
            Decision::Allow => {
                if needs_escalation {
                    EscalateAction::Escalate
                } else {
                    EscalateAction::Run
                }
            }
        };
        tracing::debug!(
            "Policy decision for command {program:?} is {decision:?}, leading to escalation action {action:?}",
        );
        Ok(action)
    }
}

#[async_trait::async_trait]
impl EscalationPolicy for CoreShellActionProvider {
    async fn determine_action(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
    ) -> anyhow::Result<EscalateAction> {
        tracing::debug!(
            "Determining escalation action for command {program:?} with args {argv:?} in {workdir:?}"
        );

        // In the usual case, the execve wrapper reports the command being
        // executed in `program`, so a direct skill lookup is sufficient.
        if let Some(skill) = self.find_skill(program).await {
            // For now, we always prompt for scripts that look like they belong
            // to skills, which means we ignore exec policy rules for those
            // scripts.
            tracing::debug!("Matched {program:?} to skill {skill:?}, prompting for approval");
            // TODO(mbolin): We should read the permissions associated with the
            // skill and use those specific permissions in the
            // EscalateAction::Run case, rather than always escalating when a
            // skill matches.
            let needs_escalation = true;
            return self
                .process_decision(
                    Decision::Prompt,
                    needs_escalation,
                    program,
                    argv,
                    workdir,
                    skill.permission_profile.clone(),
                )
                .await;
        }

        let command = join_program_and_argv(program, argv);
        let (commands, used_complex_parsing) =
            if let Some(commands) = parse_shell_lc_plain_commands(&command) {
                (commands, false)
            } else if let Some(single_command) = parse_shell_lc_single_command_prefix(&command) {
                (vec![single_command], true)
            } else {
                (vec![command.clone()], false)
            };

        let fallback = |cmd: &[String]| {
            crate::exec_policy::render_decision_for_unmatched_command(
                self.approval_policy,
                &self.sandbox_policy,
                cmd,
                self.sandbox_permissions,
                used_complex_parsing,
            )
        };
        let evaluation = {
            let policy = self.policy.read().await;
            policy.check_multiple(commands.iter(), &fallback)
        };
        // When true, means the Evaluation was due to *.rules, not the
        // fallback function.
        let decision_driven_by_policy =
            Self::decision_driven_by_policy(&evaluation.matched_rules, evaluation.decision);
        let needs_escalation =
            self.sandbox_permissions.requires_escalated_permissions() || decision_driven_by_policy;

        self.process_decision(
            evaluation.decision,
            needs_escalation,
            program,
            argv,
            workdir,
            None,
        )
        .await
    }
}

struct CoreShellCommandExecutor {
    sandbox_policy: SandboxPolicy,
    sandbox: SandboxType,
    env: HashMap<String, String>,
    network: Option<codex_network_proxy::NetworkProxy>,
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_permissions: SandboxPermissions,
    justification: Option<String>,
    arg0: Option<String>,
}

#[async_trait::async_trait]
impl ShellCommandExecutor for CoreShellCommandExecutor {
    async fn run(
        &self,
        command: Vec<String>,
        cwd: PathBuf,
        env: HashMap<String, String>,
        cancel_rx: CancellationToken,
    ) -> anyhow::Result<ExecResult> {
        let mut exec_env = self.env.clone();
        for var in ["CODEX_ESCALATE_SOCKET", "EXEC_WRAPPER", "BASH_EXEC_WRAPPER"] {
            if let Some(value) = env.get(var) {
                exec_env.insert(var.to_string(), value.clone());
            }
        }

        let result = crate::sandboxing::execute_env(
            crate::sandboxing::ExecRequest {
                command,
                cwd,
                env: exec_env,
                network: self.network.clone(),
                expiration: ExecExpiration::Cancellation(cancel_rx),
                sandbox: self.sandbox,
                windows_sandbox_level: self.windows_sandbox_level,
                sandbox_permissions: self.sandbox_permissions,
                sandbox_policy: self.sandbox_policy.clone(),
                justification: self.justification.clone(),
                arg0: self.arg0.clone(),
            },
            None,
        )
        .await?;

        Ok(ExecResult {
            exit_code: result.exit_code,
            stdout: result.stdout.text,
            stderr: result.stderr.text,
            output: result.aggregated_output.text,
            duration: result.duration,
            timed_out: result.timed_out,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedShellCommand {
    script: String,
    login: bool,
}

fn extract_shell_script(command: &[String]) -> Result<ParsedShellCommand, ToolError> {
    match command {
        [_, flag, script, ..] if flag == "-c" => Ok(ParsedShellCommand {
            script: script.clone(),
            login: false,
        }),
        [_, flag, script, ..] if flag == "-lc" => Ok(ParsedShellCommand {
            script: script.clone(),
            login: true,
        }),
        _ => Err(ToolError::Rejected(
            "unexpected shell command format for zsh-fork execution".to_string(),
        )),
    }
}

fn map_exec_result(
    sandbox: SandboxType,
    result: ExecResult,
) -> Result<ExecToolCallOutput, ToolError> {
    let output = ExecToolCallOutput {
        exit_code: result.exit_code,
        stdout: crate::exec::StreamOutput::new(result.stdout.clone()),
        stderr: crate::exec::StreamOutput::new(result.stderr.clone()),
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

/// Convert an intercepted exec `(program, argv)` into a command vector suitable
/// for display and policy parsing.
///
/// The intercepted `argv` includes `argv[0]`, but once we have normalized the
/// executable path in `program`, we should replace the original `argv[0]`
/// rather than duplicating it as an apparent user argument.
fn join_program_and_argv(program: &AbsolutePathBuf, argv: &[String]) -> Vec<String> {
    std::iter::once(program.to_string_lossy().to_string())
        .chain(argv.iter().skip(1).cloned())
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::ParsedShellCommand;
    use super::extract_shell_script;
    use super::join_program_and_argv;
    use super::map_exec_result;
    use crate::exec::SandboxType;
    use codex_shell_escalation::ExecResult;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    #[test]
    fn extract_shell_script_preserves_login_flag() {
        assert_eq!(
            extract_shell_script(&["/bin/zsh".into(), "-lc".into(), "echo hi".into()]).unwrap(),
            ParsedShellCommand {
                script: "echo hi".to_string(),
                login: true,
            }
        );
        assert_eq!(
            extract_shell_script(&["/bin/zsh".into(), "-c".into(), "echo hi".into()]).unwrap(),
            ParsedShellCommand {
                script: "echo hi".to_string(),
                login: false,
            }
        );
    }

    #[test]
    fn join_program_and_argv_replaces_original_argv_zero() {
        assert_eq!(
            join_program_and_argv(
                &AbsolutePathBuf::from_absolute_path("/tmp/tool").unwrap(),
                &["./tool".into(), "--flag".into(), "value".into()],
            ),
            vec!["/tmp/tool", "--flag", "value"]
        );
        assert_eq!(
            join_program_and_argv(
                &AbsolutePathBuf::from_absolute_path("/tmp/tool").unwrap(),
                &["./tool".into()]
            ),
            vec!["/tmp/tool"]
        );
    }

    #[test]
    fn map_exec_result_preserves_stdout_and_stderr() {
        let out = map_exec_result(
            SandboxType::None,
            ExecResult {
                exit_code: 0,
                stdout: "out".to_string(),
                stderr: "err".to_string(),
                output: "outerr".to_string(),
                duration: Duration::from_millis(1),
                timed_out: false,
            },
        )
        .unwrap();

        assert_eq!(out.stdout.text, "out");
        assert_eq!(out.stderr.text, "err");
        assert_eq!(out.aggregated_output.text, "outerr");
    }
}
