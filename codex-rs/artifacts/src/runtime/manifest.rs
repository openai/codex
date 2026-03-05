use codex_package_manager::PackageReleaseArchive;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

/// Release metadata published alongside the packaged artifact runtime.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub runtime_version: String,
    pub release_tag: String,
    #[serde(default)]
    pub node_version: Option<String>,
    pub platforms: BTreeMap<String, PackageReleaseArchive>,
}

/// Manifest shipped inside the extracted runtime payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExtractedRuntimeManifest {
    pub schema_version: u32,
    pub runtime_version: String,
    pub node: RuntimePathEntry,
    pub entrypoints: RuntimeEntrypoints,
}

/// A relative path entry inside an extracted runtime manifest.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuntimePathEntry {
    pub relative_path: String,
}

/// Entrypoints required to build and render artifacts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuntimeEntrypoints {
    pub build_js: RuntimePathEntry,
    pub render_cli: RuntimePathEntry,
}
