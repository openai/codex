use codex_api::ReqwestTransport;
use codex_api::SearchClient;
use codex_api::SearchCommands;
use codex_api::SearchRequest;
use codex_api::SearchSettings;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use codex_login::default_client::build_reqwest_client;
use codex_model_provider::SharedModelProvider;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExposure;
use codex_tools::default_namespace_description;
use http::HeaderMap;

use crate::history::recent_input;
use crate::output::EncryptedSearchOutput;
use crate::schema::commands_schema;

const WEB_NAMESPACE: &str = "web";
const RUN_TOOL_NAME: &str = "run";
const WEB_RUN_DESCRIPTION: &str = include_str!("../web_run_description.md");

pub(crate) struct WebSearchTool {
    pub(crate) session_id: String,
    pub(crate) provider: SharedModelProvider,
    pub(crate) settings: SearchSettings,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolCall> for WebSearchTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(WEB_NAMESPACE, RUN_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let parameters = match parse_tool_input_schema(&commands_schema()) {
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
        ToolExposure::DirectModelOnly
    }

    async fn handle(&self, call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let commands = parse_commands(&call)?;
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
            model: None,
            reasoning: None,
            input: recent_input(call.conversation_history.items()),
            commands: Some(commands),
            settings: Some(self.settings.clone()),
            max_output_tokens: Some(
                u64::try_from(call.truncation_policy.token_budget()).unwrap_or(u64::MAX),
            ),
        };
        let response = client
            .search(&request, HeaderMap::new())
            .await
            .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;

        Ok(Box::new(EncryptedSearchOutput::new(
            response.encrypted_output,
        )))
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
