use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributor;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::codex_apps_mcp_server_config;

struct HostedAppsMcpExtension;

#[async_trait::async_trait]
impl McpServerContributor<Config> for HostedAppsMcpExtension {
    async fn contribute(&self, config: &Config) -> Vec<McpServerContribution> {
        let name = CODEX_APPS_MCP_SERVER_NAME.to_string();
        if !config.features.enabled(codex_features::Feature::Apps) {
            return vec![McpServerContribution::Remove { name }];
        }

        vec![McpServerContribution::Set {
            name,
            config: Box::new(codex_apps_mcp_server_config(
                &config.chatgpt_base_url,
                config.apps_mcp_path_override.as_deref(),
                config.apps_mcp_product_sku.as_deref(),
            )),
        }]
    }
}

pub fn install(builder: &mut ExtensionRegistryBuilder<Config>) {
    builder.mcp_server_contributor(std::sync::Arc::new(HostedAppsMcpExtension));
}
