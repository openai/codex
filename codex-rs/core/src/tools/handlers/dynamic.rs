use crate::function_tool::FunctionCallError;
use crate::hook_environment_executor::with_tool_environment_hooks;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolExposure;
use crate::tools::registry::function_hook_tool_name;
use crate::tools::registry::function_post_tool_use_payload;
use codex_async_utils::OrCancelExt;
use codex_code_mode::CodeModeToolResult;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolCallResult;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::items::DynamicToolCallItem;
use codex_protocol::items::DynamicToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_tools::default_namespace_description;
use codex_tools::dynamic_tool_to_responses_api_tool;
use serde_json::Value;
use std::time::Instant;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::warn;

pub struct DynamicToolHandler {
    tool_name: ToolName,
    spec: ToolSpec,
    exposure: ToolExposure,
}

impl DynamicToolHandler {
    pub fn new(tool: &DynamicToolFunctionSpec) -> Option<Self> {
        Self::from_parts(tool, /*namespace*/ None)
    }

    pub fn new_in_namespace(
        namespace: &DynamicToolNamespaceSpec,
        tool: &DynamicToolFunctionSpec,
    ) -> Option<Self> {
        Self::from_parts(tool, Some(namespace))
    }

    fn from_parts(
        tool: &DynamicToolFunctionSpec,
        namespace: Option<&DynamicToolNamespaceSpec>,
    ) -> Option<Self> {
        let tool_name = ToolName::new(
            namespace.map(|namespace| namespace.name.clone()),
            tool.name.clone(),
        );
        let mut output_tool = dynamic_tool_to_responses_api_tool(tool).ok()?;
        // Exposure controls deferral; tool search restores this marker for deferred results.
        output_tool.defer_loading = None;
        let spec = match namespace {
            Some(namespace) => ToolSpec::Namespace(ResponsesApiNamespace {
                name: namespace.name.clone(),
                description: if namespace.description.trim().is_empty() {
                    default_namespace_description(&namespace.name)
                } else {
                    namespace.description.clone()
                },
                tools: vec![ResponsesApiNamespaceTool::Function(output_tool)],
            }),
            None => ToolSpec::Function(output_tool),
        };
        Some(Self {
            tool_name,
            spec,
            exposure: if tool.defer_loading {
                ToolExposure::Deferred
            } else {
                ToolExposure::Direct
            },
        })
    }
}

impl ToolExecutor<ToolInvocation> for DynamicToolHandler {
    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn tool_name(&self) -> ToolName {
        self.tool_name.clone()
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn exposure(&self) -> ToolExposure {
        self.exposure
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        ToolSearchInfo::from_tool_spec(
            self.spec(),
            Some(ToolSearchSourceInfo {
                name: "Dynamic tools".to_string(),
                description: Some("Tools provided by the current Codex thread.".to_string()),
            }),
        )
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl DynamicToolHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            cancellation_token,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "dynamic tool handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: Value = parse_arguments(&arguments)?;
        let output = request_dynamic_tool(
            &session,
            turn.as_ref(),
            call_id.into(),
            self.tool_name.clone(),
            args,
            cancellation_token,
        )
        .await
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "dynamic tool call was cancelled before receiving a response".to_string(),
            )
        })?;

        Ok(boxed_tool_output(output))
    }
}

enum DynamicToolOutput {
    ContentItems(FunctionToolOutput),
    /// The MCP-shaped value shared by optional observers and moved into Code Mode.
    StructuredContent(Value),
}

impl DynamicToolOutput {
    fn from_response(
        response: DynamicToolCallResult,
    ) -> (Self, Vec<DynamicToolCallOutputContentItem>) {
        match response {
            DynamicToolCallResult::ContentItems(response) => (
                Self::ContentItems(FunctionToolOutput::from_content(
                    response
                        .content_items
                        .iter()
                        .cloned()
                        .map(Into::into)
                        .collect(),
                    Some(response.success),
                )),
                response.content_items,
            ),
            DynamicToolCallResult::StructuredContent {
                structured_content,
                success,
            } => {
                let text = structured_content.to_string();
                Self::native_result(Some(structured_content), text, success)
            }
            DynamicToolCallResult::Error { error_message } => {
                Self::native_result(
                    /*structured_content*/ None,
                    error_message,
                    /*success*/ false,
                )
            }
        }
    }

    // Native replies always have exactly one local text block, including errors.
    fn native_text(result: &Value) -> String {
        result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    fn native_result(
        structured_content: Option<Value>,
        text: String,
        success: bool,
    ) -> (Self, Vec<DynamicToolCallOutputContentItem>) {
        let mut result = serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "isError": !success,
        });
        if let Some(value) = structured_content {
            result["structuredContent"] = value;
        }
        (
            Self::StructuredContent(result),
            vec![DynamicToolCallOutputContentItem::InputText { text }],
        )
    }
}

