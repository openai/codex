//! Resolve plugin namespace from skill file paths by walking ancestors for `plugin.json`.

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorRpcBatchCall;
use codex_exec_server::ExecutorRpcBatchResult;
use codex_exec_server::FS_GET_METADATA_METHOD;
use codex_exec_server::FS_READ_FILE_METHOD;
use codex_exec_server::FsGetMetadataParams;
use codex_exec_server::FsGetMetadataResponse;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::FsReadFileResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
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
pub struct PluginNamespaceResolver {
    manifests_by_root: HashMap<AbsolutePathBuf, String>,
}

impl PluginNamespaceResolver {
    /// Loads plugin manifest names for every ancestor of `paths`.
    pub async fn load(fs: &dyn ExecutorFileSystem, paths: &[AbsolutePathBuf]) -> Self {
        let mut seen = HashSet::new();
        let roots = paths
            .iter()
            .flat_map(AbsolutePathBuf::ancestors)
            .filter(|ancestor| seen.insert(ancestor.clone()))
            .collect::<Vec<_>>();
        let calls = roots
            .iter()
            .flat_map(|root| {
                DISCOVERABLE_PLUGIN_MANIFEST_PATHS
                    .iter()
                    .flat_map(|relative_path| {
                        let path = PathUri::from_abs_path(&root.join(relative_path));
                        [
                            rpc_batch_call(
                                FS_GET_METADATA_METHOD,
                                FsGetMetadataParams {
                                    path: path.clone(),
                                    sandbox: None,
                                },
                            ),
                            rpc_batch_call(
                                FS_READ_FILE_METHOD,
                                FsReadFileParams {
                                    path,
                                    sandbox: None,
                                },
                            ),
                        ]
                    })
            })
            .collect::<io::Result<Vec<_>>>();
        let mut results = fs
            .execute_rpc_batch(calls.unwrap_or_default())
            .await
            .unwrap_or_default()
            .into_iter();
        let manifests_by_root = roots
            .into_iter()
            .filter_map(|root| {
                let mut name = None;
                let mut selected = false;
                for _ in DISCOVERABLE_PLUGIN_MANIFEST_PATHS {
                    let metadata_result = results.next();
                    let contents_result = results.next();
                    let is_file = decode_rpc_batch_result::<FsGetMetadataResponse>(
                        metadata_result,
                        "stat plugin manifest",
                    )
                    .is_ok_and(|metadata| metadata.is_file);
                    if !selected && is_file {
                        selected = true;
                        if let Ok(response) = decode_rpc_batch_result::<FsReadFileResponse>(
                            contents_result,
                            "read plugin manifest",
                        ) && let Ok(contents) = response.into_bytes()
                        {
                            name = plugin_manifest_name(&root, &contents);
                        }
                    }
                }
                name.map(|name| (root, name))
            })
            .collect();

        Self { manifests_by_root }
    }

    /// Returns the nearest preloaded plugin namespace for `path`.
    pub fn resolve(&self, path: &AbsolutePathBuf) -> Option<&str> {
        path.ancestors()
            .find_map(|ancestor| self.manifests_by_root.get(&ancestor).map(String::as_str))
    }
}

fn rpc_batch_call<P: serde::Serialize>(
    method: &str,
    params: P,
) -> io::Result<ExecutorRpcBatchCall> {
    Ok(ExecutorRpcBatchCall {
        method: method.to_string(),
        params: serde_json::to_value(params).map_err(io::Error::other)?,
    })
}

fn decode_rpc_batch_result<T: serde::de::DeserializeOwned>(
    result: Option<ExecutorRpcBatchResult>,
    operation: &str,
) -> io::Result<T> {
    match result {
        Some(Ok(value)) => serde_json::from_value(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Some(Err(error)) => Err(error),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("filesystem returned no result for {operation}"),
        )),
    }
}

/// Returns the plugin manifest `name` for the nearest ancestor of `path` that contains a valid
/// plugin manifest (same `name` rules as full manifest loading in codex-core).
pub async fn plugin_namespace_for_skill_path(
    fs: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
) -> Option<String> {
    PluginNamespaceResolver::load(fs, std::slice::from_ref(path))
        .await
        .resolve(path)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::find_plugin_manifest_path;
    use super::plugin_namespace_for_skill_path;
    use codex_exec_server::LOCAL_FS;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use std::fs;
    use tempfile::tempdir;

    const PRIMARY_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".codex-plugin/plugin.json";
    const ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".claude-plugin/plugin.json";

    #[tokio::test]
    async fn uses_manifest_name() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("plugins/sample");
        let skill_path = plugin_root.join("skills/search/SKILL.md");

        fs::create_dir_all(skill_path.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("mkdir manifest");
        fs::write(
            plugin_root.join(PRIMARY_PLUGIN_MANIFEST_RELATIVE_PATH),
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

    #[tokio::test]
    async fn invalid_primary_manifest_does_not_fall_back_to_alternate() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("plugins/sample");
        let skill_path = plugin_root.join("skills/search/SKILL.md");
        let primary_manifest_path = plugin_root.join(PRIMARY_PLUGIN_MANIFEST_RELATIVE_PATH);
        let alternate_manifest_path = plugin_root.join(ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH);

        fs::create_dir_all(skill_path.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(primary_manifest_path.parent().expect("manifest parent"))
            .expect("mkdir primary manifest");
        fs::create_dir_all(alternate_manifest_path.parent().expect("manifest parent"))
            .expect("mkdir alternate manifest");
        fs::write(&primary_manifest_path, "invalid json").expect("write primary manifest");
        fs::write(&alternate_manifest_path, r#"{"name":"sample"}"#)
            .expect("write alternate manifest");
        fs::write(&skill_path, "---\ndescription: search\n---\n").expect("write skill");

        assert_eq!(
            plugin_namespace_for_skill_path(LOCAL_FS.as_ref(), &skill_path.abs()).await,
            None
        );
        assert_eq!(
            find_plugin_manifest_path(&plugin_root),
            Some(primary_manifest_path)
        );
    }
}
