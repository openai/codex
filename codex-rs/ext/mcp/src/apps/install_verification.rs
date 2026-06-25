use codex_apps::CodexAppsSnapshot;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::PluginInstallVerificationContext;
use codex_extension_api::PluginInstallVerifier;

use super::CodexAppsMcpExtension;

impl PluginInstallVerifier<codex_core::config::Config> for CodexAppsMcpExtension {
    fn verify<'a>(
        &'a self,
        context: PluginInstallVerificationContext<'a, codex_core::config::Config>,
    ) -> ExtensionFuture<'a, Option<bool>> {
        Box::pin(async move {
            let plugin = context.plugin();
            if plugin.remote_plugin_id.is_none() || plugin.app_connector_ids.is_empty() {
                return None;
            }
            let snapshot = self.current_snapshot(context.config()).await;
            Some(snapshot.is_some_and(|snapshot| {
                all_declared_apps_materialized(&plugin.app_connector_ids, &snapshot)
            }))
        })
    }
}

fn all_declared_apps_materialized(
    declared_app_ids: &[String],
    snapshot: &CodexAppsSnapshot,
) -> bool {
    declared_app_ids.iter().all(|declared_app_id| {
        snapshot
            .all_connectors()
            .iter()
            .any(|app| app.id() == declared_app_id.as_str())
    })
}

#[cfg(test)]
#[path = "install_verification_tests.rs"]
mod tests;
