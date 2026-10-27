//! Assembles the existing planned-action evidence for synchronous Guardian prompts.
//! The host supplies the resolved reason; action kinds and description bounds stay unchanged.

use crate::GuardianApprovalRequest;
use crate::format_guardian_action_pretty;
use codex_guardian_context::GuardianToolDescriptions;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;

pub fn build_guardian_planned_action(
    request: &GuardianApprovalRequest,
    reason: Option<String>,
) -> serde_json::Result<PlannedAction> {
    Ok(PlannedAction {
        tool_descriptions: if let GuardianApprovalRequest::McpToolCall {
            tool_description,
            connector_description,
            ..
        } = request
        {
            GuardianToolDescriptions::new(
                tool_description.as_deref(),
                connector_description.as_deref(),
            )
        } else {
            None
        },
        kind: match request {
            GuardianApprovalRequest::NativeToolCall { call, .. } => {
                PlannedActionKind::ToolCall(call.clone())
            }
            GuardianApprovalRequest::NetworkAccess { trigger, .. } => PlannedActionKind::Network {
                json: format_guardian_action_pretty(request)?,
                has_trigger: trigger.is_some(),
            },
            GuardianApprovalRequest::WriteStdin { .. } => PlannedActionKind::TerminalInput {
                json: format_guardian_action_pretty(request)?,
            },
            GuardianApprovalRequest::ExecCommand { .. }
            | GuardianApprovalRequest::ApplyPatch { .. }
            | GuardianApprovalRequest::McpToolCall { .. }
            | GuardianApprovalRequest::RequestPermissions { .. } => PlannedActionKind::Command {
                json: format_guardian_action_pretty(request)?,
            },
        },
        reason,
    })
}
