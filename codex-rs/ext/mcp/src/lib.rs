use std::sync::Arc;

use codex_core::config::Config;
use codex_core_plugins::PluginsManager;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_login::AuthManager;

mod apps;
mod executor_plugin;

pub use apps::CodexAppsMcpExtension;

/// One-call composition boundary for hosts that do not need product-specific MCP APIs.
///
/// Construction installs the process-scoped extensions into one registry while hiding their
/// concrete services. The bundle owns those services and their deterministic async shutdown.
pub struct McpHostExtensions {
    registry: Arc<ExtensionRegistry<Config>>,
    lifecycle: Arc<CodexAppsMcpExtension>,
}

impl McpHostExtensions {
    pub fn new(
        auth_manager: Arc<AuthManager>,
        environment_manager: Arc<EnvironmentManager>,
        plugins_manager: Arc<PluginsManager>,
    ) -> Self {
        let lifecycle = Arc::new(CodexAppsMcpExtension::new(
            auth_manager,
            environment_manager,
            plugins_manager,
        ));
        let mut builder = ExtensionRegistryBuilder::new();
        install(&mut builder, Arc::clone(&lifecycle));
        Self {
            registry: Arc::new(builder.build()),
            lifecycle,
        }
    }

    pub fn registry(&self) -> Arc<ExtensionRegistry<Config>> {
        Arc::clone(&self.registry)
    }

    pub async fn shutdown(&self) {
        self.lifecycle.shutdown().await;
    }
}

/// Installs a process-shared Apps service as an MCP contributor.
pub fn install(
    builder: &mut ExtensionRegistryBuilder<Config>,
    service: Arc<CodexAppsMcpExtension>,
) {
    builder.thread_data_initializer(service.clone());
    builder.mcp_server_contributor(service.clone());
    builder.plugin_install_verifier(service.clone());
    builder.prompt_contributor(service.clone());
    builder.turn_input_contributor(service.clone());
    builder.tool_lifecycle_contributor(service.clone());
    builder.turn_item_contributor(service);
}

/// Installs selected executor-plugin MCP metadata before the Apps contributor that consumes it.
pub fn install_with_executor_plugins(
    builder: &mut ExtensionRegistryBuilder<Config>,
    service: Arc<CodexAppsMcpExtension>,
    environment_manager: Arc<codex_exec_server::EnvironmentManager>,
) {
    install_executor_plugins(builder, environment_manager);
    install(builder, service);
}

/// Installs discovery for MCP servers declared by thread-selected executor plugins.
pub fn install_executor_plugins(
    builder: &mut ExtensionRegistryBuilder<Config>,
    environment_manager: Arc<codex_exec_server::EnvironmentManager>,
) {
    builder.mcp_server_contributor(Arc::new(
        executor_plugin::SelectedExecutorPluginMcpContributor::new(environment_manager),
    ));
}
