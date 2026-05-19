use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::DEFAULT_LIST_MAX_RESULTS;
use crate::LIST_TOOL_NAME;
use crate::MAX_LIST_RESULTS;
use crate::backend::ListMemoriesRequest;
use crate::backend::ListMemoriesResponse;
use crate::backend::MemoriesBackend;

use super::backend_error_to_function_call;
use super::clamp_max_results;
use super::memory_function_tool;
use super::memory_tool_name;
use super::parse_args;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    path: Option<String>,
    cursor: Option<String>,
    #[schemars(range(min = 1))]
    max_results: Option<usize>,
}

#[derive(Clone)]
pub(super) struct ListTool<B> {
    pub(super) backend: B,
}

#[async_trait::async_trait]
impl<B> ToolExecutor<ToolCall> for ListTool<B>
where
    B: MemoriesBackend,
{
    fn tool_name(&self) -> ToolName {
        memory_tool_name(LIST_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(memory_function_tool::<ListArgs, ListMemoriesResponse>(
            LIST_TOOL_NAME,
            "List immediate files and directories under a path in the Codex memories store.",
        ))
    }

    async fn handle(
        &self,
        call: ToolCall,
    ) -> Result<Box<dyn codex_extension_api::ToolOutput>, codex_extension_api::FunctionCallError>
    {
        let backend = self.backend.clone();
        let args: ListArgs = parse_args(&call)?;
        let response = backend
            .list(ListMemoriesRequest {
                path: args.path,
                cursor: args.cursor,
                max_results: clamp_max_results(
                    args.max_results,
                    DEFAULT_LIST_MAX_RESULTS,
                    MAX_LIST_RESULTS,
                ),
            })
            .await
            .map_err(backend_error_to_function_call)?;
        Ok(Box::new(JsonToolOutput::new(json!(response))))
    }
}
