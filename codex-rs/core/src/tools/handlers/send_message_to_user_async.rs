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

const TOOL_NAME: &str = "send_message_to_user_async";

pub struct SendMessageToUserAsyncHandler;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SendMessageToUserAsyncArgs {
    message: String,
}

impl ToolExecutor<ToolInvocation> for SendMessageToUserAsyncHandler {
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
            description: "Send a concise message that needs the user's attention during ongoing work. The tool returns immediately without ending the turn or waiting for a reply; any reply arrives asynchronously as a new user message. Use this tool to report a critical blocker or a finding that may change the task's direction, or to answer a user question or status request received while work is still in progress. Use this tool when a message needs the user's immediate attention; use commentary for routine progress and intermediate context. Use clear formatting, such as bolding questions, to make requests easy to notice and answer."
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

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
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
            let args: SendMessageToUserAsyncArgs = parse_arguments(&arguments)?;
            let message = args.message.trim();
            if message.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "message must not be empty".to_string(),
                ));
            }

            let item = TurnItem::AgentMessage(AgentMessageItem {
                moderation: None,
                id: call_id,
                content: vec![AgentMessageContent::Text {
                    text: message.to_string(),
                }],
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
                delivery: Some(AgentMessageDelivery::Async),
                questions: None,
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

impl CoreToolRuntime for SendMessageToUserAsyncHandler {
    fn is_builtin_control_tool(&self) -> bool {
        true
    }
}
