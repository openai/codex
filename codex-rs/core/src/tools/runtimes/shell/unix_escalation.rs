use super::ShellRequest;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::exec::is_likely_sandbox_denied;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;
use crate::shell::ShellType;
use crate::tools::runtimes::build_sandbox_command;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::MatchOptions;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use codex_features::Feature;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
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
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) struct PreparedUnifiedExecZshFork {
    pub(crate) exec_request: ExecRequest,
    pub(crate) escalation_session: EscalationSession,
}

const PROMPT_CONFLICT_REASON: &str =
    "approval required by policy, but AskForApproval is set to Never";
const REJECT_SANDBOX_APPROVAL_REASON: &str =
    "approval required by policy, but AskForApproval::Granular.sandbox_approval is false";
const REJECT_RULES_APPROVAL_REASON: &str =
    "approval required by policy rule, but AskForApproval::Granular.rules is false";
fn approval_sandbox_permissions(
    sandbox_permissions: SandboxPermissions,
    additional_permissions_preapproved: bool,
) -> SandboxPermissions {
    if additional_permissions_preapproved
        && matches!(
            sandbox_permissions,
            SandboxPermissions::WithAdditionalPermissions
        )
    {
        SandboxPermissions::UseDefault
    } else {
        sandbox_permissions
    }
}

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

    let command = build_sandbox_command(
        command,
        &req.cwd,
        &req.env,
        req.additional_permissions.clone(),
    )?;
    let options = ExecOptions {
        expiration: req.timeout_ms.into(),
        capture_policy: ExecCapturePolicy::ShellTool,
    };
    let sandbox_exec_request = attempt
        .env_for(command, options, req.network.as_ref())
        .map_err(|err| ToolError::Codex(err.into()))?;
    let crate::sandboxing::ExecRequest {
        command,
        cwd: sandbox_cwd,
        env: sandbox_env,
        network: sandbox_network,
        expiration: _sandbox_expiration,
        capture_policy: _capture_policy,
        sandbox,
        windows_sandbox_level,
        windows_sandbox_private_desktop: _windows_sandbox_private_desktop,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        windows_restricted_token_filesystem_overlay: _windows_restricted_token_filesystem_overlay,
        arg0,
    } = sandbox_exec_request;
    let ParsedShellCommand { script, login, .. } = extract_shell_script(&command)?;
    let effective_timeout = Duration::from_millis(
        req.timeout_ms
            .unwrap_or(crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
    );
    let exec_policy = Arc::new(RwLock::new(
        ctx.session.services.exec_policy.current().as_ref().clone(),
    ));
    let command_executor = CoreShellCommandExecutor {
        command,
        cwd: sandbox_cwd,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox,
        env: sandbox_env,
        network: sandbox_network,
        windows_sandbox_level,
        arg0,
        sandbox_policy_cwd: ctx.turn.cwd.to_path_buf(),
        macos_seatbelt_profile_extensions: ctx
            .turn
            .config
            .permissions
            .macos_seatbelt_profile_extensions
            .clone(),
        codex_linux_sandbox_exe: ctx.turn.codex_linux_sandbox_exe.clone(),
        use_legacy_landlock: ctx.turn.features.use_legacy_landlock(),
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
    let approval_sandbox_permissions = approval_sandbox_permissions(
        req.sandbox_permissions,
        req.additional_permissions_preapproved,
    );
    let escalation_policy = CoreShellActionProvider {
        policy: Arc::clone(&exec_policy),
        session: Arc::clone(&ctx.session),
        turn: Arc::clone(&ctx.turn),
        call_id: ctx.call_id.clone(),
        tool_name: "shell",
        approval_policy: ctx.turn.approval_policy.value(),
        sandbox_policy: command_executor.sandbox_policy.clone(),
        file_system_sandbox_policy: command_executor.file_system_sandbox_policy.clone(),
        network_sandbox_policy: command_executor.network_sandbox_policy,
        sandbox_permissions: req.sandbox_permissions,
        approval_sandbox_permissions,
        prompt_permissions: req.additional_permissions.clone(),
        stopwatch: stopwatch.clone(),
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
    _attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    exec_request: ExecRequest,
    shell_zsh_path: &std::path::Path,
    main_execve_wrapper_exe: &std::path::Path,
) -> Result<Option<PreparedUnifiedExecZshFork>, ToolError> {
    let parsed = match extract_shell_script(&exec_request.command) {
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

    let exec_policy = Arc::new(RwLock::new(
        ctx.session.services.exec_policy.current().as_ref().clone(),
    ));
    let command_executor = CoreShellCommandExecutor {
        command: exec_request.command.clone(),
        cwd: exec_request.cwd.clone(),
        sandbox_policy: exec_request.sandbox_policy.clone(),
        file_system_sandbox_policy: exec_request.file_system_sandbox_policy.clone(),
        network_sandbox_policy: exec_request.network_sandbox_policy,
        sandbox: exec_request.sandbox,
        env: exec_request.env.clone(),
        network: exec_request.network.clone(),
        windows_sandbox_level: exec_request.windows_sandbox_level,
        arg0: exec_request.arg0.clone(),
        sandbox_policy_cwd: ctx.turn.cwd.to_path_buf(),
        macos_seatbelt_profile_extensions: ctx
            .turn
            .config
            .permissions
            .macos_seatbelt_profile_extensions
            .clone(),
        codex_linux_sandbox_exe: ctx.turn.codex_linux_sandbox_exe.clone(),
        use_legacy_landlock: ctx.turn.features.use_legacy_landlock(),
    };
    let escalation_policy = CoreShellActionProvider {
        policy: Arc::clone(&exec_policy),
        session: Arc::clone(&ctx.session),
        turn: Arc::clone(&ctx.turn),
        call_id: ctx.call_id.clone(),
        tool_name: "exec_command",
        approval_policy: ctx.turn.approval_policy.value(),
        sandbox_policy: exec_request.sandbox_policy.clone(),
        file_system_sandbox_policy: exec_request.file_system_sandbox_policy.clone(),
        network_sandbox_policy: exec_request.network_sandbox_policy,
        sandbox_permissions: req.sandbox_permissions,
        approval_sandbox_permissions: approval_sandbox_permissions(
            req.sandbox_permissions,
            req.additional_permissions_preapproved,
        ),
        prompt_permissions: req.additional_permissions.clone(),
        stopwatch: Stopwatch::unlimited(),
    };

    let escalate_server = EscalateServer::new(
        shell_zsh_path.to_path_buf(),
        main_execve_wrapper_exe.to_path_buf(),
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
    policy: Arc<RwLock<Policy>>,
    session: Arc<crate::codex::Session>,
    turn: Arc<crate::codex::TurnContext>,
    call_id: String,
    tool_name: &'static str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    file_system_sandbox_policy: FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_permissions: SandboxPermissions,
    approval_sandbox_permissions: SandboxPermissions,
    prompt_permissions: Option<PermissionProfile>,
    stopwatch: Stopwatch,
}

#[allow(clippy::large_enum_variant)]
enum DecisionSource {
    PrefixRule,
    /// Often, this is `is_safe_command()`.
    UnmatchedCommandFallback,
}

fn execve_prompt_is_rejected_by_policy(
    approval_policy: AskForApproval,
    decision_source: &DecisionSource,
) -> Option<&'static str> {
    match (approval_policy, decision_source) {
        (AskForApproval::Never, _) => Some(PROMPT_CONFLICT_REASON),
        (AskForApproval::Granular(granular_config), DecisionSource::PrefixRule)
            if !granular_config.allows_rules_approval() =>
        {
            Some(REJECT_RULES_APPROVAL_REASON)
        }
        (AskForApproval::Granular(granular_config), DecisionSource::UnmatchedCommandFallback)
            if !granular_config.allows_sandbox_approval() =>
        {
            Some(REJECT_SANDBOX_APPROVAL_REASON)
        }
        _ => None,
    }
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
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        network_sandbox_policy: NetworkSandboxPolicy,
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
                            file_system_sandbox_policy: file_system_sandbox_policy.clone(),
                            network_sandbox_policy,
                            macos_seatbelt_profile_extensions: macos_seatbelt_profile_extensions
                                .cloned(),
                        },
                    ))
                })
                .unwrap_or(EscalationExecution::TurnDefault),
        }
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
        let tool_name = self.tool_name;
        Ok(stopwatch
            .pause_for(async move {
                if routes_approval_to_guardian(&turn) {
                    return review_approval_request(
                        &session,
                        &turn,
                        GuardianApprovalRequest::Execve {
                            id: call_id.clone(),
                            tool_name: tool_name.to_string(),
                            program: program.to_string_lossy().into_owned(),
                            argv: argv.to_vec(),
                            cwd: workdir,
                            additional_permissions,
                        },
                        /*retry_reason*/ None,
                    )
                    .await;
                }
                session
                    .request_command_approval(
                        &turn,
                        call_id,
                        approval_id,
                        command,
                        workdir,
                        /*reason*/ None,
                        /*network_approval_context*/ None,
                        /*proposed_execpolicy_amendment*/ None,
                        additional_permissions,
                        /*skill_metadata*/ None,
                        Some(vec![ReviewDecision::Approved, ReviewDecision::Abort]),
                    )
                    .await
            })
            .await)
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
                if execve_prompt_is_rejected_by_policy(self.approval_policy, &decision_source)
                    .is_some()
                {
                    EscalationDecision::deny(Some("Execution forbidden by policy".to_string()))
                } else {
                    match self
                        .prompt(program, argv, workdir, &self.stopwatch, prompt_permissions)
                        .await?
                    {
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedForSession
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
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

        let evaluation = {
            let policy = self.policy.read().await;
            evaluate_intercepted_exec_policy(
                &policy,
                program,
                argv,
                InterceptedExecPolicyContext {
                    approval_policy: self.approval_policy,
                    sandbox_policy: &self.sandbox_policy,
                    file_system_sandbox_policy: &self.file_system_sandbox_policy,
                    sandbox_permissions: self.approval_sandbox_permissions,
                    enable_shell_wrapper_parsing:
                        ENABLE_INTERCEPTED_EXEC_POLICY_SHELL_WRAPPER_PARSING,
                },
            )
        };
        // When true, means the Evaluation was due to *.rules, not the
        // fallback function.
        let decision_driven_by_policy =
            Self::decision_driven_by_policy(&evaluation.matched_rules, evaluation.decision);
        let needs_escalation =
            self.sandbox_permissions.requires_escalated_permissions() || decision_driven_by_policy;

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
                &self.file_system_sandbox_policy,
                self.network_sandbox_policy,
                self.prompt_permissions.as_ref(),
                self.turn
                    .config
                    .permissions
                    .macos_seatbelt_profile_extensions
                    .as_ref(),
            ),
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
    context: InterceptedExecPolicyContext<'_>,
) -> Evaluation {
    let InterceptedExecPolicyContext {
        approval_policy,
        sandbox_policy,
        file_system_sandbox_policy,
        sandbox_permissions,
        enable_shell_wrapper_parsing,
    } = context;
    let CandidateCommands {
        commands,
        used_complex_parsing,
    } = if enable_shell_wrapper_parsing {
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
            file_system_sandbox_policy,
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

#[derive(Clone, Copy)]
struct InterceptedExecPolicyContext<'a> {
    approval_policy: AskForApproval,
    sandbox_policy: &'a SandboxPolicy,
    file_system_sandbox_policy: &'a FileSystemSandboxPolicy,
    sandbox_permissions: SandboxPermissions,
    enable_shell_wrapper_parsing: bool,
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
    file_system_sandbox_policy: FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox: SandboxType,
    env: HashMap<String, String>,
    network: Option<codex_network_proxy::NetworkProxy>,
    windows_sandbox_level: WindowsSandboxLevel,
    arg0: Option<String>,
    sandbox_policy_cwd: PathBuf,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    macos_seatbelt_profile_extensions: Option<MacOsSeatbeltProfileExtensions>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    use_legacy_landlock: bool,
}

struct PrepareSandboxedExecParams<'a> {
    command: Vec<String>,
    workdir: &'a AbsolutePathBuf,
    env: HashMap<String, String>,
    sandbox_policy: &'a SandboxPolicy,
    file_system_sandbox_policy: &'a FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
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
        for var in ["CODEX_ESCALATE_SOCKET", "EXEC_WRAPPER"] {
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
                capture_policy: ExecCapturePolicy::ShellTool,
                sandbox: self.sandbox,
                windows_sandbox_level: self.windows_sandbox_level,
                windows_sandbox_private_desktop: false,
                sandbox_policy: self.sandbox_policy.clone(),
                file_system_sandbox_policy: self.file_system_sandbox_policy.clone(),
                network_sandbox_policy: self.network_sandbox_policy,
                windows_restricted_token_filesystem_overlay: None,
                arg0: self.arg0.clone(),
            },
            /*stdout_stream*/ None,
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
        let command = join_program_and_argv(program, argv);
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
                    file_system_sandbox_policy: &self.file_system_sandbox_policy,
                    network_sandbox_policy: self.network_sandbox_policy,
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
                // On macOS, additional profile extensions are unioned with the turn defaults.
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    sandbox_policy: &self.sandbox_policy,
                    file_system_sandbox_policy: &self.file_system_sandbox_policy,
                    network_sandbox_policy: self.network_sandbox_policy,
                    additional_permissions: Some(permission_profile),
                    #[cfg(target_os = "macos")]
                    macos_seatbelt_profile_extensions: self
                        .macos_seatbelt_profile_extensions
                        .as_ref(),
                })?
            }
            EscalationExecution::Permissions(EscalationPermissions::Permissions(permissions)) => {
                // Use a fully specified sandbox policy instead of merging into the turn policy.
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    sandbox_policy: &permissions.sandbox_policy,
                    file_system_sandbox_policy: &permissions.file_system_sandbox_policy,
                    network_sandbox_policy: permissions.network_sandbox_policy,
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
    #[allow(clippy::too_many_arguments)]
    fn prepare_sandboxed_exec(
        &self,
        params: PrepareSandboxedExecParams<'_>,
    ) -> anyhow::Result<PreparedExec> {
        let PrepareSandboxedExecParams {
            command,
            workdir,
            env,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            additional_permissions,
            #[cfg(target_os = "macos")]
            macos_seatbelt_profile_extensions,
        } = params;
        let (program, args) = command
            .split_first()
            .ok_or_else(|| anyhow::anyhow!("prepared command must not be empty"))?;
        let sandbox_manager = SandboxManager::new();
        let sandbox = sandbox_manager.select_initial(
            file_system_sandbox_policy,
            network_sandbox_policy,
            SandboxablePreference::Auto,
            self.windows_sandbox_level,
            self.network.is_some(),
        );
        let command = SandboxCommand {
            program: program.clone().into(),
            args: args.to_vec(),
            cwd: workdir.to_path_buf(),
            env,
            additional_permissions,
        };
        let options = ExecOptions {
            expiration: ExecExpiration::DefaultTimeout,
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let exec_request = sandbox_manager.transform(SandboxTransformRequest {
            command,
            policy: sandbox_policy,
            file_system_policy: file_system_sandbox_policy,
            network_policy: network_sandbox_policy,
            sandbox,
            enforce_managed_network: self.network.is_some(),
            network: self.network.as_ref(),
            sandbox_policy_cwd: &self.sandbox_policy_cwd,
            #[cfg(target_os = "macos")]
            macos_seatbelt_profile_extensions,
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.as_ref(),
            use_legacy_landlock: self.use_legacy_landlock,
            windows_sandbox_level: self.windows_sandbox_level,
            windows_sandbox_private_desktop: false,
        })?;
        let mut exec_request =
            crate::sandboxing::ExecRequest::from_sandbox_exec_request(exec_request, options);
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
    // Commands reaching zsh-fork can be wrapped by environment/sandbox helpers, so
    // we search for the first `-c`/`-lc` triple anywhere in the argv rather
    // than assuming it is the first positional form.
    if let Some((program, script, login)) = command.windows(3).find_map(|parts| match parts {
        [program, flag, script] if flag == "-c" => {
            Some((program.to_owned(), script.to_owned(), false))
        }
        [program, flag, script] if flag == "-lc" => {
            Some((program.to_owned(), script.to_owned(), true))
        }
        _ => None,
    }) {
        return Ok(ParsedShellCommand {
            program,
            script,
            login,
        });
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
