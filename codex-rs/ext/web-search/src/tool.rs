use codex_api::ReqwestTransport;
use codex_api::SearchClient;
use codex_api::SearchCommands;
use codex_api::SearchQuery;
use codex_api::SearchRequest;
use codex_api::SearchSettings;
use codex_core::web_search_action_detail;
use codex_extension_api::ExtensionTurnItem;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema_without_compaction;
use codex_login::default_client::build_reqwest_client;
use codex_model_provider::SharedModelProvider;
use codex_protocol::items::WebSearchItem;
use codex_protocol::items::WebSearchResult;
use codex_protocol::items::bounded_web_search_results;
use codex_protocol::models::WebSearchAction;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExposure;
use codex_tools::default_namespace_description;
use http::HeaderMap;
use std::collections::HashSet;
use url::Url;

use crate::history::recent_input;
use crate::output::SearchOutput;
use crate::schema::commands_schema;

pub(crate) const WEB_NAMESPACE: &str = "web";
pub(crate) const RUN_TOOL_NAME: &str = "run";
const WEB_RUN_DESCRIPTION: &str = include_str!("../web_run_description.md");

pub(crate) struct WebSearchTool {
    pub(crate) session_id: String,
    pub(crate) provider: SharedModelProvider,
    pub(crate) settings: SearchSettings,
}

impl ToolExecutor<ToolCall> for WebSearchTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(WEB_NAMESPACE, RUN_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        // parse schema without compaction that removes field metadata/descriptions to match hosted tool definition
        let parameters = match parse_tool_input_schema_without_compaction(&commands_schema()) {
            Ok(parameters) => parameters,
            Err(err) => panic!("search command schema should parse: {err}"),
        };

        ToolSpec::Namespace(ResponsesApiNamespace {
            name: WEB_NAMESPACE.to_string(),
            description: default_namespace_description(WEB_NAMESPACE),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: RUN_TOOL_NAME.to_string(),
                description: WEB_RUN_DESCRIPTION.to_string(),
                strict: false,
                parameters,
                output_schema: None,
                defer_loading: None,
            })],
        })
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(call))
    }
}

impl WebSearchTool {
    async fn handle_call(&self, call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let commands = parse_commands(&call)?;
        let command_action = command_action(&commands);
        let provider = self
            .provider
            .api_provider()
            .await
            .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;
        let auth = self
            .provider
            .api_auth()
            .await
            .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;
        let client = SearchClient::new(
            ReqwestTransport::new(build_reqwest_client()),
            provider,
            auth,
        );
        let request = SearchRequest {
            id: self.session_id.clone(),
            model: call.model.clone(),
            reasoning: None,
            input: recent_input(call.conversation_history.items()),
            commands: Some(commands),
            settings: Some(self.settings.clone()),
            max_output_tokens: Some(
                u64::try_from(call.truncation_policy.token_budget()).unwrap_or(u64::MAX),
            ),
        };
        call.turn_item_emitter
            .emit_started(web_search_item(
                &call.call_id,
                WebSearchAction::Other,
                /*results*/ None,
            ))
            .await;
        let response = client
            .search(&request, HeaderMap::new())
            .await
            .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;
        let results = matches!(&command_action, WebSearchAction::Search { .. })
            .then(|| search_results(&response.output))
            .filter(|results| !results.is_empty());
        call.turn_item_emitter
            .emit_completed(web_search_item(&call.call_id, command_action, results))
            .await;

        Ok(Box::new(SearchOutput::new(response.output)))
    }
}

