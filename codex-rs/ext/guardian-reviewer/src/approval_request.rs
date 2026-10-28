//! Owns Guardian approval-request data, classification, and assessment/reporting mappings.
//! Request debug output excludes action arguments, which may contain credentials.

use codex_analytics::GuardianReviewedAction;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::approvals::GuardianCommandSource;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, PartialEq)]
pub enum GuardianApprovalRequest {
    NativeToolCall {
        id: String,
        call: Box<codex_protocol::models::ResponseItem>,
    },
    ExecCommand {
        id: String,
        environment_id: String,
        command: Vec<String>,
        cwd: PathUri,
        /// Executor-native rendering sent to Guardian; `cwd` remains typed for attribution.
        guardian_cwd: LegacyAppPathString,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
        justification: Option<String>,
        tty: bool,
    },
    WriteStdin {
        id: String,
        approval_id: String,
        environment_id: String,
        process_id: i32,
        /// Command used to launch the receiving terminal, not its current state.
        original_command: String,
        input: String,
        cwd: PathUri,
        tty: bool,
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
    },
    ApplyPatch {
        id: String,
        environment_id: String,
        cwd: PathUri,
        files: Vec<PathUri>,
        patch: String,
    },
    NetworkAccess {
        id: String,
        turn_id: String,
        environment_id: String,
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
        environment_id: String,
        turn_id: String,
        reason: Option<String>,
        permissions: RequestPermissionProfile,
    },
}

// Approval arguments may contain credentials or encrypted tool messages.
impl std::fmt::Debug for GuardianApprovalRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GuardianApprovalRequest")
            .field("kind", &std::mem::discriminant(self))
            .finish_non_exhaustive()
    }
}

impl GuardianApprovalRequest {
    /// The environment owning this action, independent of the reviewer's default environment.
    pub fn target_environment_id(&self) -> Option<&str> {
        match self {
            Self::ExecCommand { environment_id, .. }
            | Self::WriteStdin { environment_id, .. }
            | Self::ApplyPatch { environment_id, .. }
            | Self::RequestPermissions { environment_id, .. }
            | Self::NetworkAccess { environment_id, .. } => Some(environment_id),
            Self::McpToolCall { .. } | Self::NativeToolCall { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianNetworkAccessTrigger {
    pub call_id: String,
    pub tool_name: String,
    pub command: Vec<String>,
    pub cwd: PathUri,
    pub sandbox_permissions: SandboxPermissions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardianMcpAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
}

impl GuardianApprovalRequest {
    pub fn guardian_scope(&self) -> GuardianScope {
        match self {
            Self::ExecCommand { .. } | Self::WriteStdin { .. } => GuardianScope::Shell,
            Self::ApplyPatch { .. } => GuardianScope::FileChanges,
            Self::McpToolCall { server, .. } => GuardianScope::for_mcp_server(server),
            Self::NetworkAccess { .. } => GuardianScope::Network,
            Self::RequestPermissions { .. } => GuardianScope::Permissions,
            Self::NativeToolCall { .. } => GuardianScope::NativeTool,
        }
    }
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

pub fn guardian_assessment_action(
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

pub fn guardian_reviewed_action(request: &GuardianApprovalRequest) -> GuardianReviewedAction {
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

pub fn guardian_request_target_item_id(request: &GuardianApprovalRequest) -> Option<&str> {
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

pub fn guardian_request_turn_id<'a>(
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

#[cfg(test)]
#[path = "approval_request_tests.rs"]
mod tests;
