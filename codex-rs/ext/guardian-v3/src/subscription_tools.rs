//! Reviewer tools for listing and changing the caller's live channel subscriptions.

use crate::inbox::ActorInbox;
use codex_message_board::MAX_TOOL_ARGUMENT_KIB;
use codex_message_board::MAX_TOOL_RESPONSE_BYTES;
use codex_message_board::MessageBoardTool;
use codex_message_board::Page;
use codex_message_board::PageRequest;
use codex_message_board::SubscriptionChange;
use codex_protocol::AgentPath;
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
use serde_json::Value;
use serde_json::json;
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeArguments {
    channel_name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArguments {
    limit: Option<NonZeroU32>,
    cursor: Option<String>,
}

#[derive(Clone, Copy)]
enum Operation {
    List,
    Change(SubscriptionChange),
}

const TOOLS: [(&str, &str, Operation); 3] = [
    (
        "subscribed_channels",
        "List the names of channels you are currently subscribed to.",
        Operation::List,
    ),
    (
        "subscribe",
        "Subscribe yourself to new evidence by exact channel name, even before the channel exists. Search or read earlier posts separately.",
        Operation::Change(SubscriptionChange::Subscribe),
    ),
    (
        "unsubscribe",
        "Stop receiving new evidence from this channel. Evidence already admitted to the reviewer conversation is retained.",
        Operation::Change(SubscriptionChange::Unsubscribe),
    ),
];

pub(super) fn names() -> impl Iterator<Item = &'static str> {
    TOOLS.into_iter().map(|(name, _, _)| name)
}

struct SubscriptionTool {
    inbox: Arc<ActorInbox>,
    path: AgentPath,
    operation: Operation,
    name: ToolName,
    spec: ToolSpec,
}

pub(super) fn tools(
    inbox: &Arc<ActorInbox>,
    path: &AgentPath,
    namespace: &str,
    namespace_description: &str,
) -> impl Iterator<Item = Arc<dyn ToolExecutor>> {
    TOOLS
        .into_iter()
        .map(move |(name, description, operation)| {
            // Use the board's argument schemas for pagination and channel names.
            let (template, fields): (_, &[&str]) = match operation {
                Operation::List => (MessageBoardTool::GetChannels, &["limit", "cursor"]),
                Operation::Change(_) => (MessageBoardTool::Subscribe, &["channel_name"]),
            };
            let ToolSpec::Function(mut function) = template.spec() else {
                unreachable!("board tool specs are functions")
            };
            function
                .parameters
                .properties
                .get_or_insert_default()
                .retain(|name, _| fields.contains(&name.as_str()));
            if matches!(operation, Operation::Change(_)) {
                function.parameters.required = Some(vec!["channel_name".into()].into_boxed_slice());
            }
            function.name = name.into();
            function.description = description.into();
            let name = ToolName::namespaced(namespace, name);
            let spec = ToolSpec::Namespace(ResponsesApiNamespace {
                name: namespace.into(),
                description: namespace_description.into(),
                tools: vec![ResponsesApiNamespaceTool::Function(function)],
            });
            Arc::new(SubscriptionTool {
                inbox: Arc::clone(inbox),
                path: path.clone(),
                operation,
                name,
                spec,
            }) as Arc<dyn ToolExecutor>
        })
}

impl ToolExecutor for SubscriptionTool {
    fn tool_name(&self) -> ToolName {
        self.name.clone()
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        matches!(self.operation, Operation::List)
    }

    fn handle<'a>(&'a self, call: ToolCall<'a>) -> ToolExecutorFuture<'a> {
        Box::pin(async move {
            let raw = call.function_arguments()?;
            if raw.len() > MAX_TOOL_ARGUMENT_KIB * 1024 {
                return Err(FunctionCallError::RespondToModel(format!(
                    "subscription arguments exceed {MAX_TOOL_ARGUMENT_KIB} KiB"
                )));
            }
            let budget = call.response_byte_budget(MAX_TOOL_RESPONSE_BYTES);
            let response = match self.operation {
                Operation::List => serde_json::to_value(self.list(raw, budget)?)
                    .map_err(|error| FunctionCallError::Fatal(error.to_string()))?,
                Operation::Change(change) => self.change(raw, change, budget).await?,
            };
            Ok(
                Box::new(JsonToolOutput::new(response).with_external_context())
                    as Box<dyn codex_tools::ToolOutput>,
            )
        })
    }
}

fn list_error(error: impl std::fmt::Display) -> FunctionCallError {
    FunctionCallError::RespondToModel(error.to_string().chars().take(512).collect())
}

impl SubscriptionTool {
    fn list(&self, raw: &str, budget: usize) -> Result<Page<String>, FunctionCallError> {
        let ListArguments { limit, cursor } = serde_json::from_str(raw).map_err(list_error)?;
        let mut request = PageRequest {
            limit: limit.unwrap_or(PageRequest::default().limit),
            cursor,
        };
        loop {
            let page = self
                .inbox
                .subscribed_channels(&request)
                .map_err(list_error)?;
            if serde_json::to_vec(&page).map_err(list_error)?.len() <= budget {
                return Ok(page);
            }
            // Re-page from the same cursor so next_cursor follows the last returned name.
            request.limit = NonZeroU32::new(page.results.len() as u32 / 2).ok_or_else(|| {
                list_error("The output budget is too small for this result's metadata.")
            })?;
        }
    }

    async fn change(
        &self,
        raw: &str,
        change: SubscriptionChange,
        budget: usize,
    ) -> Result<Value, FunctionCallError> {
        let ChangeArguments { channel_name } = serde_json::from_str(raw)
            .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
        let response = json!({
            "channel_name": channel_name,
            "thread_id": null,
            "target_agent": self.path,
            "enabled": change == SubscriptionChange::Subscribe,
        });
        if response.to_string().len() > budget {
            return Err(FunctionCallError::RespondToModel(
                "subscription response exceeds the tool-output budget".into(),
            ));
        }
        self.inbox
            .set_subscription(channel_name, change)
            .await
            .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
        Ok(response)
    }
}
