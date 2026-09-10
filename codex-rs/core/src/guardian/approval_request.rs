#[cfg(unix)]
use std::path::Path;

use codex_analytics::GuardianReviewedAction;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::approvals::GuardianCommandSource;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionProfile;
#[cfg(unix)]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GuardianApprovalRequest {
    ExecCommand {
        id: String,
        environment_id: String,
        command: Vec<String>,
        cwd: PathUri,
        /// Executor-native rendering sent to Guardian; `cwd` remains typed for attribution.
        guardian_cwd: LegacyAppPathString,
        sandbox_permissions: crate::sandboxing::SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
        justification: Option<String>,
        tty: bool,
    },
    WriteStdin {
        id: String,
        approval_id: String,
        environment_id: String,
        process_id: i32,
        input: String,
        cwd: PathUri,
        tty: bool,
        sandbox_permissions: crate::sandboxing::SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
    },
    #[cfg(unix)]
    Execve {
        id: String,
        source: GuardianCommandSource,
        program: String,
        argv: Vec<String>,
        cwd: AbsolutePathBuf,
        additional_permissions: Option<AdditionalPermissionProfile>,
    },
    ApplyPatch {
        id: String,
        cwd: PathUri,
        files: Vec<PathUri>,
        patch: String,
    },
    NetworkAccess {
        id: String,
        turn_id: String,
        target: String,
        host: String,
        protocol: NetworkApprovalProtocol,
        port: u16,
        trigger: Option<GuardianNetworkAccessTrigger>,
    },
    McpToolCall {
        id: String,
        server: String,
        tool_name: String,
        arguments: Option<Value>,
        connector_id: Option<String>,
        connector_name: Option<String>,
        connector_description: Option<String>,
        connected_account_email: Option<String>,
        tool_title: Option<String>,
        tool_description: Option<String>,
        annotations: Option<GuardianMcpAnnotations>,
    },
    RequestPermissions {
        id: String,
        turn_id: String,
        reason: Option<String>,
        permissions: RequestPermissionProfile,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GuardianNetworkAccessTrigger {
    pub(crate) call_id: String,
    pub(crate) tool_name: String,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: PathUri,
    pub(crate) sandbox_permissions: crate::sandboxing::SandboxPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) additional_permissions: Option<AdditionalPermissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) justification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GuardianMcpAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) destructive_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) open_world_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) read_only_hint: Option<bool>,
}

#[derive(Serialize)]
struct CommandApprovalAction<'a> {
    tool: &'a str,
    command: &'a [String],
    cwd: LegacyAppPathString,
    sandbox_permissions: crate::sandboxing::SandboxPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_permissions: Option<&'a AdditionalPermissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    justification: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tty: Option<bool>,
}

#[derive(Serialize)]
struct ApplyPatchApprovalAction<'a> {
    tool: &'static str,
    cwd: LegacyAppPathString,
    files: Vec<LegacyAppPathString>,
    patch: &'a str,
}

#[derive(Serialize)]
struct WriteStdinApprovalAction<'a> {
    tool: &'static str,
    environment_id: &'a str,
    session_id: i32,
    chars: &'a str,
    cwd: LegacyAppPathString,
    sandbox_permissions: crate::sandboxing::SandboxPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_permissions: Option<&'a AdditionalPermissionProfile>,
    tty: bool,
}

#[cfg(unix)]
#[derive(Serialize)]
struct ExecveApprovalAction<'a> {
    tool: &'a str,
    program: &'a str,
    argv: &'a [String],
    cwd: &'a Path,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_permissions: Option<&'a AdditionalPermissionProfile>,
}

#[derive(Serialize)]
struct McpToolCallApprovalAction<'a> {
    tool: &'static str,
    server: &'a str,
    tool_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connector_id: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connector_name: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connector_description: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connected_account_email: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_title: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_description: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<&'a GuardianMcpAnnotations>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkAccessApprovalAction<'a> {
    tool: &'static str,
    target: &'a str,
    host: &'a str,
    protocol: NetworkApprovalProtocol,
    port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger: Option<&'a GuardianNetworkAccessTrigger>,
}

#[derive(Serialize)]
struct RequestPermissionsApprovalAction<'a> {
    tool: &'static str,
    turn_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a String>,
    permissions: &'a RequestPermissionProfile,
}

fn serialize_guardian_action(value: impl Serialize) -> serde_json::Result<Value> {
    serde_json::to_value(value)
}

fn serialize_command_guardian_action(
    tool: &'static str,
    command: &[String],
    cwd: LegacyAppPathString,
    sandbox_permissions: crate::sandboxing::SandboxPermissions,
    additional_permissions: Option<&AdditionalPermissionProfile>,
    justification: Option<&String>,
    tty: Option<bool>,
) -> serde_json::Result<Value> {
    serialize_guardian_action(CommandApprovalAction {
        tool,
        command,
        cwd,
        sandbox_permissions,
        additional_permissions,
        justification,
        tty,
    })
}

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

#[cfg(unix)]
fn guardian_command_source_tool_name(source: GuardianCommandSource) -> &'static str {
    match source {
        GuardianCommandSource::Shell => "shell",
        GuardianCommandSource::UnifiedExec => "exec_command",
    }
}

