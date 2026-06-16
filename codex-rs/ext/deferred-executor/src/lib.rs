use std::collections::BTreeMap;
use std::sync::Arc;

use codex_core::EnvironmentWaiter;
use codex_core::config::Config;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_tools::JsonSchema;
use codex_tools::WAIT_FOR_ENVIRONMENT_TOOL_NAME;
use serde_json::json;

struct DeferredExecutorExtension;

impl ToolContributor for DeferredExecutorExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(waiter) = thread_store.get::<EnvironmentWaiter>() else {
            return Vec::new();
        };
        vec![Arc::new(WaitForEnvironmentTool(waiter))]
    }
}

struct WaitForEnvironmentTool(Arc<EnvironmentWaiter>);

impl ToolExecutor<ToolCall> for WaitForEnvironmentTool {
    fn tool_name(&self) -> ToolName {
        WAIT_FOR_ENVIRONMENT_TOOL_NAME.into()
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: WAIT_FOR_ENVIRONMENT_TOOL_NAME.to_string(),
            description: "Wait until the selected execution environment is ready. Use this before work that needs its shell or files."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
            output_schema: None,
        })
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        Box::pin(async move {
            let _ = call.function_arguments()?;
            self.0.wait_until_ready().await;
            Ok(Box::new(JsonToolOutput::new(json!({ "ready": true }))) as Box<dyn ToolOutput>)
        })
    }
}

pub fn install(registry: &mut ExtensionRegistryBuilder<Config>) {
    registry.tool_contributor(Arc::new(DeferredExecutorExtension));
}
