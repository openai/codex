use super::load_plugin_manifest;
use super::manifest::PLUGIN_MANIFEST_PATH;
use super::plugin_manifest_name;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const DEFAULT_MARKETPLACE_NAME: &str = "debug";
pub(crate) const DEFAULT_PLUGIN_VERSION: &str = "local";
pub(crate) const PLUGINS_CACHE_DIR: &str = "plugins/cache";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallRequest {
    pub source_path: PathBuf,
    pub marketplace_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallResult {
    pub plugin_key: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub installed_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PluginStore {
    root: AbsolutePathBuf,
}

impl PluginStore {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            root: AbsolutePathBuf::try_from(codex_home.join(PLUGINS_CACHE_DIR))
                .unwrap_or_else(|err| panic!("plugin cache root should be absolute: {err}")),
        }
    }

    pub fn root(&self) -> &AbsolutePathBuf {
        &self.root
    }

    pub fn plugin_key(plugin_name: &str, marketplace_name: &str) -> String {
        format!("{plugin_name}@{marketplace_name}")
    }

    pub fn plugin_root(
        &self,
        plugin_key: &str,
        plugin_version: &str,
    ) -> Result<AbsolutePathBuf, PluginStoreError> {
        let (plugin_name, marketplace_name) = parse_plugin_key(plugin_key)?;
        AbsolutePathBuf::try_from(
            self.root
                .as_path()
                .join(marketplace_name)
                .join(plugin_name)
                .join(plugin_version),
        )
        .map_err(|err| {
            PluginStoreError::InvalidPluginPath(format!(
                "plugin cache path should resolve to an absolute path: {err}"
            ))
        })
    }

    pub fn install(
        &self,
        request: PluginInstallRequest,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        let source_path = request.source_path;
        if !source_path.is_dir() {
            return Err(PluginStoreError::InvalidPlugin(format!(
                "plugin source path is not a directory: {}",
                source_path.display()
            )));
        }

        let plugin_name = plugin_name_for_source(&source_path)?;
        let marketplace_name = request
            .marketplace_name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MARKETPLACE_NAME.to_string());
        let plugin_version = DEFAULT_PLUGIN_VERSION.to_string();
        let installed_path = self
            .root
            .as_path()
            .join(&marketplace_name)
            .join(&plugin_name)
            .join(&plugin_version);

        if let Some(parent) = installed_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PluginStoreError::io("failed to create plugin cache directory", err)
            })?;
        }

        remove_existing_target(&installed_path)?;
        copy_dir_recursive(&source_path, &installed_path)?;

        Ok(PluginInstallResult {
            plugin_key: Self::plugin_key(&plugin_name, &marketplace_name),
            plugin_name,
            plugin_version,
            installed_path,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginStoreError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("{0}")]
    InvalidPlugin(String),

    #[error("{0}")]
    InvalidPluginPath(String),

    #[error("{0}")]
    InvalidPluginKey(String),
}

impl PluginStoreError {
    fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}

fn plugin_name_for_source(source_path: &Path) -> Result<String, PluginStoreError> {
    let manifest_path = source_path.join(PLUGIN_MANIFEST_PATH);
    if !manifest_path.is_file() {
        return Err(PluginStoreError::InvalidPlugin(format!(
            "missing plugin manifest: {}",
            manifest_path.display()
        )));
    }

    let manifest = load_plugin_manifest(source_path).ok_or_else(|| {
        PluginStoreError::InvalidPlugin(format!(
            "missing or invalid plugin manifest: {}",
            manifest_path.display()
        ))
    })?;

    let plugin_name = plugin_manifest_name(&manifest, source_path);
    validate_plugin_segment(&plugin_name, "plugin name")
        .map_err(PluginStoreError::InvalidPlugin)
        .map(|_| plugin_name)
}

fn parse_plugin_key(plugin_key: &str) -> Result<(&str, &str), PluginStoreError> {
    let Some((plugin_name, marketplace_name)) = plugin_key.rsplit_once('@') else {
        return Err(PluginStoreError::InvalidPluginKey(format!(
            "invalid plugin key `{plugin_key}`; expected <plugin>@<marketplace>"
        )));
    };
    if plugin_name.is_empty() || marketplace_name.is_empty() {
        return Err(PluginStoreError::InvalidPluginKey(format!(
            "invalid plugin key `{plugin_key}`; expected <plugin>@<marketplace>"
        )));
    }
    validate_plugin_key_segment(plugin_name, "plugin name", plugin_key)?;
    validate_plugin_key_segment(marketplace_name, "marketplace name", plugin_key)?;
    Ok((plugin_name, marketplace_name))
}

