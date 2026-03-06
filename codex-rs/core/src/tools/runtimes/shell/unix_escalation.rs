use super::ShellRequest;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::is_likely_sandbox_denied;
use crate::features::Feature;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;
use crate::shell::ShellType;
use crate::skills::SkillMetadata;
use crate::tools::runtimes::ExecveSessionApproval;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::MatchOptions;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::RejectConfig;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::bash::parse_shell_lc_single_command_prefix;
use codex_shell_escalation::EscalateServer;
use codex_shell_escalation::EscalationDecision;
use codex_shell_escalation::EscalationExecution;
use codex_shell_escalation::EscalationPermissions;
use codex_shell_escalation::EscalationPolicy;
use codex_shell_escalation::EscalationSession;
use codex_shell_escalation::ExecParams;
use codex_shell_escalation::ExecResult;
use codex_shell_escalation::Permissions as EscalatedPermissions;
use codex_shell_escalation::PreparedExec;
use codex_shell_escalation::ShellCommandExecutor;
use codex_shell_escalation::Stopwatch;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) struct PreparedUnifiedExecZshFork {
    pub(crate) exec_request: ExecRequest,
    pub(crate) escalation_session: EscalationSession,
}

pub(super) async fn try_run_zsh_fork(
    req: &ShellRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    shell_command: &[String],
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

    let ParsedShellCommand { script, login, .. } = extract_shell_script(shell_command)?;

    let spec = build_command_spec(
        shell_command,
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
    let crate::sandboxing::ExecRequest {
        command: sandbox_command,
        cwd: sandbox_cwd,
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
    let host_zsh_path =
        resolve_host_zsh_path(sandbox_env.get("PATH").map(String::as_str), &sandbox_cwd);
    let effective_timeout = Duration::from_millis(
        req.timeout_ms
            .unwrap_or(crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
    );
    let command_executor = CoreShellCommandExecutor {
        command: sandbox_command,
        cwd: sandbox_cwd,
        sandbox_policy,
        sandbox,
        env: sandbox_env,
        network: sandbox_network,
        windows_sandbox_level,
        sandbox_permissions,
        justification,
        arg0,
        sandbox_policy_cwd: ctx.turn.cwd.clone(),
        macos_seatbelt_profile_extensions: ctx
            .turn
            .config
            .permissions
            .macos_seatbelt_profile_extensions
            .clone(),
        codex_linux_sandbox_exe: ctx.turn.codex_linux_sandbox_exe.clone(),
        use_linux_sandbox_bwrap: ctx.turn.features.enabled(Feature::UseLinuxSandboxBwrap),
        shell_zsh_path: ctx.session.services.shell_zsh_path.clone(),
        host_zsh_path: host_zsh_path.clone(),
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
        session: Arc::clone(&ctx.session),
        turn: Arc::clone(&ctx.turn),
        call_id: ctx.call_id.clone(),
        approval_policy: ctx.turn.approval_policy.value(),
        sandbox_policy: attempt.policy.clone(),
        sandbox_permissions: req.sandbox_permissions,
        prompt_permissions: req.additional_permissions.clone(),
        stopwatch: stopwatch.clone(),
        host_zsh_path,
    };

    let escalate_server = EscalateServer::new(
        shell_zsh_path.clone(),
        main_execve_wrapper_exe,
        escalation_policy,
    );

    let exec_result = escalate_server
        .exec(exec_params, cancel_token, Arc::new(command_executor))
        .await
        .map_err(|err| ToolError::Rejected(err.to_string()))?;

    map_exec_result(attempt.sandbox, exec_result).map(Some)
}

pub(crate) async fn prepare_unified_exec_zsh_fork(
    req: &crate::tools::runtimes::unified_exec::UnifiedExecRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    shell_command: &[String],
    exec_request: ExecRequest,
) -> Result<Option<PreparedUnifiedExecZshFork>, ToolError> {
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

    let parsed = match extract_shell_script(shell_command) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!("ZshFork unified exec fallback: {err:?}");
            return Ok(None);
        }
    };
    if parsed.program != shell_zsh_path.to_string_lossy() {
        tracing::warn!(
            "ZshFork backend specified, but unified exec command targets `{}` instead of `{}`.",
            parsed.program,
            shell_zsh_path.display(),
        );
        return Ok(None);
    }

    let ExecRequest {
        command,
        cwd,
        env,
        network,
        expiration: _expiration,
        sandbox,
        windows_sandbox_level,
        sandbox_permissions,
        sandbox_policy,
        justification,
        arg0,
    } = &exec_request;
    let host_zsh_path = resolve_host_zsh_path(env.get("PATH").map(String::as_str), cwd);
    let command_executor = CoreShellCommandExecutor {
        command: command.clone(),
        cwd: cwd.clone(),
        sandbox_policy: sandbox_policy.clone(),
        sandbox: *sandbox,
        env: env.clone(),
        network: network.clone(),
        windows_sandbox_level: *windows_sandbox_level,
        sandbox_permissions: *sandbox_permissions,
        justification: justification.clone(),
        arg0: arg0.clone(),
        sandbox_policy_cwd: ctx.turn.cwd.clone(),
        macos_seatbelt_profile_extensions: ctx
            .turn
            .config
            .permissions
            .macos_seatbelt_profile_extensions
            .clone(),
        codex_linux_sandbox_exe: ctx.turn.codex_linux_sandbox_exe.clone(),
        use_linux_sandbox_bwrap: ctx.turn.features.enabled(Feature::UseLinuxSandboxBwrap),
        shell_zsh_path: ctx.session.services.shell_zsh_path.clone(),
        host_zsh_path: host_zsh_path.clone(),
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
    let escalation_policy = CoreShellActionProvider {
        session: Arc::clone(&ctx.session),
        turn: Arc::clone(&ctx.turn),
        call_id: ctx.call_id.clone(),
        approval_policy: ctx.turn.approval_policy.value(),
        sandbox_policy: attempt.policy.clone(),
        sandbox_permissions: req.sandbox_permissions,
        prompt_permissions: req.additional_permissions.clone(),
        stopwatch: Stopwatch::unlimited(),
        host_zsh_path,
    };

    let escalate_server = EscalateServer::new(
        shell_zsh_path.clone(),
        main_execve_wrapper_exe,
        escalation_policy,
    );
    let escalation_session = escalate_server
        .start_session(CancellationToken::new(), Arc::new(command_executor))
        .map_err(|err| ToolError::Rejected(err.to_string()))?;
    let mut exec_request = exec_request;
    exec_request.env.extend(escalation_session.env().clone());
    Ok(Some(PreparedUnifiedExecZshFork {
        exec_request,
        escalation_session,
    }))
}

struct CoreShellActionProvider {
    session: Arc<crate::codex::Session>,
    turn: Arc<crate::codex::TurnContext>,
    call_id: String,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
    prompt_permissions: Option<PermissionProfile>,
    stopwatch: Stopwatch,
    host_zsh_path: Option<PathBuf>,
}

#[allow(clippy::large_enum_variant)]
enum DecisionSource {
    SkillScript {
        skill: SkillMetadata,
    },
    PrefixRule,
    /// Often, this is `is_safe_command()`.
    UnmatchedCommandFallback,
}

fn paths_match(lhs: &Path, rhs: &Path) -> bool {
    lhs == rhs
        || match (lhs.canonicalize(), rhs.canonicalize()) {
            (Ok(lhs), Ok(rhs)) => lhs == rhs,
            _ => false,
        }
}

fn resolve_host_zsh_path(path_env: Option<&str>, cwd: &Path) -> Option<PathBuf> {
    which::which_in("zsh", path_env, cwd)
        .or_else(|_| which::which("zsh"))
        .ok()
        .map(|path| path.canonicalize().unwrap_or(path))
}

fn is_unconfigured_zsh_exec(
    program: &AbsolutePathBuf,
    shell_zsh_path: Option<&Path>,
    host_zsh_path: Option<&Path>,
) -> bool {
    let Some(shell_zsh_path) = shell_zsh_path else {
        return false;
    };
    let Some(host_zsh_path) = host_zsh_path else {
        return false;
    };
    paths_match(program.as_path(), host_zsh_path) && !paths_match(program.as_path(), shell_zsh_path)
}

impl CoreShellActionProvider {
    fn decision_driven_by_policy(matched_rules: &[RuleMatch], decision: Decision) -> bool {
        matched_rules.iter().any(|rule_match| {
            !matches!(rule_match, RuleMatch::HeuristicsRuleMatch { .. })
                && rule_match.decision() == decision
        })
    }

    fn shell_request_escalation_execution(
        sandbox_permissions: SandboxPermissions,
        sandbox_policy: &SandboxPolicy,
        additional_permissions: Option<&PermissionProfile>,
        macos_seatbelt_profile_extensions: Option<&MacOsSeatbeltProfileExtensions>,
    ) -> EscalationExecution {
        match sandbox_permissions {
            SandboxPermissions::UseDefault => EscalationExecution::TurnDefault,
            SandboxPermissions::RequireEscalated => EscalationExecution::Unsandboxed,
            SandboxPermissions::WithAdditionalPermissions => additional_permissions
                .map(|_| {
                    // Shell request additional permissions were already normalized and
                    // merged into the first-attempt sandbox policy.
                    EscalationExecution::Permissions(EscalationPermissions::Permissions(
                        EscalatedPermissions {
                            sandbox_policy: sandbox_policy.clone(),
                            macos_seatbelt_profile_extensions: macos_seatbelt_profile_extensions
                                .cloned(),
                        },
                    ))
                })
                .unwrap_or(EscalationExecution::TurnDefault),
        }
    }

    fn skill_escalation_execution(skill: &SkillMetadata) -> EscalationExecution {
        let permission_profile = skill.permission_profile.clone().unwrap_or_default();
        if permission_profile.is_empty() {
            EscalationExecution::TurnDefault
        } else {
            EscalationExecution::Permissions(EscalationPermissions::PermissionProfile(
                permission_profile,
            ))
        }
    }

    async fn prompt(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        stopwatch: &Stopwatch,
        additional_permissions: Option<PermissionProfile>,
        decision_source: &DecisionSource,
    ) -> anyhow::Result<ReviewDecision> {
        let command = join_program_and_argv(program, argv);
        let workdir = workdir.to_path_buf();
        let session = self.session.clone();
        let turn = self.turn.clone();
        let call_id = self.call_id.clone();
        let approval_id = Some(Uuid::new_v4().to_string());
        Ok(stopwatch
            .pause_for(async move {
                let available_decisions = vec![
                    Some(ReviewDecision::Approved),
                    // Currently, ApprovedForSession is only honored for skills,
                    // so only offer it for skill script approvals.
                    if matches!(decision_source, DecisionSource::SkillScript { .. }) {
                        Some(ReviewDecision::ApprovedForSession)
                    } else {
                        None
                    },
                    Some(ReviewDecision::Abort),
                ]
                .into_iter()
                .flatten()
                .collect();
                session
                    .request_command_approval(
                        &turn,
                        call_id,
                        approval_id,
                        command,
                        workdir,
                        None,
                        // Intercepted exec prompts happen after the original tool call has
                        // started, so we do not attach an execpolicy amendment payload here.
                        // Amendments are currently surfaced only from the top-level tool
                        // request path.
                        None,
                        None,
                        additional_permissions,
                        Some(available_decisions),
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

    #[allow(clippy::too_many_arguments)]
    async fn process_decision(
        &self,
        decision: Decision,
        needs_escalation: bool,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        prompt_permissions: Option<PermissionProfile>,
        escalation_execution: EscalationExecution,
        decision_source: DecisionSource,
    ) -> anyhow::Result<EscalationDecision> {
        let action = match decision {
            Decision::Forbidden => {
                EscalationDecision::deny(Some("Execution forbidden by policy".to_string()))
            }
            Decision::Prompt => {
                if matches!(
                    self.approval_policy,
                    AskForApproval::Never
                        | AskForApproval::Reject(RejectConfig { rules: true, .. })
                ) {
                    EscalationDecision::deny(Some("Execution forbidden by policy".to_string()))
                } else {
                    match self
                        .prompt(
                            program,
                            argv,
                            workdir,
                            &self.stopwatch,
                            prompt_permissions,
                            &decision_source,
                        )
                        .await?
                    {
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                            if needs_escalation {
                                EscalationDecision::escalate(escalation_execution.clone())
                            } else {
                                EscalationDecision::run()
                            }
                        }
                        ReviewDecision::ApprovedForSession => {
                            // Currently, we only add session approvals for
                            // skill scripts because we are storing only the
                            // `program` whereas prefix rules may be restricted by a longer prefix.
                            if let DecisionSource::SkillScript { skill } = decision_source {
                                tracing::debug!(
                                    "Adding session approval for {program:?} due to user approval of skill script {skill:?}"
                                );
                                self.session
                                    .services
                                    .execve_session_approvals
                                    .write()
                                    .await
                                    .insert(
                                        program.clone(),
                                        ExecveSessionApproval {
                                            skill: Some(skill.clone()),
                                        },
                                    );
                            }

                            if needs_escalation {
                                EscalationDecision::escalate(escalation_execution.clone())
                            } else {
                                EscalationDecision::run()
                            }
                        }
                        ReviewDecision::NetworkPolicyAmendment {
                            network_policy_amendment,
                        } => match network_policy_amendment.action {
                            NetworkPolicyRuleAction::Allow => {
                                if needs_escalation {
                                    EscalationDecision::escalate(escalation_execution.clone())
                                } else {
                                    EscalationDecision::run()
                                }
                            }
                            NetworkPolicyRuleAction::Deny => {
                                EscalationDecision::deny(Some("User denied execution".to_string()))
                            }
                        },
                        ReviewDecision::Denied => {
                            EscalationDecision::deny(Some("User denied execution".to_string()))
                        }
                        ReviewDecision::Abort => {
                            EscalationDecision::deny(Some("User cancelled execution".to_string()))
                        }
                    }
                }
            }
            Decision::Allow => {
                if needs_escalation {
                    EscalationDecision::escalate(escalation_execution)
                } else {
                    EscalationDecision::run()
                }
            }
        };
        tracing::debug!(
            "Policy decision for command {program:?} is {decision:?}, leading to escalation action {action:?}",
        );
        Ok(action)
    }
}

// Shell-wrapper parsing is weaker than direct exec interception because it can
// only see the script text, not the final resolved executable path. Keep it
// disabled by default so path-sensitive rules rely on the later authoritative
// execve interception.
const ENABLE_INTERCEPTED_EXEC_POLICY_SHELL_WRAPPER_PARSING: bool = false;

#[async_trait::async_trait]
impl EscalationPolicy for CoreShellActionProvider {
    async fn determine_action(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
    ) -> anyhow::Result<EscalationDecision> {
        tracing::debug!(
            "Determining escalation action for command {program:?} with args {argv:?} in {workdir:?}"
        );

        // Check to see whether `program` has an existing entry in
        // `execve_session_approvals`. If so, we can skip policy checks and user
        // prompts and go straight to allowing execution.
        let approval = {
            self.session
                .services
                .execve_session_approvals
                .read()
                .await
                .get(program)
                .cloned()
        };
        if let Some(approval) = approval {
            tracing::debug!(
                "Found session approval for {program:?}, allowing execution without further checks"
            );
            let execution = approval
                .skill
                .as_ref()
                .map(Self::skill_escalation_execution)
                .unwrap_or(EscalationExecution::TurnDefault);

            return Ok(EscalationDecision::escalate(execution));
        }

        // In the usual case, the execve wrapper reports the command being
        // executed in `program`, so a direct skill lookup is sufficient.
        if let Some(skill) = self.find_skill(program).await {
            // For now, we always prompt for scripts that look like they belong
            // to skills, which means we ignore exec policy rules for those
            // scripts.
            tracing::debug!("Matched {program:?} to skill {skill:?}, prompting for approval");
            let needs_escalation = true;
            let decision_source = DecisionSource::SkillScript {
                skill: skill.clone(),
            };
            return self
                .process_decision(
                    Decision::Prompt,
                    needs_escalation,
                    program,
                    argv,
                    workdir,
                    skill.permission_profile.clone(),
                    Self::skill_escalation_execution(&skill),
                    decision_source,
                )
                .await;
        }

        let policy = self.session.services.exec_policy.current();
        let evaluation = evaluate_intercepted_exec_policy(
            policy.as_ref(),
            program,
            argv,
            self.approval_policy,
            &self.sandbox_policy,
            self.sandbox_permissions,
            ENABLE_INTERCEPTED_EXEC_POLICY_SHELL_WRAPPER_PARSING,
        );
        // When true, means the Evaluation was due to *.rules, not the
        // fallback function.
        let decision_driven_by_policy =
            Self::decision_driven_by_policy(&evaluation.matched_rules, evaluation.decision);
        // Keep zsh-fork interception alive across nested shells: if an
        // intercepted exec targets the known host `zsh` path instead of the
        // configured zsh-fork binary, force it through escalation so the
        // executor can rewrite the program path back to the configured shell.
        let force_zsh_fork_reexec = is_unconfigured_zsh_exec(
            program,
            self.session.services.shell_zsh_path.as_deref(),
            self.host_zsh_path.as_deref(),
        );
        let needs_escalation = self.sandbox_permissions.requires_escalated_permissions()
            || decision_driven_by_policy
            || force_zsh_fork_reexec;

        let decision_source = if decision_driven_by_policy {
            DecisionSource::PrefixRule
        } else {
            DecisionSource::UnmatchedCommandFallback
        };
        let escalation_execution = match decision_source {
            DecisionSource::PrefixRule => EscalationExecution::Unsandboxed,
            DecisionSource::UnmatchedCommandFallback => Self::shell_request_escalation_execution(
                self.sandbox_permissions,
                &self.sandbox_policy,
                self.prompt_permissions.as_ref(),
                self.turn
                    .config
                    .permissions
                    .macos_seatbelt_profile_extensions
                    .as_ref(),
            ),
            DecisionSource::SkillScript { .. } => unreachable!("handled above"),
        };
        self.process_decision(
            evaluation.decision,
            needs_escalation,
            program,
            argv,
            workdir,
            self.prompt_permissions.clone(),
            escalation_execution,
            decision_source,
        )
        .await
    }
}

fn evaluate_intercepted_exec_policy(
    policy: &Policy,
    program: &AbsolutePathBuf,
    argv: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    sandbox_permissions: SandboxPermissions,
    enable_intercepted_exec_policy_shell_wrapper_parsing: bool,
) -> Evaluation {
    let CandidateCommands {
        commands,
        used_complex_parsing,
    } = if enable_intercepted_exec_policy_shell_wrapper_parsing {
        // In this codepath, the first argument in `commands` could be a bare
        // name like `find` instead of an absolute path like `/usr/bin/find`.
        // It could also be a shell built-in like `echo`.
        commands_for_intercepted_exec_policy(program, argv)
    } else {
        // In this codepath, `commands` has a single entry where the program
        // is always an absolute path.
        CandidateCommands {
            commands: vec![join_program_and_argv(program, argv)],
            used_complex_parsing: false,
        }
    };

    let fallback = |cmd: &[String]| {
        crate::exec_policy::render_decision_for_unmatched_command(
            approval_policy,
            sandbox_policy,
            cmd,
            sandbox_permissions,
            used_complex_parsing,
        )
    };

    policy.check_multiple_with_options(
        commands.iter(),
        &fallback,
        &MatchOptions {
            resolve_host_executables: true,
        },
    )
}

struct CandidateCommands {
    commands: Vec<Vec<String>>,
    used_complex_parsing: bool,
}

fn commands_for_intercepted_exec_policy(
    program: &AbsolutePathBuf,
    argv: &[String],
) -> CandidateCommands {
    if let [_, flag, script] = argv {
        let shell_command = [
            program.to_string_lossy().to_string(),
            flag.clone(),
            script.clone(),
        ];
        if let Some(commands) = parse_shell_lc_plain_commands(&shell_command) {
            return CandidateCommands {
                commands,
                used_complex_parsing: false,
            };
        }
        if let Some(single_command) = parse_shell_lc_single_command_prefix(&shell_command) {
            return CandidateCommands {
                commands: vec![single_command],
                used_complex_parsing: true,
            };
        }
    }

    CandidateCommands {
        commands: vec![join_program_and_argv(program, argv)],
        used_complex_parsing: false,
    }
}

struct CoreShellCommandExecutor {
    command: Vec<String>,
    cwd: PathBuf,
    sandbox_policy: SandboxPolicy,
    sandbox: SandboxType,
    env: HashMap<String, String>,
    network: Option<codex_network_proxy::NetworkProxy>,
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_permissions: SandboxPermissions,
    justification: Option<String>,
    arg0: Option<String>,
    sandbox_policy_cwd: PathBuf,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    macos_seatbelt_profile_extensions: Option<MacOsSeatbeltProfileExtensions>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    use_linux_sandbox_bwrap: bool,
    shell_zsh_path: Option<PathBuf>,
    host_zsh_path: Option<PathBuf>,
}

struct PrepareSandboxedExecParams<'a> {
    command: Vec<String>,
    workdir: &'a AbsolutePathBuf,
    env: HashMap<String, String>,
    sandbox_policy: &'a SandboxPolicy,
    additional_permissions: Option<PermissionProfile>,
    #[cfg(target_os = "macos")]
    macos_seatbelt_profile_extensions: Option<&'a MacOsSeatbeltProfileExtensions>,
}

#[async_trait::async_trait]
impl ShellCommandExecutor for CoreShellCommandExecutor {
    async fn run(
        &self,
        _command: Vec<String>,
        _cwd: PathBuf,
        env_overlay: HashMap<String, String>,
        cancel_rx: CancellationToken,
        after_spawn: Option<Box<dyn FnOnce() + Send>>,
    ) -> anyhow::Result<ExecResult> {
        let mut exec_env = self.env.clone();
        // `env_overlay` comes from `EscalationSession::env()`, so merge only the
        // wrapper/socket variables into the base shell environment.
        for var in ["CODEX_ESCALATE_SOCKET", "EXEC_WRAPPER", "BASH_EXEC_WRAPPER"] {
            if let Some(value) = env_overlay.get(var) {
                exec_env.insert(var.to_string(), value.clone());
            }
        }

        let result = crate::sandboxing::execute_exec_request_with_after_spawn(
            crate::sandboxing::ExecRequest {
                command: self.command.clone(),
                cwd: self.cwd.clone(),
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
            after_spawn,
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

    async fn prepare_escalated_exec(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        env: HashMap<String, String>,
        execution: EscalationExecution,
    ) -> anyhow::Result<PreparedExec> {
        let program = self.rewrite_intercepted_program_for_zsh_fork(program);
        let command = join_program_and_argv(&program, argv);
        let Some(first_arg) = argv.first() else {
            return Err(anyhow::anyhow!(
                "intercepted exec request must contain argv[0]"
            ));
        };

        let prepared = match execution {
            EscalationExecution::Unsandboxed => PreparedExec {
                command,
                cwd: workdir.to_path_buf(),
                env,
                arg0: Some(first_arg.clone()),
            },
            EscalationExecution::TurnDefault => {
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    sandbox_policy: &self.sandbox_policy,
                    additional_permissions: None,
                    #[cfg(target_os = "macos")]
                    macos_seatbelt_profile_extensions: self
                        .macos_seatbelt_profile_extensions
                        .as_ref(),
                })?
            }
            EscalationExecution::Permissions(EscalationPermissions::PermissionProfile(
                permission_profile,
            )) => {
                // Merge additive permissions into the existing turn/request sandbox policy.
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    sandbox_policy: &self.sandbox_policy,
                    additional_permissions: Some(permission_profile),
                    #[cfg(target_os = "macos")]
                    macos_seatbelt_profile_extensions: None,
                })?
            }
            EscalationExecution::Permissions(EscalationPermissions::Permissions(permissions)) => {
                // Use a fully specified sandbox policy instead of merging into the turn policy.
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    sandbox_policy: &permissions.sandbox_policy,
                    additional_permissions: None,
                    #[cfg(target_os = "macos")]
                    macos_seatbelt_profile_extensions: permissions
                        .macos_seatbelt_profile_extensions
                        .as_ref(),
                })?
            }
        };

        Ok(prepared)
    }
}

impl CoreShellCommandExecutor {
    fn rewrite_intercepted_program_for_zsh_fork(
        &self,
        program: &AbsolutePathBuf,
    ) -> AbsolutePathBuf {
        let Some(shell_zsh_path) = self.shell_zsh_path.as_ref() else {
            return program.clone();
        };
        if !is_unconfigured_zsh_exec(
            program,
            Some(shell_zsh_path.as_path()),
            self.host_zsh_path.as_deref(),
        ) {
            return program.clone();
        }
        match AbsolutePathBuf::from_absolute_path(shell_zsh_path) {
            Ok(rewritten) => rewritten,
            Err(err) => {
                tracing::warn!(
                    "failed to rewrite intercepted zsh path {} to configured shell {}: {err}",
                    program.display(),
                    shell_zsh_path.display(),
                );
                program.clone()
            }
        }
    }

    fn prepare_sandboxed_exec(
        &self,
        params: PrepareSandboxedExecParams<'_>,
    ) -> anyhow::Result<PreparedExec> {
        let PrepareSandboxedExecParams {
            command,
            workdir,
            env,
            sandbox_policy,
            additional_permissions,
            #[cfg(target_os = "macos")]
            macos_seatbelt_profile_extensions,
        } = params;
        let (program, args) = command
            .split_first()
            .ok_or_else(|| anyhow::anyhow!("prepared command must not be empty"))?;
        let sandbox_manager = crate::sandboxing::SandboxManager::new();
        let sandbox = sandbox_manager.select_initial(
            sandbox_policy,
            SandboxablePreference::Auto,
            self.windows_sandbox_level,
            self.network.is_some(),
        );
        let mut exec_request =
            sandbox_manager.transform(crate::sandboxing::SandboxTransformRequest {
                spec: crate::sandboxing::CommandSpec {
                    program: program.clone(),
                    args: args.to_vec(),
                    cwd: workdir.to_path_buf(),
                    env,
                    expiration: ExecExpiration::DefaultTimeout,
                    sandbox_permissions: if additional_permissions.is_some() {
                        SandboxPermissions::WithAdditionalPermissions
                    } else {
                        SandboxPermissions::UseDefault
                    },
                    additional_permissions,
                    justification: self.justification.clone(),
                },
                policy: sandbox_policy,
                sandbox,
                enforce_managed_network: self.network.is_some(),
                network: self.network.as_ref(),
                sandbox_policy_cwd: &self.sandbox_policy_cwd,
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions,
                codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.as_ref(),
                use_linux_sandbox_bwrap: self.use_linux_sandbox_bwrap,
                windows_sandbox_level: self.windows_sandbox_level,
            })?;
        if let Some(network) = exec_request.network.as_ref() {
            network.apply_to_env(&mut exec_request.env);
        }

        Ok(PreparedExec {
            command: exec_request.command,
            cwd: exec_request.cwd,
            env: exec_request.env,
            arg0: exec_request.arg0,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ParsedShellCommand {
    program: String,
    script: String,
    login: bool,
}

fn extract_shell_script(command: &[String]) -> Result<ParsedShellCommand, ToolError> {
    if let [program, flag, script, ..] = command {
        if flag == "-c" {
            return Ok(ParsedShellCommand {
                program: program.to_owned(),
                script: script.to_owned(),
                login: false,
            });
        }
        if flag == "-lc" {
            return Ok(ParsedShellCommand {
                program: program.to_owned(),
                script: script.to_owned(),
                login: true,
            });
        }
    }

    Err(ToolError::Rejected(
        "unexpected shell command format for zsh-fork execution".to_string(),
    ))
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
#[path = "unix_escalation_tests.rs"]
mod tests;