impl ToolOutput for DynamicToolOutput {
    fn log_output(&self) -> String {
        match self {
            Self::ContentItems(output) => output.log_output(),
            Self::StructuredContent(value) => Self::native_text(value),
        }
    }

    fn success_for_logging(&self) -> bool {
        match self {
            Self::ContentItems(output) => output.success_for_logging(),
            Self::StructuredContent(value) => value["isError"] == false,
        }
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        match self {
            Self::ContentItems(output) => output.to_response_item(call_id, payload),
            Self::StructuredContent(value) => ResponseInputItem::FunctionCallOutput {
                call_id: call_id.to_owned(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(Self::native_text(value)),
                    success: Some(self.success_for_logging()),
                },
            },
        }
    }

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<Value> {
        match self {
            Self::ContentItems(_) => None,
            Self::StructuredContent(value) => Some(value.clone()),
        }
    }

    fn code_mode_result(&self, payload: &ToolPayload) -> CodeModeToolResult {
        match self {
            Self::ContentItems(output) => output.code_mode_result(payload),
            Self::StructuredContent(value) => CodeModeToolResult::McpCallToolResult(value.clone()),
        }
    }

    fn into_code_mode_result(self: Box<Self>, payload: &ToolPayload) -> CodeModeToolResult {
        match *self {
            Self::ContentItems(output) => output.code_mode_result(payload),
            Self::StructuredContent(value) => CodeModeToolResult::McpCallToolResult(value),
        }
    }
}

impl CoreToolRuntime for DynamicToolHandler {
    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        if !result.success_for_logging() {
            return None;
        }
        let tool_name = function_hook_tool_name(invocation);
        let hooks = with_tool_environment_hooks(
            &invocation.session,
            &invocation.turn,
            &invocation.step_context.environments,
            &invocation.environments.hook_ids(),
        );
        if !hooks.has_post_tool_use_match(tool_name.name(), tool_name.matcher_aliases()) {
            return None;
        }
        function_post_tool_use_payload(invocation, result, tool_name)
    }

    fn finishes_on_cancellation(&self) -> bool {
        true
    }

    fn is_third_party_tool(&self) -> bool {
        true
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "active turn checks and dynamic tool response registration must remain atomic"
)]
async fn request_dynamic_tool(
    session: &Session,
    turn_context: &TurnContext,
    call_id: String,
    tool_name: ToolName,
    arguments: Value,
    cancellation_token: CancellationToken,
) -> Option<DynamicToolOutput> {
    if cancellation_token.is_cancelled() {
        return None;
    }
    let namespace = tool_name.namespace;
    let tool = tool_name.name;
    let (tx_response, rx_response) = oneshot::channel();
    let event_id = call_id.clone();
    let prev_entry = {
        let mut active = session.active_turn.lock().await;
        if cancellation_token.is_cancelled() {
            return None;
        }
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.insert_pending_dynamic_tool(call_id.clone(), tx_response)
            }
            None => return None,
        }
    };
    if prev_entry.is_some() {
        warn!("Overwriting existing pending dynamic tool call for call_id: {event_id}");
    }

    let started_at = Instant::now();
    session
        .emit_turn_item_started(
            turn_context,
            &TurnItem::DynamicToolCall(DynamicToolCallItem {
                id: call_id.clone(),
                namespace: namespace.clone(),
                tool: tool.clone(),
                arguments: arguments.clone(),
                status: DynamicToolCallStatus::InProgress,
                content_items: None,
                success: None,
                error: None,
                duration: None,
            }),
        )
        .await;
    let response = rx_response
        .or_cancel(&cancellation_token)
        .await
        .ok()
        .and_then(Result::ok);

    let (output, item) = match response {
        Some(response) => {
            let success = response.success();
            let (output, content_items) = DynamicToolOutput::from_response(response);
            let item = DynamicToolCallItem {
                id: call_id,
                namespace,
                tool,
                arguments,
                status: if success {
                    DynamicToolCallStatus::Completed
                } else {
                    DynamicToolCallStatus::Failed
                },
                content_items: Some(content_items),
                success: Some(success),
                error: None,
                duration: Some(started_at.elapsed()),
            };
            (Some(output), item)
        }
        None => (
            None,
            DynamicToolCallItem {
                id: call_id,
                namespace,
                tool,
                arguments,
                status: DynamicToolCallStatus::Failed,
                content_items: Some(Vec::new()),
                success: Some(false),
                error: Some(
                    "dynamic tool call was cancelled before receiving a response".to_string(),
                ),
                duration: Some(started_at.elapsed()),
            },
        ),
    };
    session
        .emit_turn_item_completed(turn_context, TurnItem::DynamicToolCall(item))
        .await;

    output
}

#[cfg(test)]
#[path = "dynamic_tests.rs"]
mod tests;
