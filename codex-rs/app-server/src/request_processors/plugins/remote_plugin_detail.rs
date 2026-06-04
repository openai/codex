use codex_core_plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core_plugins::PluginsConfigInput;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_core_plugins::remote::RemotePluginDetail;
use tracing::warn;

pub(super) async fn local_curated_mcp_server_names(
    plugins_manager: &PluginsManager,
    plugins_input: &PluginsConfigInput,
    remote_detail: &RemotePluginDetail,
) -> Vec<String> {
    if remote_detail.marketplace_name != REMOTE_GLOBAL_MARKETPLACE_NAME {
        return Vec::new();
    }

    let remote_plugin_name = &remote_detail.summary.name;
    let local_plugin = match plugins_manager.list_marketplaces_for_config(plugins_input, &[]) {
        Ok(outcome) => outcome
            .marketplaces
            .into_iter()
            .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
            .and_then(|marketplace| {
                marketplace
                    .plugins
                    .into_iter()
                    .find(|plugin| {
                        plugin.name == *remote_plugin_name
                            && remote_detail
                                .release_version
                                .as_deref()
                                .is_none_or(|version| {
                                    plugin.local_version.as_deref() == Some(version)
                                })
                    })
                    .map(|plugin| (marketplace.name, plugin))
            }),
        Err(err) => {
            warn!(
                plugin = %remote_plugin_name,
                error = %err,
                "failed to list local curated plugins for remote plugin MCP hydration"
            );
            return Vec::new();
        }
    };
    let Some((marketplace_name, plugin)) = local_plugin else {
        return Vec::new();
    };

    match plugins_manager
        .read_plugin_detail_for_marketplace_plugin(plugins_input, &marketplace_name, plugin)
        .await
    {
        Ok(plugin) => plugin.mcp_server_names,
        Err(err) => {
            warn!(
                plugin = %remote_plugin_name,
                error = %err,
                "failed to hydrate remote plugin MCP server names from local curated plugin"
            );
            Vec::new()
        }
    }
}
