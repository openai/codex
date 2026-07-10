use crate::models::AdditionalPermissionProfile;
use crate::models::FileSystemPermissions;
use crate::models::NetworkPermissions;
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[serde(bound(
    serialize = "PathType: Clone + Serialize",
    deserialize = "PathType: Deserialize<'de>"
))]
pub struct RequestPermissionProfile<PathType = AbsolutePathBuf> {
    pub network: Option<NetworkPermissions>,
    pub file_system: Option<FileSystemPermissions<PathType>>,
}

impl<PathType> RequestPermissionProfile<PathType> {
    pub fn is_empty(&self) -> bool {
        self.network.is_none() && self.file_system.is_none()
    }

    /// Maps explicit filesystem paths while preserving the rest of the request.
    pub fn map_paths<OutputPath>(
        self,
        map: impl FnMut(PathType) -> OutputPath,
    ) -> RequestPermissionProfile<OutputPath> {
        RequestPermissionProfile {
            network: self.network,
            file_system: self
                .file_system
                .map(|file_system| file_system.map_paths(map)),
        }
    }
}

impl<PathType> Default for RequestPermissionProfile<PathType> {
    fn default() -> Self {
        Self {
            network: None,
            file_system: None,
        }
    }
}

impl<PathType> From<RequestPermissionProfile<PathType>> for AdditionalPermissionProfile<PathType> {
    fn from(value: RequestPermissionProfile<PathType>) -> Self {
        Self {
            network: value.network,
            file_system: value.file_system,
        }
    }
}

impl<PathType> From<AdditionalPermissionProfile<PathType>> for RequestPermissionProfile<PathType> {
    fn from(value: AdditionalPermissionProfile<PathType>) -> Self {
        Self {
            network: value.network,
            file_system: value.file_system,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(bound(
    serialize = "PathType: Clone + Serialize",
    deserialize = "PathType: Deserialize<'de>"
))]
pub struct RequestPermissionsArgs<PathType = AbsolutePathBuf> {
    #[serde(
        default,
        rename = "environment_id",
        alias = "environmentId",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    pub environment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile<PathType>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(bound(
    serialize = "PathType: Clone + Serialize",
    deserialize = "PathType: Deserialize<'de>"
))]
pub struct RequestPermissionsResponse<PathType = AbsolutePathBuf> {
    pub permissions: RequestPermissionProfile<PathType>,
    #[serde(default)]
    pub scope: PermissionGrantScope,
    /// Review every subsequent command in this turn before normal sandboxed execution.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strict_auto_review: bool,
}

impl<PathType> RequestPermissionsResponse<PathType> {
    pub fn map_paths<OutputPath>(
        self,
        map: impl FnMut(PathType) -> OutputPath,
    ) -> RequestPermissionsResponse<OutputPath> {
        RequestPermissionsResponse {
            permissions: self.permissions.map_paths(map),
            scope: self.scope,
            strict_auto_review: self.strict_auto_review,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionsEvent {
    /// Responses API call id for the associated tool call, if available.
    pub call_id: String,
    /// Turn ID that this request belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    #[serde(
        default,
        rename = "environmentId",
        alias = "environment_id",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional)]
    #[ts(rename = "environmentId")]
    pub environment_id: Option<String>,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cwd: Option<AbsolutePathBuf>,
}
