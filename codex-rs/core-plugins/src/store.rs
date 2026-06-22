use crate::manifest::PluginManifest;
use crate::manifest::load_plugin_manifest;
use crate::manifest::parse_plugin_manifest;
use codex_plugin::PluginId;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::find_plugin_manifest_path;
use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

pub const DEFAULT_PLUGIN_VERSION: &str = "local";
pub const PLUGINS_CACHE_DIR: &str = "plugins/cache";
pub const PLUGINS_DATA_DIR: &str = "plugins/data";
const REMOTE_PLUGIN_INSTALL_METADATA_FILE: &str = ".codex-remote-plugin-install.json";
const REMOTE_PLUGIN_INSTALL_METADATA_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct RemotePluginInstallMetadata {
    schema_version: u8,
    remote_plugin_id: String,
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
    data_root: AbsolutePathBuf,
}

#[derive(Clone, Copy)]
enum InstallManifest<'a> {
    OnDisk,
    Fallback(&'a str),
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
        let mut discovered_versions = fs::read_dir(self.plugin_base_root(plugin_id).as_path())
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry.file_type().ok().filter(std::fs::FileType::is_dir)?;
                entry.file_name().into_string().ok()
            })
            .filter(|version| validate_plugin_version_segment(version).is_ok())
            .collect::<Vec<_>>();
        discovered_versions.sort_unstable_by(|left, right| compare_plugin_versions(left, right));
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

    pub fn remote_plugin_id(
        &self,
        plugin_id: &PluginId,
    ) -> Result<Option<String>, PluginStoreError> {
        if !self.is_installed(plugin_id) {
            return Ok(None);
        }
        let path = self.remote_plugin_install_metadata_path(plugin_id);
        let contents = match fs::read_to_string(path.as_path()) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(PluginStoreError::io(
                    "failed to read remote plugin install metadata",
                    err,
                ));
            }
        };
        let metadata: RemotePluginInstallMetadata =
            serde_json::from_str(&contents).map_err(|err| {
                PluginStoreError::Invalid(format!(
                    "failed to parse remote plugin install metadata: {err}"
                ))
            })?;
        if metadata.schema_version != REMOTE_PLUGIN_INSTALL_METADATA_SCHEMA_VERSION {
            return Err(PluginStoreError::Invalid(format!(
                "unsupported remote plugin install metadata schema version: {}",
                metadata.schema_version
            )));
        }
        let remote_plugin_id = metadata.remote_plugin_id.trim();
        if remote_plugin_id.is_empty() {
            return Err(PluginStoreError::Invalid(
                "invalid remote plugin install metadata: remote plugin id must not be blank"
                    .to_string(),
            ));
        }
        Ok(Some(remote_plugin_id.to_string()))
    }

    pub fn write_remote_plugin_id(
        &self,
        plugin_id: &PluginId,
        remote_plugin_id: &str,
    ) -> Result<(), PluginStoreError> {
        if !self.is_installed(plugin_id) {
            return Err(PluginStoreError::Invalid(format!(
                "cannot write remote identity for uninstalled plugin `{}`",
                plugin_id.as_key()
            )));
        }
        let remote_plugin_id = remote_plugin_id.trim();
        if remote_plugin_id.is_empty() {
            return Err(PluginStoreError::Invalid(
                "invalid remote plugin install metadata: remote plugin id must not be blank"
                    .to_string(),
            ));
        }
        let path = self.remote_plugin_install_metadata_path(plugin_id);
        let parent = path.as_path().parent().ok_or_else(|| {
            PluginStoreError::Invalid(format!(
                "remote plugin install metadata path has no parent: {}",
                path.display()
            ))
        })?;
        let mut contents = serde_json::to_vec_pretty(&RemotePluginInstallMetadata {
            schema_version: REMOTE_PLUGIN_INSTALL_METADATA_SCHEMA_VERSION,
            remote_plugin_id: remote_plugin_id.to_string(),
        })
        .map_err(|err| {
            PluginStoreError::Invalid(format!(
                "failed to serialize remote plugin install metadata: {err}"
            ))
        })?;
        contents.push(b'\n');
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|err| {
            PluginStoreError::io(
                "failed to create temporary remote plugin install metadata",
                err,
            )
        })?;
        temporary.write_all(&contents).map_err(|err| {
            PluginStoreError::io("failed to write remote plugin install metadata", err)
        })?;
        temporary.as_file_mut().flush().map_err(|err| {
            PluginStoreError::io("failed to flush remote plugin install metadata", err)
        })?;
        temporary.persist(path.as_path()).map_err(|err| {
            PluginStoreError::io(
                "failed to persist remote plugin install metadata",
                err.error,
            )
        })?;
        Ok(())
    }

    pub fn install(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        self.install_with_manifest(source_path, plugin_id, InstallManifest::OnDisk)
    }

    pub(crate) fn install_with_fallback_manifest(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
        manifest_contents: &str,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        self.install_with_manifest(
            source_path,
            plugin_id,
            InstallManifest::Fallback(manifest_contents),
        )
    }

    pub fn install_with_version(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
        plugin_version: String,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        self.install_with_version_and_manifest(
            source_path,
            plugin_id,
            plugin_version,
            InstallManifest::OnDisk,
        )
    }

    pub(crate) fn install_with_version_and_fallback_manifest(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
        plugin_version: String,
        manifest_contents: &str,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        self.install_with_version_and_manifest(
            source_path,
            plugin_id,
            plugin_version,
            InstallManifest::Fallback(manifest_contents),
        )
    }

    fn install_with_manifest(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
        manifest: InstallManifest<'_>,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        let manifest = resolve_install_manifest(source_path.as_path(), manifest);
        let plugin_version = plugin_version_for_install_manifest(source_path.as_path(), manifest)?;
        self.install_with_version_and_manifest(source_path, plugin_id, plugin_version, manifest)
    }

    fn install_with_version_and_manifest(
        &self,
        source_path: AbsolutePathBuf,
        plugin_id: PluginId,
        plugin_version: String,
        manifest: InstallManifest<'_>,
    ) -> Result<PluginInstallResult, PluginStoreError> {
        if !source_path.as_path().is_dir() {
            return Err(PluginStoreError::Invalid(format!(
                "plugin source path is not a directory: {}",
                source_path.display()
            )));
        }

        let manifest = resolve_install_manifest(source_path.as_path(), manifest);
        let plugin_name = plugin_name_for_source(source_path.as_path(), manifest)?;
        if plugin_name != plugin_id.plugin_name {
            return Err(PluginStoreError::Invalid(format!(
                "plugin.json name `{plugin_name}` does not match marketplace plugin name `{}`",
                plugin_id.plugin_name
            )));
        }
        validate_plugin_version_segment(&plugin_version).map_err(PluginStoreError::Invalid)?;
        let installed_path = self.plugin_root(&plugin_id, &plugin_version);
        replace_plugin_root_atomically(
            source_path.as_path(),
            self.plugin_base_root(&plugin_id).as_path(),
            &plugin_version,
            manifest,
        )?;
        self.remove_remote_plugin_install_metadata(&plugin_id)?;

        Ok(PluginInstallResult {
            plugin_id,
            plugin_version,
            installed_path,
        })
    }

    pub fn uninstall(&self, plugin_id: &PluginId) -> Result<(), PluginStoreError> {
        remove_existing_target(self.plugin_base_root(plugin_id).as_path())
    }

    fn remote_plugin_install_metadata_path(&self, plugin_id: &PluginId) -> AbsolutePathBuf {
        self.plugin_base_root(plugin_id)
            .join(REMOTE_PLUGIN_INSTALL_METADATA_FILE)
    }

    fn remove_remote_plugin_install_metadata(
        &self,
        plugin_id: &PluginId,
    ) -> Result<(), PluginStoreError> {
        let path = self.remote_plugin_install_metadata_path(plugin_id);
        match fs::remove_file(path.as_path()) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(PluginStoreError::io(
                "failed to remove remote plugin install metadata",
                err,
            )),
        }
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
    plugin_version_for_install_manifest(source_path, InstallManifest::OnDisk)
}

