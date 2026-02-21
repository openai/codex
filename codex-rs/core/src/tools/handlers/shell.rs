use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use std::sync::Arc;

use crate::codex::TurnContext;
use crate::exec::ExecParams;
use crate::exec_env::create_env;
use crate::exec_policy::ExecApprovalRequest;
use crate::function_tool::FunctionCallError;
use crate::is_safe_command::is_known_safe_command;
use crate::protocol::ExecCommandSource;
use crate::shell::Shell;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::apply_patch::intercept_apply_patch;
use crate::tools::handlers::parse_arguments;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::runtimes::shell::ShellRuntime;
use crate::tools::sandboxing::ToolCtx;

pub struct ShellHandler;

pub struct ShellCommandHandler;

struct RunExecLikeArgs {
    tool_name: String,
    exec_params: ExecParams,
    prefix_rule: Option<Vec<String>>,
    session: Arc<crate::codex::Session>,
    turn: Arc<TurnContext>,
    tracker: crate::tools::context::SharedTurnDiffTracker,
    call_id: String,
    freeform: bool,
}

impl ShellHandler {
    fn to_exec_params(
        params: &ShellToolCallParams,
        turn_context: &TurnContext,
        thread_id: ThreadId,
    ) -> ExecParams {
        ExecParams {
            command: params.command.clone(),
            cwd: turn_context.resolve_path(params.workdir.clone()),
            expiration: params.timeout_ms.into(),
            env: create_env(&turn_context.shell_environment_policy, Some(thread_id)),
            network: turn_context.network.clone(),
            sandbox_permissions: params.sandbox_permissions.unwrap_or_default(),
            windows_sandbox_level: turn_context.windows_sandbox_level,
            justification: params.justification.clone(),
            arg0: None,
        }
    }
}

impl ShellCommandHandler {
    fn resolve_use_login_shell(
        login: Option<bool>,
        allow_login_shell: bool,
    ) -> Result<bool, FunctionCallError> {
        if !allow_login_shell && login == Some(true) {
            return Err(FunctionCallError::RespondToModel(
                "login shell is disabled by config; omit `login` or set it to false.".to_string(),
            ));
        }

        Ok(login.unwrap_or(allow_login_shell))
    }

    fn base_command(shell: &Shell, command: &str, use_login_shell: bool) -> Vec<String> {
        shell.derive_exec_args(command, use_login_shell)
    }

    fn to_exec_params(
        params: &ShellCommandToolCallParams,
        session: &crate::codex::Session,
        turn_context: &TurnContext,
        thread_id: ThreadId,
        allow_login_shell: bool,
    ) -> Result<ExecParams, FunctionCallError> {
        let shell = session.user_shell();
        let use_login_shell = Self::resolve_use_login_shell(params.login, allow_login_shell)?;
        let command = Self::base_command(shell.as_ref(), &params.command, use_login_shell);

        Ok(ExecParams {
            command,
            cwd: turn_context.resolve_path(params.workdir.clone()),
            expiration: params.timeout_ms.into(),
            env: create_env(&turn_context.shell_environment_policy, Some(thread_id)),
            network: turn_context.network.clone(),
            sandbox_permissions: params.sandbox_permissions.unwrap_or_default(),
            windows_sandbox_level: turn_context.windows_sandbox_level,
            justification: params.justification.clone(),
            arg0: None,
        })
    }
}

#[async_trait]
impl ToolHandler for ShellHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::LocalShell { .. }
        )
    }

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        match &invocation.payload {
            ToolPayload::Function { arguments } => {
                serde_json::from_str::<ShellToolCallParams>(arguments)
                    .map(|params| !is_known_safe_command(&params.command))
                    .unwrap_or(true)
            }
            ToolPayload::LocalShell { params } => !is_known_safe_command(&params.command),
            _ => true, // unknown payloads => assume mutating
        }
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            payload,
        } = invocation;

        match payload {
            ToolPayload::Function { arguments } => {
                let params: ShellToolCallParams = parse_arguments(&arguments)?;
                let prefix_rule = params.prefix_rule.clone();
                let exec_params =
                    Self::to_exec_params(&params, turn.as_ref(), session.conversation_id);
                Self::run_exec_like(RunExecLikeArgs {
                    tool_name: tool_name.clone(),
                    exec_params,
                    prefix_rule,
                    session,
                    turn,
                    tracker,
                    call_id,
                    freeform: false,
                })
                .await
            }
            ToolPayload::LocalShell { params } => {
                let exec_params =
                    Self::to_exec_params(&params, turn.as_ref(), session.conversation_id);
                Self::run_exec_like(RunExecLikeArgs {
                    tool_name: tool_name.clone(),
                    exec_params,
                    prefix_rule: None,
                    session,
                    turn,
                    tracker,
                    call_id,
                    freeform: false,
                })
                .await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for shell handler: {tool_name}"
            ))),
        }
    }
}

