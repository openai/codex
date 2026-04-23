use crate::models::FileSystemPermissions;
use crate::models::NetworkPermissions;
use crate::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PermissionGrantScope {
    #[default]
    Turn,
    Session,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestPermissionProfile {
    pub network: Option<NetworkPermissions>,
    pub file_system: Option<FileSystemPermissions>,
}

impl RequestPermissionProfile {
    pub fn is_empty(&self) -> bool {
        self.network.is_none() && self.file_system.is_none()
    }
}

impl From<RequestPermissionProfile> for PermissionProfile {
    fn from(value: RequestPermissionProfile) -> Self {
        Self {
            network: value.network,
            file_system: value.file_system,
        }
    }
}

impl From<PermissionProfile> for RequestPermissionProfile {
    fn from(value: PermissionProfile) -> Self {
        Self {
            network: value.network,
            file_system: value.file_system,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionsArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionsResponse {
    pub permissions: RequestPermissionProfile,
    #[serde(default)]
    pub scope: PermissionGrantScope,
    /// Review every subsequent command in this turn before normal sandboxed execution.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strict_auto_review: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionsEvent {
    /// Responses API call id for the associated tool call, if available.
    pub call_id: String,
    /// Turn ID that this request belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<AbsolutePathBuf>,
}
