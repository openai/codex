//! Compact MCP-side inputs for per-session tool-search feedback diagnostics.
//!
//! These records deliberately carry identities only. Schemas, descriptions,
//! metadata, auth state, and searchable text never enter this path.

use crate::tools::ToolInfo;
use codex_rmcp_client::ToolWithConnectorId;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;

const MAX_MCP_DIAGNOSTIC_IDENTITIES: usize = 256;
const MAX_IDENTITY_CHARS: usize = 160;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ToolSearchDiagnosticIdentity {
    pub server_name: String,
    pub raw_tool_name: String,
    pub callable_namespace: Option<String>,
    pub callable_name: Option<String>,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    pub plugin_display_names: Vec<String>,
}

impl ToolSearchDiagnosticIdentity {
    pub(crate) fn from_raw_tool(server_name: &str, tool: &ToolWithConnectorId) -> Self {
        Self {
            server_name: bounded(server_name.to_string()),
            raw_tool_name: bounded(tool.tool.name.to_string()),
            callable_namespace: None,
            callable_name: None,
            connector_id: tool.connector_id.clone().map(bounded),
            connector_name: tool.connector_name.clone().map(bounded),
            plugin_display_names: Vec::new(),
        }
    }

    pub fn from_tool_info(tool: &ToolInfo) -> Self {
        Self {
            server_name: bounded(tool.server_name.clone()),
            raw_tool_name: bounded(tool.tool.name.to_string()),
            callable_namespace: Some(bounded(tool.callable_namespace.clone())),
            callable_name: Some(bounded(tool.callable_name.clone())),
            connector_id: tool.connector_id.clone().map(bounded),
            connector_name: tool.connector_name.clone().map(bounded),
            plugin_display_names: tool
                .plugin_display_names
                .iter()
                .take(4)
                .cloned()
                .map(bounded)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSearchToolsListSource {
    LiveResponse,
    CachedToolsWithoutLiveResponse,
    NotObserved,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolSearchToolsListResponseSnapshot {
    pub server_name: String,
    pub source: ToolSearchToolsListSource,
    pub total_count: usize,
    pub fingerprint: String,
    pub identities: Vec<ToolSearchDiagnosticIdentity>,
    pub identities_truncated: bool,
}

impl ToolSearchToolsListResponseSnapshot {
    pub(crate) fn from_live_response(server_name: &str, tools: &[ToolWithConnectorId]) -> Self {
        Self::new(
            server_name,
            ToolSearchToolsListSource::LiveResponse,
            tools
                .iter()
                .map(|tool| ToolSearchDiagnosticIdentity::from_raw_tool(server_name, tool))
                .collect(),
        )
    }

    pub(crate) fn from_cached_tools(server_name: &str, tools: &[ToolInfo]) -> Self {
        Self::new(
            server_name,
            ToolSearchToolsListSource::CachedToolsWithoutLiveResponse,
            tools
                .iter()
                .map(ToolSearchDiagnosticIdentity::from_tool_info)
                .collect(),
        )
    }

    pub(crate) fn not_observed(server_name: &str) -> Self {
        Self::new(
            server_name,
            ToolSearchToolsListSource::NotObserved,
            Vec::new(),
        )
    }

    fn new(
        server_name: &str,
        source: ToolSearchToolsListSource,
        mut identities: Vec<ToolSearchDiagnosticIdentity>,
    ) -> Self {
        identities.sort();
        identities.dedup();
        let total_count = identities.len();
        let fingerprint = fingerprint(&identities);
        let identities_truncated = identities.len() > MAX_MCP_DIAGNOSTIC_IDENTITIES;
        identities.truncate(MAX_MCP_DIAGNOSTIC_IDENTITIES);
        Self {
            server_name: bounded(server_name.to_string()),
            source,
            total_count,
            fingerprint,
            identities,
            identities_truncated,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ToolSearchMcpDiagnosticsSnapshot {
    pub tools_list_responses: Vec<ToolSearchToolsListResponseSnapshot>,
    pub server_count: usize,
    pub cached_server_count: usize,
    pub startup_complete_server_count: usize,
}

fn bounded(value: String) -> String {
    let mut chars = value.chars();
    let bounded = chars.by_ref().take(MAX_IDENTITY_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn fingerprint(identities: &[ToolSearchDiagnosticIdentity]) -> String {
    let mut hasher = Sha1::new();
    for identity in identities {
        hasher.update(identity.server_name.as_bytes());
        hasher.update([0]);
        hasher.update(identity.raw_tool_name.as_bytes());
        hasher.update([0]);
        hasher.update(
            identity
                .connector_id
                .as_deref()
                .unwrap_or_default()
                .as_bytes(),
        );
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}
