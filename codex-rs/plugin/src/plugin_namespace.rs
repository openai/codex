//! Resolve plugin namespace from skill file paths by walking ancestors for `plugin.json`.

use std::fs;
use std::path::Path;

/// Relative path from a plugin root to its manifest file.
pub const PLUGIN_MANIFEST_PATH: &str = ".codex-plugin/plugin.json";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestName {
    #[serde(default)]
    name: String,
}

fn plugin_manifest_name(plugin_root: &Path) -> Option<String> {
    let manifest_path = plugin_root.join(PLUGIN_MANIFEST_PATH);
    if !manifest_path.is_file() {
        return None;
    }
    let contents = fs::read_to_string(&manifest_path).ok()?;
    let RawPluginManifestName { name: raw_name } = serde_json::from_str(&contents).ok()?;
    Some(
        plugin_root
            .file_name()
            .and_then(|entry| entry.to_str())
            .filter(|_| raw_name.trim().is_empty())
            .unwrap_or(raw_name.as_str())
            .to_string(),
    )
}

/// Returns the plugin manifest `name` for the nearest ancestor of `path` that contains a valid
/// plugin manifest (same `name` rules as full manifest loading in codex-core).
pub fn plugin_namespace_for_skill_path(path: &Path) -> Option<String> {
    for ancestor in path.ancestors() {
        if let Some(name) = plugin_manifest_name(ancestor) {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::plugin_namespace_for_skill_path;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn uses_manifest_name() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("plugins/sample");
        let skill_path = plugin_root.join("skills/search/SKILL.md");

        fs::create_dir_all(skill_path.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("mkdir manifest");
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        )
        .expect("write manifest");
        fs::write(&skill_path, "---\ndescription: search\n---\n").expect("write skill");

        assert_eq!(
            plugin_namespace_for_skill_path(&skill_path),
            Some("sample".to_string())
        );
    }
}