fn parse_commands(call: &ToolCall) -> Result<SearchCommands, FunctionCallError> {
    let arguments = call.function_arguments()?;
    if arguments.trim().is_empty() {
        return Ok(SearchCommands::default());
    }

    serde_json::from_str(arguments)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

fn command_action(commands: &SearchCommands) -> WebSearchAction {
    commands
        .search_query
        .as_deref()
        .and_then(query_action)
        .or_else(|| commands.image_query.as_deref().and_then(query_action))
        .or_else(|| {
            commands
                .open
                .as_deref()
                .and_then(|operations| operations.first())
                .and_then(|operation| {
                    literal_url(&operation.ref_id)
                        .map(|url| WebSearchAction::OpenPage { url: Some(url) })
                })
        })
        .or_else(|| {
            commands
                .find
                .as_deref()
                .and_then(|operations| operations.first())
                .map(|operation| WebSearchAction::FindInPage {
                    url: literal_url(&operation.ref_id),
                    pattern: Some(operation.pattern.clone()),
                })
        })
        .unwrap_or(WebSearchAction::Other)
}

fn query_action(queries: &[SearchQuery]) -> Option<WebSearchAction> {
    match queries {
        [] => None,
        [query] => Some(WebSearchAction::Search {
            query: Some(query.q.clone()),
            queries: None,
        }),
        queries => Some(WebSearchAction::Search {
            query: None,
            queries: Some(queries.iter().map(|query| query.q.clone()).collect()),
        }),
    }
}

fn search_results(output: &str) -> Vec<WebSearchResult> {
    let mut urls = HashSet::new();
    let mut lines = output.lines().peekable();
    bounded_web_search_results(std::iter::from_fn(|| {
        loop {
            let (title, url) = loop {
                let line = lines.next()?;
                if let Some(header) = search_result_header(line) {
                    break header;
                }
            };
            let mut snippet = String::new();
            while lines
                .peek()
                .is_some_and(|line| search_result_header(line).is_none())
            {
                let line = lines.next()?.trim();
                if line.is_empty() {
                    continue;
                }
                if !snippet.is_empty() {
                    snippet.push('\n');
                }
                snippet.push_str(line);
            }
            let Ok(parsed) = Url::parse(url) else {
                continue;
            };
            if !matches!(parsed.scheme(), "http" | "https") || !urls.insert(url) {
                continue;
            }
            return Some(WebSearchResult {
                url: url.to_string(),
                title: Some(title.to_string()),
                snippet: Some(snippet),
            });
        }
    }))
}

fn search_result_header(line: &str) -> Option<(&str, &str)> {
    let (title, url) = line.rsplit_once(" (")?;
    Some((title, url.strip_suffix(')')?))
}

fn literal_url(ref_id: &str) -> Option<String> {
    Url::parse(ref_id).is_ok().then(|| ref_id.to_string())
}

fn web_search_item(
    call_id: &str,
    action: WebSearchAction,
    results: Option<Vec<WebSearchResult>>,
) -> ExtensionTurnItem {
    ExtensionTurnItem::WebSearch(WebSearchItem {
        id: call_id.to_string(),
        query: web_search_action_detail(&action),
        action,
        results,
    })
}

#[cfg(test)]
mod tests {
    use codex_api::SearchCommands;
    use codex_protocol::items::WebSearchResult;
    use codex_protocol::models::WebSearchAction;
    use pretty_assertions::assert_eq;

    use super::command_action;
    use super::search_results;

    #[test]
    fn command_action_reports_queries_and_navigation_detail() {
        let cases = [
            (
                r#"{"image_query":[{"q":"waterfalls"},{"q":"mountains"}]}"#,
                WebSearchAction::Search {
                    query: None,
                    queries: Some(vec!["waterfalls".to_string(), "mountains".to_string()]),
                },
            ),
            (
                r#"{"open":[{"ref_id":"https://example.com/docs"}]}"#,
                WebSearchAction::OpenPage {
                    url: Some("https://example.com/docs".to_string()),
                },
            ),
            (
                r#"{"find":[{"ref_id":"https://example.com/docs","pattern":"install"}]}"#,
                WebSearchAction::FindInPage {
                    url: Some("https://example.com/docs".to_string()),
                    pattern: Some("install".to_string()),
                },
            ),
            (
                r#"{"find":[{"ref_id":"turn0search0","pattern":"install"}]}"#,
                WebSearchAction::FindInPage {
                    url: None,
                    pattern: Some("install".to_string()),
                },
            ),
            (
                r#"{"open":[{"ref_id":"turn0search0"}]}"#,
                WebSearchAction::Other,
            ),
        ];

        for (arguments, expected) in cases {
            let commands: SearchCommands =
                serde_json::from_str(arguments).expect("valid search command arguments");
            assert_eq!(command_action(&commands), expected);
        }
    }

    #[test]
    fn search_results_extract_unique_http_urls_titles_and_snippets() {
        assert_eq!(
            search_results(
                r#"OpenAI docs (https://openai.com/docs)
Official API documentation

OpenAI docs duplicate (https://openai.com/docs)
Duplicate result

FTP mirror (ftp://example.com/archive)
Archive

Example article (https://example.com/article)
First excerpt line
Second excerpt line"#,
            ),
            vec![
                WebSearchResult {
                    url: "https://openai.com/docs".to_string(),
                    title: Some("OpenAI docs".to_string()),
                    snippet: Some("Official API documentation".to_string()),
                },
                WebSearchResult {
                    url: "https://example.com/article".to_string(),
                    title: Some("Example article".to_string()),
                    snippet: Some("First excerpt line\nSecond excerpt line".to_string()),
                },
            ]
        );
    }

    #[test]
    fn search_results_are_bounded() {
        let output = (0..=20)
            .map(|index| format!("Result {index} (https://example{index}.com/article)"))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(search_results(&output).len(), 20);
    }

    #[test]
    fn search_results_drop_oversized_urls() {
        let output = format!("Long result (https://example.com/{})", "x".repeat(600));

        assert_eq!(search_results(&output), Vec::new());
    }
}
