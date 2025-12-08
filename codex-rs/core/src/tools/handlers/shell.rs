use async_trait::async_trait;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use std::sync::Arc;

use crate::codex::TurnContext;
use crate::exec::ExecParams;
use crate::exec_env::create_env;
use crate::exec_policy::create_exec_approval_requirement_for_command;
use crate::function_tool::FunctionCallError;
use crate::is_safe_command::is_known_safe_command;
use crate::protocol::ExecCommandSource;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::apply_patch::intercept_apply_patch;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::runtimes::shell::ShellRuntime;
use crate::tools::sandboxing::ToolCtx;

pub struct ShellHandler;

pub struct ShellCommandHandler;

impl ShellHandler {
    fn to_exec_params(params: ShellToolCallParams, turn_context: &TurnContext) -> ExecParams {
        ExecParams {
            command: params.command,
            cwd: turn_context.resolve_path(params.workdir.clone()),
            expiration: params.timeout_ms.into(),
            env: create_env(&turn_context.shell_environment_policy),
            with_escalated_permissions: params.with_escalated_permissions,
            justification: params.justification,
            arg0: None,
        }
    }
}

impl ShellCommandHandler {
    fn to_exec_params(
        params: ShellCommandToolCallParams,
        session: &crate::codex::Session,
        turn_context: &TurnContext,
    ) -> ExecParams {
        let shell = session.user_shell();
        let command = shell.derive_exec_args(&params.command, params.login.unwrap_or(true));

        ExecParams {
            command,
            cwd: turn_context.resolve_path(params.workdir.clone()),
            expiration: params.timeout_ms.into(),
            env: create_env(&turn_context.shell_environment_policy),
            with_escalated_permissions: params.with_escalated_permissions,
            justification: params.justification,
            arg0: None,
        }
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

    fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
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
                let params: ShellToolCallParams =
                    serde_json::from_str(&arguments).map_err(|e| {
                        FunctionCallError::RespondToModel(format!(
                            "failed to parse function arguments: {e:?}"
                        ))
                    })?;
                let exec_params = Self::to_exec_params(params, turn.as_ref());
                Self::run_exec_like(
                    tool_name.as_str(),
                    exec_params,
                    session,
                    turn,
                    tracker,
                    call_id,
                    false,
                )
                .await
            }
            ToolPayload::LocalShell { params } => {
                let exec_params = Self::to_exec_params(params, turn.as_ref());
                Self::run_exec_like(
                    tool_name.as_str(),
                    exec_params,
                    session,
                    turn,
                    tracker,
                    call_id,
                    false,
                )
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

    fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return true;
        };

