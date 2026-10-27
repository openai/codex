use codex_analytics::GuardianReviewedAction;
pub(crate) use codex_guardian_reviewer::GuardianApprovalRequest;
pub(crate) use codex_guardian_reviewer::GuardianMcpAnnotations;
pub(crate) use codex_guardian_reviewer::GuardianNetworkAccessTrigger;
pub(crate) use codex_guardian_reviewer::format_guardian_action_pretty;
pub(crate) use codex_guardian_reviewer::guardian_approval_request_to_json;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::approvals::GuardianCommandSource;
use codex_utils_path_uri::LegacyAppPathString;

fn command_assessment_action(
    source: GuardianCommandSource,
    command: &[String],
    cwd: &LegacyAppPathString,
) -> GuardianAssessmentAction {
    GuardianAssessmentAction::Command {
        source,
        command: codex_shell_command::parse_command::shlex_join(command),
        cwd: cwd.clone(),
    }
}

pub(crate) fn guardian_assessment_action(
    action: &GuardianApprovalRequest,
) -> Option<GuardianAssessmentAction> {
    Some(match action {
        GuardianApprovalRequest::NativeToolCall { .. } => return None,
        GuardianApprovalRequest::ExecCommand {
            command,
            guardian_cwd,
            ..
        } => command_assessment_action(GuardianCommandSource::UnifiedExec, command, guardian_cwd),
        GuardianApprovalRequest::WriteStdin {
            approval_id,
            process_id,
            input,
            cwd,
            ..
        } => GuardianAssessmentAction::WriteStdin {
            approval_id: approval_id.clone(),
            process_id: process_id.to_string(),
            stdin: input.clone(),
            cwd: cwd.clone(),
        },
        GuardianApprovalRequest::ApplyPatch { cwd, files, .. } => {
            GuardianAssessmentAction::ApplyPatch {
                cwd: cwd.clone().into(),
                files: files.iter().cloned().map(Into::into).collect(),
            }
        }
        GuardianApprovalRequest::NetworkAccess {
            id: _id,
            turn_id: _turn_id,
            environment_id: _,
            target,
            host,
            protocol,
            port,
            trigger: _trigger,
        } => GuardianAssessmentAction::NetworkAccess {
            target: target.clone(),
            host: host.clone(),
            protocol: *protocol,
            port: *port,
        },
        GuardianApprovalRequest::McpToolCall {
            server,
            tool_name,
            connector_id,
            connector_name,
            tool_title,
            ..
        } => GuardianAssessmentAction::McpToolCall {
            server: server.clone(),
            tool_name: tool_name.clone(),
            connector_id: connector_id.clone(),
            connector_name: connector_name.clone(),
            tool_title: tool_title.clone(),
        },
        GuardianApprovalRequest::RequestPermissions {
            reason,
            permissions,
            ..
        } => GuardianAssessmentAction::RequestPermissions {
            reason: reason.clone(),
            permissions: permissions.clone(),
        },
    })
}

pub(crate) fn guardian_reviewed_action(
    request: &GuardianApprovalRequest,
) -> GuardianReviewedAction {
    match request {
        GuardianApprovalRequest::NativeToolCall { .. } => GuardianReviewedAction::NativeToolCall {},
        GuardianApprovalRequest::ExecCommand {
            sandbox_permissions,
            additional_permissions,
            tty,
            ..
        } => GuardianReviewedAction::UnifiedExec {
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.as_ref().map(Into::into),
            tty: *tty,
        },
        GuardianApprovalRequest::WriteStdin { tty, .. } => {
            GuardianReviewedAction::WriteStdin { tty: *tty }
        }
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
        GuardianApprovalRequest::RequestPermissions { .. } => {
            GuardianReviewedAction::RequestPermissions {}
        }
    }
}

pub(crate) fn guardian_request_target_item_id(request: &GuardianApprovalRequest) -> Option<&str> {
    match request {
        GuardianApprovalRequest::ExecCommand { id, .. }
        | GuardianApprovalRequest::WriteStdin { id, .. }
        | GuardianApprovalRequest::ApplyPatch { id, .. }
        | GuardianApprovalRequest::McpToolCall { id, .. }
        | GuardianApprovalRequest::RequestPermissions { id, .. }
        | GuardianApprovalRequest::NativeToolCall { id, .. } => Some(id),
        GuardianApprovalRequest::NetworkAccess { .. } => None,
    }
}

pub(crate) fn guardian_request_turn_id<'a>(
    request: &'a GuardianApprovalRequest,
    default_turn_id: &'a str,
) -> &'a str {
    match request {
        GuardianApprovalRequest::NetworkAccess { turn_id, .. }
        | GuardianApprovalRequest::RequestPermissions { turn_id, .. } => turn_id,
        GuardianApprovalRequest::ExecCommand { .. }
        | GuardianApprovalRequest::WriteStdin { .. }
        | GuardianApprovalRequest::ApplyPatch { .. }
        | GuardianApprovalRequest::McpToolCall { .. }
        | GuardianApprovalRequest::NativeToolCall { .. } => default_turn_id,
    }
}
