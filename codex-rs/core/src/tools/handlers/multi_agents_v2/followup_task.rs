use super::message_tool::FollowupTaskArgs;
use super::message_tool::MessageDeliveryMode;
use super::message_tool::handle_message_string_tool;
use super::*;
use crate::tools::handlers::multi_agents_spec::create_followup_task_tool;
use codex_tools::ToolSpec;

pub(crate) struct Handler;

impl ToolExecutor<ToolInvocation> for Handler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("followup_task")
    }

    fn spec(&self) -> ToolSpec {
        create_followup_task_tool()
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutionFuture<'a> {
        Box::pin(async move {
            let _self = self;
            let arguments = function_arguments(invocation.payload.clone())?;
            let args: FollowupTaskArgs = parse_arguments(&arguments)?;
            handle_message_string_tool(
                invocation,
                MessageDeliveryMode::TriggerTurn,
                args.target,
                args.message,
            )
            .await
            .map(boxed_tool_output)
        })
    }
}

impl CoreToolRuntime for Handler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}
