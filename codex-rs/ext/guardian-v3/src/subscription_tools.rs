//! Self-only subscription tools over the captured reviewer catalog.
//! Shared schemas are narrowed to channel names; replies never reveal a later live post ID.

use crate::ReviewerSubscriptions;
use codex_message_board::MAX_TOOL_ARGUMENT_KIB;
use codex_message_board::MAX_TOOL_RESPONSE_BYTES;
use codex_message_board::MessageBoardTool;
use codex_message_board::SubscriptionChange;
use codex_message_board::SubscriptionRequest;
use codex_message_board::SubscriptionState;
use codex_message_board::SubscriptionTarget;
use codex_tools::FunctionCallError;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolExecutorFuture;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    channel_name: String,
}

struct SubscriptionTool {
    subscriptions: Arc<ReviewerSubscriptions>,
    change: SubscriptionChange,
    name: ToolName,
    spec: ToolSpec,
}

pub(crate) fn tools(
    subscriptions: &Arc<ReviewerSubscriptions>,
    namespace: Option<&str>,
    namespace_description: &str,
) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
    [
        (MessageBoardTool::Subscribe, SubscriptionChange::Subscribe),
        (MessageBoardTool::Unsubscribe, SubscriptionChange::Unsubscribe),
    ]
    .into_iter()
    .map(|(tool, change)| {
        let ToolSpec::Function(mut function) = tool.spec() else {
            unreachable!("shared subscription specs are functions")
        };
        function
            .parameters
            .properties
            .get_or_insert_default()
            .retain(|name, _| name == "channel_name");
        function.parameters.required = Some(vec!["channel_name".into()].into_boxed_slice());
        function.description = match change {
            SubscriptionChange::Subscribe => "Subscribe yourself to an evidence channel in the captured catalog. This changes future evidence selection, without reading posts or authorizing any action.",
            SubscriptionChange::Unsubscribe => "Unsubscribe yourself from an evidence channel in the captured catalog. Evidence already admitted to the reviewer conversation is retained.",
        }
        .into();
        let name = ToolName::new(namespace.map(str::to_owned), function.name.clone());
        let spec = match namespace {
            Some(namespace) => ToolSpec::Namespace(ResponsesApiNamespace {
                name: namespace.into(),
                description: namespace_description.into(),
                tools: vec![ResponsesApiNamespaceTool::Function(function)],
            }),
            None => ToolSpec::Function(function),
        };
        Arc::new(SubscriptionTool {
            subscriptions: Arc::clone(subscriptions),
            change,
            name,
            spec,
        }) as Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>
    })
    .collect()
}

impl<'call> ToolExecutor<ToolCall<'call>> for SubscriptionTool {
    fn tool_name(&self) -> ToolName {
        self.name.clone()
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let raw = call.function_arguments()?;
            if raw.len() > MAX_TOOL_ARGUMENT_KIB * 1024 {
                return Err(FunctionCallError::RespondToModel(format!(
                    "subscription arguments exceed {MAX_TOOL_ARGUMENT_KIB} KiB"
                )));
            }
            let arguments: Arguments = serde_json::from_str(raw)
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
            let subscriptions = &self.subscriptions;
            let channel = subscriptions
                .view
                .channels()
                .iter()
                .find(|channel| channel.channel_name == arguments.channel_name)
                .ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "channel is outside the captured catalog".into(),
                    )
                })?;
            let response = serde_json::to_value(SubscriptionState {
                channel_name: channel.channel_name.clone(),
                thread_id: None,
                target_agent: subscriptions.path.clone(),
                enabled: self.change == SubscriptionChange::Subscribe,
                last_message_id: channel.last_message_id,
            })
            .map_err(|error| FunctionCallError::Fatal(error.to_string()))?;
            if response.to_string().len() > call.response_byte_budget(MAX_TOOL_RESPONSE_BYTES) {
                return Err(FunctionCallError::RespondToModel(
                    "subscription response exceeds the tool-output budget".into(),
                ));
            }
            // The ordinary backend operation owns atomic membership updates. Its live
            // last_message_id is deliberately not returned through this captured view.
            subscriptions
                .board
                .set_subscription(
                    subscriptions.caller,
                    SubscriptionRequest {
                        target: SubscriptionTarget::Channel(arguments.channel_name),
                        target_agent: None,
                        change: self.change,
                    },
                )
                .await
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
            Ok(
                Box::new(JsonToolOutput::new(response).with_external_context())
                    as Box<dyn codex_tools::ToolOutput>,
            )
        })
    }
}
