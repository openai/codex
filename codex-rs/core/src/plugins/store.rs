use super::load_plugin_manifest;
use super::manifest::PLUGIN_MANIFEST_PATH;
use super::plugin_manifest_name;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const DEFAULT_PLUGIN_VERSION: &str = "local";
pub(crate) const PLUGINS_CACHE_DIR: &str = "plugins/cache";

#[derive(Debug, thiserror::Error)]
pub enum PluginIdError {
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginId {
    pub plugin_name: String,
    pub marketplace_name: String,
}

impl PluginId {
    pub fn new(plugin_name: String, marketplace_name: String) -> Result<Self, PluginIdError> {
        validate_plugin_segment(&plugin_name, "plugin name").map_err(PluginIdError::Invalid)?;
        validate_plugin_segment(&marketplace_name, "marketplace name")
            .map_err(PluginIdError::Invalid)?;
        Ok(Self {
            plugin_name,
            marketplace_name,
        })
    }

    pub fn parse(plugin_key: &str) -> Result<Self, PluginIdError> {
        let Some((plugin_name, marketplace_name)) = plugin_key.rsplit_once('@') else {
            return Err(PluginIdError::Invalid(format!(
                "invalid plugin key `{plugin_key}`; expected <plugin>@<marketplace>"
            )));
        };
        if plugin_name.is_empty() || marketplace_name.is_empty() {
            return Err(PluginIdError::Invalid(format!(
                "invalid plugin key `{plugin_key}`; expected <plugin>@<marketplace>"
            )));
        }

        Self::new(plugin_name.to_string(), marketplace_name.to_string()).map_err(|err| match err {
            PluginIdError::Invalid(message) => {
                PluginIdError::Invalid(format!("{message} in `{plugin_key}`"))
            }
        })
    }

