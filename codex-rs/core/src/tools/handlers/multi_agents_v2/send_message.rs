use super::message_tool::MessageDeliveryMode;
use super::message_tool::SendMessageArgs;
use super::message_tool::handle_message_string_tool;
use super::*;
use crate::tools::context::FunctionToolOutput;
use crate::tools::handlers::multi_agents_spec::create_send_message_tool;
use codex_tools::ToolSpec;

pub(crate) struct Handler;

impl ToolHandler for Handler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("send_message")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_send_message_tool())
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let arguments = function_arguments(invocation.payload.clone())?;
        let args: SendMessageArgs = parse_arguments(&arguments)?;
        handle_message_string_tool(
            invocation,
            MessageDeliveryMode::QueueOnly,
            args.target,
            args.message,
        )
        .await
    }
}
