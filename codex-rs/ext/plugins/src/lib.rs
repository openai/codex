use std::collections::BTreeMap;
use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_tools::JsonSchema;
use codex_tools::LIST_INSTALLABLE_PLUGINS_TOOL_NAME;
use codex_tools::RequestPluginInstallEntry;
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Copy, Debug, Default)]
struct PluginsExtension;

#[async_trait::async_trait]
pub trait InstallablePluginsProvider: Send + Sync {
    async fn list_installable_plugins(&self) -> Result<Vec<RequestPluginInstallEntry>, String>;
}

#[derive(Clone)]
pub struct InstallablePluginsProviderHandle {
    provider: Arc<dyn InstallablePluginsProvider>,
}

impl InstallablePluginsProviderHandle {
    pub fn new(provider: Arc<dyn InstallablePluginsProvider>) -> Self {
        Self { provider }
    }

    async fn list_installable_plugins(&self) -> Result<Vec<RequestPluginInstallEntry>, String> {
        self.provider.list_installable_plugins().await
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ListInstallablePluginsResponse {
    entries: Vec<RequestPluginInstallEntry>,
}

impl ToolContributor for PluginsExtension {
    fn tools(
        &self,
        session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(provider) = session_store.get::<InstallablePluginsProviderHandle>() else {
            return Vec::new();
        };

        vec![Arc::new(ListInstallablePluginsTool {
            provider: provider.as_ref().clone(),
        })]
    }
}

#[derive(Clone)]
struct ListInstallablePluginsTool {
    provider: InstallablePluginsProviderHandle,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolCall> for ListInstallablePluginsTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(LIST_INSTALLABLE_PLUGINS_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::Function(ResponsesApiTool {
            name: LIST_INSTALLABLE_PLUGINS_TOOL_NAME.to_string(),
            description: "Use this ONLY when all of the following are true:\n- The user explicitly asks to use a specific plugin or connector that is not already available in the current context or active `tools` list.\n- `tool_search` is not available, or it has already been called and did not find or make the requested tool callable.\nReturns a list of plugins eligible to be installed."
                .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
            output_schema: None,
        }))
    }

    async fn handle(&self, _call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let mut entries = self
            .provider
            .list_installable_plugins()
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        entries.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(Box::new(JsonToolOutput::new(json!(
            ListInstallablePluginsResponse { entries }
        ))))
    }
}

/// Installs plugins extension contributors into the supplied extension registry.
pub fn install<C>(registry: &mut ExtensionRegistryBuilder<C>) {
    registry.tool_contributor(Arc::new(PluginsExtension));
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::DiscoverableToolType;
    use pretty_assertions::assert_eq;

    #[derive(Clone)]
    struct StaticInstallablePluginsProvider {
        entries: Vec<RequestPluginInstallEntry>,
    }

    #[async_trait::async_trait]
    impl InstallablePluginsProvider for StaticInstallablePluginsProvider {
        async fn list_installable_plugins(&self) -> Result<Vec<RequestPluginInstallEntry>, String> {
            Ok(self.entries.clone())
        }
    }

    #[test]
    fn tools_are_not_contributed_without_provider() {
        let extension = PluginsExtension;

        assert!(
            extension
                .tools(
                    &ExtensionData::new("session"),
                    &ExtensionData::new("thread"),
                )
                .is_empty()
        );
    }

    #[tokio::test]
    async fn list_tool_returns_provider_installable_entries() {
        let extension = PluginsExtension;
        let session_store = ExtensionData::new("session");
        session_store.insert(InstallablePluginsProviderHandle::new(Arc::new(
            StaticInstallablePluginsProvider {
                entries: vec![RequestPluginInstallEntry {
                    id: "sample@openai-curated".to_string(),
                    name: "Sample Plugin".to_string(),
                    description: Some("Adds sample capabilities.".to_string()),
                    tool_type: DiscoverableToolType::Plugin,
                    has_skills: true,
                    mcp_server_names: vec!["sample-docs".to_string()],
                    app_connector_ids: vec!["connector_sample".to_string()],
                }],
            },
        )));

        let tools = extension.tools(&session_store, &ExtensionData::new("thread"));
        assert_eq!(tools.len(), 1);
        assert_eq!(
            tools[0].tool_name(),
            ToolName::plain(LIST_INSTALLABLE_PLUGINS_TOOL_NAME)
        );

        let payload = codex_extension_api::ToolPayload::Function {
            arguments: "{}".to_string(),
        };
        let output = tools[0]
            .handle(ToolCall {
                call_id: "call-1".to_string(),
                tool_name: ToolName::plain(LIST_INSTALLABLE_PLUGINS_TOOL_NAME),
                payload: payload.clone(),
            })
            .await
            .expect("list tool should succeed");

        assert_eq!(
            output.code_mode_result(&payload),
            json!({
                "entries": [{
                    "id": "sample@openai-curated",
                    "name": "Sample Plugin",
                    "description": "Adds sample capabilities.",
                    "tool_type": "plugin",
                    "has_skills": true,
                    "mcp_server_names": ["sample-docs"],
                    "app_connector_ids": ["connector_sample"]
                }]
            })
        );
    }
}
