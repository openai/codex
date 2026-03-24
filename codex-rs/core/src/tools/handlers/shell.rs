use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use std::sync::Arc;

use crate::codex::TurnContext;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecParams;
use crate::exec_env::create_env;
use crate::exec_policy::ExecApprovalRequest;
use crate::function_tool::FunctionCallError;
use crate::is_safe_command::is_known_safe_command;
use crate::protocol::ExecCommandSource;
use crate::shell::Shell;
use crate::skills::maybe_emit_implicit_skill_invocation;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::handlers::apply_granted_turn_permissions;
use crate::tools::handlers::apply_patch::intercept_apply_patch;
use crate::tools::handlers::implicit_granted_permissions;
use crate::tools::handlers::normalize_and_validate_additional_permissions;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::handlers::resolve_workdir_base_path;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::runtimes::shell::ShellRuntime;
use crate::tools::runtimes::shell::ShellRuntimeBackend;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::spec::ShellCommandBackendConfig;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;

pub struct ShellHandler;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellCommandBackend {
    Classic,
    ZshFork,
}

pub struct ShellCommandHandler {
    backend: ShellCommandBackend,
}

struct RunExecLikeArgs {
    tool_name: String,
    exec_params: ExecParams,
    additional_permissions: Option<PermissionProfile>,
    prefix_rule: Option<Vec<String>>,
    session: Arc<crate::codex::Session>,
    turn: Arc<TurnContext>,
    tracker: crate::tools::context::SharedTurnDiffTracker,
    call_id: String,
    freeform: bool,
    shell_runtime_backend: ShellRuntimeBackend,
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
            capture_policy: ExecCapturePolicy::ShellTool,
            env: create_env(&turn_context.shell_environment_policy, Some(thread_id)),
            network: turn_context.network.clone(),
            parent_tool_item_id: None,
            sandbox_permissions: params.sandbox_permissions.unwrap_or_default(),
            windows_sandbox_level: turn_context.windows_sandbox_level,
            windows_sandbox_private_desktop: turn_context
                .config
                .permissions
                .windows_sandbox_private_desktop,
            justification: params.justification.clone(),
            arg0: None,
        }
    }
}

impl ShellCommandHandler {
    fn shell_runtime_backend(&self) -> ShellRuntimeBackend {
        match self.backend {
            ShellCommandBackend::Classic => ShellRuntimeBackend::ShellCommandClassic,
            ShellCommandBackend::ZshFork => ShellRuntimeBackend::ShellCommandZshFork,
        }
    }

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
            capture_policy: ExecCapturePolicy::ShellTool,
            env: create_env(&turn_context.shell_environment_policy, Some(thread_id)),
            network: turn_context.network.clone(),
            parent_tool_item_id: None,
            sandbox_permissions: params.sandbox_permissions.unwrap_or_default(),
            windows_sandbox_level: turn_context.windows_sandbox_level,
            windows_sandbox_private_desktop: turn_context
                .config
                .permissions
                .windows_sandbox_private_desktop,
            justification: params.justification.clone(),
            arg0: None,
        })
    }
}

impl From<ShellCommandBackendConfig> for ShellCommandHandler {
    fn from(config: ShellCommandBackendConfig) -> Self {
        let backend = match config {
            ShellCommandBackendConfig::Classic => ShellCommandBackend::Classic,
            ShellCommandBackendConfig::ZshFork => ShellCommandBackend::ZshFork,
        };
        Self { backend }
    }
}

#[async_trait]
impl ToolHandler for ShellHandler {
    type Output = FunctionToolOutput;

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

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Function { arguments } => {
                let cwd = resolve_workdir_base_path(&arguments, turn.cwd.as_path())?;
                let params: ShellToolCallParams =
                    parse_arguments_with_base_path(&arguments, cwd.as_path())?;
                let prefix_rule = params.prefix_rule.clone();
                let exec_params =
                    Self::to_exec_params(&params, turn.as_ref(), session.conversation_id);
                Self::run_exec_like(RunExecLikeArgs {
                    tool_name: tool_name.clone(),
                    exec_params,
                    additional_permissions: params.additional_permissions.clone(),
                    prefix_rule,
                    session,
                    turn,
                    tracker,
                    call_id,
                    freeform: false,
                    shell_runtime_backend: ShellRuntimeBackend::Generic,
                })
                .await
            }
            ToolPayload::LocalShell { params } => {
                let exec_params =
                    Self::to_exec_params(&params, turn.as_ref(), session.conversation_id);
                Self::run_exec_like(RunExecLikeArgs {
                    tool_name: tool_name.clone(),
                    exec_params,
                    additional_permissions: None,
                    prefix_rule: None,
                    session,
                    turn,
                    tracker,
                    call_id,
                    freeform: false,
                    shell_runtime_backend: ShellRuntimeBackend::Generic,
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
    type Output = FunctionToolOutput;

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

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for shell_command handler: {tool_name}"
            )));
        };

        let cwd = resolve_workdir_base_path(&arguments, turn.cwd.as_path())?;
        let params: ShellCommandToolCallParams =
            parse_arguments_with_base_path(&arguments, cwd.as_path())?;
        maybe_emit_implicit_skill_invocation(
            session.as_ref(),
            turn.as_ref(),
            &params.command,
            params.workdir.as_deref(),
        )
        .await;
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
            additional_permissions: params.additional_permissions.clone(),
            prefix_rule,
            session,
            turn,
            tracker,
            call_id,
            freeform: true,
            shell_runtime_backend: self.shell_runtime_backend(),
        })
        .await
    }
}

