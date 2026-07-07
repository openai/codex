use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolExposure;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::items::DynamicToolCallItem;
use codex_protocol::items::DynamicToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::FunctionCallOutputContentItem;
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
        let response = request_dynamic_tool(
            &session,
            turn.as_ref(),
            call_id,
            self.tool_name.clone(),
            args,
        )
        .await
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "dynamic tool call was cancelled before receiving a response".to_string(),
            )
        })?;

        let DynamicToolResponse {
            content_items,
            success,
        } = response;
        let body = content_items
            .into_iter()
            .map(FunctionCallOutputContentItem::from)
            .collect::<Vec<_>>();
        Ok(boxed_tool_output(FunctionToolOutput::from_content(
            body,
            Some(success),
        )))
    }
}

impl CoreToolRuntime for DynamicToolHandler {}

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
) -> Option<DynamicToolResponse> {
    let namespace = tool_name.namespace;
    let tool = tool_name.name;
    let (tx_response, rx_response) = oneshot::channel();
    let event_id = call_id.clone();
    let prev_entry = {
        let mut active = session.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.insert_pending_dynamic_tool(call_id.clone(), tx_response)
            }
            None => None,
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
    let response = rx_response.await.ok();

    let item = match &response {
        Some(response) => DynamicToolCallItem {
            id: call_id,
            namespace,
            tool,
            arguments,
            status: if response.success {
                DynamicToolCallStatus::Completed
            } else {
                DynamicToolCallStatus::Failed
            },
            content_items: Some(response.content_items.clone()),
            success: Some(response.success),
            error: None,
            duration: Some(started_at.elapsed()),
        },
        None => DynamicToolCallItem {
            id: call_id,
            namespace,
            tool,
            arguments,
            status: DynamicToolCallStatus::Failed,
            content_items: Some(Vec::new()),
            success: Some(false),
            error: Some("dynamic tool call was cancelled before receiving a response".to_string()),
            duration: Some(started_at.elapsed()),
        },
    };
    session
        .emit_turn_item_completed(turn_context, TurnItem::DynamicToolCall(item))
        .await;

    response
}