fn validate_plugin_key_segment(
    segment: &str,
    kind: &str,
    plugin_key: &str,
) -> Result<(), PluginStoreError> {
    validate_plugin_segment(segment, kind)
        .map_err(|err| PluginStoreError::InvalidPluginKey(format!("{err} in `{plugin_key}`")))
}

fn validate_plugin_segment(segment: &str, kind: &str) -> Result<(), String> {
    if segment.trim().is_empty() {
        return Err(format!("invalid {kind}: must not be empty"));
    }
    if segment == "." || segment == ".." {
        return Err(format!("invalid {kind}: `.` and `..` are not allowed"));
    }
    if segment.contains(['/', '\\']) {
        return Err(format!("invalid {kind}: path separators are not allowed"));
    }
    Ok(())
}

fn remove_existing_target(path: &Path) -> Result<(), PluginStoreError> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|err| {
            PluginStoreError::io("failed to remove existing plugin cache entry", err)
        })
    } else {
        fs::remove_file(path).map_err(|err| {
            PluginStoreError::io("failed to remove existing plugin cache entry", err)
        })
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), PluginStoreError> {
    fs::create_dir_all(target)
        .map_err(|err| PluginStoreError::io("failed to create plugin target directory", err))?;

    for entry in fs::read_dir(source)
        .map_err(|err| PluginStoreError::io("failed to read plugin source directory", err))?
    {
        let entry =
            entry.map_err(|err| PluginStoreError::io("failed to enumerate plugin source", err))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|err| PluginStoreError::io("failed to inspect plugin source entry", err))?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path)
                .map_err(|err| PluginStoreError::io("failed to copy plugin file", err))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn write_plugin(root: &Path, dir_name: &str, manifest_name: &str) {
        let plugin_root = root.join(dir_name);
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::create_dir_all(plugin_root.join("skills")).unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(r#"{{"name":"{manifest_name}"}}"#),
        )
        .unwrap();
        fs::write(plugin_root.join("skills/SKILL.md"), "skill").unwrap();
        fs::write(plugin_root.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();
    }

    #[test]
    fn install_copies_plugin_into_default_marketplace() {
        let tmp = tempdir().unwrap();
        write_plugin(tmp.path(), "sample-plugin", "sample-plugin");

        let result = PluginStore::new(tmp.path().to_path_buf())
            .install(PluginInstallRequest {
                source_path: tmp.path().join("sample-plugin"),
                marketplace_name: None,
            })
            .unwrap();

        let installed_path = tmp.path().join("plugins/cache/debug/sample-plugin/local");
        assert_eq!(
            result,
            PluginInstallResult {
                plugin_key: "sample-plugin@debug".to_string(),
                plugin_name: "sample-plugin".to_string(),
                plugin_version: "local".to_string(),
                installed_path: installed_path.clone(),
            }
        );
        assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
        assert!(installed_path.join("skills/SKILL.md").is_file());
    }

    #[test]
    fn install_uses_manifest_name_for_destination_and_key() {
        let tmp = tempdir().unwrap();
        write_plugin(tmp.path(), "source-dir", "manifest-name");

        let result = PluginStore::new(tmp.path().to_path_buf())
            .install(PluginInstallRequest {
                source_path: tmp.path().join("source-dir"),
                marketplace_name: Some("market".to_string()),
            })
            .unwrap();

        assert_eq!(
            result,
            PluginInstallResult {
                plugin_key: "manifest-name@market".to_string(),
                plugin_name: "manifest-name".to_string(),
                plugin_version: "local".to_string(),
                installed_path: tmp.path().join("plugins/cache/market/manifest-name/local"),
            }
        );
    }

    #[test]
    fn plugin_root_derives_path_from_key_and_version() {
        let tmp = tempdir().unwrap();
        let store = PluginStore::new(tmp.path().to_path_buf());

        assert_eq!(
            store
                .plugin_root("sample@debug", "local")
                .unwrap()
                .as_path(),
            tmp.path().join("plugins/cache/debug/sample/local")
        );
    }

    #[test]
    fn plugin_root_rejects_path_separators_in_key_segments() {
        let tmp = tempdir().unwrap();
        let store = PluginStore::new(tmp.path().to_path_buf());

        let err = store.plugin_root("../../etc@debug", "local").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid plugin name: path separators are not allowed in `../../etc@debug`"
        );

        let err = store.plugin_root("sample@../../etc", "local").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid marketplace name: path separators are not allowed in `sample@../../etc`"
        );
    }

    #[test]
    fn install_rejects_manifest_names_with_path_separators() {
        let tmp = tempdir().unwrap();
        write_plugin(tmp.path(), "source-dir", "../../etc");

        let err = PluginStore::new(tmp.path().to_path_buf())
            .install(PluginInstallRequest {
                source_path: tmp.path().join("source-dir"),
                marketplace_name: None,
            })
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "invalid plugin name: path separators are not allowed"
        );
    }
}
