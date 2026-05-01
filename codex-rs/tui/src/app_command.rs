use std::path::PathBuf;

use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ThreadRealtimeAudioChunk;
use codex_app_server_protocol::ThreadRealtimeStartTransport;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_app_server_protocol::UserInput;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::approvals::GuardianAssessmentEvent;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use serde::Serialize;
use serde_json::Value;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) enum AppCommand {
    Interrupt,
    CleanBackgroundTerminals,
    RealtimeConversationStart {
        transport: Option<ThreadRealtimeStartTransport>,
        voice: Option<Value>,
    },
    RealtimeConversationAudio(ThreadRealtimeAudioChunk),
    RealtimeConversationClose,
    RunUserShellCommand {
        command: String,
    },
    UserTurn {
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        approvals_reviewer: Option<ApprovalsReviewer>,
        permission_profile: PermissionProfile,
        model: String,
        effort: Option<ReasoningEffortConfig>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<ServiceTier>>,
        final_output_json_schema: Option<Value>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    },
    OverrideTurnContext {
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        approvals_reviewer: Option<ApprovalsReviewer>,
        permission_profile: Option<PermissionProfile>,
        windows_sandbox_level: Option<WindowsSandboxLevel>,
        model: Option<String>,
        effort: Option<Option<ReasoningEffortConfig>>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<ServiceTier>>,
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
    Shutdown,
    ThreadRollback {
        num_turns: u32,
    },
    Review {
        target: ReviewTarget,
    },
    AddToHistory {
        text: String,
    },
    GetHistoryEntryRequest {
        offset: usize,
        log_id: u64,
    },
    ApproveGuardianDeniedAction {
        event: GuardianAssessmentEvent,
    },
}

impl AppCommand {
    pub(crate) fn interrupt() -> Self {
        Self::Interrupt
    }

    pub(crate) fn clean_background_terminals() -> Self {
        Self::CleanBackgroundTerminals
    }

    pub(crate) fn realtime_conversation_start(
        transport: Option<ThreadRealtimeStartTransport>,
        voice: Option<Value>,
    ) -> Self {
        Self::RealtimeConversationStart { transport, voice }
    }

    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub(crate) fn realtime_conversation_audio(frame: ThreadRealtimeAudioChunk) -> Self {
        Self::RealtimeConversationAudio(frame)
    }

    pub(crate) fn realtime_conversation_close() -> Self {
        Self::RealtimeConversationClose
    }

    pub(crate) fn run_user_shell_command(command: String) -> Self {
        Self::RunUserShellCommand { command }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn user_turn(
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        permission_profile: PermissionProfile,
        model: String,
        effort: Option<ReasoningEffortConfig>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<ServiceTier>>,
        final_output_json_schema: Option<Value>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    ) -> Self {
        Self::UserTurn {
            items,
            cwd,
            approval_policy,
            approvals_reviewer: None,
            permission_profile,
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
        windows_sandbox_level: Option<WindowsSandboxLevel>,
        model: Option<String>,
        effort: Option<Option<ReasoningEffortConfig>>,
        summary: Option<ReasoningSummaryConfig>,
        service_tier: Option<Option<ServiceTier>>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    ) -> Self {
        Self::OverrideTurnContext {
            cwd,
            approval_policy,
            approvals_reviewer,
            permission_profile,
            windows_sandbox_level,
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

    #[allow(dead_code)]
    pub(crate) fn shutdown() -> Self {
        Self::Shutdown
    }

    pub(crate) fn thread_rollback(num_turns: u32) -> Self {
        Self::ThreadRollback { num_turns }
    }

    pub(crate) fn review(target: ReviewTarget) -> Self {
        Self::Review { target }
    }

    pub(crate) fn add_to_history(text: String) -> Self {
        Self::AddToHistory { text }
    }

    pub(crate) fn history_lookup(offset: usize, log_id: u64) -> Self {
        Self::GetHistoryEntryRequest { offset, log_id }
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