#[async_trait]
impl ToolHandler for ShellCommandHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return true;
        };

        serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .map(|params| {
                let use_login_shell = match Self::resolve_use_login_shell(
                    params.login,
                    invocation.turn.tools_config.allow_login_shell,
                ) {
                    Ok(use_login_shell) => use_login_shell,
                    Err(_) => return true,
                };
                let shell = invocation.session.user_shell();
                let command = Self::base_command(shell.as_ref(), &params.command, use_login_shell);
                !is_known_safe_command(&command)
            })
            .unwrap_or(true)
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            payload,
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for shell_command handler: {tool_name}"
            )));
        };

        let params: ShellCommandToolCallParams = parse_arguments(&arguments)?;
        let prefix_rule = params.prefix_rule.clone();
        let exec_params = Self::to_exec_params(
            &params,
            session.as_ref(),
            turn.as_ref(),
            session.conversation_id,
            turn.tools_config.allow_login_shell,
        )?;
        ShellHandler::run_exec_like(RunExecLikeArgs {
            tool_name,
            exec_params,
            prefix_rule,
            session,
            turn,
            tracker,
            call_id,
            freeform: true,
        })
        .await
    }
}

impl ShellHandler {
    async fn run_exec_like(args: RunExecLikeArgs) -> Result<ToolOutput, FunctionCallError> {
        let RunExecLikeArgs {
            tool_name,
            exec_params,
            prefix_rule,
            session,
            turn,
            tracker,
            call_id,
            freeform,
        } = args;

        let mut exec_params = exec_params;
        let dependency_env = session.dependency_env().await;
        if !dependency_env.is_empty() {
            exec_params.env.extend(dependency_env.clone());
        }

        let mut explicit_env_overrides = turn.shell_environment_policy.r#set.clone();
        for key in dependency_env.keys() {
            if let Some(value) = exec_params.env.get(key) {
                explicit_env_overrides.insert(key.clone(), value.clone());
            }
        }

        // Approval policy guard for explicit escalation in non-OnRequest modes.
        if exec_params
            .sandbox_permissions
            .requires_escalated_permissions()
            && !matches!(
                turn.approval_policy,
                codex_protocol::protocol::AskForApproval::OnRequest
            )
        {
            let approval_policy = turn.approval_policy;
            return Err(FunctionCallError::RespondToModel(format!(
                "approval policy is {approval_policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {approval_policy:?}"
            )));
        }

        // Intercept apply_patch if present.
        if let Some(output) = intercept_apply_patch(
            &exec_params.command,
            &exec_params.cwd,
            exec_params.expiration.timeout_ms(),
            session.as_ref(),
            turn.as_ref(),
            Some(&tracker),
            &call_id,
            tool_name.as_str(),
        )
        .await?
        {
            return Ok(output);
        }

        let source = ExecCommandSource::Agent;
        let emitter = ToolEmitter::shell(
            exec_params.command.clone(),
            exec_params.cwd.clone(),
            source,
            freeform,
        );
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, None);
        emitter.begin(event_ctx).await;

