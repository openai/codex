use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageDelivery;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::MessagePhase;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use std::collections::BTreeMap;

const TOOL_NAME: &str = "send_user_message_async";

pub struct SendUserMessageAsyncHandler;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SendUserMessageAsyncArgs {
    message: String,
}

impl ToolExecutor<ToolInvocation> for SendUserMessageAsyncHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let properties = BTreeMap::from([(
            "message".to_string(),
            JsonSchema::string(Some(
                "The concise question or update to send to the user.".to_string(),
            )),
        )]);

        ToolSpec::Function(ResponsesApiTool {
            name: TOOL_NAME.to_string(),
            description: "Send a concise, user-visible acknowledgment, important update, or blocking question. Returns immediately; any reply arrives asynchronously as a new user message."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                properties,
                Some(vec!["message".to_string()]),
                /*additional_properties*/ Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            let ToolInvocation {
                session,
                turn,
                call_id,
                payload,
                ..
            } = invocation;
            let ToolPayload::Function { arguments } = payload else {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{TOOL_NAME} handler received unsupported payload"
                )));
            };
            let args: SendUserMessageAsyncArgs = parse_arguments(&arguments)?;
            let message = args.message.trim();
            if message.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "message must not be empty".to_string(),
                ));
            }

            let item = TurnItem::AgentMessage(AgentMessageItem {
                id: call_id,
                content: vec![AgentMessageContent::Text {
                    text: message.to_string(),
                }],
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
                delivery: Some(AgentMessageDelivery::Async),
            });
            session.emit_turn_item_started(turn.as_ref(), &item).await;
            session.emit_turn_item_completed(turn.as_ref(), item).await;

            Ok(boxed_tool_output(FunctionToolOutput::from_text(
                r#"{"accepted":true}"#.to_string(),
                /*success*/ Some(true),
            )))
        })
    }
}

impl CoreToolRuntime for SendUserMessageAsyncHandler {}
