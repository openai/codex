use crate::PluginsConfigInput;
use crate::remote::RemotePluginCatalogError;
use codex_login::CodexAuth;
use codex_tools::DiscoverableTool;

pub async fn hydrate_selected_recommended_plugin_install_metadata(
    config: &PluginsConfigInput,
    auth: Option<&CodexAuth>,
    mut tool: DiscoverableTool,
) -> Result<Option<DiscoverableTool>, RemotePluginCatalogError> {
    let DiscoverableTool::Plugin(plugin) = &mut tool else {
        return Ok(Some(tool));
    };
    let remote_plugin_id = plugin.remote_plugin_id.clone().ok_or_else(|| {
        RemotePluginCatalogError::UnexpectedResponse(format!(
            "recommended plugin `{}` is missing remote plugin identity",
            plugin.id
        ))
    })?;
    let Some(app_connector_ids) = crate::remote::fetch_recommended_plugin_install_metadata(
        &config.remote_plugin_service_config(),
        auth,
        &remote_plugin_id,
    )
    .await?
    else {
        return Ok(None);
    };
    plugin.app_connector_ids = app_connector_ids;
    Ok(Some(tool))
}