impl ShellHandler {
    async fn run_exec_like(args: RunExecLikeArgs) -> Result<FunctionToolOutput, FunctionCallError> {
        let RunExecLikeArgs {
            tool_name,
            exec_params,
            additional_permissions,
            prefix_rule,
            session,
            turn,
            tracker,
            call_id,
            freeform,
            shell_runtime_backend,
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

        let exec_permission_approvals_enabled =
            session.features().enabled(Feature::ExecPermissionApprovals);
        let requested_additional_permissions = additional_permissions.clone();
        let effective_additional_permissions = apply_granted_turn_permissions(
            session.as_ref(),
            exec_params.sandbox_permissions,
            additional_permissions,
        )
        .await;
        let additional_permissions_allowed = exec_permission_approvals_enabled
            || (session.features().enabled(Feature::RequestPermissionsTool)
                && effective_additional_permissions.permissions_preapproved);
        let normalized_additional_permissions = implicit_granted_permissions(
            exec_params.sandbox_permissions,
            requested_additional_permissions.as_ref(),
            &effective_additional_permissions,
        )
        .map_or_else(
            || {
                normalize_and_validate_additional_permissions(
                    additional_permissions_allowed,
                    turn.approval_policy.value(),
                    effective_additional_permissions.sandbox_permissions,
                    effective_additional_permissions.additional_permissions,
                    effective_additional_permissions.permissions_preapproved,
                    &exec_params.cwd,
                )
            },
            |permissions| Ok(Some(permissions)),
        )
        .map_err(FunctionCallError::RespondToModel)?;

        // Approval policy guard for explicit escalation in non-OnRequest modes.
        // Sticky turn permissions have already been approved, so they should
        // continue through the normal exec approval flow for the command.
        if effective_additional_permissions
            .sandbox_permissions
            .requests_sandbox_override()
            && !effective_additional_permissions.permissions_preapproved
            && !matches!(
                turn.approval_policy.value(),
                codex_protocol::protocol::AskForApproval::OnRequest
            )
        {
            let approval_policy = turn.approval_policy.value();
            return Err(FunctionCallError::RespondToModel(format!(
                "approval policy is {approval_policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {approval_policy:?}"
            )));
        }

        // Intercept apply_patch if present.
        if let Some(output) = intercept_apply_patch(
            &exec_params.command,
            &exec_params.cwd,
            exec_params.expiration.timeout_ms(),
            session.clone(),
            turn.clone(),
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
        let event_ctx = ToolEventCtx::new(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            /*turn_diff_tracker*/ None,
        );
        emitter.begin(event_ctx).await;

        let exec_approval_requirement = session
            .services
            .exec_policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &exec_params.command,
                approval_policy: turn.approval_policy.value(),
                sandbox_policy: turn.sandbox_policy.get(),
                file_system_sandbox_policy: &turn.file_system_sandbox_policy,
                sandbox_permissions: if effective_additional_permissions.permissions_preapproved {
                    codex_protocol::models::SandboxPermissions::UseDefault
                } else {
                    effective_additional_permissions.sandbox_permissions
                },
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
            sandbox_permissions: effective_additional_permissions.sandbox_permissions,
            additional_permissions: normalized_additional_permissions,
            #[cfg(unix)]
            additional_permissions_preapproved: effective_additional_permissions
                .permissions_preapproved,
            justification: exec_params.justification.clone(),
            exec_approval_requirement,
        };
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = {
            use ShellRuntimeBackend::*;
            match shell_runtime_backend {
                Generic => ShellRuntime::new(),
                backend @ (ShellCommandClassic | ShellCommandZshFork) => {
                    ShellRuntime::for_shell_command(backend)
                }
            }
        };
        let tool_ctx = ToolCtx {
            session: session.clone(),
            turn: turn.clone(),
            call_id: call_id.clone(),
            tool_name,
        };
        let out = orchestrator
            .run(
                &mut runtime,
                &req,
                &tool_ctx,
                &turn,
                turn.approval_policy.value(),
            )
            .await
            .map(|result| result.output);
        let event_ctx = ToolEventCtx::new(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            /*turn_diff_tracker*/ None,
        );
        let content = emitter.finish(event_ctx, out).await?;
        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
