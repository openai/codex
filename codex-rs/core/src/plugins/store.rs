use super::load_plugin_manifest;
use super::manifest::PLUGIN_MANIFEST_PATH;
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

    pub fn plugin_base_root(&self, plugin_id: &PluginId) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(
            self.root
                .as_path()
                .join(&plugin_id.marketplace_name)
                .join(&plugin_id.plugin_name),
        )
        .unwrap_or_else(|err| panic!("plugin cache path should resolve to an absolute path: {err}"))
    }

    pub fn plugin_root(&self, plugin_id: &PluginId, plugin_version: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(
            self.plugin_base_root(plugin_id)
                .as_path()
                .join(plugin_version),
        )
        .unwrap_or_else(|err| panic!("plugin cache path should resolve to an absolute path: {err}"))
    }

    pub fn active_plugin_version(&self, plugin_id: &PluginId) -> Option<String> {
        let mut discovered_versions = fs::read_dir(self.plugin_base_root(plugin_id).as_path())
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry.file_type().ok().filter(std::fs::FileType::is_dir)?;
                entry.file_name().into_string().ok()
            })
            .filter(|version| validate_plugin_segment(version, "plugin version").is_ok())
            .collect::<Vec<_>>();
        discovered_versions.sort_unstable();
        if discovered_versions.is_empty() {
            None
        } else if discovered_versions
            .iter()
            .any(|version| version == DEFAULT_PLUGIN_VERSION)
        {
            Some(DEFAULT_PLUGIN_VERSION.to_string())
        } else {
            discovered_versions.pop()
        }
    }

    pub fn active_plugin_root(&self, plugin_id: &PluginId) -> Option<AbsolutePathBuf> {
        self.active_plugin_version(plugin_id)
            .map(|plugin_version| self.plugin_root(plugin_id, &plugin_version))
    }

    pub fn is_installed(&self, plugin_id: &PluginId) -> bool {
        self.active_plugin_version(plugin_id).is_some()
    }

    pub fn install(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        self.install_with_version(source_path, plugin_id, DEFAULT_PLUGIN_VERSION.to_string())
    }

    pub fn install_with_version(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
        plugin_version: String,
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
        validate_plugin_segment(&plugin_version, "plugin version")
            .map_err(PluginStoreError::Invalid)?;
        let installed_path = self.plugin_root(&plugin_id, &plugin_version);
        replace_plugin_root_atomically(
            source_path.as_path(),
            self.plugin_base_root(&plugin_id).as_path(),
            &plugin_version,
        )?;

        Ok(PluginInstallResult {
            plugin_id,
            plugin_version,
            installed_path,
        })
    }

    pub fn uninstall(&self, plugin_id: &PluginId) -> Result<(), PluginStoreError> {
        remove_existing_target(self.plugin_base_root(plugin_id).as_path())
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

    let plugin_name = manifest.name;
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

fn replace_plugin_root_atomically(
    source: &Path,
    target_root: &Path,
    plugin_version: &str,
) -> Result<(), PluginStoreError> {
    let Some(parent) = target_root.parent() else {
        return Err(PluginStoreError::Invalid(format!(
            "plugin cache path has no parent: {}",
            target_root.display()
        )));
    };

    fs::create_dir_all(parent)
        .map_err(|err| PluginStoreError::io("failed to create plugin cache directory", err))?;

    let Some(plugin_dir_name) = target_root.file_name() else {
        return Err(PluginStoreError::Invalid(format!(
            "plugin cache path has no directory name: {}",
            target_root.display()
        )));
    };
    let staged_dir = tempfile::Builder::new()
        .prefix("plugin-install-")
        .tempdir_in(parent)
        .map_err(|err| {
            PluginStoreError::io("failed to create temporary plugin cache directory", err)
        })?;
    let staged_root = staged_dir.path().join(plugin_dir_name);
    let staged_version_root = staged_root.join(plugin_version);
    copy_dir_recursive(source, &staged_version_root)?;

    if target_root.exists() {
        let backup_dir = tempfile::Builder::new()
            .prefix("plugin-backup-")
            .tempdir_in(parent)
            .map_err(|err| {
                PluginStoreError::io("failed to create plugin cache backup directory", err)
            })?;
        let backup_root = backup_dir.path().join(plugin_dir_name);
        fs::rename(target_root, &backup_root)
            .map_err(|err| PluginStoreError::io("failed to back up plugin cache entry", err))?;

        if let Err(err) = fs::rename(&staged_root, target_root) {
            let rollback_result = fs::rename(&backup_root, target_root);
            return match rollback_result {
                Ok(()) => Err(PluginStoreError::io(
                    "failed to activate updated plugin cache entry",
                    err,
                )),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join(plugin_dir_name);
                    Err(PluginStoreError::Invalid(format!(
                        "failed to activate updated plugin cache entry at {}: {err}; failed to restore previous cache entry (left at {}): {rollback_err}",
                        target_root.display(),
                        backup_path.display()
                    )))
                }
            };
        }
    } else {
        fs::rename(&staged_root, target_root)
            .map_err(|err| PluginStoreError::io("failed to activate plugin cache entry", err))?;
    }

    Ok(())
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
#[path = "store_tests.rs"]
mod tests;
