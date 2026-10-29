use std::sync::Arc;

use codex_api::ReqwestTransport;
use codex_api::SearchClient;
use codex_api::SearchCommands;
use codex_api::SearchQuery;
use codex_api::SearchRequest;
use codex_api::SearchSettings;
use codex_core::X_CODEX_TURN_METADATA_HEADER;
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
use codex_extension_items::ExtensionItem;
use codex_extension_items::web_search::WebSearchAction;
use codex_extension_items::web_search::WebSearchItem;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::create_client_for_route_async;
use codex_login::default_client::create_transport_for_routes_async;
use codex_model_provider::ACCOUNT_ROUTING_HEADER;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::WorkspaceRoutingContext;
use codex_model_provider::prepare_provider_request;
use codex_model_provider::provider_uses_first_party_auth_path;
use codex_protocol::models::WebSearchAction as CoreWebSearchAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WebSearchBeginEvent;
use codex_protocol::protocol::WebSearchEndEvent;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExposure;
use codex_tools::default_namespace_description;
use http::HeaderMap;
use http::HeaderValue;
use url::Url;

use crate::history::recent_input;
use crate::output::SearchOutput;
use crate::schema::commands_schema;

pub(crate) const WEB_NAMESPACE: &str = "web";
pub(crate) const RUN_TOOL_NAME: &str = "run";
const WEB_RUN_DESCRIPTION: &str = include_str!("../web_run_description.md");
const RESULTS_PAYLOAD_BYTES_METRIC: &str = "codex.web_search.results.payload_bytes";
const MAX_SEARCH_ERROR_BYTES: usize = 1024;

pub(crate) struct WebSearchTool {
    pub(crate) session_id: String,
    pub(crate) http_client_factory: HttpClientFactory,
    pub(crate) provider: SharedModelProvider,
    pub(crate) workspace_routing: Arc<WorkspaceRoutingContext>,
    pub(crate) settings: SearchSettings,
    pub(crate) supports_literature_search: bool,
    pub(crate) originator: Option<String>,
}

impl<'call> ToolExecutor<ToolCall<'call>> for WebSearchTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(WEB_NAMESPACE, RUN_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        // parse schema without compaction that removes field metadata/descriptions to match hosted tool definition
        let schema = commands_schema(self.supports_literature_search);
        let parameters = match parse_tool_input_schema_without_compaction(&schema) {
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

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> codex_extension_api::ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(self.handle_call(call))
    }
}

impl WebSearchTool {
    async fn handle_call(
        &self,
        call: ToolCall<'_>,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let commands = parse_commands(call.function_arguments()?, self.supports_literature_search)?;
        let command_action = command_action(&commands);
        let provider = self.provider.as_ref();
        let first_party = provider_uses_first_party_auth_path(provider.info());
        let client = prepare_provider_request(provider, first_party, |_| async {
            let routing_auth = if first_party {
                provider.auth().await
            } else {
                None
            };
            let (endpoint, api_auth) = provider.api_provider_and_auth().await?;
            let resolved = provider
                .route_responses_api_provider(
                    endpoint,
                    routing_auth.as_ref(),
                    &self.workspace_routing,
                )
                .await?;
            let endpoint = resolved.provider;
            let transport = if resolved.redirect_policy == ClientRedirectPolicy::Reject
                || endpoint.headers.contains_key(ACCOUNT_ROUTING_HEADER)
            {
                create_client_for_route_async(
                    self.http_client_factory.clone(),
                    endpoint.url_for_path("alpha/search"),
                    ClientRouteClass::Api,
                    ClientRedirectPolicy::Reject,
                )
                .await
                .map(ReqwestTransport::from_http_client)
            } else {
                create_transport_for_routes_async(
                    self.http_client_factory.clone(),
                    ClientRouteClass::Api,
                )
                .await
            }?;
            Ok(SearchClient::new(transport, endpoint, api_auth))
        })
        .await
        .map_err(|err| FunctionCallError::RespondToModel(search_error_message(err)))?;
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
        let extra_headers = search_request_headers(
            self.originator.as_deref(),
            call.codex_turn_metadata.as_deref(),
        );
        call.turn_item_emitter
            .emit_started(extension_turn_item(
                WebSearchItem {
                    id: call.call_id.clone(),
                    query: String::new(),
                    action: None,
                    results: None,
                },
                EventMsg::WebSearchBegin(WebSearchBeginEvent {
                    call_id: call.call_id.clone(),
                }),
            ))
            .await;
        let (output, results) = match client.search(&request, extra_headers).await {
            Ok(response) => (Ok(response.output), response.results),
            Err(err) => (Err(search_error_message(err)), None),
        };
        if let Some(results) = results.as_ref()
            && let Some(metrics) = codex_otel::global()
            && let Ok(payload) = serde_json::to_vec(results)
        {
            let payload_bytes = i64::try_from(payload.len()).unwrap_or(i64::MAX);
            let _ = metrics.histogram(RESULTS_PAYLOAD_BYTES_METRIC, payload_bytes, &[]);
        }
        let legacy_action = match &command_action {
            WebSearchAction::Search { query, queries } => CoreWebSearchAction::Search {
                query: query.clone(),
                queries: queries.clone(),
            },
            WebSearchAction::OpenPage { url } => CoreWebSearchAction::OpenPage { url: url.clone() },
            WebSearchAction::FindInPage { url, pattern } => CoreWebSearchAction::FindInPage {
                url: url.clone(),
                pattern: pattern.clone(),
            },
            WebSearchAction::Other => CoreWebSearchAction::Other,
        };
        let query = web_search_action_detail(&legacy_action);
        call.turn_item_emitter
            .emit_completed(extension_turn_item(
                WebSearchItem {
                    id: call.call_id.clone(),
                    query: query.clone(),
                    action: Some(command_action),
                    results: results.clone(),
                },
                EventMsg::WebSearchEnd(WebSearchEndEvent {
                    call_id: call.call_id.clone(),
                    query,
                    action: legacy_action,
                    results,
                }),
            ))
            .await;

        Ok(Box::new(SearchOutput::new(output)))
    }
}

