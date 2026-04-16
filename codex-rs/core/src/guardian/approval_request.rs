use std::path::Path;

use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use serde_json::Value;

use super::GUARDIAN_MAX_ACTION_STRING_TOKENS;
use super::prompt::guardian_truncate_text;
use crate::tools::approval::ApprovalRequest;
use crate::tools::approval::ApprovalRequestKind;
use crate::tools::approval::McpToolApprovalAnnotations;

#[derive(Serialize)]
struct CommandApprovalAction<'a> {
    tool: &'a str,
    command: &'a [String],
    cwd: &'a Path,
    sandbox_permissions: crate::sandboxing::SandboxPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_permissions: Option<&'a PermissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    justification: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tty: Option<bool>,
}

#[cfg(unix)]
#[derive(Serialize)]
struct ExecveApprovalAction<'a> {
    tool: &'a str,
    program: &'a str,
    argv: &'a [String],
    cwd: &'a Path,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_permissions: Option<&'a PermissionProfile>,
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
    tool_title: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_description: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<&'a McpToolApprovalAnnotations>,
}

fn serialize_guardian_action(value: impl Serialize) -> serde_json::Result<Value> {
    serde_json::to_value(value)
}

fn serialize_command_guardian_action(
    tool: &'static str,
    command: &[String],
    cwd: &Path,
    sandbox_permissions: crate::sandboxing::SandboxPermissions,
    additional_permissions: Option<&PermissionProfile>,
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
    source: codex_protocol::approvals::GuardianCommandSource,
    command: &[String],
    cwd: &AbsolutePathBuf,
) -> GuardianAssessmentAction {
    GuardianAssessmentAction::Command {
        source,
        command: codex_shell_command::parse_command::shlex_join(command),
        cwd: cwd.clone(),
    }
}

#[cfg(unix)]
fn guardian_command_source_tool_name(
    source: codex_protocol::approvals::GuardianCommandSource,
) -> &'static str {
    match source {
        codex_protocol::approvals::GuardianCommandSource::Shell => "shell",
        codex_protocol::approvals::GuardianCommandSource::UnifiedExec => "exec_command",
    }
}

fn truncate_guardian_action_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(guardian_truncate_text(
            &text,
            GUARDIAN_MAX_ACTION_STRING_TOKENS,
        )),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(truncate_guardian_action_value)
                .collect::<Vec<_>>(),
        ),
        Value::Object(values) => {
            let mut entries = values.into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, truncate_guardian_action_value(value)))
                    .collect(),
            )
        }
        other => other,
    }
}

pub(crate) fn guardian_approval_request_to_json(
    action: &ApprovalRequest,
) -> serde_json::Result<Value> {
    match &action.kind {
        ApprovalRequestKind::Command(request) => {
            let tool = match request.source {
                codex_protocol::approvals::GuardianCommandSource::Shell => "shell",
                codex_protocol::approvals::GuardianCommandSource::UnifiedExec => "exec_command",
            };
            let tty = match request.source {
                codex_protocol::approvals::GuardianCommandSource::Shell => None,
                codex_protocol::approvals::GuardianCommandSource::UnifiedExec => Some(request.tty),
            };
            serialize_command_guardian_action(
                tool,
                &request.command,
                &request.cwd,
                request.sandbox_permissions,
                request.additional_permissions.as_ref(),
                request.justification.as_ref(),
                tty,
            )
        }
        #[cfg(unix)]
        ApprovalRequestKind::Execve(request) => serialize_guardian_action(ExecveApprovalAction {
            tool: guardian_command_source_tool_name(request.source),
            program: &request.program,
            argv: &request.argv,
            cwd: &request.cwd,
            additional_permissions: request.additional_permissions.as_ref(),
        }),
        ApprovalRequestKind::Patch(request) => {
            let cwd = &request.cwd;
            let files = &request.files;
            let patch = &request.patch;
            Ok(serde_json::json!({
                "tool": "apply_patch",
                "cwd": cwd,
                "files": files,
                "patch": patch,
            }))
        }
        ApprovalRequestKind::NetworkAccess(request) => {
            let target = &request.target;
            let host = &request.host;
            let protocol = request.protocol;
            let port = request.port;
            Ok(serde_json::json!({
                "tool": "network_access",
                "target": target,
                "host": host,
                "protocol": protocol,
                "port": port,
            }))
        }
        ApprovalRequestKind::McpToolCall(request) => {
            serialize_guardian_action(McpToolCallApprovalAction {
                tool: "mcp_tool_call",
                server: &request.server,
                tool_name: &request.tool_name,
                arguments: request.arguments.as_ref(),
                connector_id: request.connector_id.as_ref(),
                connector_name: request.connector_name.as_ref(),
                connector_description: request.connector_description.as_ref(),
                tool_title: request.tool_title.as_ref(),
                tool_description: request.tool_description.as_ref(),
                annotations: request.annotations.as_ref(),
            })
        }
    }
}

pub(crate) fn guardian_assessment_action(action: &ApprovalRequest) -> GuardianAssessmentAction {
    match &action.kind {
        ApprovalRequestKind::Command(request) => {
            command_assessment_action(request.source, &request.command, &request.cwd)
        }
        #[cfg(unix)]
        ApprovalRequestKind::Execve(request) => GuardianAssessmentAction::Execve {
            source: request.source,
            program: request.program.clone(),
            argv: request.argv.clone(),
            cwd: request.cwd.clone(),
        },
        ApprovalRequestKind::Patch(request) => GuardianAssessmentAction::ApplyPatch {
            cwd: request.cwd.clone(),
            files: request.files.clone(),
        },
        ApprovalRequestKind::NetworkAccess(request) => GuardianAssessmentAction::NetworkAccess {
            target: request.target.clone(),
            host: request.host.clone(),
            protocol: request.protocol,
            port: request.port,
        },
        ApprovalRequestKind::McpToolCall(request) => GuardianAssessmentAction::McpToolCall {
            server: request.server.clone(),
            tool_name: request.tool_name.clone(),
            connector_id: request.connector_id.clone(),
            connector_name: request.connector_name.clone(),
            tool_title: request.tool_title.clone(),
        },
    }
}

pub(crate) fn guardian_request_target_item_id(request: &ApprovalRequest) -> Option<&str> {
    match &request.kind {
        ApprovalRequestKind::Command(request) => Some(&request.id),
        ApprovalRequestKind::Patch(request) => Some(&request.id),
        ApprovalRequestKind::McpToolCall(request) => Some(&request.id),
        ApprovalRequestKind::NetworkAccess(_) => None,
        #[cfg(unix)]
        ApprovalRequestKind::Execve(request) => Some(&request.id),
    }
}

pub(crate) fn guardian_request_turn_id<'a>(
    request: &'a ApprovalRequest,
    default_turn_id: &'a str,
) -> &'a str {
    match &request.kind {
        ApprovalRequestKind::NetworkAccess(request) => &request.turn_id,
        ApprovalRequestKind::Command(_)
        | ApprovalRequestKind::Patch(_)
        | ApprovalRequestKind::McpToolCall(_) => default_turn_id,
        #[cfg(unix)]
        ApprovalRequestKind::Execve(_) => default_turn_id,
    }
}

pub(crate) fn format_guardian_action_pretty(
    action: &ApprovalRequest,
) -> serde_json::Result<String> {
    let mut value = guardian_approval_request_to_json(action)?;
    value = truncate_guardian_action_value(value);
    serde_json::to_string_pretty(&value)
}
