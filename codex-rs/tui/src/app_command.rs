use std::path::PathBuf;

use codex_config::types::ApprovalsReviewer;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::GuardianAssessmentEvent;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::mcp::RequestId as McpRequestId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use serde::Serialize;
use serde_json::Value;

use crate::permission_compat::legacy_compatible_permission_profile;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) enum AppCommand {
    Interrupt,
    CleanBackgroundTerminals,
    RealtimeConversationStart(ConversationStartParams),
    RealtimeConversationAudio(ConversationAudioParams),
    RealtimeConversationText(ConversationTextParams),
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
        decision: ReviewDecision,
    },
    PatchApproval {
        id: String,
        decision: ReviewDecision,
    },
    ResolveElicitation {
        server_name: String,
        request_id: McpRequestId,
        decision: ElicitationAction,
        content: Option<Value>,
        meta: Option<Value>,
    },
    UserInputAnswer {
        id: String,
        response: RequestUserInputResponse,
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
        review_request: ReviewRequest,
    },
    ApproveGuardianDeniedAction {
        event: GuardianAssessmentEvent,
    },
    Other(Op),
}

#[allow(clippy::large_enum_variant)]
#[allow(dead_code)]
pub(crate) enum AppCommandView<'a> {
    Interrupt,
    CleanBackgroundTerminals,
    RealtimeConversationStart(&'a ConversationStartParams),
    RealtimeConversationAudio(&'a ConversationAudioParams),
    RealtimeConversationText(&'a ConversationTextParams),
    RealtimeConversationClose,
    RunUserShellCommand {
        command: &'a str,
    },
    UserTurn {
        items: &'a [UserInput],
        cwd: &'a PathBuf,
        approval_policy: AskForApproval,
        approvals_reviewer: &'a Option<ApprovalsReviewer>,
        permission_profile: &'a PermissionProfile,
        model: &'a str,
        effort: Option<ReasoningEffortConfig>,
        summary: &'a Option<ReasoningSummaryConfig>,
        service_tier: &'a Option<Option<ServiceTier>>,
        final_output_json_schema: &'a Option<Value>,
        collaboration_mode: &'a Option<CollaborationMode>,
        personality: &'a Option<Personality>,
    },
    OverrideTurnContext {
        cwd: &'a Option<PathBuf>,
        approval_policy: &'a Option<AskForApproval>,
        approvals_reviewer: &'a Option<ApprovalsReviewer>,
        permission_profile: &'a Option<PermissionProfile>,
        windows_sandbox_level: &'a Option<WindowsSandboxLevel>,
        model: &'a Option<String>,
        effort: &'a Option<Option<ReasoningEffortConfig>>,
        summary: &'a Option<ReasoningSummaryConfig>,
        service_tier: &'a Option<Option<ServiceTier>>,
        collaboration_mode: &'a Option<CollaborationMode>,
        personality: &'a Option<Personality>,
    },
    ExecApproval {
        id: &'a str,
        turn_id: &'a Option<String>,
        decision: &'a ReviewDecision,
    },
    PatchApproval {
        id: &'a str,
        decision: &'a ReviewDecision,
    },
    ResolveElicitation {
        server_name: &'a str,
        request_id: &'a McpRequestId,
        decision: &'a ElicitationAction,
        content: &'a Option<Value>,
        meta: &'a Option<Value>,
    },
    UserInputAnswer {
        id: &'a str,
        response: &'a RequestUserInputResponse,
    },
    RequestPermissionsResponse {
        id: &'a str,
        response: &'a RequestPermissionsResponse,
    },
    ReloadUserConfig,
    ListSkills {
        cwds: &'a [PathBuf],
        force_reload: bool,
    },
    Compact,
    SetThreadName {
        name: &'a str,
    },
    Shutdown,
    ThreadRollback {
        num_turns: u32,
    },
    Review {
        review_request: &'a ReviewRequest,
    },
    ApproveGuardianDeniedAction {
        event: &'a GuardianAssessmentEvent,
    },
    Other(&'a Op),
}

impl AppCommand {
    pub(crate) fn interrupt() -> Self {
        Self::Interrupt
    }

    pub(crate) fn clean_background_terminals() -> Self {
        Self::CleanBackgroundTerminals
    }

    pub(crate) fn realtime_conversation_start(params: ConversationStartParams) -> Self {
        Self::RealtimeConversationStart(params)
    }

    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub(crate) fn realtime_conversation_audio(params: ConversationAudioParams) -> Self {
        Self::RealtimeConversationAudio(params)
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
        decision: ReviewDecision,
    ) -> Self {
        Self::ExecApproval {
            id,
            turn_id,
            decision,
        }
    }

    pub(crate) fn patch_approval(id: String, decision: ReviewDecision) -> Self {
        Self::PatchApproval { id, decision }
    }

    pub(crate) fn resolve_elicitation(
        server_name: String,
        request_id: McpRequestId,
        decision: ElicitationAction,
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

    pub(crate) fn user_input_answer(id: String, response: RequestUserInputResponse) -> Self {
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

    pub(crate) fn thread_rollback(num_turns: u32) -> Self {
        Self::ThreadRollback { num_turns }
    }

    pub(crate) fn review(review_request: ReviewRequest) -> Self {
        Self::Review { review_request }
    }

    pub(crate) fn into_core(self) -> Op {
        match self {
            Self::Interrupt => Op::Interrupt,
            Self::CleanBackgroundTerminals => Op::CleanBackgroundTerminals,
            Self::RealtimeConversationStart(params) => Op::RealtimeConversationStart(params),
            Self::RealtimeConversationAudio(params) => Op::RealtimeConversationAudio(params),
            Self::RealtimeConversationText(params) => Op::RealtimeConversationText(params),
            Self::RealtimeConversationClose => Op::RealtimeConversationClose,
            Self::RunUserShellCommand { command } => Op::RunUserShellCommand { command },
            Self::UserTurn {
                items,
                cwd,
                approval_policy,
                approvals_reviewer,
                permission_profile,
                model,
                effort,
                summary,
                service_tier,
                final_output_json_schema,
                collaboration_mode,
                personality,
            } => {
                let legacy_profile =
                    legacy_compatible_permission_profile(&permission_profile, cwd.as_path());
                let sandbox_policy = legacy_profile
                    .to_legacy_sandbox_policy(cwd.as_path())
                    .unwrap_or_else(|err| {
                        unreachable!(
                            "legacy-compatible permissions must project to legacy policy: {err}"
                        )
                    });
                Op::UserTurn {
                    items,
                    environments: None,
                    cwd,
                    approval_policy,
                    approvals_reviewer,
                    sandbox_policy,
                    permission_profile: Some(permission_profile),
                    model,
                    effort,
                    summary,
                    service_tier,
                    final_output_json_schema,
                    collaboration_mode,
                    personality,
                }
            }
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
            } => Op::OverrideTurnContext {
                cwd,
                approval_policy,
                approvals_reviewer,
                sandbox_policy: None,
                permission_profile,
                windows_sandbox_level,
                model,
                effort,
                summary,
                service_tier,
                collaboration_mode,
                personality,
            },
            Self::ExecApproval {
                id,
                turn_id,
                decision,
            } => Op::ExecApproval {
                id,
                turn_id,
                decision,
            },
            Self::PatchApproval { id, decision } => Op::PatchApproval { id, decision },
            Self::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => Op::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            },
            Self::UserInputAnswer { id, response } => Op::UserInputAnswer { id, response },
            Self::RequestPermissionsResponse { id, response } => {
                Op::RequestPermissionsResponse { id, response }
            }
            Self::ReloadUserConfig => Op::ReloadUserConfig,
            Self::ListSkills { cwds, force_reload } => Op::ListSkills { cwds, force_reload },
            Self::Compact => Op::Compact,
            Self::SetThreadName { name } => Op::SetThreadName { name },
            Self::Shutdown => Op::Shutdown,
            Self::ThreadRollback { num_turns } => Op::ThreadRollback { num_turns },
            Self::Review { review_request } => Op::Review { review_request },
            Self::ApproveGuardianDeniedAction { event } => {
                Op::ApproveGuardianDeniedAction { event }
            }
            Self::Other(op) => op,
        }
    }

    pub(crate) fn is_review(&self) -> bool {
        matches!(self, Self::Review { .. })
    }

    pub(crate) fn view(&self) -> AppCommandView<'_> {
        match self {
            Self::Interrupt => AppCommandView::Interrupt,
            Self::CleanBackgroundTerminals => AppCommandView::CleanBackgroundTerminals,
            Self::RealtimeConversationStart(params) => {
                AppCommandView::RealtimeConversationStart(params)
            }
            Self::RealtimeConversationAudio(params) => {
                AppCommandView::RealtimeConversationAudio(params)
            }
            Self::RealtimeConversationText(params) => {
                AppCommandView::RealtimeConversationText(params)
            }
            Self::RealtimeConversationClose => AppCommandView::RealtimeConversationClose,
            Self::RunUserShellCommand { command } => {
                AppCommandView::RunUserShellCommand { command }
            }
            Self::UserTurn {
                items,
                cwd,
                approval_policy,
                approvals_reviewer,
                permission_profile,
                model,
                effort,
                summary,
                service_tier,
                final_output_json_schema,
                collaboration_mode,
                personality,
            } => AppCommandView::UserTurn {
                items,
                cwd,
                approval_policy: *approval_policy,
                approvals_reviewer,
                permission_profile,
                model,
                effort: *effort,
                summary,
                service_tier,
                final_output_json_schema,
                collaboration_mode,
                personality,
            },
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
            } => AppCommandView::OverrideTurnContext {
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
            },
            Self::ExecApproval {
                id,
                turn_id,
                decision,
            } => AppCommandView::ExecApproval {
                id,
                turn_id,
                decision,
            },
            Self::PatchApproval { id, decision } => AppCommandView::PatchApproval { id, decision },
            Self::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => AppCommandView::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            },
            Self::UserInputAnswer { id, response } => {
                AppCommandView::UserInputAnswer { id, response }
            }
            Self::RequestPermissionsResponse { id, response } => {
                AppCommandView::RequestPermissionsResponse { id, response }
            }
            Self::ReloadUserConfig => AppCommandView::ReloadUserConfig,
            Self::ListSkills { cwds, force_reload } => AppCommandView::ListSkills {
                cwds,
                force_reload: *force_reload,
            },
            Self::Compact => AppCommandView::Compact,
            Self::SetThreadName { name } => AppCommandView::SetThreadName { name },
            Self::Shutdown => AppCommandView::Shutdown,
            Self::ThreadRollback { num_turns } => AppCommandView::ThreadRollback {
                num_turns: *num_turns,
            },
            Self::Review { review_request } => AppCommandView::Review { review_request },
            Self::ApproveGuardianDeniedAction { event } => {
                AppCommandView::ApproveGuardianDeniedAction { event }
            }
            Self::Other(op) => AppCommandView::Other(op),
        }
    }
}

