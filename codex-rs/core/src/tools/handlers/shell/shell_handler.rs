use codex_protocol::ThreadId;
use codex_protocol::models::ShellToolCallParams;
use codex_shell_command::is_safe_command::is_known_safe_command;
use codex_tools::ToolName;

use crate::exec::ExecCapturePolicy;
use crate::exec::ExecParams;
use crate::exec_env::create_env;
use crate::function_tool::FunctionCallError;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::handlers::resolve_workdir_base_path;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::shell::ShellRuntimeBackend;
use codex_tools::ToolSpec;

use super::super::shell_spec::ShellToolOptions;
use super::super::shell_spec::create_shell_tool;
use super::RunExecLikeArgs;
use super::run_exec_like;
use super::shell_function_post_tool_use_payload;
use super::shell_function_pre_tool_use_payload;

#[derive(Default)]
pub struct ShellHandler {
    options: Option<ShellToolOptions>,
}

impl ShellHandler {
    pub(crate) fn new(options: ShellToolOptions) -> Self {
        Self {
            options: Some(options),
        }
    }

    pub(super) fn to_exec_params(
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

impl ToolHandler for ShellHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("shell")
    }

    fn spec(&self) -> Option<ToolSpec> {
        self.options.map(create_shell_tool)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.options.is_some()
    }

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

        serde_json::from_str::<ShellToolCallParams>(arguments)
            .map(|params| !is_known_safe_command(&params.command))
            .unwrap_or(true)
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        shell_function_pre_tool_use_payload(invocation)
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        shell_function_post_tool_use_payload(invocation, result)
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "unsupported payload for shell handler".to_string(),
                ));
            }
        };

        let cwd = resolve_workdir_base_path(&arguments, &turn.cwd)?;
        let params: ShellToolCallParams = parse_arguments_with_base_path(&arguments, &cwd)?;
        let prefix_rule = params.prefix_rule.clone();
        let exec_params =
            ShellHandler::to_exec_params(&params, turn.as_ref(), session.conversation_id);
        run_exec_like(RunExecLikeArgs {
            tool_name: "shell".to_string(),
            exec_params,
            hook_command: codex_shell_command::parse_command::shlex_join(&params.command),
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
}
