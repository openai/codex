use super::PluginCapabilitySummary;
use super::PluginsManager;
use crate::config::Config;
use codex_capabilities::plugins::list_tool_suggest_discoverable_plugins as list_discoverable;

pub(crate) fn list_tool_suggest_discoverable_plugins(
    config: &Config,
) -> anyhow::Result<Vec<PluginCapabilitySummary>> {
    let plugins_manager = PluginsManager::new(config.codex_home.clone());
    list_discoverable(
        plugins_manager.inner(),
        &plugins_manager.load_request(config),
        &config.tool_suggest.discoverables,
    )
}

#[cfg(test)]
#[path = "discoverable_tests.rs"]
mod tests;
