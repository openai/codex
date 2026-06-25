use std::collections::HashSet;

use codex_config::types::ToolSuggestDiscoverableType;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::ToolSuggestPluginDiscoveryInput;
use codex_login::CodexAuth;
use codex_tools::DiscoverablePluginInfo;
use tracing::instrument;

use crate::config::Config;

#[instrument(level = "trace", skip_all)]
pub(crate) async fn list_tool_suggest_discoverable_plugins(
    config: &Config,
    plugins_manager: &PluginsManager,
    auth: Option<&CodexAuth>,
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    let input = ToolSuggestPluginDiscoveryInput::new(
        config.plugins_config_input(),
        config
            .tool_suggest
            .discoverables
            .iter()
            .filter(|discoverable| discoverable.kind == ToolSuggestDiscoverableType::Plugin)
            .map(|discoverable| discoverable.id.clone())
            .collect::<HashSet<_>>(),
        config
            .tool_suggest
            .disabled_tools
            .iter()
            .filter(|disabled_tool| disabled_tool.kind == ToolSuggestDiscoverableType::Plugin)
            .map(|disabled_tool| disabled_tool.id.clone())
            .collect::<HashSet<_>>(),
    );

    plugins_manager
        .list_tool_suggest_discoverable_plugins(&input, auth)
        .await
        .map(|plugins| {
            plugins
                .into_iter()
                .map(DiscoverablePluginInfo::from)
                .collect()
        })
}
