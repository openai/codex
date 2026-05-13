use super::message_tool::FollowupTaskArgs;
use super::message_tool::MessageDeliveryMode;
use super::message_tool::handle_message_string_tool;
use super::*;
use crate::tools::context::FunctionToolOutput;
use crate::tools::handlers::multi_agents_spec::create_followup_task_tool;
use codex_tools::ToolSpec;

pub(crate) struct Handler;

impl ToolExecutor<ToolInvocation> for Handler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("followup_task")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_followup_task_tool())
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let arguments = function_arguments(invocation.payload.clone())?;
        let args: FollowupTaskArgs = parse_arguments(&arguments)?;
        handle_message_string_tool(
            invocation,
            MessageDeliveryMode::TriggerTurn,
            args.target,
            args.message,
        )
        .await
    }
}

impl ToolHandler for Handler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}
