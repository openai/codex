//! Serializes Guardian approval requests and formats their model-visible action JSON.
//! Field names, optional-field omission, and review formatting match the existing wire shapes.

use crate::GuardianApprovalRequest;
use crate::GuardianMcpAnnotations;
use crate::GuardianNetworkAccessTrigger;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_utils_path_uri::LegacyAppPathString;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct CommandApprovalAction<'a> {
    tool: &'a str,
    command: &'a [String],
    cwd: LegacyAppPathString,
    sandbox_permissions: SandboxPermissions,
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
    session_id: i32,
    original_command: &'a str,
    chars: &'a str,
    cwd: LegacyAppPathString,
    sandbox_permissions: SandboxPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_permissions: Option<&'a AdditionalPermissionProfile>,
    tty: bool,
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
    sandbox_permissions: SandboxPermissions,
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

pub fn guardian_approval_request_to_json(
    action: &GuardianApprovalRequest,
) -> serde_json::Result<Value> {
    let mut value = match action {
        GuardianApprovalRequest::NativeToolCall { call, .. } => serde_json::to_value(call.as_ref()),
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
            process_id,
            original_command,
            input,
            cwd,
            tty,
            sandbox_permissions,
            additional_permissions,
            ..
        } => serialize_guardian_action(WriteStdinApprovalAction {
            tool: "write_stdin",
            session_id: *process_id,
            original_command,
            chars: input,
            cwd: cwd.clone().into(),
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.as_ref(),
            tty: *tty,
        }),
        GuardianApprovalRequest::ApplyPatch {
            id: _,
            environment_id: _,
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
            environment_id: _,
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
            environment_id: _,
            turn_id,
            reason,
            permissions,
        } => serialize_guardian_action(RequestPermissionsApprovalAction {
            tool: "request_permissions",
            turn_id,
            reason: reason.as_ref(),
            permissions,
        }),
    }?;
    if let Some(environment_id) = action.target_environment_id() {
        value["environment_id"] = environment_id.into();
    }
    Ok(value)
}

pub fn format_guardian_action_pretty(
    action: &GuardianApprovalRequest,
) -> serde_json::Result<String> {
    let mut value =
        codex_guardian_context::action_for_review(guardian_approval_request_to_json(action)?);
    value.sort_all_objects();
    serde_json::to_string_pretty(&value)
}

#[cfg(test)]
#[path = "approval_request_json_tests.rs"]
mod tests;
