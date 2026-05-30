//! Thread targeting helpers for app-server requests and notifications.

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_protocol::ThreadId;

pub(super) fn server_request_thread_id(request: &ServerRequest) -> Option<ThreadId> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::FileChangeRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::ToolRequestUserInput { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::McpServerElicitationRequest { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::PermissionsRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::DynamicToolCall { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::ChatgptAuthTokensRefresh { .. }
        | ServerRequest::AttestationGenerate { .. }
        | ServerRequest::ApplyPatchApproval { .. }
        | ServerRequest::ExecCommandApproval { .. } => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ServerNotificationThreadTarget {
    Thread(ThreadId),
    InvalidThreadId(String),
    AppScoped,
    Global,
}

pub(super) fn server_notification_thread_target(
    notification: &ServerNotification,
) -> ServerNotificationThreadTarget {
    let thread_id = match notification {
        ServerNotification::Error(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadStarted(notification) => Some(notification.thread.id.as_str()),
        ServerNotification::ThreadStatusChanged(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadArchived(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadDeleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadUnarchived(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadClosed(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadNameUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadTokenUsageUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadGoalUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadGoalCleared(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadSettingsUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TurnStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::HookStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::HookCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnDiffUpdated(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnPlanUpdated(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ItemStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ItemCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::RawResponseItemCompleted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::AgentMessageDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::PlanDelta(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::CommandExecutionOutputDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TerminalInteraction(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::FileChangeOutputDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::FileChangePatchUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ServerRequestResolved(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::McpToolCallProgress(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningSummaryTextDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningSummaryPartAdded(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningTextDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ContextCompacted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ModelRerouted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ModelVerification(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TurnModerationMetadata(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeStarted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeItemAdded(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeTranscriptDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeTranscriptDone(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeSdp(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeError(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeClosed(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::Warning(notification) => notification.thread_id.as_deref(),
        ServerNotification::GuardianWarning(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::McpServerStatusUpdated(notification) => {
            match notification.thread_id.as_deref() {
                Some(thread_id) => Some(thread_id),
                None => return ServerNotificationThreadTarget::AppScoped,
            }
        }
        ServerNotification::SkillsChanged(_)
        | ServerNotification::McpServerOauthLoginCompleted(_)
        | ServerNotification::AccountUpdated(_)
        | ServerNotification::AccountRateLimitsUpdated(_)
        | ServerNotification::AppListUpdated(_)
        | ServerNotification::RemoteControlStatusChanged(_)
        | ServerNotification::ExternalAgentConfigImportCompleted(_)
        | ServerNotification::DeprecationNotice(_)
        | ServerNotification::ConfigWarning(_)
        | ServerNotification::FuzzyFileSearchSessionUpdated(_)
        | ServerNotification::FuzzyFileSearchSessionCompleted(_)
        | ServerNotification::CommandExecOutputDelta(_)
        | ServerNotification::ProcessOutputDelta(_)
        | ServerNotification::ProcessExited(_)
        | ServerNotification::FsChanged(_)
        | ServerNotification::WindowsWorldWritableWarning(_)
        | ServerNotification::WindowsSandboxSetupCompleted(_)
        | ServerNotification::AccountLoginCompleted(_) => None,
    };

    match thread_id {
        Some(thread_id) => match ThreadId::from_string(thread_id) {
            Ok(thread_id) => ServerNotificationThreadTarget::Thread(thread_id),
            Err(_) => ServerNotificationThreadTarget::InvalidThreadId(thread_id.to_string()),
        },
        None => ServerNotificationThreadTarget::Global,
    }
}

#[cfg(test)]
mod tests {
    use super::ServerNotificationThreadTarget;
    use super::server_notification_thread_target;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use codex_app_server_protocol::GuardianWarningNotification;
    use codex_app_server_protocol::McpServerStartupState;
    use codex_app_server_protocol::McpServerStatusUpdatedNotification;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ThreadSettings;
    use codex_app_server_protocol::ThreadSettingsUpdatedNotification;
    use codex_app_server_protocol::WarningNotification;
    use codex_protocol::ThreadId;
    use codex_protocol::config_types::CollaborationMode;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::config_types::Settings;
    use codex_protocol::openai_models::ReasoningEffort;
    use pretty_assertions::assert_eq;

    fn test_thread_settings() -> ThreadSettings {
        ThreadSettings {
            cwd: test_path_buf("/tmp/thread-settings").abs(),
            runtime_workspace_roots: vec![test_path_buf("/tmp/thread-settings").abs()],
            approval_policy: codex_app_server_protocol::AskForApproval::Never,
            approvals_reviewer: codex_app_server_protocol::ApprovalsReviewer::User,
            sandbox_policy: codex_app_server_protocol::SandboxPolicy::ReadOnly {
                network_access: false,
            },
            active_permission_profile: None,
            model: "gpt-5.4".to_string(),
            model_provider: "openai".to_string(),
            service_tier: None,
            effort: Some(ReasoningEffort::High),
            summary: None,
            collaboration_mode: CollaborationMode {
                mode: ModeKind::Default,
                settings: Settings {
                    model: "gpt-5.4".to_string(),
                    reasoning_effort: Some(ReasoningEffort::High),
                    developer_instructions: None,
                },
            },
            personality: None,
        }
    }

    #[test]
    fn warning_notifications_without_threads_are_global() {
        let notification = ServerNotification::Warning(WarningNotification {
            thread_id: None,
            message: "warning".to_string(),
        });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Global);
    }

    #[test]
    fn warning_notifications_route_to_threads_when_thread_id_is_present() {
        let thread_id = ThreadId::new();
        let notification = ServerNotification::Warning(WarningNotification {
            thread_id: Some(thread_id.to_string()),
            message: "warning".to_string(),
        });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Thread(thread_id));
    }

    #[test]
    fn guardian_warning_notifications_route_to_threads() {
        let thread_id = ThreadId::new();
        let notification = ServerNotification::GuardianWarning(GuardianWarningNotification {
            thread_id: thread_id.to_string(),
            message: "warning".to_string(),
        });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Thread(thread_id));
    }

    #[test]
    fn mcp_startup_notifications_route_to_threads() {
        let thread_id = ThreadId::new();
        let notification =
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                thread_id: Some(thread_id.to_string()),
                name: "sentry".to_string(),
                status: McpServerStartupState::Failed,
                error: Some("sentry is not logged in".to_string()),
            });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Thread(thread_id));
    }

    #[test]
    fn mcp_startup_notifications_without_threads_are_app_scoped() {
        let notification =
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                thread_id: None,
                name: "sentry".to_string(),
                status: McpServerStartupState::Failed,
                error: Some("sentry is not logged in".to_string()),
            });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::AppScoped);
    }

    #[test]
    fn thread_settings_updated_notifications_route_to_threads() {
        let thread_id = ThreadId::new();
        let notification =
            ServerNotification::ThreadSettingsUpdated(ThreadSettingsUpdatedNotification {
                thread_id: thread_id.to_string(),
                thread_settings: test_thread_settings(),
            });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Thread(thread_id));
    }
}
