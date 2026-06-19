//! Resolve plugin namespace from skill file paths by walking ancestors for `plugin.json`.

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemOperation;
use codex_exec_server::FileSystemOperationOutput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

/// Ordered plugin manifest paths recognized beneath a plugin root.
pub const DISCOVERABLE_PLUGIN_MANIFEST_PATHS: &[&str] =
    &[".codex-plugin/plugin.json", ".claude-plugin/plugin.json"];

pub fn find_plugin_manifest_path(plugin_root: &Path) -> Option<PathBuf> {
    DISCOVERABLE_PLUGIN_MANIFEST_PATHS
        .iter()
        .map(|relative_path| plugin_root.join(relative_path))
        .find(|manifest_path| manifest_path.is_file())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestName {
    #[serde(default)]
    name: String,
}

fn plugin_manifest_name(plugin_root: &AbsolutePathBuf, contents: &[u8]) -> Option<String> {
    let RawPluginManifestName { name: raw_name } = serde_json::from_slice(contents).ok()?;
    Some(
        plugin_root
            .file_name()
            .and_then(|entry| entry.to_str())
            .filter(|_| raw_name.trim().is_empty())
            .unwrap_or(raw_name.as_str())
            .to_string(),
    )
}

/// Resolves plugin namespaces while caching and batching ancestor manifest probes.
#[derive(Default)]
pub struct PluginNamespaceResolver {
    manifests_by_root: HashMap<AbsolutePathBuf, Option<String>>,
}

impl PluginNamespaceResolver {
    pub async fn prime(&mut self, fs: &dyn ExecutorFileSystem, paths: &[AbsolutePathBuf]) {
        let mut seen = HashSet::new();
        let unresolved_roots = paths
            .iter()
            .flat_map(AbsolutePathBuf::ancestors)
            .filter(|ancestor| !self.manifests_by_root.contains_key(ancestor))
            .filter(|ancestor| seen.insert(ancestor.clone()))
            .collect::<Vec<_>>();
        let operations = unresolved_roots
            .iter()
            .flat_map(|root| {
                DISCOVERABLE_PLUGIN_MANIFEST_PATHS
                    .iter()
                    .map(|relative_path| FileSystemOperation::ReadFile {
                        path: PathUri::from_abs_path(&root.join(relative_path)),
                    })
            })
            .collect();
        let mut results = match fs.execute_batch(operations, /*sandbox*/ None).await {
            Ok(results) => results.into_iter(),
            Err(_) => Vec::new().into_iter(),
        };
        for root in unresolved_roots {
            let mut name = None;
            for _ in DISCOVERABLE_PLUGIN_MANIFEST_PATHS {
                if let Some(Ok(FileSystemOperationOutput::ReadFile(contents))) = results.next()
                    && name.is_none()
                {
                    name = plugin_manifest_name(&root, &contents);
                }
            }
            self.manifests_by_root.insert(root, name);
        }
    }

    pub async fn resolve(
        &mut self,
        fs: &dyn ExecutorFileSystem,
        path: &AbsolutePathBuf,
    ) -> Option<String> {
        self.prime(fs, std::slice::from_ref(path)).await;

        path.ancestors()
            .find_map(|ancestor| self.manifests_by_root.get(&ancestor).cloned().flatten())
    }
}

/// Returns the plugin manifest `name` for the nearest ancestor of `path` that contains a valid
/// plugin manifest (same `name` rules as full manifest loading in codex-core).
pub async fn plugin_namespace_for_skill_path(
    fs: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
) -> Option<String> {
    PluginNamespaceResolver::default().resolve(fs, path).await
}

#[cfg(test)]
mod tests {
    use super::find_plugin_manifest_path;
    use super::plugin_namespace_for_skill_path;
    use codex_exec_server::LOCAL_FS;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use std::fs;
    use tempfile::tempdir;

    const ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".claude-plugin/plugin.json";

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
            plugin_namespace_for_skill_path(LOCAL_FS.as_ref(), &skill_path.abs()).await,
            Some("sample".to_string())
        );
    }

    #[tokio::test]
    async fn uses_name_from_alternate_discoverable_manifest_path() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("plugins/sample");
        let skill_path = plugin_root.join("skills/search/SKILL.md");
        let manifest_path = plugin_root.join(ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH);

        fs::create_dir_all(skill_path.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
            .expect("mkdir manifest");
        fs::write(&manifest_path, r#"{"name":"sample"}"#).expect("write manifest");
        fs::write(&skill_path, "---\ndescription: search\n---\n").expect("write skill");

        assert_eq!(
            plugin_namespace_for_skill_path(LOCAL_FS.as_ref(), &skill_path.abs()).await,
            Some("sample".to_string())
        );
        assert_eq!(find_plugin_manifest_path(&plugin_root), Some(manifest_path));
    }
}
