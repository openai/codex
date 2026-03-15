use std::path::PathBuf;

use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Serialize;
use serde_json::Value;

use super::GUARDIAN_MAX_ACTION_STRING_TOKENS;
use super::prompt::guardian_truncate_text;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GuardianApprovalRequest {
    Shell {
        id: String,
        command: Vec<String>,
        cwd: PathBuf,
        sandbox_permissions: crate::sandboxing::SandboxPermissions,
        additional_permissions: Option<PermissionProfile>,
        justification: Option<String>,
    },
    ExecCommand {
        id: String,
        command: Vec<String>,
        cwd: PathBuf,
        sandbox_permissions: crate::sandboxing::SandboxPermissions,
        additional_permissions: Option<PermissionProfile>,
        justification: Option<String>,
        tty: bool,
    },
    #[cfg(unix)]
    Execve {
        id: String,
        tool_name: String,
        program: String,
        argv: Vec<String>,
        cwd: PathBuf,
        additional_permissions: Option<PermissionProfile>,
    },
    ApplyPatch {
        id: String,
        cwd: PathBuf,
        files: Vec<AbsolutePathBuf>,
        change_count: usize,
        patch: String,
    },
    NetworkAccess {
        id: String,
        turn_id: String,
        target: String,
        host: String,
        protocol: NetworkApprovalProtocol,
        port: u16,
    },
    McpToolCall {
        id: String,
        server: String,
        tool_name: String,
        arguments: Option<Value>,
        connector_id: Option<String>,
        connector_name: Option<String>,
        connector_description: Option<String>,
        tool_title: Option<String>,
        tool_description: Option<String>,
        annotations: Option<GuardianMcpAnnotations>,
    },
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

pub(crate) fn guardian_approval_request_to_json(action: &GuardianApprovalRequest) -> Value {
    match action {
        GuardianApprovalRequest::Shell {
            id: _,
            command,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
        } => {
            let mut action = serde_json::json!({
                "tool": "shell",
                "command": command,
                "cwd": cwd,
                "sandbox_permissions": sandbox_permissions,
                "additional_permissions": additional_permissions,
                "justification": justification,
            });
            if let Some(action) = action.as_object_mut() {
                if additional_permissions.is_none() {
                    action.remove("additional_permissions");
                }
                if justification.is_none() {
                    action.remove("justification");
                }
            }
            action
        }
        GuardianApprovalRequest::ExecCommand {
            id: _,
            command,
            cwd,
            sandbox_permissions,
            additional_permissions,
            justification,
            tty,
        } => {
            let mut action = serde_json::json!({
                "tool": "exec_command",
                "command": command,
                "cwd": cwd,
                "sandbox_permissions": sandbox_permissions,
                "additional_permissions": additional_permissions,
                "justification": justification,
                "tty": tty,
            });
            if let Some(action) = action.as_object_mut() {
                if additional_permissions.is_none() {
                    action.remove("additional_permissions");
                }
                if justification.is_none() {
                    action.remove("justification");
                }
            }
            action
        }
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            id: _,
            tool_name,
            program,
            argv,
            cwd,
            additional_permissions,
        } => {
            let mut action = serde_json::json!({
                "tool": tool_name,
                "program": program,
                "argv": argv,
                "cwd": cwd,
                "additional_permissions": additional_permissions,
            });
            if let Some(action) = action.as_object_mut()
                && additional_permissions.is_none()
            {
                action.remove("additional_permissions");
            }
            action
        }
        GuardianApprovalRequest::ApplyPatch {
            id: _,
            cwd,
            files,
            change_count,
            patch,
        } => serde_json::json!({
            "tool": "apply_patch",
            "cwd": cwd,
            "files": files,
            "change_count": change_count,
            "patch": patch,
        }),
        GuardianApprovalRequest::NetworkAccess {
            id: _,
            turn_id: _,
            target,
            host,
            protocol,
            port,
        } => serde_json::json!({
            "tool": "network_access",
            "target": target,
            "host": host,
            "protocol": protocol,
            "port": port,
        }),
        GuardianApprovalRequest::McpToolCall {
            id: _,
            server,
            tool_name,
            arguments,
            connector_id,
            connector_name,
            connector_description,
            tool_title,
            tool_description,
            annotations,
        } => {
            let mut action = serde_json::json!({
                "tool": "mcp_tool_call",
                "server": server,
                "tool_name": tool_name,
                "arguments": arguments,
                "connector_id": connector_id,
                "connector_name": connector_name,
                "connector_description": connector_description,
                "tool_title": tool_title,
                "tool_description": tool_description,
                "annotations": annotations,
            });
            if let Some(action) = action.as_object_mut() {
                for (key, remove) in [
                    ("arguments", arguments.is_none()),
                    ("connector_id", connector_id.is_none()),
                    ("connector_name", connector_name.is_none()),
                    ("connector_description", connector_description.is_none()),
                    ("tool_title", tool_title.is_none()),
                    ("tool_description", tool_description.is_none()),
                    ("annotations", annotations.is_none()),
                ] {
                    if remove {
                        action.remove(key);
                    }
                }
            }
            action
        }
    }
}

