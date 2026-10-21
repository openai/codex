//! Managed application network and storage requirements, separate from agent permissions.

use super::NetworkDomainPermission;
use crate::JsonSchema;
use crate::TS;
use codex_utils_path_uri::LegacyAppPathString;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ApplicationRequirements {
    pub network: Option<ApplicationNetworkRequirements>,
    pub storage: Option<ApplicationStorageRequirements>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ApplicationNetworkRequirements {
    /// When enabled, only explicitly allowed exact domains may be contacted.
    pub enabled: bool,
    pub domains: BTreeMap<String, NetworkDomainPermission>,
}

/// Managed storage settings for new Desktop workspaces, not filesystem access restrictions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ApplicationStorageRequirements {
    pub default_project_parent: Option<LegacyAppPathString>,
    pub projectless_workspace_root: Option<LegacyAppPathString>,
}
