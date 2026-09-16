use std::fmt;
use std::path::PathBuf;

use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::UserVerificationProof;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::ThreadId;
use codex_protocol::approvals::GuardianAssessmentEvent;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use serde::Serialize;
use serde::Serializer;
use serde_json::Value;

/// Keeps ICE credentials out of command diagnostics and session recordings.
/// Convert back to a string only when sending the offer to app-server signaling.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RealtimeOfferSdp(String);

impl From<String> for RealtimeOfferSdp {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<RealtimeOfferSdp> for String {
    fn from(value: RealtimeOfferSdp) -> Self {
        value.0
    }
}

impl fmt::Debug for RealtimeOfferSdp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

impl Serialize for RealtimeOfferSdp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("<redacted>")
    }
}

/// Keeps spoken answers out of command diagnostics and session recordings.
/// Convert back to a string only at the app-server signaling boundary.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RealtimeSpeechText(String);

impl From<String> for RealtimeSpeechText {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<RealtimeSpeechText> for String {
    fn from(value: RealtimeSpeechText) -> Self {
        value.0
    }
}

impl RealtimeSpeechText {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RealtimeSpeechText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

impl Serialize for RealtimeSpeechText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("<redacted>")
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) enum AppCommand {
    Interrupt,
    CleanBackgroundTerminals,
    RealtimeConversationStart {
        thread_id: ThreadId,
        offer_sdp: RealtimeOfferSdp,
    },
    RealtimeConversationStop {
        thread_id: ThreadId,
    },
    RealtimeConversationSpeech {
        thread_id: ThreadId,
        attempt_id: u64,
        input_generation: u64,
        delivery_id: u64,
        text: RealtimeSpeechText,
    },
    RunUserShellCommand {
        command: String,
    },
    UserTurn {
        client_user_message_id: String,
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        approvals_reviewer: Option<ApprovalsReviewer>,
        active_permission_profile: Option<ActivePermissionProfile>,
        model: String,
        effort: Option<ReasoningEffortConfig>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<String>>,
        final_output_json_schema: Option<Value>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    },
    OverrideTurnContext {
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        approvals_reviewer: Option<ApprovalsReviewer>,
        permission_profile: Option<PermissionProfile>,
        active_permission_profile: Option<ActivePermissionProfile>,
        model: Option<String>,
        effort: Option<Option<ReasoningEffortConfig>>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<String>>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    },
    ExecApproval {
        id: String,
        turn_id: Option<String>,
        decision: CommandExecutionApprovalDecision,
    },
    PatchApproval {
        id: String,
        decision: FileChangeApprovalDecision,
    },
    ResolveElicitation {
        server_name: String,
        request_id: AppServerRequestId,
        decision: McpServerElicitationAction,
        content: Option<Value>,
        meta: Option<Value>,
    },
    ResolveUserVerification {
        server_name: String,
        request_id: AppServerRequestId,
        response: UserVerificationResponse,
    },
    UserInputAnswer {
        id: String,
        response: ToolRequestUserInputResponse,
    },
    RequestPermissionsResponse {
        id: String,
        response: RequestPermissionsResponse,
    },
    ReloadUserConfig,
    ListSkills {
        cwds: Vec<PathBuf>,
        force_reload: bool,
    },
    Compact,
    SetThreadName {
        name: String,
    },
    Review {
        target: ReviewTarget,
    },
    ApproveGuardianDeniedAction {
        event: GuardianAssessmentEvent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) enum UserVerificationResponse {
    Accept {
        // AppCommand serialization is used by session recording, not the RPC wire response.
        // The controller serializes the assertion only into McpServerElicitationRequestResponse.
        #[serde(skip_serializing)]
        proof: UserVerificationProof,
    },
    Cancel,
}

impl AppCommand {
    pub(crate) fn interrupt() -> Self {
        Self::Interrupt
    }

    pub(crate) fn clean_background_terminals() -> Self {
        Self::CleanBackgroundTerminals
    }

    pub(crate) fn run_user_shell_command(command: String) -> Self {
        Self::RunUserShellCommand { command }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn user_turn(
        client_user_message_id: String,
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        active_permission_profile: Option<ActivePermissionProfile>,
        model: String,
        effort: Option<ReasoningEffortConfig>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<String>>,
        final_output_json_schema: Option<Value>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    ) -> Self {
        Self::UserTurn {
            client_user_message_id,
            items,
            cwd,
            approval_policy,
            approvals_reviewer: None,
            active_permission_profile,
            model,
            effort,
            summary,
            service_tier,
            final_output_json_schema,
            collaboration_mode,
            personality,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn override_turn_context(
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        approvals_reviewer: Option<ApprovalsReviewer>,
        permission_profile: Option<PermissionProfile>,
        active_permission_profile: Option<ActivePermissionProfile>,
        model: Option<String>,
        effort: Option<Option<ReasoningEffortConfig>>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<String>>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    ) -> Self {
        Self::OverrideTurnContext {
            cwd,
            approval_policy,
            approvals_reviewer,
            permission_profile,
            active_permission_profile,
            model,
            effort,
            summary,
            service_tier,
            collaboration_mode,
            personality,
        }
    }

    pub(crate) fn exec_approval(
        id: String,
        turn_id: Option<String>,
        decision: CommandExecutionApprovalDecision,
    ) -> Self {
        Self::ExecApproval {
            id,
            turn_id,
            decision,
        }
    }

    pub(crate) fn patch_approval(id: String, decision: FileChangeApprovalDecision) -> Self {
        Self::PatchApproval { id, decision }
    }

    pub(crate) fn resolve_elicitation(
        server_name: String,
        request_id: AppServerRequestId,
        decision: McpServerElicitationAction,
        content: Option<Value>,
        meta: Option<Value>,
    ) -> Self {
        Self::ResolveElicitation {
            server_name,
            request_id,
            decision,
            content,
            meta,
        }
    }

    pub(crate) fn resolve_user_verification(
        server_name: String,
        request_id: AppServerRequestId,
        response: UserVerificationResponse,
    ) -> Self {
        Self::ResolveUserVerification {
            server_name,
            request_id,
            response,
        }
    }

    pub(crate) fn user_input_answer(id: String, response: ToolRequestUserInputResponse) -> Self {
        Self::UserInputAnswer { id, response }
    }

    pub(crate) fn request_permissions_response(
        id: String,
        response: RequestPermissionsResponse,
    ) -> Self {
        Self::RequestPermissionsResponse { id, response }
    }

    pub(crate) fn reload_user_config() -> Self {
        Self::ReloadUserConfig
    }

    pub(crate) fn list_skills(cwds: Vec<PathBuf>, force_reload: bool) -> Self {
        Self::ListSkills { cwds, force_reload }
    }

    pub(crate) fn compact() -> Self {
        Self::Compact
    }

    pub(crate) fn set_thread_name(name: String) -> Self {
        Self::SetThreadName { name }
    }

    pub(crate) fn review(target: ReviewTarget) -> Self {
        Self::Review { target }
    }

    pub(crate) fn approve_guardian_denied_action(event: GuardianAssessmentEvent) -> Self {
        Self::ApproveGuardianDeniedAction { event }
    }

    pub(crate) fn is_review(&self) -> bool {
        matches!(self, Self::Review { .. })
    }
}

impl From<&AppCommand> for AppCommand {
    fn from(value: &AppCommand) -> Self {
        value.clone()
    }
}

#[cfg(test)]
#[path = "app_command_tests.rs"]
mod tests;