pub(crate) fn guardian_assessment_action_value(action: &GuardianApprovalRequest) -> Value {
    match action {
        GuardianApprovalRequest::Shell { command, cwd, .. } => serde_json::json!({
            "tool": "shell",
            "command": codex_shell_command::parse_command::shlex_join(command),
            "cwd": cwd,
        }),
        GuardianApprovalRequest::ExecCommand { command, cwd, .. } => serde_json::json!({
            "tool": "exec_command",
            "command": codex_shell_command::parse_command::shlex_join(command),
            "cwd": cwd,
        }),
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            tool_name,
            program,
            argv,
            cwd,
            ..
        } => serde_json::json!({
            "tool": tool_name,
            "program": program,
            "argv": argv,
            "cwd": cwd,
        }),
        GuardianApprovalRequest::ApplyPatch {
            cwd,
            files,
            change_count,
            ..
        } => serde_json::json!({
            "tool": "apply_patch",
            "cwd": cwd,
            "files": files,
            "change_count": change_count,
        }),
        GuardianApprovalRequest::NetworkAccess {
            id: _,
            turn_id: _,
            target,
            host,
            protocol,
            port,
        } => serde_json::json!({
            "tool": "network_access",
            "target": target,
            "host": host,
            "protocol": protocol,
            "port": port,
        }),
        GuardianApprovalRequest::McpToolCall {
            server, tool_name, ..
        } => serde_json::json!({
            "tool": "mcp_tool_call",
            "server": server,
            "tool_name": tool_name,
        }),
    }
}

pub(crate) fn guardian_request_id(request: &GuardianApprovalRequest) -> &str {
    match request {
        GuardianApprovalRequest::Shell { id, .. }
        | GuardianApprovalRequest::ExecCommand { id, .. }
        | GuardianApprovalRequest::ApplyPatch { id, .. }
        | GuardianApprovalRequest::NetworkAccess { id, .. }
        | GuardianApprovalRequest::McpToolCall { id, .. } => id,
        #[cfg(unix)]
        GuardianApprovalRequest::Execve { id, .. } => id,
    }
}

pub(crate) fn guardian_request_turn_id<'a>(
    request: &'a GuardianApprovalRequest,
    default_turn_id: &'a str,
) -> &'a str {
    match request {
        GuardianApprovalRequest::NetworkAccess { turn_id, .. } => turn_id,
        GuardianApprovalRequest::Shell { .. }
        | GuardianApprovalRequest::ExecCommand { .. }
        | GuardianApprovalRequest::ApplyPatch { .. }
        | GuardianApprovalRequest::McpToolCall { .. } => default_turn_id,
        #[cfg(unix)]
        GuardianApprovalRequest::Execve { .. } => default_turn_id,
    }
}

pub(crate) fn format_guardian_action_pretty(action: &GuardianApprovalRequest) -> String {
    let mut value = guardian_approval_request_to_json(action);
    value = truncate_guardian_action_value(value);
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".to_string())
}
