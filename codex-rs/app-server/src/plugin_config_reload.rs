//! Adapts existing config loaders for blocking plugin-upgrade workers.
//! Reloads must use the same normal or fallback entry point as the initial load.

use crate::config_manager::ConfigManager;
use codex_core_plugins::ConfigLayerReload;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::Arc;

/// The config-loading path used to select marketplaces for startup tasks.
pub(crate) enum PluginStartupConfig {
    Current,
    Defaults,
}

pub(crate) fn for_cwd(manager: ConfigManager, cwd: AbsolutePathBuf) -> ConfigLayerReload {
    let runtime = tokio::runtime::Handle::current();
    Arc::new(move || runtime.block_on(manager.load_config_layers_for_cwd(cwd.clone())))
}

/// Reloads config without deriving project context from a process cwd.
pub(crate) fn global(manager: ConfigManager) -> ConfigLayerReload {
    let runtime = tokio::runtime::Handle::current();
    Arc::new(move || {
        runtime
            .block_on(manager.load_non_project_config())
            .map(|config| config.config_layer_stack)
    })
}

pub(crate) fn defaults(manager: ConfigManager) -> ConfigLayerReload {
    let runtime = tokio::runtime::Handle::current();
    Arc::new(move || {
        runtime
            .block_on(manager.load_default_config())
            .map(|config| config.config_layer_stack)
    })
}