    pub fn as_key(&self) -> String {
        format!("{}@{}", self.plugin_name, self.marketplace_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallResult {
    pub plugin_id: PluginId,
    pub plugin_version: String,
    pub installed_path: AbsolutePathBuf,
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

    pub fn plugin_root(&self, plugin_id: &PluginId, plugin_version: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(
            self.root
                .as_path()
                .join(&plugin_id.marketplace_name)
                .join(&plugin_id.plugin_name)
                .join(plugin_version),
        )
        .unwrap_or_else(|err| panic!("plugin cache path should resolve to an absolute path: {err}"))
    }

    pub fn is_installed(&self, plugin_id: &PluginId) -> bool {
        self.plugin_root(plugin_id, DEFAULT_PLUGIN_VERSION)
            .as_path()
            .is_dir()
    }

    pub fn install(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        if !source_path.as_path().is_dir() {
            return Err(PluginStoreError::Invalid(format!(
                "plugin source path is not a directory: {}",
                source_path.display()
            )));
        }

        let plugin_name = plugin_name_for_source(source_path.as_path())?;
        if plugin_name != plugin_id.plugin_name {
            return Err(PluginStoreError::Invalid(format!(
                "plugin manifest name `{plugin_name}` does not match marketplace plugin name `{}`",
                plugin_id.plugin_name
            )));
        }
        let plugin_version = DEFAULT_PLUGIN_VERSION.to_string();
        let installed_path = self.plugin_root(&plugin_id, &plugin_version);

        if let Some(parent) = installed_path.parent() {
            fs::create_dir_all(parent.as_path()).map_err(|err| {
                PluginStoreError::io("failed to create plugin cache directory", err)
            })?;
        }

        remove_existing_target(installed_path.as_path())?;
        copy_dir_recursive(source_path.as_path(), installed_path.as_path())?;

        Ok(PluginInstallResult {
            plugin_id,
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
    Invalid(String),
}

impl PluginStoreError {
    fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}

fn plugin_name_for_source(source_path: &Path) -> Result<String, PluginStoreError> {
    let manifest_path = source_path.join(PLUGIN_MANIFEST_PATH);
    if !manifest_path.is_file() {
        return Err(PluginStoreError::Invalid(format!(
            "missing plugin manifest: {}",
            manifest_path.display()
        )));
    }

    let manifest = load_plugin_manifest(source_path).ok_or_else(|| {
        PluginStoreError::Invalid(format!(
            "missing or invalid plugin manifest: {}",
            manifest_path.display()
        ))
    })?;

    let plugin_name = plugin_manifest_name(&manifest, source_path);
    validate_plugin_segment(&plugin_name, "plugin name")
        .map_err(PluginStoreError::Invalid)
        .map(|_| plugin_name)
}

fn validate_plugin_segment(segment: &str, kind: &str) -> Result<(), String> {
    if segment.is_empty() {
        return Err(format!("invalid {kind}: must not be empty"));
    }
    if !segment
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(format!(
            "invalid {kind}: only ASCII letters, digits, `_`, and `-` are allowed"
        ));
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
        let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

        let result = PluginStore::new(tmp.path().to_path_buf())
            .install(
                AbsolutePathBuf::try_from(tmp.path().join("sample-plugin")).unwrap(),
                plugin_id.clone(),
            )
            .unwrap();

        let installed_path = tmp.path().join("plugins/cache/debug/sample-plugin/local");
        assert_eq!(
            result,
            PluginInstallResult {
                plugin_id,
                plugin_version: "local".to_string(),
                installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
            }
        );
        assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
        assert!(installed_path.join("skills/SKILL.md").is_file());
    }

    #[test]
    fn install_uses_manifest_name_for_destination_and_key() {
        let tmp = tempdir().unwrap();
        write_plugin(tmp.path(), "source-dir", "manifest-name");
        let plugin_id = PluginId::new("manifest-name".to_string(), "market".to_string()).unwrap();

        let result = PluginStore::new(tmp.path().to_path_buf())
            .install(
                AbsolutePathBuf::try_from(tmp.path().join("source-dir")).unwrap(),
                plugin_id.clone(),
            )
            .unwrap();

        assert_eq!(
            result,
            PluginInstallResult {
                plugin_id,
                plugin_version: "local".to_string(),
                installed_path: AbsolutePathBuf::try_from(
                    tmp.path().join("plugins/cache/market/manifest-name/local"),
                )
                .unwrap(),
            }
        );
    }

    #[test]
    fn plugin_root_derives_path_from_key_and_version() {
        let tmp = tempdir().unwrap();
        let store = PluginStore::new(tmp.path().to_path_buf());
        let plugin_id = PluginId::new("sample".to_string(), "debug".to_string()).unwrap();

        assert_eq!(
            store.plugin_root(&plugin_id, "local").as_path(),
            tmp.path().join("plugins/cache/debug/sample/local")
        );
    }

    #[test]
    fn plugin_root_rejects_path_separators_in_key_segments() {
        let err = PluginId::parse("../../etc@debug").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid plugin name: only ASCII letters, digits, `_`, and `-` are allowed in `../../etc@debug`"
        );

        let err = PluginId::parse("sample@../../etc").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed in `sample@../../etc`"
        );
    }

    #[test]
    fn install_rejects_manifest_names_with_path_separators() {
        let tmp = tempdir().unwrap();
        write_plugin(tmp.path(), "source-dir", "../../etc");

        let err = PluginStore::new(tmp.path().to_path_buf())
            .install(
                AbsolutePathBuf::try_from(tmp.path().join("source-dir")).unwrap(),
                PluginId::new("source-dir".to_string(), "debug".to_string()).unwrap(),
            )
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "invalid plugin name: only ASCII letters, digits, `_`, and `-` are allowed"
        );
    }

    #[test]
    fn install_rejects_marketplace_names_with_path_separators() {
        let err = PluginId::new("sample-plugin".to_string(), "../../etc".to_string()).unwrap_err();

        assert_eq!(
            err.to_string(),
            "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed"
        );
    }

    #[test]
    fn install_rejects_manifest_names_that_do_not_match_marketplace_plugin_name() {
        let tmp = tempdir().unwrap();
        write_plugin(tmp.path(), "source-dir", "manifest-name");

        let err = PluginStore::new(tmp.path().to_path_buf())
            .install(
                AbsolutePathBuf::try_from(tmp.path().join("source-dir")).unwrap(),
                PluginId::new("different-name".to_string(), "debug".to_string()).unwrap(),
            )
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "plugin manifest name `manifest-name` does not match marketplace plugin name `different-name`"
        );
    }
}
