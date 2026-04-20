use codex_analytics::GuardianReviewedAction;

use super::GuardianApprovalRequest;

pub(super) fn guardian_reviewed_action(
    request: &GuardianApprovalRequest,
) -> GuardianReviewedAction {
    match request {
        GuardianApprovalRequest::Shell {
            sandbox_permissions,
            additional_permissions,
            ..
        } => GuardianReviewedAction::Shell {
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.clone(),
        },
        GuardianApprovalRequest::ExecCommand {
            sandbox_permissions,
            additional_permissions,
            tty,
            ..
        } => GuardianReviewedAction::UnifiedExec {
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.clone(),
            tty: *tty,
        },
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            source,
            program,
            additional_permissions,
            ..
        } => GuardianReviewedAction::Execve {
            source: *source,
            program: program.clone(),
            additional_permissions: additional_permissions.clone(),
        },
        GuardianApprovalRequest::ApplyPatch { .. } => GuardianReviewedAction::ApplyPatch {},
        GuardianApprovalRequest::NetworkAccess { protocol, port, .. } => {
            GuardianReviewedAction::NetworkAccess {
                protocol: *protocol,
                port: *port,
            }
        }
        GuardianApprovalRequest::McpToolCall {
            server,
            tool_name,
            connector_id,
            connector_name,
            tool_title,
            ..
        } => GuardianReviewedAction::McpToolCall {
            server: server.clone(),
            tool_name: tool_name.clone(),
            connector_id: connector_id.clone(),
            connector_name: connector_name.clone(),
            tool_title: tool_title.clone(),
        },
    }
}
