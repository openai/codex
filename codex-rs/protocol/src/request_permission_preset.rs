//! Protocol types for conversational permission-mode preset requests.
//!
//! These messages represent the narrow bridge between a model tool call and a
//! client-owned confirmation UI. Core resolves a requested preset into concrete
//! sandbox, approval, and reviewer settings before emitting the event, and the
//! settings only become active after the client returns an accepted response.

use crate::config_types::ApprovalsReviewer;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A built-in permission-mode preset that can be requested conversationally.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum PermissionPresetId {
    Auto,
    FullAccess,
    ReadOnly,
    GuardianApprovals,
}

impl PermissionPresetId {
    /// Returns the stable kebab-case identifier used in tool arguments and UI payloads.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::FullAccess => "full-access",
            Self::ReadOnly => "read-only",
            Self::GuardianApprovals => "guardian-approvals",
        }
    }

    /// Parses a stable preset identifier into a permission preset enum value.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "auto" => Some(Self::Auto),
            "full-access" => Some(Self::FullAccess),
            "read-only" => Some(Self::ReadOnly),
            "guardian-approvals" => Some(Self::GuardianApprovals),
            _ => None,
        }
    }
}

/// Arguments passed by the model when it asks the client to open the preset picker.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionPresetArgs {
    pub preset: PermissionPresetId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The user's completed decision from the permission preset confirmation UI.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RequestPermissionPresetDecision {
    Accepted,
    Declined,
}

/// The result returned to the model after the permission preset picker is resolved.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionPresetResponse {
    pub decision: RequestPermissionPresetDecision,
    pub preset: PermissionPresetId,
    pub message: String,
}

/// Event sent from core to a client so the client can confirm a preset change.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestPermissionPresetEvent {
    /// Responses API call id for the associated tool call, if available.
    pub call_id: String,
    /// Turn ID that this request belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    pub preset: PermissionPresetId,
    pub label: String,
    pub description: String,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub sandbox_policy: SandboxPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