impl From<Op> for AppCommand {
    fn from(value: Op) -> Self {
        match value {
            Op::Interrupt => Self::Interrupt,
            Op::CleanBackgroundTerminals => Self::CleanBackgroundTerminals,
            Op::RealtimeConversationStart(params) => Self::RealtimeConversationStart(params),
            Op::RealtimeConversationAudio(params) => Self::RealtimeConversationAudio(params),
            Op::RealtimeConversationText(params) => Self::RealtimeConversationText(params),
            Op::RealtimeConversationClose => Self::RealtimeConversationClose,
            Op::RunUserShellCommand { command } => Self::RunUserShellCommand { command },
            Op::UserTurn {
                items,
                environments,
                cwd,
                approval_policy,
                approvals_reviewer,
                sandbox_policy,
                permission_profile: Some(permission_profile),
                model,
                effort,
                summary,
                service_tier,
                final_output_json_schema,
                collaboration_mode,
                personality,
            } => {
                if environments.is_none()
                    && legacy_compatible_permission_profile(&permission_profile, cwd.as_path())
                        .to_legacy_sandbox_policy(cwd.as_path())
                        .is_ok_and(|compatible_policy| compatible_policy == sandbox_policy)
                {
                    Self::UserTurn {
                        items,
                        cwd,
                        approval_policy,
                        approvals_reviewer,
                        permission_profile,
                        model,
                        effort,
                        summary,
                        service_tier,
                        final_output_json_schema,
                        collaboration_mode,
                        personality,
                    }
                } else {
                    Self::Other(Op::UserTurn {
                        items,
                        cwd,
                        approval_policy,
                        approvals_reviewer,
                        sandbox_policy,
                        permission_profile: Some(permission_profile),
                        model,
                        effort,
                        summary,
                        service_tier,
                        final_output_json_schema,
                        collaboration_mode,
                        personality,
                        environments,
                    })
                }
            }
            Op::OverrideTurnContext {
                cwd,
                approval_policy,
                approvals_reviewer,
                sandbox_policy,
                permission_profile,
                windows_sandbox_level,
                model,
                effort,
                summary,
                service_tier,
                collaboration_mode,
                personality,
            } => {
                if sandbox_policy.is_none() {
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
                } else {
                    Self::Other(Op::OverrideTurnContext {
                        cwd,
                        approval_policy,
                        approvals_reviewer,
                        sandbox_policy,
                        permission_profile,
                        windows_sandbox_level,
                        model,
                        effort,
                        summary,
                        service_tier,
                        collaboration_mode,
                        personality,
                    })
                }
            }
            Op::ExecApproval {
                id,
                turn_id,
                decision,
            } => Self::ExecApproval {
                id,
                turn_id,
                decision,
            },
            Op::PatchApproval { id, decision } => Self::PatchApproval { id, decision },
            Op::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            } => Self::ResolveElicitation {
                server_name,
                request_id,
                decision,
                content,
                meta,
            },
            Op::UserInputAnswer { id, response } => Self::UserInputAnswer { id, response },
            Op::RequestPermissionsResponse { id, response } => {
                Self::RequestPermissionsResponse { id, response }
            }
            Op::ReloadUserConfig => Self::ReloadUserConfig,
            Op::ListSkills { cwds, force_reload } => Self::ListSkills { cwds, force_reload },
            Op::Compact => Self::Compact,
            Op::SetThreadName { name } => Self::SetThreadName { name },
            Op::Shutdown => Self::Shutdown,
            Op::ThreadRollback { num_turns } => Self::ThreadRollback { num_turns },
            Op::Review { review_request } => Self::Review { review_request },
            Op::ApproveGuardianDeniedAction { event } => {
                Self::ApproveGuardianDeniedAction { event }
            }
            op => Self::Other(op),
        }
    }
}

impl PartialEq<Op> for AppCommand {
    fn eq(&self, other: &Op) -> bool {
        self.clone().into_core() == *other
    }
}

impl PartialEq<AppCommand> for Op {
    fn eq(&self, other: &AppCommand) -> bool {
        *self == other.clone().into_core()
    }
}

impl From<&Op> for AppCommand {
    fn from(value: &Op) -> Self {
        Self::from(value.clone())
    }
}

impl From<&AppCommand> for AppCommand {
    fn from(value: &AppCommand) -> Self {
        value.clone()
    }
}

impl From<AppCommand> for Op {
    fn from(value: AppCommand) -> Self {
        value.into_core()
    }
}
