mod manager;
mod manifest;
mod store;

pub use manager::AppConnectorId;
pub use manager::LoadedPlugin;
pub use manager::PluginInstallError;
pub use manager::PluginLoadOutcome;
pub use manager::PluginsManager;
pub(crate) use manager::plugin_namespace_for_skill_path;
pub(crate) use manifest::load_plugin_manifest;
pub(crate) use manifest::plugin_manifest_name;
pub use store::PluginId;
pub use store::PluginInstallRequest;
pub use store::PluginInstallResult;
