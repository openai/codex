//! Supplies action categories to the review extension and enforces host-owned requirements.

use super::GuardianApprovalRequest;
use crate::tools::sandboxing::ApprovalAction;
use codex_protocol::openai_models::GuardianScope;

impl GuardianApprovalRequest {
    pub(crate) fn guardian_scope(&self) -> GuardianScope {
        match self {
            Self::ExecCommand { .. } | Self::WriteStdin { .. } => GuardianScope::Shell,
            #[cfg(unix)]
            Self::Execve { .. } => GuardianScope::Shell,
            Self::ApplyPatch { .. } => GuardianScope::FileChanges,
            Self::McpToolCall { server, .. } => GuardianScope::for_mcp_server(server),
            Self::NetworkAccess { .. } => GuardianScope::Network,
            Self::RequestPermissions { .. } => GuardianScope::Permissions,
        }
    }
}

impl ApprovalAction {
    pub(crate) fn guardian_scope(&self) -> GuardianScope {
        match self {
            Self::ExecCommand { .. } | Self::WriteStdin { .. } => GuardianScope::Shell,
            #[cfg(unix)]
            Self::Execve { .. } => GuardianScope::Shell,
            Self::ApplyPatch { .. } => GuardianScope::FileChanges,
            Self::McpToolCall {
                server,
                connector_id,
                ..
            } => GuardianScope::for_mcp_connector(server, connector_id.as_deref()),
            Self::NetworkAccess { .. } => GuardianScope::Network,
            Self::RequestPermissions { .. } => GuardianScope::Permissions,
        }
    }
}
