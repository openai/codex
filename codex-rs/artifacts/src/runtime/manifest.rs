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
