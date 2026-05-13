use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::ExtensionToolFuture;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::DEFAULT_SEARCH_MAX_RESULTS;
use crate::MAX_SEARCH_RESULTS;
use crate::SEARCH_TOOL_NAME;
use crate::backend::MemoriesBackend;
use crate::backend::SearchMatchMode;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;

use super::backend_error_to_function_call;
use super::clamp_max_results;
use super::function_tool;
use super::parse_args;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    #[schemars(length(min = 1))]
    queries: Vec<String>,
    match_mode: Option<SearchMatchMode>,
    path: Option<String>,
    cursor: Option<String>,
    #[schemars(range(min = 0))]
    context_lines: Option<usize>,
    case_sensitive: Option<bool>,
    normalized: Option<bool>,
    #[schemars(range(min = 1))]
    max_results: Option<usize>,
}

#[derive(Clone)]
pub(super) struct SearchTool<B> {
    pub(super) backend: B,
}

impl<B> ExtensionToolExecutor for SearchTool<B>
where
    B: MemoriesBackend,
{
    fn tool_name(&self) -> ToolName {
        ToolName::plain(SEARCH_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(function_tool::<SearchArgs, SearchMemoriesResponse>(
            SEARCH_TOOL_NAME,
            "Search Codex memory files for substring matches, optionally normalizing separators or requiring all query substrings on the same line or within a line window.",
        ))
    }

    fn handle(&self, call: ToolCall) -> ExtensionToolFuture<'_> {
        let backend = self.backend.clone();
        Box::pin(async move {
            let args: SearchArgs = parse_args(&call)?;
            let response = backend
                .search(args.into_request())
                .await
                .map_err(backend_error_to_function_call)?;
            Ok(JsonToolOutput::new(json!(response)))
        })
    }
}

impl SearchArgs {
    fn into_request(self) -> SearchMemoriesRequest {
        SearchMemoriesRequest {
            queries: self.queries,
            match_mode: self.match_mode.unwrap_or(SearchMatchMode::Any),
            path: self.path,
            cursor: self.cursor,
            context_lines: self.context_lines.unwrap_or(0),
            case_sensitive: self.case_sensitive.unwrap_or(true),
            normalized: self.normalized.unwrap_or(false),
            max_results: clamp_max_results(
                self.max_results,
                DEFAULT_SEARCH_MAX_RESULTS,
                MAX_SEARCH_RESULTS,
            ),
        }
    }
}
