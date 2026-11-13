//! Projects the displayed conversation into the provider-facing statusline snapshot.

mod account;
mod usage;

use super::ChatWidget;
use super::normalize_thread_name;
use super::status_state::TerminalTitleStatusKind as TerminalTitleStatus;
use super::status_surfaces::permissions_display;
use crate::bottom_pane::StatusLineItem;
use crate::version::CODEX_CLI_VERSION;
use codex_protocol::config_types::ApprovalsReviewer as ConfigReviewer;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::models::ManagedFileSystemPermissions as ManagedFileSystem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::turn_input::CyberAccessProgram;
use serde::Serialize;
use strum_macros::Display;

#[derive(Serialize)]
pub(crate) struct StatuslineState {
    account: account::Account,
    app: StatuslineStateApp,
    config: StatuslineStateConfig,
    thread: StatuslineStateThread,
}

#[derive(Serialize)]
struct StatuslineStateApp {
    cli_version: &'static str,
    display_mode: crate::history_cell::HistoryRenderMode,
}

#[derive(Serialize)]
struct StatuslineStateConfig {
    statusline_items: Vec<StatusLineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
}

#[derive(Serialize)]
struct StatuslineStateThread {
    id: codex_protocol::ThreadId,
    name: Option<String>,
    agent: StatuslineStateAgent,
    model: StatuslineStateModel,
    workspace: StatuslineStateWorkspace,
    usage: usage::Usage,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval: Option<StatuslineStateApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<StatuslineStatePermissions>,
}

#[derive(Serialize)]
struct StatuslineStateWorkspace {
    cwd: codex_utils_path_uri::LegacyAppPathString,
}

#[derive(Serialize)]
struct StatuslineStateAgent {
    state: StatuslineStateRunState,
}

#[derive(Serialize, Display)]
#[serde(rename_all = "lowercase")]
pub(super) enum StatuslineStateRunState {
    Starting,
    Ready,
    Working,
    Waiting,
    Thinking,
}

#[derive(Serialize)]
struct StatuslineStateModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    access_program: Option<CyberAccessProgram>,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_level: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier_display_name: Option<String>,
}

#[derive(Serialize)]
struct StatuslineStateApproval {
    policy: StatuslineStateApprovalPolicy,
    reviewer: StatuslineStateApprovalReviewer,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum StatuslineStateApprovalPolicy {
    Untrusted,
    OnRequest,
    Granular,
    Never,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum StatuslineStateApprovalReviewer {
    User,
    AutoReview,
}

#[derive(Serialize)]
struct StatuslineStatePermissions {
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
    #[serde(flatten)]
    enforcement: StatuslineStateEnforcement,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum StatuslineStateEnforcement {
    Managed {
        file_system: StatuslineStateFileSystem,
        network: NetworkSandboxPolicy,
    },
    Disabled,
    External {
        network: NetworkSandboxPolicy,
    },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum StatuslineStateFileSystem {
    Restricted,
    Unrestricted,
}

impl From<PermissionProfile> for StatuslineStateEnforcement {
    fn from(profile: PermissionProfile) -> Self {
        match profile {
            PermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system: match file_system {
                    ManagedFileSystem::Restricted { .. } => StatuslineStateFileSystem::Restricted,
                    ManagedFileSystem::Unrestricted => StatuslineStateFileSystem::Unrestricted,
                },
                network,
            },
            PermissionProfile::Disabled => Self::Disabled,
            PermissionProfile::External { network } => Self::External { network },
        }
    }
}

impl ChatWidget {
    pub(super) fn status_line_run_state(&self) -> StatuslineStateRunState {
        if self.mcp_startup_status.is_some() {
            return StatuslineStateRunState::Starting;
        }
        if !self.bottom_pane.is_task_running() {
            return StatuslineStateRunState::Ready;
        }
        match self.status_state.terminal_title_status_kind {
            TerminalTitleStatus::Working => StatuslineStateRunState::Working,
            TerminalTitleStatus::WaitingForBackgroundTerminal => StatuslineStateRunState::Waiting,
            TerminalTitleStatus::Thinking => StatuslineStateRunState::Thinking,
        }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "staged for provider reads"))]
    pub(crate) fn statusline_state(&self) -> Option<StatuslineState> {
        if self.read_only_history {
            return None;
        }
        let id = self.thread_id?;
        let permissions = &self.config.permissions;
        let selected_model = self.current_model();
        let app = StatuslineStateApp {
            cli_version: CODEX_CLI_VERSION,
            display_mode: self.history_render_mode(),
        };
        let config = StatuslineStateConfig {
            statusline_items: self.status_line_items_with_invalids().0,
            profile: self.status_line_config_profile(),
        };
        let name = self.thread_name.as_deref().and_then(normalize_thread_name);
        let agent = StatuslineStateAgent {
            state: self.status_line_run_state(),
        };
        let model = StatuslineStateModel {
            access_program: crate::daybreak::program_for_turn(
                &self.model_catalog.models,
                selected_model,
                self.daybreak_account_eligible(),
                self.daybreak_enabled && !self.side_conversation_active(),
            )
            .ok()
            .flatten(),
            display_name: self.model_display_name().to_owned(),
            id: (!selected_model.is_empty()).then(|| selected_model.to_owned()),
            provider: self.config.model_provider_id.clone(),
            reasoning_level: self.effective_reasoning_effort(),
            service_tier: self.thread_settings_confirmed.then(|| {
                match self.current_service_tier() {
                    None | Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE) => "normal",
                    Some("priority") => "fast",
                    Some(tier) => tier,
                }
                .to_owned()
            }),
            service_tier_display_name: self
                .processing_tier_display_name()
                .filter(|_| self.thread_settings_confirmed)
                .map(str::to_owned),
        };
        let approval = self
            .thread_settings_confirmed
            .then(|| StatuslineStateApproval {
                policy: match permissions.approval_policy.value() {
                    AskForApproval::UnlessTrusted => StatuslineStateApprovalPolicy::Untrusted,
                    AskForApproval::OnRequest => StatuslineStateApprovalPolicy::OnRequest,
                    AskForApproval::Granular(_) => StatuslineStateApprovalPolicy::Granular,
                    AskForApproval::Never => StatuslineStateApprovalPolicy::Never,
                },
                reviewer: match self.config.approvals_reviewer {
                    ConfigReviewer::User => StatuslineStateApprovalReviewer::User,
                    ConfigReviewer::AutoReview => StatuslineStateApprovalReviewer::AutoReview,
                },
            });
        let permissions = self
            .thread_settings_confirmed
            .then(|| StatuslineStatePermissions {
                display_name: permissions_display(&self.config),
                profile_id: permissions
                    .active_permission_profile()
                    .map(|profile| profile.id),
                enforcement: permissions.effective_permission_profile().into(),
            });
        Some(StatuslineState {
            account: self.into(),
            app,
            config,
            thread: StatuslineStateThread {
                id,
                name,
                agent,
                model,
                usage: self.into(),
                workspace: StatuslineStateWorkspace {
                    cwd: codex_utils_path_uri::LegacyAppPathString::from_path(
                        self.status_line_workspace_cwd(),
                    ),
                },
                approval,
                permissions,
            },
        })
    }
}
