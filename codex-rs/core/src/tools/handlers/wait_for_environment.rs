use codex_tools::JsonSchema;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

const WAIT_FOR_ENVIRONMENT_TOOL_NAME: &str = "wait_for_environment";
const DEFAULT_TOOL_DESCRIPTION: &str = "Wait for a selected execution environment marked as `starting` to become available. Use this when the current task needs that environment's files, commands, or installed capabilities. Do not wait if the task can be completed using tools already available, such as connectors. Waiting may take several minutes and blocks other tool calls. If startup fails, continue without that environment.";
const DEFAULT_ENVIRONMENT_ID_DESCRIPTION: &str =
    "The exact environment ID marked as `starting` in `<environment_context>`.";
const MAX_COMBINED_DESCRIPTION_BYTES: usize = 1_024;
const MAX_SERIALIZED_TOOL_SPEC_BYTES: usize = 1_000;

/// Model-visible descriptions supplied by a host that supports deferred environments.
///
/// The two tool-schema descriptions must not exceed 1,024 UTF-8 bytes in total, and Core
/// also limits the complete serialized tool specification to 1,000 bytes. Oversized
/// descriptions fall back to Core defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitForEnvironmentToolConfig {
    /// Explains when and why the model should call `wait_for_environment`.
    pub tool_description: String,
    /// Explains how the model should select the `environment_id` argument.
    pub environment_id_description: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitForEnvironmentArgs {
    environment_id: String,
}

pub(crate) struct WaitForEnvironmentHandler {
    tool_description: String,
    environment_id_description: String,
}

impl WaitForEnvironmentHandler {
    pub(crate) fn new(config: &WaitForEnvironmentToolConfig) -> Self {
        let combined_description_bytes = config
            .tool_description
            .len()
            .saturating_add(config.environment_id_description.len());
        if combined_description_bytes <= MAX_COMBINED_DESCRIPTION_BYTES {
            let handler = Self {
                tool_description: config.tool_description.clone(),
                environment_id_description: config.environment_id_description.clone(),
            };
            if serde_json::to_vec(&handler.spec())
                .is_ok_and(|serialized| serialized.len() <= MAX_SERIALIZED_TOOL_SPEC_BYTES)
            {
                return handler;
            }
        }

        tracing::warn!(
            "oversized wait_for_environment tool configuration; falling back to Core defaults"
        );
        Self::default()
    }
}

impl Default for WaitForEnvironmentHandler {
    fn default() -> Self {
        Self {
            tool_description: DEFAULT_TOOL_DESCRIPTION.to_string(),
            environment_id_description: DEFAULT_ENVIRONMENT_ID_DESCRIPTION.to_string(),
        }
    }
}

impl ToolExecutor<ToolInvocation> for WaitForEnvironmentHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(WAIT_FOR_ENVIRONMENT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: WAIT_FOR_ENVIRONMENT_TOOL_NAME.to_string(),
            description: self.tool_description.clone(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "environment_id".to_string(),
                    JsonSchema::string(Some(self.environment_id_description.clone())),
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
