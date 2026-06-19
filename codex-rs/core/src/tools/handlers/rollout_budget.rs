use crate::context::ContextualUserFragment;
use crate::context::RolloutBudgetContext;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

const NAMESPACE: &str = "rollout";
const TOOL_NAME: &str = "remaining_budget";

pub struct RolloutBudgetHandler;

impl ToolExecutor<ToolInvocation> for RolloutBudgetHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(NAMESPACE, TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: NAMESPACE.to_string(),
            description: "Tools for inspecting the current rollout state.".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: TOOL_NAME.to_string(),
                description:
                    "Return the weighted tokens remaining in the shared session token budget."
                        .to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(
                    BTreeMap::new(),
                    /*required*/ None,
                    /*additional_properties*/ Some(false.into()),
                ),
                output_schema: None,
            })],
        })
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            if !matches!(invocation.payload, ToolPayload::Function { .. }) {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{TOOL_NAME} handler received unsupported payload"
                )));
            }

            let Some(remaining_tokens) = invocation
                .session
                .services
                .agent_control
                .rollout_budget()
                .remaining_tokens()
            else {
                return Err(FunctionCallError::RespondToModel(
                    "rollout budget is not configured".to_string(),
                ));
            };
            let output = RolloutBudgetContext { remaining_tokens }.render();

            Ok(boxed_tool_output(FunctionToolOutput::from_text(
                output,
                /*success*/ Some(true),
            )))
        })
    }
}

impl CoreToolRuntime for RolloutBudgetHandler {}