pub(crate) fn guardian_approval_request_to_json(
    action: &GuardianApprovalRequest,
) -> serde_json::Result<Value> {
    match action {
        GuardianApprovalRequest::ExecCommand {
            id: _,
            environment_id: _,
            command,
            cwd: _,
            guardian_cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
        } => serialize_command_guardian_action(
            "exec_command",
            command,
            guardian_cwd.clone(),
            *sandbox_permissions,
            additional_permissions.as_ref(),
            justification.as_ref(),
            Some(*tty),
        ),
        GuardianApprovalRequest::WriteStdin {
            environment_id,
            process_id,
            input,
            cwd,
            tty,
            sandbox_permissions,
            additional_permissions,
            ..
        } => serialize_guardian_action(WriteStdinApprovalAction {
            tool: "write_stdin",
            environment_id,
            session_id: *process_id,
            chars: input,
            cwd: cwd.clone().into(),
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.as_ref(),
            tty: *tty,
        }),
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            id: _,
            source,
            program,
            argv,
            cwd,
            additional_permissions,
        } => serialize_guardian_action(ExecveApprovalAction {
            tool: guardian_command_source_tool_name(*source),
            program,
            argv,
            cwd,
            additional_permissions: additional_permissions.as_ref(),
        }),
        GuardianApprovalRequest::ApplyPatch {
            id: _,
            cwd,
            files,
            patch,
        } => serialize_guardian_action(ApplyPatchApprovalAction {
            tool: "apply_patch",
            cwd: cwd.clone().into(),
            files: files.iter().cloned().map(Into::into).collect(),
            patch,
        }),
        GuardianApprovalRequest::NetworkAccess {
            id: _,
            turn_id: _,
            target,
            host,
            protocol,
            port,
            trigger,
        } => {
            let mut value = serialize_guardian_action(NetworkAccessApprovalAction {
                tool: "network_access",
                target,
                host,
                protocol: *protocol,
                port: *port,
                trigger: trigger.as_ref(),
            })?;
            if let Some(trigger) = trigger {
                value["trigger"]["cwd"] = trigger.cwd.inferred_native_path_string().into();
            }
            Ok(value)
        }
        GuardianApprovalRequest::McpToolCall {
            id: _,
            server,
            tool_name,
            arguments,
            connector_id,
            connector_name,
            connector_description,
            connected_account_email,
            tool_title,
            tool_description,
            annotations,
        } => serialize_guardian_action(McpToolCallApprovalAction {
            tool: "mcp_tool_call",
            server,
            tool_name,
            arguments: arguments.as_ref(),
            connector_id: connector_id.as_ref(),
            connector_name: connector_name.as_ref(),
            connector_description: connector_description.as_ref(),
            connected_account_email: connected_account_email.as_ref(),
            tool_title: tool_title.as_ref(),
            tool_description: tool_description.as_ref(),
            annotations: annotations.as_ref(),
        }),
        GuardianApprovalRequest::RequestPermissions {
            id: _,
            turn_id,
            reason,
            permissions,
        } => serialize_guardian_action(RequestPermissionsApprovalAction {
            tool: "request_permissions",
            turn_id,
            reason: reason.as_ref(),
            permissions,
        }),
    }
}

pub(crate) fn guardian_assessment_action(
    action: &GuardianApprovalRequest,
) -> GuardianAssessmentAction {
    match action {
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
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            source,
            program,
            argv,
            cwd,
            ..
        } => GuardianAssessmentAction::Execve {
            source: *source,
            program: program.clone(),
            argv: argv.clone(),
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
    }
}

pub(crate) fn guardian_reviewed_action(
    request: &GuardianApprovalRequest,
) -> GuardianReviewedAction {
    match request {
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
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            source,
            additional_permissions,
            ..
        } => GuardianReviewedAction::Execve {
            source: *source,
            additional_permissions: additional_permissions.as_ref().map(Into::into),
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
        | GuardianApprovalRequest::RequestPermissions { id, .. } => Some(id),
        GuardianApprovalRequest::NetworkAccess { .. } => None,
        #[cfg(unix)]
        GuardianApprovalRequest::Execve { id, .. } => Some(id),
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
        | GuardianApprovalRequest::McpToolCall { .. } => default_turn_id,
        #[cfg(unix)]
        GuardianApprovalRequest::Execve { .. } => default_turn_id,
    }
}

pub(crate) fn format_guardian_action_pretty(
    action: &GuardianApprovalRequest,
) -> serde_json::Result<String> {
    let mut value = guardian_action_for_review(action)?;
    value.sort_all_objects();
    serde_json::to_string_pretty(&value)
}

fn guardian_action_for_review(action: &GuardianApprovalRequest) -> serde_json::Result<Value> {
    let mut value = guardian_approval_request_to_json(action)?;
    if matches!(action, GuardianApprovalRequest::McpToolCall { .. })
        && let Some(fields) = value.as_object_mut()
    {
        // Only host-provided metadata is optional. A nested argument named
        // "description" is still part of the exact action under review.
        fields.remove("tool_description");
        fields.remove("connector_description");
    }
    Ok(value)
}
