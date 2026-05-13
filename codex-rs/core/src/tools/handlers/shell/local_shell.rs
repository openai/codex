use codex_tools::ToolName;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::updated_hook_command;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;
use crate::tools::runtimes::shell::ShellRuntimeBackend;
use codex_tools::ToolSpec;

use super::super::shell_spec::create_local_shell_tool;
use super::RunExecLikeArgs;
use super::local_shell_payload_command;
use super::run_exec_like;
use super::shell_handler::ShellHandler;

#[derive(Default)]
pub struct LocalShellHandler {
    include_spec: bool,
}

impl LocalShellHandler {
    pub(crate) fn new() -> Self {
        Self { include_spec: true }
    }
}

impl ToolExecutor<ToolInvocation> for LocalShellHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("local_shell")
    }

    fn spec(&self) -> Option<ToolSpec> {
        self.include_spec.then(create_local_shell_tool)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.include_spec
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

        let ToolPayload::LocalShell { params } = payload else {
            return Err(FunctionCallError::RespondToModel(
                "unsupported payload for local_shell handler".to_string(),
            ));
        };

        let exec_params =
            ShellHandler::to_exec_params(&params, turn.as_ref(), session.conversation_id);
        run_exec_like(RunExecLikeArgs {
            tool_name: ToolName::plain("local_shell"),
            exec_params,
            hook_command: codex_shell_command::parse_command::shlex_join(&params.command),
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
}

impl ToolHandler for LocalShellHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::LocalShell { .. })
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        local_shell_payload_command(&invocation.payload).map(|command| PreToolUsePayload {
            tool_name: HookToolName::bash(),
            tool_input: serde_json::json!({ "command": command }),
        })
    }

    fn with_updated_hook_input(
        &self,
        mut invocation: ToolInvocation,
        updated_input: serde_json::Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        let command = updated_hook_command(&updated_input)?;
        invocation.payload = match invocation.payload {
            ToolPayload::LocalShell { mut params } => {
                params.command = shlex::split(command).ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "hook returned shell input with an invalid command string".to_string(),
                    )
                })?;
                ToolPayload::LocalShell { params }
            }
            payload => payload,
        };
        Ok(invocation)
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        let tool_response =
            result.post_tool_use_response(&invocation.call_id, &invocation.payload)?;
        let command = local_shell_payload_command(&invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: HookToolName::bash(),
            tool_use_id: invocation.call_id.clone(),
            tool_input: serde_json::json!({ "command": command }),
            tool_response,
        })
    }
}
