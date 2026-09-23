//! Read-only installed-version selection and persisted installation identity.

use codex_utils_absolute_path::AbsolutePathBuf;
use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::fs;

pub const DEFAULT_PLUGIN_VERSION: &str = "local";
pub const PLUGINS_CACHE_DIR: &str = "plugins/cache";
pub const REMOTE_PLUGIN_INSTALL_METADATA_FILE: &str = ".codex-remote-plugin-install.json";
pub const REMOTE_PLUGIN_INSTALL_METADATA_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Deserialize, Serialize)]
pub struct RemotePluginInstallMetadata {
    pub schema_version: u8,
    pub remote_plugin_id: String,
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

pub fn compare_plugin_versions(left: &str, right: &str) -> Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

pub fn active_plugin_version(plugin_base_root: &AbsolutePathBuf) -> Option<String> {
    let mut discovered_versions = fs::read_dir(plugin_base_root.as_path())
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

/// Validates persisted installation metadata and returns its normalized remote identity.
pub fn parse_remote_plugin_id(contents: &str) -> Result<String, String> {
    let metadata: RemotePluginInstallMetadata = serde_json::from_str(contents)
        .map_err(|err| format!("failed to parse remote plugin install metadata: {err}"))?;
    if metadata.schema_version != REMOTE_PLUGIN_INSTALL_METADATA_SCHEMA_VERSION {
        return Err(format!(
            "unsupported remote plugin install metadata schema version: {}",
            metadata.schema_version
        ));
    }
    let remote_plugin_id = metadata.remote_plugin_id.trim();
    if remote_plugin_id.is_empty() {
        return Err(
            "invalid remote plugin install metadata: remote plugin id must not be blank"
                .to_string(),
        );
    }
    Ok(remote_plugin_id.to_string())
}
