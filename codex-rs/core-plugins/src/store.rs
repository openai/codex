use crate::manifest::PluginManifest;
use crate::manifest::load_plugin_manifest;
use codex_plugin::PluginId;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::find_plugin_manifest_path;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tracing::warn;

pub const DEFAULT_PLUGIN_VERSION: &str = "local";
pub const PLUGINS_CACHE_DIR: &str = "plugins/cache";
pub const PLUGINS_DATA_DIR: &str = "plugins/data";
const ACTIVE_PLUGIN_VERSION_FILE: &str = ".active-version";
const ACTIVE_PLUGIN_VERSION_LOCK_FILE: &str = ".active-version.lock";
const ACTIVE_PLUGIN_VERSION_LOCK_RETRIES: usize = 20;
const ACTIVE_PLUGIN_VERSION_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallResult {
    pub plugin_id: PluginId,
    pub plugin_version: String,
    pub installed_path: AbsolutePathBuf,
}

#[derive(Debug, Clone)]
pub struct PluginStore {
    root: AbsolutePathBuf,
    data_root: AbsolutePathBuf,
}

impl PluginStore {
    pub fn new(codex_home: PathBuf) -> Self {
        Self::try_new(codex_home)
            .unwrap_or_else(|err| panic!("plugin cache root should be absolute: {err}"))
    }

    pub fn try_new(codex_home: PathBuf) -> Result<Self, PluginStoreError> {
        let root = AbsolutePathBuf::from_absolute_path_checked(codex_home.join(PLUGINS_CACHE_DIR))
            .map_err(|err| PluginStoreError::io("failed to resolve plugin cache root", err))?;
        let data_root =
            AbsolutePathBuf::from_absolute_path_checked(codex_home.join(PLUGINS_DATA_DIR))
                .map_err(|err| PluginStoreError::io("failed to resolve plugin data root", err))?;

        Ok(Self { root, data_root })
    }

    pub fn root(&self) -> &AbsolutePathBuf {
        &self.root
    }

    pub fn plugin_base_root(&self, plugin_id: &PluginId) -> AbsolutePathBuf {
        self.root
            .join(&plugin_id.marketplace_name)
            .join(&plugin_id.plugin_name)
    }

    pub fn plugin_root(&self, plugin_id: &PluginId, plugin_version: &str) -> AbsolutePathBuf {
        self.plugin_base_root(plugin_id).join(plugin_version)
    }

    pub fn plugin_data_root(&self, plugin_id: &PluginId) -> AbsolutePathBuf {
        self.data_root.join(format!(
            "{}-{}",
            plugin_id.plugin_name, plugin_id.marketplace_name
        ))
    }

    pub fn active_plugin_version(&self, plugin_id: &PluginId) -> Option<String> {
        let plugin_base_root = self.plugin_base_root(plugin_id);
        match active_plugin_version_marker(plugin_base_root.as_path()) {
            Ok(Some(active_version)) => return Some(active_version),
            Ok(None) => {}
            Err(err) => {
                warn!(
                    plugin = %plugin_id.as_key(),
                    path = %plugin_base_root.display(),
                    error = %err,
                    "failed to read active plugin version marker"
                );
            }
        }

        legacy_active_plugin_version(plugin_base_root.as_path())
    }

    pub fn active_plugin_root(&self, plugin_id: &PluginId) -> Option<AbsolutePathBuf> {
        self.active_plugin_version(plugin_id)
            .map(|plugin_version| self.plugin_root(plugin_id, &plugin_version))
    }

    pub fn is_installed(&self, plugin_id: &PluginId) -> bool {
        self.active_plugin_version(plugin_id).is_some()
    }

    pub fn cleanup_inactive_versions(&self, plugin_id: &PluginId) {
        cleanup_inactive_versions_with_remover(
            plugin_id,
            self.plugin_base_root(plugin_id).as_path(),
            |path| fs::remove_dir_all(path),
        );
    }

    pub fn install(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        let plugin_version = plugin_version_for_source(source_path.as_path())?;
        self.install_with_version(source_path, plugin_id, plugin_version)
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
                "plugin.json name `{plugin_name}` does not match marketplace plugin name `{}`",
                plugin_id.plugin_name
            )));
        }
        validate_plugin_version_segment(&plugin_version).map_err(PluginStoreError::Invalid)?;
        let plugin_base_root = self.plugin_base_root(&plugin_id);
        let installed_path = plugin_base_root.join(&plugin_version);

        if plugin_version == DEFAULT_PLUGIN_VERSION {
            replace_plugin_root_atomically(
                source_path.as_path(),
                plugin_base_root.as_path(),
                &plugin_version,
            )?;
        } else {
            self.cleanup_inactive_versions(&plugin_id);
            if !installed_path.as_path().is_dir() {
                install_plugin_version_into_existing_base(
                    source_path.as_path(),
                    plugin_base_root.as_path(),
                    &plugin_version,
                )?;
            }
            write_active_plugin_version_marker(plugin_base_root.as_path(), &plugin_version)?;
        }

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