pub(crate) fn plugin_version_for_source_with_fallback_manifest(
    source_path: &Path,
    manifest_contents: &str,
) -> Result<String, PluginStoreError> {
    let manifest =
        resolve_install_manifest(source_path, InstallManifest::Fallback(manifest_contents));
    plugin_version_for_install_manifest(source_path, manifest)
}

fn resolve_install_manifest<'a>(
    source_path: &Path,
    manifest: InstallManifest<'a>,
) -> InstallManifest<'a> {
    // A real plugin manifest always wins. The fallback only fills the gap for marketplace
    // sources that cannot be changed in place because they may be user-owned directories.
    match manifest {
        InstallManifest::Fallback(_) if find_plugin_manifest_path(source_path).is_some() => {
            InstallManifest::OnDisk
        }
        manifest => manifest,
    }
}

fn plugin_version_for_install_manifest(
    source_path: &Path,
    manifest: InstallManifest<'_>,
) -> Result<String, PluginStoreError> {
    let plugin_version = plugin_manifest_version_for_source(source_path, manifest)?
        .unwrap_or_else(|| DEFAULT_PLUGIN_VERSION.to_string());
    validate_plugin_version_segment(&plugin_version).map_err(PluginStoreError::Invalid)?;
    Ok(plugin_version)
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

fn plugin_manifest_for_source(
    source_path: &Path,
    manifest: InstallManifest<'_>,
) -> Result<PluginManifest, PluginStoreError> {
    match manifest {
        InstallManifest::OnDisk => load_plugin_manifest(source_path)
            .ok_or_else(|| PluginStoreError::Invalid("missing or invalid plugin.json".to_string())),
        InstallManifest::Fallback(contents) => parse_plugin_manifest(
            source_path,
            &source_path.join(".codex-plugin/plugin.json"),
            contents,
        )
        .map_err(|err| PluginStoreError::Invalid(format!("failed to parse plugin.json: {err}"))),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestVersion {
    #[serde(default)]
    version: Option<JsonValue>,
}

fn plugin_manifest_version_for_source(
    source_path: &Path,
    manifest: InstallManifest<'_>,
) -> Result<Option<String>, PluginStoreError> {
    let contents = match manifest {
        InstallManifest::OnDisk => {
            let manifest_path = find_plugin_manifest_path(source_path)
                .ok_or_else(|| PluginStoreError::Invalid("missing plugin.json".to_string()))?;
            fs::read_to_string(&manifest_path)
                .map_err(|err| PluginStoreError::io("failed to read plugin.json", err))?
        }
        InstallManifest::Fallback(contents) => contents.to_string(),
    };
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

fn plugin_name_for_source(
    source_path: &Path,
    manifest: InstallManifest<'_>,
) -> Result<String, PluginStoreError> {
    let manifest = plugin_manifest_for_source(source_path, manifest)?;

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

fn replace_plugin_root_atomically(
    source: &Path,
    target_root: &Path,
    plugin_version: &str,
    manifest: InstallManifest<'_>,
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
    if let InstallManifest::Fallback(contents) = manifest {
        // Inject the generated manifest into Store's existing atomic copy so install does not
        // mutate the original source or require a second staging directory.
        let manifest_path = staged_version_root.join(".codex-plugin/plugin.json");
        let Some(manifest_parent) = manifest_path.parent() else {
            return Err(PluginStoreError::Invalid(
                "plugin manifest path has no parent".to_string(),
            ));
        };
        fs::create_dir_all(manifest_parent).map_err(|err| {
            PluginStoreError::io("failed to create plugin manifest directory", err)
        })?;
        fs::write(&manifest_path, contents)
            .map_err(|err| PluginStoreError::io("failed to write fallback plugin manifest", err))?;
    }

    let target_version_root = target_root.join(plugin_version);
    if target_root.exists() && !target_version_root.exists() {
        fs::rename(&staged_version_root, &target_version_root).map_err(|err| {
            PluginStoreError::io("failed to activate updated plugin cache version", err)
        })?;
        remove_old_plugin_versions(target_root, plugin_version)?;
        return Ok(());
    }

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

fn remove_old_plugin_versions(
    target_root: &Path,
    plugin_version: &str,
) -> Result<(), PluginStoreError> {
    let Ok(entries) = fs::read_dir(target_root) else {
        return Ok(());
    };

    for entry in entries.filter_map(Result::ok) {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Ok(version) = entry.file_name().into_string() else {
            continue;
        };
        if version == plugin_version || validate_plugin_version_segment(&version).is_err() {
            continue;
        }

        if fs::remove_dir_all(entry.path()).is_err()
            && old_plugin_version_would_stay_active(&version, plugin_version)
        {
            return Err(PluginStoreError::Invalid(format!(
                "failed to activate updated plugin cache version `{plugin_version}` while `{version}` remains active"
            )));
        }
    }

    Ok(())
}

fn old_plugin_version_would_stay_active(old_version: &str, new_version: &str) -> bool {
    old_version == DEFAULT_PLUGIN_VERSION
        || compare_plugin_versions(old_version, new_version).is_gt()
}

fn compare_plugin_versions(left: &str, right: &str) -> Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
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
#[path = "store_tests.rs"]
mod tests;
