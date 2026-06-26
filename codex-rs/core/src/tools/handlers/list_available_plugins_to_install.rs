use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::ListAvailablePluginsToInstallResult;
use codex_tools::RequestPluginInstallEntry;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::list_available_plugins_to_install_spec::create_list_available_plugins_to_install_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

const MAX_DESCRIPTION_CHARS: usize = 240;

pub struct ListAvailablePluginsToInstallHandler {
    plugins: Vec<RequestPluginInstallEntry>,
}

impl ListAvailablePluginsToInstallHandler {
    pub(crate) fn new(mut plugins: Vec<RequestPluginInstallEntry>) -> Self {
        plugins.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        Self { plugins }
    }

    fn result(&self) -> ListAvailablePluginsToInstallResult {
        ListAvailablePluginsToInstallResult {
            tools: self
                .plugins
                .iter()
                .cloned()
                .map(|mut plugin| {
                    plugin.description = plugin.description.map(|description| {
                        truncate_to_char_boundary(&description, MAX_DESCRIPTION_CHARS).to_string()
                    });
                    plugin
                })
                .collect(),
        }
    }
}

impl ToolExecutor<ToolInvocation> for ListAvailablePluginsToInstallHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_list_available_plugins_to_install_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        false
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl ListAvailablePluginsToInstallHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        if !matches!(invocation.payload, ToolPayload::Function { .. }) {
            return Err(FunctionCallError::Fatal(format!(
                "{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME} handler received unsupported payload"
            )));
        }

        let content = serde_json::to_string(&self.result()).map_err(|err| {
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

fn truncate_to_char_boundary(value: &str, max_chars: usize) -> &str {
    match value.char_indices().nth(max_chars) {
        Some((index, _)) => &value[..index],
        None => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::DiscoverablePluginInfo;
    use codex_tools::collect_request_plugin_install_entries;
    use pretty_assertions::assert_eq;

    #[test]
    fn result_clones_sorts_and_truncates_plugin_entries() {
        let candidates = [
            DiscoverablePluginInfo {
                id: "sample@openai-curated".to_string(),
                remote_plugin_id: None,
                name: "Sample Plugin".to_string(),
                description: Some("x".repeat(MAX_DESCRIPTION_CHARS + 1)),
                has_skills: true,
                mcp_server_names: vec!["sample-mcp".to_string()],
                ..DiscoverablePluginInfo::default()
            },
            DiscoverablePluginInfo {
                id: "calendar@openai-curated".to_string(),
                remote_plugin_id: None,
                name: "Calendar".to_string(),
                description: Some("calendar".to_string()),
                has_skills: false,
                mcp_server_names: Vec::new(),
                ..DiscoverablePluginInfo::default()
            },
        ]
        .to_vec();
        let handler = ListAvailablePluginsToInstallHandler::new(
            collect_request_plugin_install_entries(&candidates),
        );

        let result = handler.result();
        assert_eq!(result.tools[0].name, "Calendar");
        assert_eq!(result.tools[1].name, "Sample Plugin");
        assert_eq!(
            result.tools[1].description,
            Some("x".repeat(MAX_DESCRIPTION_CHARS))
        );
    }
}
