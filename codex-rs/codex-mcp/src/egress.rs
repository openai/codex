use crate::catalog::McpServerSource;
use crate::rmcp_client::MCP_SANDBOX_STATE_META_CAPABILITY;
use serde_json::Value;

const LEGACY_HOOPA_MCP_SERVER_NAME: &str = "hoopa";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum McpEgressProfile {
    DirectMcpV1,
    HostOwned,
}

impl McpEgressProfile {
    pub(crate) fn for_configured_name(server_name: &str) -> Self {
        // TODO: Remove this exception after Hoopa uses a host-owned registration or stops using
        // MCP request metadata for thread routing.
        if server_name == LEGACY_HOOPA_MCP_SERVER_NAME {
            Self::HostOwned
        } else {
            Self::DirectMcpV1
        }
    }

    pub(crate) fn for_registration(server_name: &str, source: &McpServerSource) -> Self {
        if Self::for_configured_name(server_name) == Self::HostOwned {
            return Self::HostOwned;
        }

        match source {
            McpServerSource::Config
            | McpServerSource::Plugin(_)
            | McpServerSource::SelectedPlugin(_) => Self::DirectMcpV1,
            McpServerSource::Compatibility { .. } | McpServerSource::Extension { .. } => {
                Self::HostOwned
            }
        }
    }
}

pub(crate) fn sanitize_tool_call_meta(
    profile: McpEgressProfile,
    meta: Option<Value>,
    allow_sandbox_state_meta: bool,
) -> Option<Value> {
    if profile == McpEgressProfile::HostOwned {
        return meta;
    }

    let Some(Value::Object(mut meta)) = meta else {
        return meta;
    };
    meta.retain(|key, _| {
        (allow_sandbox_state_meta && key == MCP_SANDBOX_STATE_META_CAPABILITY)
            || !is_reserved_direct_mcp_meta_key(key)
    });
    (!meta.is_empty()).then_some(Value::Object(meta))
}

fn is_reserved_direct_mcp_meta_key(key: &str) -> bool {
    matches!(
        key,
        "installation_id"
            | "installationId"
            | "session_id"
            | "sessionId"
            | "thread_id"
            | "threadId"
            | "conversation_id"
            | "conversationId"
            | "turn_id"
            | "turnId"
            | "workspace_id"
            | "workspaceId"
            | "window_id"
            | "windowId"
            | "request_kind"
            | "requestKind"
            | "compaction"
            | "turn_started_at_unix_ms"
            | "turnStartedAtUnixMs"
            | "forked_from_thread_id"
            | "forkedFromThreadId"
            | "parent_thread_id"
            | "parentThreadId"
            | "subagent_kind"
            | "subagentKind"
            | "thread_source"
            | "threadSource"
            | "sandbox"
            | "workspaces"
            | "plugin_id"
            | "pluginId"
            | "connector_id"
            | "connector_name"
            | "connector_display_name"
            | "connector_description"
            | "connectorDescription"
            | "connected_account_email"
            | "connectedAccountEmail"
            | "link_id"
            | "linkId"
            | "template_id"
            | "templateId"
            | "resource_uri"
            | "resourceUri"
    ) || key.starts_with("openai/")
        || key.starts_with("x-openai-")
        || key.starts_with("_openai_")
        || key.starts_with("codex/")
        || key.starts_with("x-codex-")
        || key.starts_with("_codex_")
        || key.starts_with("codex_")
}