fn search_error_message(error: impl std::fmt::Display) -> String {
    let mut message = error.to_string();
    if message.len() > MAX_SEARCH_ERROR_BYTES {
        const SUFFIX: &str = "… [truncated]";
        message.truncate(message.floor_char_boundary(MAX_SEARCH_ERROR_BYTES - SUFFIX.len()));
        message.push_str(SUFFIX);
    }
    message
}

fn search_request_headers(originator: Option<&str>, turn_metadata: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(turn_metadata) = turn_metadata
        && let Ok(header_value) = HeaderValue::from_str(turn_metadata)
    {
        headers.insert(X_CODEX_TURN_METADATA_HEADER, header_value);
    }

    if let Some(originator) = originator {
        add_originator_header(&mut headers, originator);
    }
    headers
}

fn parse_commands(
    arguments: &str,
    supports_literature_search: bool,
) -> Result<SearchCommands, FunctionCallError> {
    if arguments.trim().is_empty() {
        return Ok(SearchCommands::default());
    }

    let commands: SearchCommands = serde_json::from_str(arguments)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
    if !supports_literature_search && commands.literature_search_query.is_some() {
        return Err(FunctionCallError::RespondToModel(
            "literature_search_query is unavailable for this model".to_string(),
        ));
    }
    Ok(commands)
}

fn command_action(commands: &SearchCommands) -> WebSearchAction {
    commands
        .search_query
        .as_deref()
        .and_then(query_action)
        .or_else(|| {
            commands
                .literature_search_query
                .as_deref()
                .and_then(query_action)
        })
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

fn literal_url(ref_id: &str) -> Option<String> {
    Url::parse(ref_id).is_ok().then(|| ref_id.to_string())
}

fn extension_turn_item(item: WebSearchItem, legacy_event: EventMsg) -> ExtensionTurnItem {
    ExtensionTurnItem {
        item: ExtensionItem::WebSearch(item),
        legacy_events: vec![legacy_event],
    }
}

#[cfg(test)]
mod tests {
    use codex_api::SearchCommands;
    use codex_extension_items::web_search::WebSearchAction;
    use pretty_assertions::assert_eq;

    use super::command_action;
    use super::parse_commands;
    use super::search_error_message;
    use super::search_request_headers;
    use codex_core::X_CODEX_TURN_METADATA_HEADER;
    use codex_extension_api::FunctionCallError;

    #[test]
    fn search_errors_preserve_short_messages_and_bound_utf8_output() {
        let fits = "x".repeat(/*n*/ 1024);
        assert_eq!(search_error_message(&fits), fits);

        let oversized = "🦀".repeat(/*n*/ 512);
        let message = search_error_message(&oversized);
        assert!(message.len() <= 1024);
        assert!(oversized.starts_with(message.strip_suffix("… [truncated]").unwrap()));
    }

    #[test]
    fn search_request_headers_forward_thread_originator_and_turn_metadata() {
        let headers = search_request_headers(Some("chatgpt_cca"), Some("turn-metadata"));
        assert_eq!(
            headers
                .get("originator")
                .and_then(|value| value.to_str().ok()),
            Some("chatgpt_cca")
        );
        assert_eq!(
            headers
                .get(X_CODEX_TURN_METADATA_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some("turn-metadata")
        );
    }

    #[test]
    fn rejects_literature_search_without_the_model_capability() {
        let arguments =
            r#"{"search_query":[{"q":"regular"}],"literature_search_query":[{"q":"papers"}]}"#;
        assert!(matches!(
            parse_commands(arguments, /*supports_literature_search*/ false),
            Err(FunctionCallError::RespondToModel(message))
                if message == "literature_search_query is unavailable for this model"
        ));
    }

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
}
