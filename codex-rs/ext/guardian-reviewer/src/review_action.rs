//! Captures one approval action for the synchronous reviewer.
//! Tool-call provenance is separate from the item's display identity.

use crate::GuardianApprovalRequest;
use crate::guardian_approval_request_to_json;
use crate::guardian_request_target_item_id;

/// Carries the original action even when the legacy synchronous renderer cannot handle its paths.
/// This lets the extension return AskUser before invoking that renderer.
#[derive(Clone)]
pub struct ReviewAction {
    pub action: Result<serde_json::Value, String>,
    pub category: codex_protocol::openai_models::GuardianScope,
    /// Actual host invocation, absent when the approval has no originating tool call.
    pub tool_call_id: Option<String>,
    pub request: Result<GuardianApprovalRequest, String>,
}

impl From<GuardianApprovalRequest> for ReviewAction {
    fn from(request: GuardianApprovalRequest) -> Self {
        let tool_call_id = match &request {
            // Stdin's display target is the launch; freshness belongs to this write.
            GuardianApprovalRequest::WriteStdin { approval_id, .. } => Some(approval_id.clone()),
            GuardianApprovalRequest::ExecCommand { .. }
            | GuardianApprovalRequest::ApplyPatch { .. }
            | GuardianApprovalRequest::NetworkAccess { .. }
            | GuardianApprovalRequest::McpToolCall { .. }
            | GuardianApprovalRequest::RequestPermissions { .. }
            | GuardianApprovalRequest::NativeToolCall { .. } => {
                guardian_request_target_item_id(&request).map(str::to_owned)
            }
        };
        Self {
            action: guardian_approval_request_to_json(&request).map_err(|error| error.to_string()),
            category: request.guardian_scope(),
            tool_call_id,
            request: Ok(request),
        }
    }
}

#[cfg(test)]
#[path = "review_action_tests.rs"]
mod tests;
