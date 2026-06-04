use codex_core_plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core_plugins::PluginReadRequest;
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
    let marketplace_path = match plugins_manager.list_marketplaces_for_config(plugins_input, &[]) {
        Ok(outcome) => outcome
            .marketplaces
            .into_iter()
            .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
            .map(|marketplace| marketplace.path),
        Err(err) => {
            warn!(
                plugin = %remote_plugin_name,
                error = %err,
                "failed to list local curated plugins for remote plugin MCP hydration"
            );
            return Vec::new();
        }
    };
    let Some(marketplace_path) = marketplace_path else {
        return Vec::new();
    };

    let outcome = match plugins_manager
        .read_plugin_for_config(
            plugins_input,
            &PluginReadRequest {
                plugin_name: remote_plugin_name.clone(),
                marketplace_path,
            },
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(err) => {
            warn!(
                plugin = %remote_plugin_name,
                error = %err,
                "failed to hydrate remote plugin MCP server names from local curated plugin"
            );
            return Vec::new();
        }
    };
    if remote_detail
        .release_version
        .as_deref()
        .is_some_and(|version| outcome.plugin.local_version.as_deref() != Some(version))
    {
        return Vec::new();
    }

    outcome.plugin.mcp_server_names
}
