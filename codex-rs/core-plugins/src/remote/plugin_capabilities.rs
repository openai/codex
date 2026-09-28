//! Discovers best-effort runtime invalidation hints from locally cached plugin bundles.
//!
//! Discovery ignores runtime policy and does not prepare runtime directories. Repeated inclusion
//! unions old and new declarations for updates; removals capture declarations before deletion.

use crate::loader::load_plugin_apps_from_manifest;
use crate::loader::load_plugin_hooks;
use crate::loader::load_plugin_mcp_servers_from_manifest_with_format;
use crate::loader::plugin_skill_roots;
use crate::manifest::PluginManifestFormat;
use crate::store::PluginStore;
use codex_hooks::plugin_hook_declarations;
use codex_plugin::PluginId;

/// Runtime categories affected by a plugin change, before runtime policy filtering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemotePluginCapabilities {
    pub has_mcps: bool,
    pub has_apps: bool,
    pub has_hooks: bool,
    pub has_skills: bool,
}

impl RemotePluginCapabilities {
    pub(crate) async fn include_active_bundle(
        &mut self,
        store: &PluginStore,
        plugin_id: &PluginId,
    ) {
        let Some(root) = store.active_plugin_root(plugin_id) else {
            return;
        };
        let Some(loaded) = store.manifest_cache.load(root.as_path()) else {
            return;
        };
        let paths = &loaded.manifest.paths;
        self.has_skills |= !plugin_skill_roots(&root, paths, loaded.format).is_empty();
        self.has_mcps |= !load_plugin_mcp_servers_from_manifest_with_format(
            root.as_path(),
            paths,
            /*plugin_policy*/ None,
            // Declaration discovery must not create runtime directories or depend on their readiness.
            /*plugin_data_root*/
            None,
            loaded.format,
        )
        .await
        .is_empty();
        // Agent Plugins can declare Apps through an OpenAI extension even when the local
        // runtime does not activate them. Refresh hints describe declarations, not activation.
        self.has_apps |= !load_plugin_apps_from_manifest(root.as_path(), paths)
            .await
            .is_empty();
        if loaded.format == PluginManifestFormat::Legacy {
            let (sources, _) =
                load_plugin_hooks(&root, plugin_id, &store.plugin_data_root(plugin_id), paths);
            self.has_hooks |= !plugin_hook_declarations(&sources).is_empty();
        }
    }
}

#[cfg(test)]
#[path = "plugin_capabilities_tests.rs"]
mod tests;
