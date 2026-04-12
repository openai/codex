//! Protocol types for conversational requests to grant narrow permissions.
//!
//! These messages are scoped to concrete filesystem and network grants, not
//! broad sandbox-mode changes. The model asks for a specific permission profile,
//! core normalizes it, and the client returns the permissions the user actually
//! granted so partial approvals and denials stay explicit.

use crate::models::FileSystemPermissions;
use crate::models::NetworkPermissions;
use crate::models::PermissionProfile;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// How long an approved narrow permission grant should remain active.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PermissionGrantScope {
    #[default]
    Turn,
    Session,
}

/// A narrow permission profile that can be requested through the confirmation UI.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestPermissionProfile {
    pub network: Option<NetworkPermissions>,
    pub file_system: Option<FileSystemPermissions>,
}

impl RequestPermissionProfile {
    /// Returns true when no filesystem or network permission was requested or granted.
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

/// Arguments passed by the model when it asks the client to grant specific permissions.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionsArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub permissions: RequestPermissionProfile,
    #[serde(default)]
    pub scope: PermissionGrantScope,
}

/// The permissions the user granted after resolving the confirmation UI.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionsResponse {
    pub permissions: RequestPermissionProfile,
    #[serde(default)]
    pub scope: PermissionGrantScope,
}

/// Event sent from core to a client so the client can confirm a narrow permission grant.
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
    #[serde(default)]
    pub suggested_scope: PermissionGrantScope,
}
