use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::ExtensionToolFuture;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::backend::DEFAULT_LIST_MAX_RESULTS;
use crate::backend::ListMemoriesRequest;
use crate::backend::ListMemoriesResponse;
use crate::backend::MAX_LIST_RESULTS;
use crate::backend::MemoriesBackend;
use crate::local::LocalMemoriesBackend;

use super::LIST_TOOL_NAME;
use super::backend_error_to_function_call;
use super::clamp_max_results;
use super::function_tool;
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
pub(super) struct ListTool {
    pub(super) backend: LocalMemoriesBackend,
}

impl ExtensionToolExecutor for ListTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(LIST_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(function_tool::<ListArgs, ListMemoriesResponse>(
            LIST_TOOL_NAME,
            "List immediate files and directories under a path in the Codex memories store.",
        ))
    }

    fn handle(&self, call: ToolCall) -> ExtensionToolFuture<'_> {
        let backend = self.backend.clone();
        Box::pin(async move {
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
            Ok(JsonToolOutput::new(json!(response)))
        })
    }
}
