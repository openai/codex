use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::hosted_plugin_runtime_mcp_server_config;

mod executor_plugin;

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

            vec![McpServerContribution::Set {
                name,
                config: Box::new(hosted_plugin_runtime_mcp_server_config(
                    &config.chatgpt_base_url,
                    config.apps_mcp_product_sku.as_deref(),
                    context.originator(),
                )),
            }]
        })
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::McpServerTransportConfig;
    use codex_core::config::ConfigBuilder;
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionDataInit;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn hosted_plugin_runtime_forwards_thread_originator()
    -> Result<(), Box<dyn std::error::Error>> {
        let codex_home = tempfile::tempdir()?;
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .fallback_cwd(Some(codex_home.path().to_path_buf()))
            .cli_overrides(vec![
                ("features.apps".to_string(), true.into()),
                ("chatgpt_base_url".to_string(), "https://chatgpt.com".into()),
            ])
            .build()
            .await?;
        let thread_init = ExtensionDataInit::new();
        let thread_store = ExtensionData::new("thread");

        let contributions = HostedPluginRuntimeExtension
            .contribute(McpServerContributionContext::for_step(
                &config,
                &thread_init,
                &thread_store,
                "codex_work_desktop",
                /*available_environment_ids*/ &[],
            ))
            .await;
        let [McpServerContribution::Set { config: server, .. }] = contributions.as_slice() else {
            panic!("hosted plugin runtime should contribute one server");
        };
        let McpServerTransportConfig::StreamableHttp { http_headers, .. } = &server.transport
        else {
            panic!("hosted plugin runtime should use streamable HTTP");
        };

        assert_eq!(
            http_headers
                .as_ref()
                .and_then(|headers| headers.get("originator")),
            Some(&"codex_work_desktop".to_string())
        );

        Ok(())
    }
}
