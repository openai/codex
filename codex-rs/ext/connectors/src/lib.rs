//! Executor-backed connector declaration loading.

mod executor_plugin;
mod selected;

pub use executor_plugin::ExecutorPluginConnectorProvider;
pub use executor_plugin::ExecutorPluginConnectorProviderError;
pub use selected::SelectedExecutorConnectorProvider;

/// Installs thread-scoped connector discovery for selected executor plugins.
pub fn install_selected_executor_connectors<C: Sync>(
    builder: &mut codex_extension_api::ExtensionRegistryBuilder<C>,
) {
    builder.thread_extension_init_contributor(std::sync::Arc::new(
        SelectedExecutorConnectorProvider::new(),
    ));
}
