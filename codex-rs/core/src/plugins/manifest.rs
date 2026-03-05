use serde::Deserialize;
use std::fs;
use std::path::Path;

pub(crate) const PLUGIN_MANIFEST_PATH: &str = ".codex-plugin/plugin.json";

#[derive(Debug, Default, Deserialize)]
pub(crate) struct PluginManifest {
    name: String,
}

pub(crate) fn load_plugin_manifest(plugin_root: &Path) -> Option<PluginManifest> {
    let manifest_path = plugin_root.join(PLUGIN_MANIFEST_PATH);
    if !manifest_path.is_file() {
        return None;
    }
    let contents = fs::read_to_string(&manifest_path).ok()?;
    match serde_json::from_str(&contents) {
        Ok(manifest) => Some(manifest),
        Err(err) => {
            tracing::warn!(
                path = %manifest_path.display(),
                "failed to parse plugin manifest: {err}"
            );
            None
        }
    }
}

pub(crate) fn plugin_manifest_name(manifest: &PluginManifest, plugin_root: &Path) -> String {
    plugin_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|_| manifest.name.trim().is_empty())
        .unwrap_or(&manifest.name)
        .to_string()
}