pub fn plugin_version_for_source(source_path: &Path) -> Result<String, PluginStoreError> {
    let plugin_version = plugin_manifest_version_for_source(source_path)?
        .unwrap_or_else(|| DEFAULT_PLUGIN_VERSION.to_string());
    validate_plugin_version_segment(&plugin_version).map_err(PluginStoreError::Invalid)?;
    Ok(plugin_version)
}

fn active_plugin_version_marker(plugin_base_root: &Path) -> io::Result<Option<String>> {
    if !plugin_base_root.is_dir() {
        return Ok(None);
    }

    let _lock_file =
        lock_active_plugin_version_marker(plugin_base_root, ActivePluginVersionLockKind::Shared)?;
    let marker_path = plugin_base_root.join(ACTIVE_PLUGIN_VERSION_FILE);
    let version = match fs::read_to_string(&marker_path) {
        Ok(version) => version,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let version = version.trim();
    if validate_plugin_version_segment(version).is_err() {
        warn!(
            marker_path = %marker_path.display(),
            "ignoring invalid active plugin version marker"
        );
        return Ok(None);
    }

    if plugin_base_root.join(version).is_dir() {
        Ok(Some(version.to_string()))
    } else {
        warn!(
            marker_path = %marker_path.display(),
            plugin_version = version,
            "ignoring active plugin version marker for missing version directory"
        );
        Ok(None)
    }
}

fn legacy_active_plugin_version(plugin_base_root: &Path) -> Option<String> {
    let mut discovered_versions = fs::read_dir(plugin_base_root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry.file_type().ok().filter(std::fs::FileType::is_dir)?;
            entry.file_name().into_string().ok()
        })
        .filter(|version| validate_plugin_version_segment(version).is_ok())
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

pub fn validate_plugin_version_segment(plugin_version: &str) -> Result<(), String> {
    if plugin_version.is_empty() {
        return Err("invalid plugin version: must not be empty".to_string());
    }
    if matches!(plugin_version, "." | "..") {
        return Err("invalid plugin version: path traversal is not allowed".to_string());
    }
    if !plugin_version
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+'))
    {
        return Err(
            "invalid plugin version: only ASCII letters, digits, `.`, `+`, `_`, and `-` are allowed"
                .to_string(),
        );
    }
    Ok(())
}

fn plugin_manifest_for_source(source_path: &Path) -> Result<PluginManifest, PluginStoreError> {
    load_plugin_manifest(source_path)
        .ok_or_else(|| PluginStoreError::Invalid("missing or invalid plugin.json".to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestVersion {
    #[serde(default)]
    version: Option<JsonValue>,
}

fn plugin_manifest_version_for_source(
    source_path: &Path,
) -> Result<Option<String>, PluginStoreError> {
    let manifest_path = find_plugin_manifest_path(source_path)
        .ok_or_else(|| PluginStoreError::Invalid("missing plugin.json".to_string()))?;

    let contents = fs::read_to_string(&manifest_path)
        .map_err(|err| PluginStoreError::io("failed to read plugin.json", err))?;
    let manifest: RawPluginManifestVersion = serde_json::from_str(&contents)
        .map_err(|err| PluginStoreError::Invalid(format!("failed to parse plugin.json: {err}")))?;
    let Some(version) = manifest.version else {
        return Ok(None);
    };
    let Some(version) = version.as_str() else {
        return Err(PluginStoreError::Invalid(
            "invalid plugin version in plugin.json: expected string".to_string(),
        ));
    };
    let version = version.trim();
    if version.is_empty() {
        return Err(PluginStoreError::Invalid(
            "invalid plugin version in plugin.json: must not be blank".to_string(),
        ));
    }
    Ok(Some(version.to_string()))
}

fn plugin_name_for_source(source_path: &Path) -> Result<String, PluginStoreError> {
    let manifest = plugin_manifest_for_source(source_path)?;

    let plugin_name = manifest.name;
    validate_plugin_segment(&plugin_name, "plugin name")
        .map_err(PluginStoreError::Invalid)
        .map(|_| plugin_name)
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

fn write_active_plugin_version_marker(
    plugin_base_root: &Path,
    plugin_version: &str,
) -> Result<(), PluginStoreError> {
    fs::create_dir_all(plugin_base_root)
        .map_err(|err| PluginStoreError::io("failed to create plugin cache directory", err))?;
    let _lock_file =
        lock_active_plugin_version_marker(plugin_base_root, ActivePluginVersionLockKind::Exclusive)
            .map_err(|err| {
                PluginStoreError::io("failed to lock active plugin version marker", err)
            })?;

    let marker_path = plugin_base_root.join(ACTIVE_PLUGIN_VERSION_FILE);
    let mut marker_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(marker_path)
        .map_err(|err| PluginStoreError::io("failed to open active plugin version marker", err))?;
    marker_file
        .write_all(format!("{plugin_version}\n").as_bytes())
        .map_err(|err| PluginStoreError::io("failed to write active plugin version marker", err))?;
    marker_file
        .flush()
        .map_err(|err| PluginStoreError::io("failed to flush active plugin version marker", err))
}

#[derive(Clone, Copy)]
enum ActivePluginVersionLockKind {
    Shared,
    Exclusive,
}

fn lock_active_plugin_version_marker(
    plugin_base_root: &Path,
    lock_kind: ActivePluginVersionLockKind,
) -> io::Result<File> {
    let lock_path = plugin_base_root.join(ACTIVE_PLUGIN_VERSION_LOCK_FILE);
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;

    for _ in 0..ACTIVE_PLUGIN_VERSION_LOCK_RETRIES {
        let lock_result = match lock_kind {
            ActivePluginVersionLockKind::Shared => lock_file.try_lock_shared(),
            ActivePluginVersionLockKind::Exclusive => lock_file.try_lock(),
        };
        match lock_result {
            Ok(()) => return Ok(lock_file),
            Err(fs::TryLockError::WouldBlock) => {
                std::thread::sleep(ACTIVE_PLUGIN_VERSION_LOCK_RETRY_SLEEP);
            }
            Err(err) => return Err(err.into()),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        format!(
            "could not acquire active plugin version marker lock: {}",
            lock_path.display()
        ),
    ))
}

fn install_plugin_version_into_existing_base(
    source: &Path,
    plugin_base_root: &Path,
    plugin_version: &str,
) -> Result<(), PluginStoreError> {
    let Some(parent) = plugin_base_root.parent() else {
        return Err(PluginStoreError::Invalid(format!(
            "plugin cache path has no parent: {}",
            plugin_base_root.display()
        )));
    };

    fs::create_dir_all(plugin_base_root)
        .map_err(|err| PluginStoreError::io("failed to create plugin cache directory", err))?;

    let staged_dir = tempfile::Builder::new()
        .prefix("plugin-install-")
        .tempdir_in(parent)
        .map_err(|err| {
            PluginStoreError::io("failed to create temporary plugin cache directory", err)
        })?;
    let staged_version_root = staged_dir.path().join(plugin_version);
    copy_dir_recursive(source, &staged_version_root)?;

    fs::rename(&staged_version_root, plugin_base_root.join(plugin_version)).map_err(|err| {
        PluginStoreError::io("failed to activate plugin cache version entry", err)
    })?;

    Ok(())
}

fn cleanup_inactive_versions_with_remover<F>(
    plugin_id: &PluginId,
    plugin_base_root: &Path,
    mut remove_dir_all: F,
) where
    F: FnMut(&Path) -> io::Result<()>,
{
    let active_version = match active_plugin_version_marker(plugin_base_root) {
        Ok(Some(active_version)) => Some(active_version),
        Ok(None) => legacy_active_plugin_version(plugin_base_root),
        Err(err) => {
            warn!(
                plugin = %plugin_id.as_key(),
                path = %plugin_base_root.display(),
                error = %err,
                "failed to read active plugin version marker while cleaning inactive versions"
            );
            None
        }
    };
    let Some(active_version) = active_version else {
        return;
    };

    let entries = match fs::read_dir(plugin_base_root) {
        Ok(entries) => entries,
        Err(err) => {
            warn!(
                plugin = %plugin_id.as_key(),
                path = %plugin_base_root.display(),
                error = %err,
                "failed to read plugin cache while cleaning inactive versions"
            );
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!(
                    plugin = %plugin_id.as_key(),
                    path = %plugin_base_root.display(),
                    error = %err,
                    "failed to enumerate plugin cache while cleaning inactive versions"
                );
                continue;
            }
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Ok(version) = entry.file_name().into_string() else {
            continue;
        };
        if version == active_version || validate_plugin_version_segment(&version).is_err() {
            continue;
        }

        let path = entry.path();
        if let Err(err) = remove_dir_all(&path) {
            warn!(
                plugin = %plugin_id.as_key(),
                plugin_version = %version,
                path = %path.display(),
                error = %err,
                "failed to remove inactive plugin cache version"
            );
        }
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