        let exec_approval_requirement = session
            .services
            .exec_policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &exec_params.command,
                approval_policy: turn.approval_policy,
                sandbox_policy: &turn.sandbox_policy,
                sandbox_permissions: exec_params.sandbox_permissions,
                prefix_rule,
            })
            .await;

        let req = ShellRequest {
            command: exec_params.command.clone(),
            cwd: exec_params.cwd.clone(),
            timeout_ms: exec_params.expiration.timeout_ms(),
            env: exec_params.env.clone(),
            explicit_env_overrides,
            network: exec_params.network.clone(),
            sandbox_permissions: exec_params.sandbox_permissions,
            justification: exec_params.justification.clone(),
            exec_approval_requirement,
        };
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ShellRuntime::new();
        let tool_ctx = ToolCtx {
            session: session.as_ref(),
            turn: turn.as_ref(),
            call_id: call_id.clone(),
            tool_name,
        };
        let out = orchestrator
            .run(&mut runtime, &req, &tool_ctx, &turn, turn.approval_policy)
            .await
            .map(|result| result.output);
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, None);
        let content = emitter.finish(event_ctx, out).await?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use codex_protocol::models::ShellCommandToolCallParams;
    use pretty_assertions::assert_eq;

    use crate::codex::make_session_and_context;
    use crate::exec_env::create_env;
    use crate::is_safe_command::is_known_safe_command;
    use crate::powershell::try_find_powershell_executable_blocking;
    use crate::powershell::try_find_pwsh_executable_blocking;
    use crate::sandboxing::SandboxPermissions;
    use crate::shell::Shell;
    use crate::shell::ShellType;
    use crate::shell_snapshot::ShellSnapshot;
    use crate::tools::handlers::ShellCommandHandler;
    use tokio::sync::watch;

    /// The logic for is_known_safe_command() has heuristics for known shells,
    /// so we must ensure the commands generated by [ShellCommandHandler] can be
    /// recognized as safe if the `command` is safe.
    #[test]
    fn commands_generated_by_shell_command_handler_can_be_matched_by_is_known_safe_command() {
        let bash_shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        };
        assert_safe(&bash_shell, "ls -la");

        let zsh_shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        };
        assert_safe(&zsh_shell, "ls -la");

        if let Some(path) = try_find_powershell_executable_blocking() {
            let powershell = Shell {
                shell_type: ShellType::PowerShell,
                shell_path: path.to_path_buf(),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            };
            assert_safe(&powershell, "ls -Name");
        }

        if let Some(path) = try_find_pwsh_executable_blocking() {
            let pwsh = Shell {
                shell_type: ShellType::PowerShell,
                shell_path: path.to_path_buf(),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            };
            assert_safe(&pwsh, "ls -Name");
        }
    }

    fn assert_safe(shell: &Shell, command: &str) {
        assert!(is_known_safe_command(
            &shell.derive_exec_args(command, /* use_login_shell */ true)
        ));
        assert!(is_known_safe_command(
            &shell.derive_exec_args(command, /* use_login_shell */ false)
        ));
    }

    #[tokio::test]
    async fn shell_command_handler_to_exec_params_uses_session_shell_and_turn_context() {
        let (session, turn_context) = make_session_and_context().await;

        let command = "echo hello".to_string();
        let workdir = Some("subdir".to_string());
        let login = None;
        let timeout_ms = Some(1234);
        let sandbox_permissions = SandboxPermissions::RequireEscalated;
        let justification = Some("because tests".to_string());

        let expected_command = session.user_shell().derive_exec_args(&command, true);
        let expected_cwd = turn_context.resolve_path(workdir.clone());
        let expected_env = create_env(
            &turn_context.shell_environment_policy,
            Some(session.conversation_id),
        );

        let params = ShellCommandToolCallParams {
            command,
            workdir,
            login,
            timeout_ms,
            sandbox_permissions: Some(sandbox_permissions),
            prefix_rule: None,
            justification: justification.clone(),
        };

        let exec_params = ShellCommandHandler::to_exec_params(
            &params,
            &session,
            &turn_context,
            session.conversation_id,
            true,
        )
        .expect("login shells should be allowed");

        // ExecParams cannot derive Eq due to the CancellationToken field, so we manually compare the fields.
        assert_eq!(exec_params.command, expected_command);
        assert_eq!(exec_params.cwd, expected_cwd);
        assert_eq!(exec_params.env, expected_env);
        assert_eq!(exec_params.network, turn_context.network);
        assert_eq!(exec_params.expiration.timeout_ms(), timeout_ms);
        assert_eq!(exec_params.sandbox_permissions, sandbox_permissions);
        assert_eq!(exec_params.justification, justification);
        assert_eq!(exec_params.arg0, None);
    }

    #[test]
    fn shell_command_handler_respects_explicit_login_flag() {
        let (_tx, shell_snapshot) = watch::channel(Some(Arc::new(ShellSnapshot {
            path: PathBuf::from("/tmp/snapshot.sh"),
            cwd: PathBuf::from("/tmp"),
        })));
        let shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_snapshot,
        };

        let login_command = ShellCommandHandler::base_command(&shell, "echo login shell", true);
        assert_eq!(
            login_command,
            shell.derive_exec_args("echo login shell", true)
        );

        let non_login_command =
            ShellCommandHandler::base_command(&shell, "echo non login shell", false);
        assert_eq!(
            non_login_command,
            shell.derive_exec_args("echo non login shell", false)
        );
    }

    #[tokio::test]
    async fn shell_command_handler_defaults_to_non_login_when_disallowed() {
        let (session, turn_context) = make_session_and_context().await;
        let params = ShellCommandToolCallParams {
            command: "echo hello".to_string(),
            workdir: None,
            login: None,
            timeout_ms: None,
            sandbox_permissions: None,
            prefix_rule: None,
            justification: None,
        };

        let exec_params = ShellCommandHandler::to_exec_params(
            &params,
            &session,
            &turn_context,
            session.conversation_id,
            false,
        )
        .expect("non-login shells should still be allowed");

        assert_eq!(
            exec_params.command,
            session.user_shell().derive_exec_args("echo hello", false)
        );
    }

    #[test]
    fn shell_command_handler_rejects_login_when_disallowed() {
        let err = ShellCommandHandler::resolve_use_login_shell(Some(true), false)
            .expect_err("explicit login should be rejected");

        assert!(
            err.to_string()
                .contains("login shell is disabled by config"),
            "unexpected error: {err}"
        );
    }
}
