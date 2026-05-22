//! Session-scoped MCP policy and elicitation state.

use crate::client_pool::McpClientPool;
use crate::elicitation::ElicitationRequestManager;
use anyhow::Result;
use codex_config::Constrained;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::RequestId;
use std::sync::Arc;

pub(crate) struct McpSessionView {
    _client_pool: Arc<McpClientPool>,
    elicitation_requests: ElicitationRequestManager,
}

impl McpSessionView {
    pub(crate) fn new_uninitialized(
        client_pool: Arc<McpClientPool>,
        approval_policy: &Constrained<AskForApproval>,
        permission_profile: &PermissionProfile,
    ) -> Self {
        Self {
            _client_pool: client_pool,
            elicitation_requests: ElicitationRequestManager::new(
                approval_policy.value(),
                permission_profile.clone(),
                /*reviewer*/ None,
            ),
        }
    }

    pub(crate) fn from_parts(
        client_pool: Arc<McpClientPool>,
        elicitation_requests: ElicitationRequestManager,
    ) -> Self {
        Self {
            _client_pool: client_pool,
            elicitation_requests,
        }
    }

    pub(crate) fn set_approval_policy(&self, approval_policy: &Constrained<AskForApproval>) {
        if let Ok(mut policy) = self.elicitation_requests.approval_policy.lock() {
            *policy = approval_policy.value();
        }
    }

    pub(crate) fn set_permission_profile(&self, permission_profile: PermissionProfile) {
        if let Ok(mut profile) = self.elicitation_requests.permission_profile.lock() {
            *profile = permission_profile;
        }
    }

    pub(crate) fn elicitations_auto_deny(&self) -> bool {
        self.elicitation_requests.auto_deny()
    }

    pub(crate) fn set_elicitations_auto_deny(&self, auto_deny: bool) {
        self.elicitation_requests.set_auto_deny(auto_deny);
    }

    pub(crate) async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.elicitation_requests
            .resolve(server_name, id, response)
            .await
    }
}

#[cfg(test)]
#[path = "session_view_tests.rs"]
mod tests;
