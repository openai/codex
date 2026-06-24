//! Executor-backed connector declaration loading.

mod executor_plugin;
mod selected;

pub use executor_plugin::ExecutorPluginConnectorProvider;
pub use executor_plugin::ExecutorPluginConnectorProviderError;
pub use selected::SelectedExecutorConnectorProvider;
