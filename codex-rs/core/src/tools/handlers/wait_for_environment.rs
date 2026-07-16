use std::collections::BTreeMap;

use codex_tools::JsonSchema;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::json;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

const WAIT_FOR_ENVIRONMENT_TOOL_NAME: &str = "wait_for_environment";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitForEnvironmentArgs {
    environment_id: String,
}

pub(crate) struct WaitForEnvironmentHandler;

impl ToolExecutor<ToolInvocation> for WaitForEnvironmentHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(WAIT_FOR_ENVIRONMENT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: WAIT_FOR_ENVIRONMENT_TOOL_NAME.to_string(),
            description: "Wait for a selected execution environment marked as `starting` to become available. Use this when the current task needs that environment's files, commands, or installed capabilities. Do not wait if the task can be completed using tools already available, such as connectors. Waiting may take several minutes and blocks other tool calls. If startup fails, continue without that environment."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "environment_id".to_string(),
                    JsonSchema::string(Some(
                        "The exact environment ID marked as `starting` in `<environment_context>`."
                            .to_string(),
                    )),
                )]),
                /*required*/ Some(vec!["environment_id".to_string()]),
                /*additional_properties*/ Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            let ToolInvocation {
                payload,
                step_context,
                ..
            } = invocation;
            let arguments = match payload {
                ToolPayload::Function { arguments } => arguments,
                _ => {
                    return Err(FunctionCallError::Fatal(format!(
                        "{WAIT_FOR_ENVIRONMENT_TOOL_NAME} handler received unsupported payload"
                    )));
                }
            };
            let args: WaitForEnvironmentArgs = parse_arguments(&arguments)?;
            let environment_id = args.environment_id;
            let already_ready = step_context
                .environments
                .turn_environments()
                .any(|environment| environment.environment_id == environment_id);
            if !already_ready {
                let Some(environment) = step_context
                    .environments
                    .starting()
                    .find(|environment| environment.selection.environment_id == environment_id)
                    .cloned()
                else {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "environment `{environment_id}` is neither ready nor starting"
                    )));
                };

                environment.wait_until_ready().await.map_err(|_| {
                    FunctionCallError::RespondToModel(format!(
                        "Environment `{environment_id}` failed to start and is unavailable. Continue without it."
                    ))
                })?;
            }

            Ok(boxed_tool_output(JsonToolOutput::new(json!({
                "environment_id": environment_id,
                "status": "ready",
            }))))
        })
    }
}

impl CoreToolRuntime for WaitForEnvironmentHandler {}
