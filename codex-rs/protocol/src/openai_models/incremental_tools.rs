//! Optional model-catalog overrides for harness-owned incremental tool notices.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Incremental tool notice overrides; missing/null or >512-byte values use bundled defaults.
/// Empty strings omit static text but retain runtime names and instructions.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct IncrementalToolMessages {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_update_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_tools_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_namespaces_header: Option<String>,
    /// Prefix supporting `{name}`; the namespace instructions are appended verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_instructions_prefix: Option<String>,
    /// Message supporting `{name}` when a namespace's instructions are cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_instructions_cleared: Option<String>,
}
