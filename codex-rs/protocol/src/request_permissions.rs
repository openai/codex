use crate::models::AdditionalPermissionProfile;
use crate::models::FileSystemPermissions;
use crate::models::NetworkPermissions;
use crate::permissions::FileSystemPath;
use crate::permissions::FileSystemSandboxEntry;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
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
    #[serde(
        serialize_with = "serialize_request_permission_profile_as_native_paths",
        deserialize_with = "deserialize_request_permission_profile_from_native_paths"
    )]
    pub permissions: RequestPermissionProfile<PathUri>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_path_uri_as_native_path",
        deserialize_with = "deserialize_optional_path_uri_from_native_path"
    )]
    #[ts(optional)]
    pub cwd: Option<PathUri>,
}

pub(crate) fn serialize_request_permission_profile_as_native_paths<S>(
    permissions: &RequestPermissionProfile<PathUri>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    permissions
        .clone()
        .map_paths(|path| path.inferred_native_path_string())
        .serialize(serializer)
}

pub(crate) fn deserialize_request_permission_profile_from_native_paths<'de, D>(
    deserializer: D,
) -> Result<RequestPermissionProfile<PathUri>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let permissions = RequestPermissionProfile::<LegacyAppPathString>::deserialize(deserializer)?;
    try_map_request_permission_profile_paths(permissions, path_uri_from_legacy_app_path)
        .map_err(serde::de::Error::custom)
}

fn serialize_optional_path_uri_as_native_path<S>(
    path: &Option<PathUri>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    path.as_ref()
        .map(PathUri::inferred_native_path_string)
        .serialize(serializer)
}

fn deserialize_optional_path_uri_from_native_path<'de, D>(
    deserializer: D,
) -> Result<Option<PathUri>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<LegacyAppPathString>::deserialize(deserializer)?
        .map(path_uri_from_legacy_app_path)
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn path_uri_from_legacy_app_path(
    path: LegacyAppPathString,
) -> Result<PathUri, codex_utils_path_uri::LegacyAppPathStringError> {
    if let Ok(path_uri) = PathUri::parse(path.as_str()) {
        return Ok(path_uri);
    }
    path.try_into()
}

fn try_map_request_permission_profile_paths<InputPath, OutputPath, Error>(
    permissions: RequestPermissionProfile<InputPath>,
    mut map: impl FnMut(InputPath) -> Result<OutputPath, Error>,
) -> Result<RequestPermissionProfile<OutputPath>, Error> {
    let file_system = permissions
        .file_system
        .map(|file_system| {
            let entries = file_system
                .entries
                .into_iter()
                .map(|entry| {
                    let path = match entry.path {
                        FileSystemPath::Path { path } => FileSystemPath::Path { path: map(path)? },
                        FileSystemPath::GlobPattern { pattern } => {
                            FileSystemPath::GlobPattern { pattern }
                        }
                        FileSystemPath::Special { value } => FileSystemPath::Special { value },
                    };
                    Ok(FileSystemSandboxEntry {
                        path,
                        access: entry.access,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            Ok(FileSystemPermissions {
                entries,
                glob_scan_max_depth: file_system.glob_scan_max_depth,
            })
        })
        .transpose()?;
    Ok(RequestPermissionProfile {
        network: permissions.network,
        file_system,
    })
}
