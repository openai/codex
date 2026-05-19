use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::ListAvailablePluginsToInstallResult;
use codex_tools::RequestPluginInstallEntry;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::list_available_plugins_to_install_spec::create_list_available_plugins_to_install_tool;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

const MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RESULTS: usize = 20;
const MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_DESCRIPTION_CHARS: usize = 240;
const MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS: usize = 8;

pub struct ListAvailablePluginsToInstallHandler {
    tools: Vec<RequestPluginInstallEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListAvailablePluginsToInstallArgs {
    query: String,
}

impl ListAvailablePluginsToInstallHandler {
    pub(crate) fn new(mut tools: Vec<RequestPluginInstallEntry>) -> Self {
        tools.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        Self { tools }
    }

    fn result_for_query(&self, query: &str) -> ListAvailablePluginsToInstallResult {
        let normalized_query = query.to_lowercase();
        let matching_tools = self
            .tools
            .iter()
            .filter(|tool| install_candidate_matches_query(tool, normalized_query.as_str()))
            .collect::<Vec<_>>();
        let total_matches = matching_tools.len();
        let mut truncated = total_matches > MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RESULTS;
        let tools = matching_tools
            .into_iter()
            .take(MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RESULTS)
            .map(|tool| {
                let (tool, tool_truncated) = bounded_install_candidate(tool);
                truncated |= tool_truncated;
                tool
            })
            .collect();

        ListAvailablePluginsToInstallResult {
            tools,
            total_matches,
            truncated,
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ListAvailablePluginsToInstallHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_list_available_plugins_to_install_tool())
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        false
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;
        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };
        let args: ListAvailablePluginsToInstallArgs = parse_arguments(&arguments)?;
        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }

        let content = serde_json::to_string(&self.result_for_query(query)).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME} response: {err}"
            ))
        })?;

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            content,
            Some(true),
        )))
    }
}

impl CoreToolRuntime for ListAvailablePluginsToInstallHandler {}

fn install_candidate_matches_query(tool: &RequestPluginInstallEntry, query: &str) -> bool {
    tool.id.to_lowercase().contains(query)
        || tool.name.to_lowercase().contains(query)
        || tool
            .description
            .as_deref()
            .is_some_and(|description| description.to_lowercase().contains(query))
}

fn bounded_install_candidate(
    tool: &RequestPluginInstallEntry,
) -> (RequestPluginInstallEntry, bool) {
    let mut truncated = false;
    let description = tool.description.as_ref().map(|description| {
        let truncated_description = truncate_to_char_boundary(
            description,
            MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_DESCRIPTION_CHARS,
        );
        truncated |= truncated_description.len() != description.len();
        truncated_description.to_string()
    });
    let mut mcp_server_names = tool.mcp_server_names.clone();
    if mcp_server_names.len() > MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS {
        mcp_server_names.truncate(MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS);
        truncated = true;
    }
    let mut app_connector_ids = tool.app_connector_ids.clone();
    if app_connector_ids.len() > MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS {
        app_connector_ids.truncate(MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS);
        truncated = true;
    }

    (
        RequestPluginInstallEntry {
            id: tool.id.clone(),
            name: tool.name.clone(),
            description,
            tool_type: tool.tool_type,
            has_skills: tool.has_skills,
            mcp_server_names,
            app_connector_ids,
        },
        truncated,
    )
}

fn truncate_to_char_boundary(value: &str, max_chars: usize) -> &str {
    match value.char_indices().nth(max_chars) {
        Some((index, _)) => &value[..index],
        None => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::DiscoverableToolType;
    use pretty_assertions::assert_eq;

    #[test]
    fn list_tool_does_not_support_parallel_calls() {
        assert!(
            !ListAvailablePluginsToInstallHandler::new(Vec::new()).supports_parallel_tool_calls()
        );
    }

    #[test]
    fn result_for_query_filters_and_truncates_candidate_details() {
        let handler = ListAvailablePluginsToInstallHandler::new(vec![
            RequestPluginInstallEntry {
                id: "sample@openai-curated".to_string(),
                name: "Sample Plugin".to_string(),
                description: Some(
                    "x".repeat(MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_DESCRIPTION_CHARS + 1),
                ),
                tool_type: DiscoverableToolType::Plugin,
                has_skills: true,
                mcp_server_names: (0..=MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS)
                    .map(|index| format!("server-{index}"))
                    .collect(),
                app_connector_ids: (0..=MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS)
                    .map(|index| format!("connector-{index}"))
                    .collect(),
            },
            RequestPluginInstallEntry {
                id: "calendar@openai-curated".to_string(),
                name: "Calendar".to_string(),
                description: Some("calendar".to_string()),
                tool_type: DiscoverableToolType::Plugin,
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            },
        ]);

        assert_eq!(
            handler.result_for_query("sample"),
            ListAvailablePluginsToInstallResult {
                tools: vec![RequestPluginInstallEntry {
                    id: "sample@openai-curated".to_string(),
                    name: "Sample Plugin".to_string(),
                    description: Some(
                        "x".repeat(MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_DESCRIPTION_CHARS),
                    ),
                    tool_type: DiscoverableToolType::Plugin,
                    has_skills: true,
                    mcp_server_names: (0..MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS)
                        .map(|index| format!("server-{index}"))
                        .collect(),
                    app_connector_ids: (0..MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RELATED_IDS)
                        .map(|index| format!("connector-{index}"))
                        .collect(),
                }],
                total_matches: 1,
                truncated: true,
            }
        );
    }

    #[test]
    fn result_for_query_caps_matching_candidates() {
        let handler = ListAvailablePluginsToInstallHandler::new(
            (0..=MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RESULTS)
                .map(|index| RequestPluginInstallEntry {
                    id: format!("sample-{index}"),
                    name: format!("Sample {index}"),
                    description: None,
                    tool_type: DiscoverableToolType::Connector,
                    has_skills: false,
                    mcp_server_names: Vec::new(),
                    app_connector_ids: Vec::new(),
                })
                .collect(),
        );

        let result = handler.result_for_query("sample");

        assert_eq!(
            result.total_matches,
            MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RESULTS + 1
        );
        assert!(result.truncated);
        assert_eq!(
            result.tools.len(),
            MAX_LIST_AVAILABLE_PLUGINS_TO_INSTALL_RESULTS
        );
    }
}
