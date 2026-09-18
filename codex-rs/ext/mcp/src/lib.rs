use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::hosted_plugin_runtime_mcp_server_config;

#[cfg(test)]
#[path = "event_stream_tests.rs"]
mod event_stream_tests;
mod plugin;
mod stream_manager;

pub use codex_core_plugins::PluginListQuery;
pub use codex_core_plugins::PluginProvider;
pub use codex_core_plugins::PluginProviderError;
pub use codex_core_plugins::PluginProviderFuture;
pub use codex_core_plugins::PluginProviderResult;
pub use stream_manager::McpEventStreamManager;
pub use stream_manager::McpEventStreamUpdate;

#[cfg(test)]
#[path = "stream_manager_tests.rs"]
mod stream_manager_tests;

struct HostedPluginRuntimeExtension;

impl McpServerContributor<Config> for HostedPluginRuntimeExtension {
    fn id(&self) -> &'static str {
        "hosted_plugin_runtime"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let config = context.config();
            let name = CODEX_APPS_MCP_SERVER_NAME.to_string();
            if !config.features.enabled(codex_features::Feature::Apps) {
                return vec![McpServerContribution::Remove { name }];
            }

            vec![McpServerContribution::HostedApps {
                config: Box::new(hosted_plugin_runtime_mcp_server_config(
                    &config.chatgpt_base_url,
                    config.apps_mcp_product_sku.as_deref(),
                    context.originator(),
                )),
                protocol_mode: None,
            }]
        })
    }
}

pub fn install(builder: &mut ExtensionRegistryBuilder<Config>) {
    builder.mcp_server_contributor(std::sync::Arc::new(HostedPluginRuntimeExtension));
}

/// Installs discovery for MCP servers and apps declared by thread-selected plugins.
pub fn install_plugins(
    builder: &mut ExtensionRegistryBuilder<Config>,
    environment_manager: std::sync::Arc<codex_exec_server::EnvironmentManager>,
) {
    builder.mcp_server_contributor(std::sync::Arc::new(plugin::PluginContributor::new(
        environment_manager,
    )));
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
