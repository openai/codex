use std::collections::BTreeMap;
use std::sync::Arc;

use codex_extension_api::EnvironmentStartupOutcome;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::StartingEnvironment;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributionInput;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use serde::Deserialize;
use serde_json::json;

pub const WAIT_FOR_ENVIRONMENT_TOOL_NAME: &str = "wait_for_environment";

struct DeferredExecutorExtension;

impl ToolContributor for DeferredExecutorExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        Vec::new()
    }

    fn tools_for_step(
        &self,
        input: ToolContributionInput<'_>,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        if input.starting_environments.is_empty() {
            return Vec::new();
        }

        vec![Arc::new(WaitForEnvironmentTool {
            environments: input.starting_environments.to_vec(),
        })]
    }
}

struct WaitForEnvironmentTool {
    environments: Vec<Arc<dyn StartingEnvironment>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitForEnvironmentArgs {
    environment_id: String,
}

impl ToolExecutor<ToolCall> for WaitForEnvironmentTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(WAIT_FOR_ENVIRONMENT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: WAIT_FOR_ENVIRONMENT_TOOL_NAME.to_string(),
            description: "Wait for a starting environment to become available before continuing."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "environment_id".to_string(),
                    JsonSchema::string(Some(
                        "The id of an environment currently marked as starting.".to_string(),
                    )),
                )]),
                /*required*/ Some(vec!["environment_id".to_string()]),
                Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(call))
    }
}

impl WaitForEnvironmentTool {
    async fn handle_call(&self, call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let args: WaitForEnvironmentArgs = serde_json::from_str(call.function_arguments()?)
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        let Some(environment) = self
            .environments
            .iter()
            .find(|environment| environment.environment_id() == args.environment_id)
        else {
            return Err(FunctionCallError::RespondToModel(format!(
                "environment `{}` is not starting",
                args.environment_id
            )));
        };

        let environment_id = environment.environment_id();
        let output = match environment.wait_until_ready().await {
            EnvironmentStartupOutcome::Ready => JsonToolOutput::new(json!({
                "environment_id": environment_id,
                "status": "ready",
            })),
            EnvironmentStartupOutcome::Failed => JsonToolOutput::with_success(
                json!({
                    "environment_id": environment_id,
                    "message": "The environment failed to start and is unavailable. Continue without it.",
                    "status": "failed",
                }),
                Some(false),
            ),
        };
        Ok(Box::new(output))
    }
}

/// Installs the tool that waits for environments exposed as starting by the host.
pub fn install<C: Sync>(registry: &mut ExtensionRegistryBuilder<C>) {
    registry.tool_contributor(Arc::new(DeferredExecutorExtension));
}
