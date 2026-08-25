use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::guardian::GuardianReviewContext;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;
use crate::tools::approvals::ApprovalAction;
use crate::tools::approvals::ApprovalContext;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::unsandboxed_execution_allowed;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::MatchOptions;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErr;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GuardianCommandSource;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_escalation::EscalateServer;
use codex_shell_escalation::EscalationDecision;
use codex_shell_escalation::EscalationExecution;
use codex_shell_escalation::EscalationPermissions;
use codex_shell_escalation::EscalationPolicy;
use codex_shell_escalation::EscalationPolicyFuture;
use codex_shell_escalation::EscalationSession;
use codex_shell_escalation::ExecResult;
use codex_shell_escalation::PreparedExec;
use codex_shell_escalation::ResolvedPermissionProfile;
use codex_shell_escalation::ShellCommandExecutor;
use codex_shell_escalation::ShellCommandExecutorFuture;
use codex_shell_escalation::Stopwatch;
use codex_tools::ToolName;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::error;
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
        ctx.session
            .services
            .exec_policy
            .current_for_environment(
                req.turn_environment.config().exec_policy.as_ref(),
                ctx.step_context.turn.allow_prefix_rules(),
            )
            .as_ref()
            .clone(),
    ));
    // TODO(anp): Keep PathUri through the zsh-fork executor boundary.
    let cwd = exec_request
        .cwd
        .to_abs_path()
        .map_err(|err| ToolError::Rejected(err.to_string()))?;
    // TODO(anp): Keep PathUri through the zsh-fork sandbox policy boundary.
    let sandbox_policy_cwd = exec_request
        .windows_sandbox_policy_cwd
        .to_abs_path()
        .map_err(|err| ToolError::Rejected(err.to_string()))?;
    let command_executor = CoreShellCommandExecutor {
        command: exec_request.command.clone(),
        cwd,
        permission_profile: exec_request.permission_profile.clone(),
        sandbox: exec_request.sandbox,
        env: exec_request.env.clone(),
        network: exec_request.network.clone(),
        network_environment_id: exec_request.network_environment_id.clone(),
        windows_sandbox_level: exec_request.windows_sandbox_level,
        arg0: exec_request.arg0.clone(),
        sandbox_policy_cwd,
        windows_sandbox_workspace_roots: exec_request.windows_sandbox_workspace_roots.clone(),
        codex_linux_sandbox_exe: ctx.step_context.turn.config.codex_linux_sandbox_exe.clone(),
        use_legacy_landlock: ctx.step_context.turn.config.features.use_legacy_landlock(),
    };
    let escalation_policy = CoreShellActionProvider {
        policy: Arc::clone(&exec_policy),
        session: Arc::clone(&ctx.session),
        review_context: GuardianReviewContext::from(&ctx.step_context),
        call_id: ctx.call_id.clone(),
        environment_id: req.turn_environment.selection.environment_id.clone(),
        source: GuardianCommandSource::UnifiedExec,
        tool_name: ctx.tool_name.clone(),
        approval_policy: ctx.step_context.turn.approval_policy(),
        permission_profile: exec_request.permission_profile.clone(),
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
    session: Arc<crate::session::session::Session>,
    review_context: GuardianReviewContext,
    call_id: String,
    environment_id: String,
    source: GuardianCommandSource,
    tool_name: ToolName,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    sandbox_permissions: SandboxPermissions,
    approval_sandbox_permissions: SandboxPermissions,
    prompt_permissions: Option<AdditionalPermissionProfile>,
    stopwatch: Stopwatch,
}

