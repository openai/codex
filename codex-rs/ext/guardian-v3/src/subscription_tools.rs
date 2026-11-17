//! Self-only channel subscriptions. Captured tools constrain membership and replies to their view.

use codex_message_board::AgentMessageBoard;
use codex_message_board::BoardReadView;
use codex_message_board::MAX_TOOL_ARGUMENT_KIB;
use codex_message_board::MAX_TOOL_RESPONSE_BYTES;
use codex_message_board::MessageBoardTool;
use codex_message_board::SubscriptionChange;
use codex_message_board::SubscriptionRequest;
use codex_message_board::SubscriptionState;
use codex_message_board::SubscriptionTarget;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
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
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    channel_name: String,
}

struct SubscriptionTool {
    board: Arc<dyn AgentMessageBoard>,
    caller: ThreadId,
    path: AgentPath,
    view: Option<Arc<dyn BoardReadView>>,
    change: SubscriptionChange,
    name: ToolName,
    spec: ToolSpec,
}

pub(crate) fn tools(
    board: &Arc<dyn AgentMessageBoard>,
    caller: ThreadId,
    path: &AgentPath,
    view: Option<&Arc<dyn BoardReadView>>,
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
            SubscriptionChange::Subscribe => "Subscribe yourself to new evidence in this channel. Search or read its earlier posts separately.",
            SubscriptionChange::Unsubscribe => "Stop receiving new evidence from this channel. Evidence already admitted to the reviewer conversation is retained.",
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
            board: Arc::clone(board),
            caller,
            path: path.clone(),
            view: view.cloned(),
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
            let captured = if let Some(view) = &self.view {
                let channel = view
                    .channels()
                    .iter()
                    .find(|channel| channel.channel_name == arguments.channel_name)
                    .ok_or_else(|| {
                        FunctionCallError::RespondToModel(
                            "channel is outside the captured catalog".into(),
                        )
                    })?;
                Some(channel.last_message_id)
            } else {
                None
            };
            // For live subscriptions the last post ID is known only after the write.
            // Budget for a full UUID to avoid accepting a mutation we cannot acknowledge.
            let mut response = SubscriptionState {
                channel_name: arguments.channel_name.clone(),
                thread_id: None,
                target_agent: self.path.clone(),
                enabled: self.change == SubscriptionChange::Subscribe,
                last_message_id: captured.unwrap_or(Some(Uuid::nil())),
            };
            if serde_json::to_vec(&response)
                .map_err(|error| FunctionCallError::Fatal(error.to_string()))?
                .len()
                > call.response_byte_budget(MAX_TOOL_RESPONSE_BYTES)
            {
                return Err(FunctionCallError::RespondToModel(
                    "subscription response exceeds the tool-output budget".into(),
                ));
            }
            let state = self
                .board
                .set_subscription(
                    self.caller,
                    SubscriptionRequest {
                        target: SubscriptionTarget::Channel(arguments.channel_name),
                        target_agent: None,
                        change: self.change,
                    },
                )
                .await
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
            response.last_message_id = captured.unwrap_or(state.last_message_id);
            let response = serde_json::to_value(response)
                .map_err(|error| FunctionCallError::Fatal(error.to_string()))?;
            Ok(
                Box::new(JsonToolOutput::new(response).with_external_context())
                    as Box<dyn codex_tools::ToolOutput>,
            )
        })
    }
}
