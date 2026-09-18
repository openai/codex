//! Selects one authenticated installed GLOBAL plugin without capability loading.

use super::*;
use crate::store::validate_plugin_version_segment;
use codex_utils_path_uri::PathConvention;

/// Fetches installation authorization and download metadata, but materializes no
/// other installed plugins and never publishes to the shared plugin catalog.
pub(crate) async fn fetch_measurement_reference_bundle(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    target: &crate::script_attribution::PluginMeasurementTarget,
) -> anyhow::Result<Option<(String, crate::remote_bundle::ValidatedRemotePluginBundle)>> {
    ensure_chatgpt_auth(Some(auth))?;
    let installed = fetch_installed_plugins(
        config,
        auth,
        RemoteInstalledPluginScope::Single(RemotePluginScope::Global),
        /*include_download_urls*/ true,
    )
    .await?;
    let mut matching = installed.into_iter().filter(|installed| {
        let plugin = &installed.plugin;
        let Some(version) = plugin.release.version.as_deref().map(str::trim) else {
            return false;
        };
        if validate_plugin_version_segment(version).is_err() {
            return false;
        }
        // Compare with the executor's path rules, retaining the catalog's
        // spelling for the validated bundle and cache identity below.
        let matches_target = match target.path_convention {
            PathConvention::Windows => target
                .plugin_id
                .plugin_name
                .eq_ignore_ascii_case(&plugin.name),
            PathConvention::Posix => target.plugin_id.plugin_name == plugin.name,
        };
        installed.enabled
            && plugin.scope == RemotePluginScope::Global
            && matches_target
            && plugin.availability == PluginAvailability::Available
            && plugin.installation_policy != PluginInstallPolicy::NotAvailable
            && plugin.disabled_reason.is_none()
    });
    let Some(installed) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Ok(None);
    }
    let plugin = installed.plugin;
    let bundle = crate::remote_bundle::validate_remote_plugin_bundle(
        &plugin.id,
        REMOTE_GLOBAL_MARKETPLACE_NAME,
        &plugin.name,
        plugin.release.version.as_deref(),
        plugin.release.bundle_download_url.as_deref(),
        plugin.release.app_manifest,
    )?;
    Ok(Some((plugin.id, bundle)))
}