#[allow(clippy::large_enum_variant)]
enum DecisionSource {
    PrefixRule,
    /// Derived from the unmatched-command sandbox fallback.
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
        permission_profile: &PermissionProfile,
        additional_permissions: Option<&AdditionalPermissionProfile>,
    ) -> EscalationExecution {
        match sandbox_permissions {
            SandboxPermissions::UseDefault => EscalationExecution::TurnDefault,
            SandboxPermissions::RequireEscalated => {
                if unsandboxed_execution_allowed(&permission_profile.file_system_sandbox_policy()) {
                    EscalationExecution::Unsandboxed
                } else {
                    EscalationExecution::TurnDefault
                }
            }
            SandboxPermissions::WithAdditionalPermissions => additional_permissions
                .map(|_| {
                    // Shell request additional permissions were already normalized and
                    // merged into the first-attempt sandbox policy.
                    EscalationExecution::Permissions(
                        EscalationPermissions::ResolvedPermissionProfile(
                            ResolvedPermissionProfile {
                                permission_profile: permission_profile.clone(),
                            },
                        ),
                    )
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
        additional_permissions: Option<AdditionalPermissionProfile>,
    ) -> anyhow::Result<ReviewDecision> {
        let command = join_program_and_argv(program, argv);
        let action = ApprovalAction::Execve {
            id: self.call_id.clone(),
            approval_id: Uuid::new_v4().to_string(),
            environment_id: self.environment_id.clone(),
            source: self.source,
            program: program.clone(),
            argv: argv.to_vec(),
            command,
            cwd: workdir.clone(),
            additional_permissions,
        };
        match stopwatch
            .pause_for(async {
                let (turn_context, strict_auto_review) = self
                    .session
                    .active_turn_context_and_strict_auto_review()
                    .await
                    .ok_or_else(|| {
                        ToolError::Rejected(
                            "cannot approve intercepted execution without an active turn"
                                .to_string(),
                        )
                    })?;
                let approval_ctx = ApprovalContext {
                    review_context: GuardianReviewContext::from(turn_context),
                    // The running process can outlive its launching tool or code-mode cell.
                    cancellation_token: None,
                    call_id: self.call_id.clone(),
                    tool_name: self.tool_name.clone(),
                    strict_auto_review,
                    approval_reason: None,
                    retry_reason: None,
                    network_approval_context: None,
                };
                self.session.request_approval(action, approval_ctx).await
            })
            .await
        {
            Ok(decision) => Ok(decision),
            Err(ToolError::Rejected(rejection)) => Ok(ReviewDecision::denied(rejection)),
            Err(ToolError::Codex(err)) => Err(err.into()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn process_decision(
        &self,
        decision: Decision,
        needs_escalation: bool,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        prompt_permissions: Option<AdditionalPermissionProfile>,
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
                    let decision = self
                        .prompt(program, argv, workdir, &self.stopwatch, prompt_permissions)
                        .await?;
                    match decision {
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
                        ReviewDecision::Denied { rejection } => {
                            EscalationDecision::deny(Some(rejection))
                        }
                        ReviewDecision::TimedOut => EscalationDecision::deny(Some(
                            crate::guardian::guardian_timeout_message(
                                &self.review_context.turn().model_info,
                            ),
                        )),
                        ReviewDecision::ApprovedMcpPolicyAmendment => {
                            error!("Shell escalation received ApprovedMcpPolicyAmendment");

                            EscalationDecision::deny(Some(
                                "Error while requesting approval".to_string(),
                            ))
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

impl CoreShellActionProvider {
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
                    permission_profile: self.permission_profile.clone(),
                    windows_sandbox_level: self.review_context.turn().windows_sandbox_level,
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
        let unsandboxed_allowed =
            unsandboxed_execution_allowed(&self.permission_profile.file_system_sandbox_policy());
        let needs_escalation = match self.sandbox_permissions {
            SandboxPermissions::UseDefault => unsandboxed_allowed && decision_driven_by_policy,
            SandboxPermissions::RequireEscalated => unsandboxed_allowed,
            SandboxPermissions::WithAdditionalPermissions => true,
        };

        let decision_source = if decision_driven_by_policy {
            DecisionSource::PrefixRule
        } else {
            DecisionSource::UnmatchedCommandFallback
        };
        let escalation_execution = match decision_source {
            DecisionSource::PrefixRule if unsandboxed_allowed => EscalationExecution::Unsandboxed,
            DecisionSource::PrefixRule => EscalationExecution::TurnDefault,
            DecisionSource::UnmatchedCommandFallback => Self::shell_request_escalation_execution(
                self.sandbox_permissions,
                &self.permission_profile,
                self.prompt_permissions.as_ref(),
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

impl EscalationPolicy for CoreShellActionProvider {
    fn determine_action<'a>(
        &'a self,
        program: &'a AbsolutePathBuf,
        argv: &'a [String],
        workdir: &'a AbsolutePathBuf,
    ) -> EscalationPolicyFuture<'a> {
        Box::pin(CoreShellActionProvider::determine_action(
            self, program, argv, workdir,
        ))
    }
}

fn evaluate_intercepted_exec_policy(
    policy: &Policy,
    program: &AbsolutePathBuf,
    argv: &[String],
    context: InterceptedExecPolicyContext,
) -> Evaluation {
    let InterceptedExecPolicyContext {
        approval_policy,
        permission_profile,
        windows_sandbox_level,
        sandbox_permissions,
        enable_shell_wrapper_parsing,
    } = context;
    let commands = if enable_shell_wrapper_parsing {
        // In this codepath, the first argument in `commands` could be a bare
        // name like `find` instead of an absolute path like `/usr/bin/find`.
        // It could also be a shell built-in like `echo`.
        commands_for_intercepted_exec_policy(program, argv)
    } else {
        // In this codepath, `commands` has a single entry where the program
        // is always an absolute path.
        vec![join_program_and_argv(program, argv)]
    };

    let fallback = |cmd: &[String]| {
        crate::exec_policy::render_decision_for_unmatched_command(
            cmd,
            crate::exec_policy::UnmatchedCommandContext {
                approval_policy,
                permission_profile: &permission_profile,
                windows_sandbox_level,
                sandbox_permissions,
                command_origin: crate::exec_policy::ExecPolicyCommandOrigin::Generic,
            },
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

#[derive(Clone)]
struct InterceptedExecPolicyContext {
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    // TODO(anp): Reconcile this policy input with TurnEnvironment::sandbox_context
    // so intercepted commands use the selected environment's Windows backend.
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_permissions: SandboxPermissions,
    enable_shell_wrapper_parsing: bool,
}

fn commands_for_intercepted_exec_policy(
    program: &AbsolutePathBuf,
    argv: &[String],
) -> Vec<Vec<String>> {
    if let [_, flag, script] = argv {
        let shell_command = [
            program.to_string_lossy().to_string(),
            flag.clone(),
            script.clone(),
        ];
        if let Some(commands) = parse_shell_lc_plain_commands(&shell_command)
            && !commands.is_empty()
        {
            return commands;
        }
    }

    vec![join_program_and_argv(program, argv)]
}

// TODO(anp): Capture these Windows and Landlock settings from
// TurnEnvironment::sandbox_context when preparing this executor, preserving its snapshot.
struct CoreShellCommandExecutor {
    command: Vec<String>,
    cwd: AbsolutePathBuf,
    permission_profile: PermissionProfile,
    sandbox: SandboxType,
    env: HashMap<String, String>,
    network: Option<codex_network_proxy::NetworkProxy>,
    network_environment_id: Option<String>,
    windows_sandbox_level: WindowsSandboxLevel,
    arg0: Option<String>,
    sandbox_policy_cwd: AbsolutePathBuf,
    windows_sandbox_workspace_roots: Vec<AbsolutePathBuf>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    use_legacy_landlock: bool,
}

struct PrepareSandboxedExecParams<'a> {
    command: Vec<String>,
    workdir: &'a AbsolutePathBuf,
    env: HashMap<String, String>,
    permission_profile: &'a PermissionProfile,
    additional_permissions: Option<AdditionalPermissionProfile>,
}

impl ShellCommandExecutor for CoreShellCommandExecutor {
    fn run(
        &self,
        _command: Vec<String>,
        _cwd: PathBuf,
        env_overlay: HashMap<String, String>,
        cancel_rx: CancellationToken,
        after_spawn: Option<Box<dyn FnOnce() + Send>>,
    ) -> ShellCommandExecutorFuture<'_, ExecResult> {
        Box::pin(CoreShellCommandExecutor::run(
            self,
            env_overlay,
            cancel_rx,
            after_spawn,
        ))
    }

    fn prepare_escalated_exec<'a>(
        &'a self,
        program: &'a AbsolutePathBuf,
        argv: &'a [String],
        workdir: &'a AbsolutePathBuf,
        env: HashMap<String, String>,
        execution: EscalationExecution,
    ) -> ShellCommandExecutorFuture<'a, PreparedExec> {
        Box::pin(CoreShellCommandExecutor::prepare_escalated_exec(
            self, program, argv, workdir, env, execution,
        ))
    }
}

impl CoreShellCommandExecutor {
    async fn run(
        &self,
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
                cwd: self.cwd.clone().into(),
                env: exec_env,
                exec_server_env_config: None,
                exec_server_shell_snapshot: None,
                network: self.network.clone(),
                network_environment_id: self.network_environment_id.clone(),
                expiration: ExecExpiration::Cancellation(cancel_rx),
                capture_policy: ExecCapturePolicy::ShellTool,
                sandbox: self.sandbox,
                windows_sandbox_policy_cwd: self.sandbox_policy_cwd.clone().into(),
                windows_sandbox_workspace_roots: self.windows_sandbox_workspace_roots.clone(),
                windows_sandbox_level: self.windows_sandbox_level,
                windows_sandbox_private_desktop: false,
                permission_profile: self.permission_profile.clone(),
                windows_sandbox_filesystem_overrides: None,
                arg0: self.arg0.clone(),
                exec_server_sandbox: None,
                exec_server_enforce_managed_network: false,
                exec_server_managed_network: None,
                exec_server_network_proxy: None,
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
        mut env: HashMap<String, String>,
        execution: EscalationExecution,
    ) -> anyhow::Result<PreparedExec> {
        let command = join_program_and_argv(program, argv);
        let Some(first_arg) = argv.first() else {
            return Err(anyhow::anyhow!(
                "intercepted exec request must contain argv[0]"
            ));
        };

        let prepared = match execution {
            EscalationExecution::Unsandboxed => {
                if let Some(network) = self.network.as_ref() {
                    network.restore_brokered_credentials(&mut env, &mut []);
                }
                PreparedExec {
                    command,
                    cwd: workdir.to_path_buf(),
                    env: exec_env_for_sandbox_permissions(
                        &env,
                        SandboxPermissions::RequireEscalated,
                    ),
                    arg0: Some(first_arg.clone()),
                }
            }
            EscalationExecution::TurnDefault => {
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    permission_profile: &self.permission_profile,
                    additional_permissions: None,
                })?
            }
            EscalationExecution::Permissions(
                EscalationPermissions::AdditionalPermissionProfile(permission_profile),
            ) => {
                // Merge additive permissions into the existing turn/request sandbox policy.
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    permission_profile: &self.permission_profile,
                    additional_permissions: Some(permission_profile),
                })?
            }
            EscalationExecution::Permissions(EscalationPermissions::ResolvedPermissionProfile(
                permissions,
            )) => {
                // Use a fully specified permission profile instead of merging into the turn policy.
                self.prepare_sandboxed_exec(PrepareSandboxedExecParams {
                    command,
                    workdir,
                    env,
                    permission_profile: &permissions.permission_profile,
                    additional_permissions: None,
                })?
            }
        };

        Ok(prepared)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_sandboxed_exec(
        &self,
        params: PrepareSandboxedExecParams<'_>,
    ) -> anyhow::Result<PreparedExec> {
        let PrepareSandboxedExecParams {
            command,
            workdir,
            env,
            permission_profile,
            additional_permissions,
        } = params;
        let (program, args) = command
            .split_first()
            .ok_or_else(|| anyhow::anyhow!("prepared command must not be empty"))?;
        let sandbox_manager = SandboxManager::new();
        let sandbox = sandbox_manager.select_initial(
            permission_profile,
            SandboxablePreference::Auto,
            self.windows_sandbox_level,
            self.network.is_some(),
        );
        let cwd = PathUri::from_abs_path(workdir);
        let sandbox_policy_cwd = PathUri::from_abs_path(&self.sandbox_policy_cwd);
        let command = SandboxCommand {
            program: program.clone().into(),
            args: args.to_vec(),
            cwd,
            env,
            managed_network: None,
            additional_permissions,
        };
        let options = ExecOptions {
            expiration: ExecExpiration::DefaultTimeout,
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let exec_request = sandbox_manager.transform(SandboxTransformRequest {
            command,
            permissions: permission_profile,
            sandbox,
            enforce_managed_network: self.network.is_some(),
            environment_id: self.network_environment_id.as_deref(),
            network: self.network.as_ref(),
            sandbox_policy_cwd: &sandbox_policy_cwd,
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.as_deref(),
            use_legacy_landlock: self.use_legacy_landlock,
            windows_sandbox_level: self.windows_sandbox_level,
            windows_sandbox_private_desktop: false,
        })?;
        let mut exec_request = crate::sandboxing::ExecRequest::from_sandbox_exec_request(
            exec_request,
            options,
            self.windows_sandbox_workspace_roots.clone(),
        )?;
        if let Some(network) = exec_request.network.as_ref() {
            network
                .apply_to_env_for_optional_environment(
                    &mut exec_request.env,
                    self.network_environment_id.as_deref(),
                )
                .map_err(|err| {
                    let environment_id =
                        self.network_environment_id.as_deref().unwrap_or("default");
                    CodexErr::Io(io::Error::other(format!(
                        "failed to prepare network proxy for environment `{environment_id}`: {err}"
                    )))
                })?;
        }

        Ok(PreparedExec {
            command: exec_request.command,
            // TODO(anp): Keep PathUri through the execve-wrapper boundary.
            cwd: exec_request.cwd.to_abs_path()?.to_path_buf(),
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