        serde_json::from_str::<ShellCommandToolCallParams>(arguments)
            .map(|params| {
                let shell = invocation.session.user_shell();
                let command = shell.derive_exec_args(&params.command, params.login.unwrap_or(true));
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

        let params: ShellCommandToolCallParams = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;
        let exec_params = Self::to_exec_params(params, session.as_ref(), turn.as_ref());
        ShellHandler::run_exec_like(
            tool_name.as_str(),
            exec_params,
            session,
            turn,
            tracker,
            call_id,
            true,
        )
        .await
    }
}

impl ShellHandler {
    async fn run_exec_like(
        tool_name: &str,
        exec_params: ExecParams,
        session: Arc<crate::codex::Session>,
        turn: Arc<TurnContext>,
        tracker: crate::tools::context::SharedTurnDiffTracker,
        call_id: String,
        freeform: bool,
    ) -> Result<ToolOutput, FunctionCallError> {
        // Approval policy guard for explicit escalation in non-OnRequest modes.
        if exec_params.with_escalated_permissions.unwrap_or(false)
            && !matches!(
                turn.approval_policy,
                codex_protocol::protocol::AskForApproval::OnRequest
            )
        {
            return Err(FunctionCallError::RespondToModel(format!(
                "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}",
                policy = turn.approval_policy
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
            tool_name,
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

        let features = session.features();
        let exec_approval_requirement = create_exec_approval_requirement_for_command(
            &turn.exec_policy,
            &features,
            &exec_params.command,
            turn.approval_policy,
            &turn.sandbox_policy,
            SandboxPermissions::from(exec_params.with_escalated_permissions.unwrap_or(false)),
        )
        .await;

        let req = ShellRequest {
            command: exec_params.command.clone(),
            cwd: exec_params.cwd.clone(),
            timeout_ms: exec_params.expiration.timeout_ms(),
            env: exec_params.env.clone(),
            with_escalated_permissions: exec_params.with_escalated_permissions,
            justification: exec_params.justification.clone(),
            exec_approval_requirement,
        };
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ShellRuntime::new();
        let tool_ctx = ToolCtx {
            session: session.as_ref(),
            turn: turn.as_ref(),
            call_id: call_id.clone(),
            tool_name: tool_name.to_string(),
        };
        let out = orchestrator
            .run(&mut runtime, &req, &tool_ctx, &turn, turn.approval_policy)
            .await;
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn.as_ref(), &call_id, None);
        let content = emitter.finish(event_ctx, out).await?;
        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::models::ShellCommandToolCallParams;
    use pretty_assertions::assert_eq;

    use crate::codex::make_session_and_context;
    use crate::exec_env::create_env;
    use crate::is_safe_command::is_known_safe_command;
    use crate::shell::Shell;
    use crate::shell::ShellType;
    use crate::tools::handlers::ShellCommandHandler;

    /// The logic for is_known_safe_command() has heuristics for known shells,
    /// so we must ensure the commands generated by [ShellCommandHandler] can be
    /// recognized as safe if the `command` is safe.
    #[test]
    fn commands_generated_by_shell_command_handler_can_be_matched_by_is_known_safe_command() {
        let bash_shell = Shell {
            shell_type: ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
        };
        assert_safe(&bash_shell, "ls -la");

        let zsh_shell = Shell {
            shell_type: ShellType::Zsh,
            shell_path: PathBuf::from("/bin/zsh"),
        };
        assert_safe(&zsh_shell, "ls -la");

        let powershell = Shell {
            shell_type: ShellType::PowerShell,
            shell_path: PathBuf::from("pwsh.exe"),
        };
        assert_safe(&powershell, "ls -Name");
    }

    fn assert_safe(shell: &Shell, command: &str) {
        assert!(is_known_safe_command(
            &shell.derive_exec_args(command, /* use_login_shell */ true)
        ));
        assert!(is_known_safe_command(
            &shell.derive_exec_args(command, /* use_login_shell */ false)
        ));
    }

    #[test]
    fn shell_command_handler_to_exec_params_uses_session_shell_and_turn_context() {
        let (session, turn_context) = make_session_and_context();

        let command = "echo hello".to_string();
        let workdir = Some("subdir".to_string());
        let login = None;
        let timeout_ms = Some(1234);
        let with_escalated_permissions = Some(true);
        let justification = Some("because tests".to_string());

        let expected_command = session.user_shell().derive_exec_args(&command, true);
        let expected_cwd = turn_context.resolve_path(workdir.clone());
        let expected_env = create_env(&turn_context.shell_environment_policy);

        let params = ShellCommandToolCallParams {
            command,
            workdir,
            login,
            timeout_ms,
            with_escalated_permissions,
            justification: justification.clone(),
        };

        let exec_params = ShellCommandHandler::to_exec_params(params, &session, &turn_context);

        // ExecParams cannot derive Eq due to the CancellationToken field, so we manually compare the fields.
        assert_eq!(exec_params.command, expected_command);
        assert_eq!(exec_params.cwd, expected_cwd);
        assert_eq!(exec_params.env, expected_env);
        assert_eq!(exec_params.expiration.timeout_ms(), timeout_ms);
        assert_eq!(
            exec_params.with_escalated_permissions,
            with_escalated_permissions
        );
        assert_eq!(exec_params.justification, justification);
        assert_eq!(exec_params.arg0, None);
    }
}
