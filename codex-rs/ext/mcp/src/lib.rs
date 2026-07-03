use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::codex_apps_mcp_server_config;
use codex_mcp::hosted_plugin_runtime_mcp_server_config;

mod executor_plugin;

const CODEX_APPS_MCP_BASE_URL_ENV_VAR: &str = "CODEX_APPS_MCP_BASE_URL";

#[derive(Debug, Eq, PartialEq)]
enum AppsMcpServerTarget<'a> {
    HostedPluginRuntime(&'a str),
    CodexApps(&'a str),
}

fn apps_mcp_server_target<'a>(
    chatgpt_base_url: &'a str,
    apps_mcp_base_url_override: Option<&'a str>,
) -> AppsMcpServerTarget<'a> {
    if let Some(apps_mcp_base_url) = apps_mcp_base_url_override
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        return AppsMcpServerTarget::CodexApps(apps_mcp_base_url);
    }

    AppsMcpServerTarget::HostedPluginRuntime(chatgpt_base_url)
}

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

            let apps_mcp_base_url_override = std::env::var(CODEX_APPS_MCP_BASE_URL_ENV_VAR).ok();
            let apps_mcp_product_sku = config.apps_mcp_product_sku.as_deref();
            let server_config = match apps_mcp_server_target(
                &config.chatgpt_base_url,
                apps_mcp_base_url_override.as_deref(),
            ) {
                AppsMcpServerTarget::HostedPluginRuntime(base_url) => {
                    hosted_plugin_runtime_mcp_server_config(base_url, apps_mcp_product_sku)
                }
                AppsMcpServerTarget::CodexApps(base_url) => {
                    codex_apps_mcp_server_config(base_url, apps_mcp_product_sku)
                }
            };

            vec![McpServerContribution::Set {
                name,
                config: Box::new(server_config),
            }]
        })
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

pub fn install(builder: &mut ExtensionRegistryBuilder<Config>) {
    builder.mcp_server_contributor(std::sync::Arc::new(HostedPluginRuntimeExtension));
}

/// Installs discovery for MCP servers declared by thread-selected executor plugins.
pub fn install_executor_plugins(
    builder: &mut ExtensionRegistryBuilder<Config>,
    environment_manager: std::sync::Arc<codex_exec_server::EnvironmentManager>,
) {
    builder.mcp_server_contributor(std::sync::Arc::new(
        executor_plugin::SelectedExecutorPluginMcpContributor::new(environment_manager),
    ));
}
