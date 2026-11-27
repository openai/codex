//! Projects the displayed conversation into the provider-facing statusline snapshot.

mod account;
mod usage;

use super::ChatWidget;
use super::normalize_thread_name;
use super::status_state::TerminalTitleStatusKind as TerminalTitleStatus;
use super::status_surfaces::permissions_display;
use crate::app_server_session::AppServerSession;
use crate::bottom_pane::StatusLineItem;
use crate::session_state::ConfirmedPermissions;
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
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
pub(crate) struct StatuslineState {
    account: account::Account,
    app: StatuslineStateApp,
    config: StatuslineStateConfig,
    thread: StatuslineStateThread,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "App")
)]
struct StatuslineStateApp {
    cli_version: &'static str,
    display_mode: crate::history_cell::HistoryRenderMode,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Config")
)]
struct StatuslineStateConfig {
    #[cfg_attr(test, ts(type = "StatuslineItemId[]"))]
    statusline_items: Vec<StatusLineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    profile: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Thread")
)]
struct StatuslineStateThread {
    #[cfg_attr(test, ts(inline))]
    id: codex_protocol::ThreadId,
    name: Option<String>,
    agent: StatuslineStateAgent,
    model: StatuslineStateModel,
    workspace: StatuslineStateWorkspace,
    usage: usage::Usage,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    approval: Option<StatuslineStateApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    permissions: Option<StatuslineStatePermissions>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Workspace")
)]
struct StatuslineStateWorkspace {
    #[cfg_attr(test, ts(inline))]
    cwd: codex_utils_path_uri::LegacyAppPathString,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Agent")
)]
struct StatuslineStateAgent {
    state: StatuslineStateRunState,
}

#[derive(Serialize, Display)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "RunState")
)]
pub(super) enum StatuslineStateRunState {
    Starting,
    Ready,
    Working,
    Waiting,
    Thinking,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Model", optional_fields)
)]
struct StatuslineStateModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(inline))]
    access_program: Option<CyberAccessProgram>,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(inline))]
    reasoning_level: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier_display_name: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Approval")
)]
struct StatuslineStateApproval {
    policy: StatuslineStateApprovalPolicy,
    reviewer: StatuslineStateApprovalReviewer,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "ApprovalPolicy")
)]
enum StatuslineStateApprovalPolicy {
    Untrusted,
    OnRequest,
    Granular,
    Never,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "ApprovalReviewer")
)]
enum StatuslineStateApprovalReviewer {
    User,
    AutoReview,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Permissions")
)]
struct StatuslineStatePermissions {
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    profile_id: Option<String>,
    #[serde(flatten)]
    enforcement: StatuslineStateEnforcement,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Enforcement")
)]
enum StatuslineStateEnforcement {
    Managed {
        file_system: StatuslineStateFileSystem,
        #[cfg_attr(test, ts(inline))]
        network: NetworkSandboxPolicy,
    },
    Disabled,
    External {
        #[cfg_attr(test, ts(inline))]
        network: NetworkSandboxPolicy,
    },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "FileSystem")
)]
enum StatuslineStateFileSystem {
    Restricted,
    Unrestricted,
}

impl StatuslineStatePermissions {
    fn new(snapshot: &ConfirmedPermissions, profile: &PermissionProfile) -> Self {
        Self {
            display_name: permissions_display(snapshot),
            profile_id: snapshot.active.as_ref().map(|active| active.id.clone()),
            enforcement: profile.into(),
        }
    }
}

impl From<&PermissionProfile> for StatuslineStateEnforcement {
    fn from(profile: &PermissionProfile) -> Self {
        match profile {
            PermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system: match file_system {
                    ManagedFileSystem::Restricted { .. } => StatuslineStateFileSystem::Restricted,
                    ManagedFileSystem::Unrestricted => StatuslineStateFileSystem::Unrestricted,
                },
                network: *network,
            },
            PermissionProfile::Disabled => Self::Disabled,
            PermissionProfile::External { network } => Self::External { network: *network },
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

    pub(crate) async fn provider_statusline_state(
        &self,
        owner: &AppServerSession,
    ) -> Option<StatuslineState> {
        let mut state = self.statusline_state()?;
        // Full-disk managed profiles share a legacy sandbox with external or disabled
        // enforcement. Only the embedded server can disambiguate those projections.
        if state.thread.permissions.is_none()
            && let Some(snapshot) = self.confirmed_permissions.as_deref()
            && let Ok(Some(live)) = owner.model_preview_permissions(state.thread.id).await
            && let Ok(cwd) = snapshot.cwd.to_abs_path()
            && let Ok(projected) = live.profile.to_legacy_sandbox_policy(cwd.as_path())
            && Some(projected)
                == snapshot
                    .profile
                    .to_legacy_sandbox_policy(cwd.as_path())
                    .ok()
        {
            state.thread.permissions =
                Some(StatuslineStatePermissions::new(snapshot, &live.profile));
        }
        Some(state)
    }

    pub(crate) fn statusline_state(&self) -> Option<StatuslineState> {
        if self.read_only_history {
            return None;
        }
        let id = self.thread_id?;
        let permissions = self.confirmed_permissions.as_deref();
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
                self.model_catalog.models(),
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
        let approval = permissions.map(|permissions| StatuslineStateApproval {
            policy: match permissions.policy.to_core() {
                AskForApproval::UnlessTrusted => StatuslineStateApprovalPolicy::Untrusted,
                AskForApproval::OnRequest => StatuslineStateApprovalPolicy::OnRequest,
                AskForApproval::Granular(_) => StatuslineStateApprovalPolicy::Granular,
                AskForApproval::Never => StatuslineStateApprovalPolicy::Never,
            },
            reviewer: match permissions.reviewer {
                ConfigReviewer::User => StatuslineStateApprovalReviewer::User,
                ConfigReviewer::AutoReview => StatuslineStateApprovalReviewer::AutoReview,
            },
        });
        let permissions = permissions
            .filter(|snapshot| match &snapshot.profile {
                PermissionProfile::Managed { .. } => true,
                PermissionProfile::External { network } => network.is_enabled(),
                PermissionProfile::Disabled => snapshot.active.as_ref().is_some_and(|active| {
                    active.id == codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS
                }),
            })
            .map(|snapshot| StatuslineStatePermissions::new(snapshot, &snapshot.profile));
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
                        &self.status_line_workspace_cwd(),
                    ),
                },
                approval,
                permissions,
            },
        })
    }
}

#[cfg(test)]
#[path = "statusline_contract_tests.rs"]
mod contract_tests;
