use crate::remote::RemoteInstalledPlugin;
use crate::remote::RemotePluginScope;
use crate::store::ActivePluginInstallation;
use codex_config::types::PluginConfig;
use codex_plugin::PluginId;
use std::collections::HashMap;
use tracing::warn;

#[derive(Default)]
pub(crate) struct RemoteInstalledPluginsSnapshot {
    pub(crate) configs: HashMap<String, PluginConfig>,
    pub(crate) remote_plugin_id_resolver: RemotePluginIdResolver,
}

#[derive(Default)]
pub(crate) struct RemotePluginIdResolver {
    snapshot_ids: Option<HashMap<PluginId, String>>,
}

impl RemotePluginIdResolver {
    pub(crate) fn new(plugins: &[RemoteInstalledPlugin]) -> Self {
        let mut snapshot_ids = HashMap::with_capacity(plugins.len());
        for plugin in plugins {
            let Ok(plugin_id) = PluginId::new(plugin.name.clone(), plugin.marketplace_name.clone())
            else {
                continue;
            };
            snapshot_ids
                .entry(plugin_id)
                .or_insert_with(|| plugin.id.clone());
        }
        Self {
            snapshot_ids: Some(snapshot_ids),
        }
    }

    pub(crate) fn remote_plugin_id_for_installation(
        &self,
        installation: &ActivePluginInstallation,
    ) -> Option<String> {
        if let Some(snapshot_ids) = &self.snapshot_ids {
            return snapshot_ids.get(&installation.plugin_id).cloned();
        }

        persisted_remote_plugin_id_for_installation(installation)
    }
}

pub(crate) fn persisted_remote_plugin_id_for_installation(
    installation: &ActivePluginInstallation,
) -> Option<String> {
    RemotePluginScope::from_marketplace_name(&installation.plugin_id.marketplace_name)?;

    match installation.persisted_remote_plugin_id() {
        Ok(remote_plugin_id) => remote_plugin_id,
        Err(err) => {
            warn!(
                plugin_id = %installation.plugin_id.as_key(),
                error = %err,
                "failed to read persisted remote plugin identity"
            );
            None
        }
    }
}
