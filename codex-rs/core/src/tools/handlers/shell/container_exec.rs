use codex_protocol::models::ShellToolCallParams;
use codex_tools::ToolName;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::handlers::resolve_workdir_base_path;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;
use crate::tools::runtimes::shell::ShellRuntimeBackend;

use super::RunExecLikeArgs;
use super::rewrite_shell_function_updated_hook_input;
use super::run_exec_like;
use super::shell_function_post_tool_use_payload;
use super::shell_function_pre_tool_use_payload;
use super::shell_handler::ShellHandler;

pub struct ContainerExecHandler;

impl ToolExecutor<ToolInvocation> for ContainerExecHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("container.exec")
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
                    "unsupported payload for container.exec handler".to_string(),
                ));
            }
        };

        let cwd = resolve_workdir_base_path(&arguments, &turn.cwd)?;
        let params: ShellToolCallParams = parse_arguments_with_base_path(&arguments, &cwd)?;
        let prefix_rule = params.prefix_rule.clone();
        let exec_params =
            ShellHandler::to_exec_params(&params, turn.as_ref(), session.conversation_id);
        run_exec_like(RunExecLikeArgs {
            tool_name: ToolName::plain("container.exec"),
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

impl ToolHandler for ContainerExecHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        shell_function_pre_tool_use_payload(invocation)
    }

    fn with_updated_hook_input(
        &self,
        invocation: ToolInvocation,
        updated_input: serde_json::Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        rewrite_shell_function_updated_hook_input(invocation, updated_input, "container.exec")
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        shell_function_post_tool_use_payload(invocation, result)
    }
}
