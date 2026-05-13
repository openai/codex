use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::ExtensionToolFuture;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::backend::DEFAULT_SEARCH_MAX_RESULTS;
use crate::backend::MAX_SEARCH_RESULTS;
use crate::backend::MemoriesBackend;
use crate::backend::SearchMatchMode;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;
use crate::local::LocalMemoriesBackend;

use super::SEARCH_TOOL_NAME;
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
pub(super) struct SearchTool {
    pub(super) backend: LocalMemoriesBackend,
}

impl ExtensionToolExecutor for SearchTool {
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn search_args_accept_multiple_queries() {
        let args: SearchArgs = serde_json::from_value(json!({
            "queries": ["alpha", "needle"],
            "case_sensitive": false
        }))
        .expect("multi-query args should parse");

        let request = args.into_request();

        assert_eq!(
            request,
            SearchMemoriesRequest {
                queries: vec!["alpha".to_string(), "needle".to_string()],
                match_mode: SearchMatchMode::Any,
                path: None,
                cursor: None,
                context_lines: 0,
                case_sensitive: false,
                normalized: false,
                max_results: DEFAULT_SEARCH_MAX_RESULTS,
            }
        );
    }

    #[test]
    fn search_args_accept_windowed_all_match_mode() {
        let args: SearchArgs = serde_json::from_value(json!({
            "queries": ["alpha", "needle"],
            "match_mode": {
                "type": "all_within_lines",
                "line_count": 3
            }
        }))
        .expect("windowed all args should parse");

        let request = args.into_request();

        assert_eq!(
            request,
            SearchMemoriesRequest {
                queries: vec!["alpha".to_string(), "needle".to_string()],
                match_mode: SearchMatchMode::AllWithinLines { line_count: 3 },
                path: None,
                cursor: None,
                context_lines: 0,
                case_sensitive: true,
                normalized: false,
                max_results: DEFAULT_SEARCH_MAX_RESULTS,
            }
        );
    }

    #[test]
    fn search_args_reject_legacy_single_query() {
        let err = serde_json::from_value::<SearchArgs>(json!({
            "query": "needle",
        }))
        .expect_err("legacy query field should be rejected");

        assert!(err.to_string().contains("unknown field"));
        assert!(err.to_string().contains("query"));
    }
}
