use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RuntimeInstallManifestParams {
    #[ts(optional = nullable)]
    pub archive_name: Option<String>,
    pub archive_sha256: String,
    #[ts(optional = nullable)]
    pub archive_size_bytes: Option<u64>,
    pub archive_url: String,
    #[ts(optional = nullable)]
    pub bundle_format_version: Option<u32>,
    #[ts(optional = nullable)]
    pub bundle_version: Option<String>,
    #[ts(optional = nullable)]
    pub format: Option<String>,
    #[ts(optional = nullable)]
    pub runtime_root_directory_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RuntimeInstallParams {
    #[ts(optional = nullable)]
    pub environment_id: Option<String>,
    pub manifest: Box<RuntimeInstallManifestParams>,
    pub release: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export_to = "v2/")]
pub enum RuntimeInstallCancelStatus {
    Canceled,
    NotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RuntimeInstallCancelResponse {
    pub status: RuntimeInstallCancelStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export_to = "v2/")]
pub enum RuntimeInstallStatus {
    AlreadyCurrent,
    Installed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RuntimeInstallPaths {
    pub bundled_plugin_marketplace_paths: Vec<AbsolutePathBuf>,
    pub bundled_skill_paths: Vec<AbsolutePathBuf>,
    pub node_modules_path: AbsolutePathBuf,
    pub node_path: AbsolutePathBuf,
    pub python_path: AbsolutePathBuf,
    pub skills_to_remove: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RuntimeInstallResponse {
    pub bundle_version: Option<String>,
    pub paths: RuntimeInstallPaths,
    pub status: RuntimeInstallStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export_to = "v2/")]
pub enum RuntimeInstallProgressPhase {
    Checking,
    Downloading,
    Verifying,
    Extracting,
    Validating,
    Installed,
    Configuring,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RuntimeInstallProgressNotification {
    pub bundle_version: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub phase: RuntimeInstallProgressPhase,
    pub total_bytes: Option<u64>,
}
