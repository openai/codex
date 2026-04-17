//! Resolve plugin namespace from skill file paths by walking ancestors for `plugin.json`.

use codex_exec_server::ExecutorPathRef;

/// Relative path from a plugin root to its manifest file.
pub const PLUGIN_MANIFEST_PATH: &str = ".codex-plugin/plugin.json";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestName {
    #[serde(default)]
    name: String,
}

async fn plugin_manifest_name(plugin_root: &ExecutorPathRef<'_>) -> Option<String> {
    let manifest_path = plugin_root.join(PLUGIN_MANIFEST_PATH);
    match manifest_path.unsandboxed().is_file().await {
        Ok(true) => {}
        Ok(false) | Err(_) => return None,
    }
    let contents = manifest_path.unsandboxed().read_file_text().await.ok()?;
    let RawPluginManifestName { name: raw_name } = serde_json::from_str(&contents).ok()?;
    Some(
        plugin_root
            .path()
            .file_name()
            .and_then(|entry| entry.to_str())
            .filter(|_| raw_name.trim().is_empty())
            .unwrap_or(raw_name.as_str())
            .to_string(),
    )
}

/// Returns the plugin manifest `name` for the nearest ancestor of `path` that contains a valid
/// plugin manifest (same `name` rules as full manifest loading in codex-core).
pub async fn plugin_namespace_for_skill_path(path: &ExecutorPathRef<'_>) -> Option<String> {
    for ancestor in path.ancestors() {
        if let Some(name) = plugin_manifest_name(&ancestor).await {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::plugin_namespace_for_skill_path;
    use codex_exec_server::ExecutorPathRef;
    use codex_exec_server::LOCAL_FS;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn uses_manifest_name() {
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
            plugin_namespace_for_skill_path(&ExecutorPathRef::new(
                LOCAL_FS.as_ref(),
                skill_path.abs(),
            ))
            .await,
            Some("sample".to_string())
        );
    }
}
