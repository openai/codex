use super::message_tool::AssignTaskArgs;
use super::message_tool::MessageDeliveryMode;
use super::message_tool::MessageToolResult;
use super::message_tool::handle_message_tool;
use super::*;

pub(crate) struct Handler;

#[async_trait]
impl ToolHandler for Handler {
    type Output = MessageToolResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let arguments = function_arguments(invocation.payload.clone())?;
        let args: AssignTaskArgs = parse_arguments(&arguments)?;
        handle_message_tool(
            invocation,
            MessageDeliveryMode::TriggerTurn,
            args.target,
            args.items,
            args.interrupt,
        )
        .await
    }
}
