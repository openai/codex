//! Retained startup metadata, separate from the replay sent in startup responses.

use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionNetworkProxyRuntime;
use codex_protocol::protocol::ThreadSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::PathBuf;

/// The configuration reported when a thread started, without its replay history.
/// Current settings are available through [`crate::CodexThread::config_snapshot`].
#[derive(Debug)]
pub struct ThreadStartupMetadata {
    pub session_id: SessionId,
    pub thread_id: ThreadId,
    forked_from_id: Option<ThreadId>,
    parent_thread_id: Option<ThreadId>,
    thread_source: Option<ThreadSource>,
    thread_name: Option<String>,
    model: String,
    model_provider_id: String,
    service_tier: Option<String>,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    permission_profile: PermissionProfile,
    active_permission_profile: Option<ActivePermissionProfile>,
    cwd: AbsolutePathBuf,
    reasoning_effort: Option<ReasoningEffort>,
    network_proxy: Option<SessionNetworkProxyRuntime>,
    rollout_path: Option<PathBuf>,
}

impl From<&SessionConfiguredEvent> for ThreadStartupMetadata {
    fn from(event: &SessionConfiguredEvent) -> Self {
        let SessionConfiguredEvent {
            session_id,
            thread_id,
            forked_from_id,
            parent_thread_id,
            thread_source,
            thread_name,
            model,
            model_provider_id,
            service_tier,
            approval_policy,
            approvals_reviewer,
            permission_profile,
            active_permission_profile,
            cwd,
            reasoning_effort,
            initial_messages: _,
            network_proxy,
            rollout_path,
        } = event;
        Self {
            session_id: *session_id,
            thread_id: *thread_id,
            forked_from_id: *forked_from_id,
            parent_thread_id: *parent_thread_id,
            thread_source: thread_source.clone(),
            thread_name: thread_name.clone(),
            model: model.clone(),
            model_provider_id: model_provider_id.clone(),
            service_tier: service_tier.clone(),
            approval_policy: *approval_policy,
            approvals_reviewer: *approvals_reviewer,
            permission_profile: permission_profile.clone(),
            active_permission_profile: active_permission_profile.clone(),
            cwd: cwd.clone(),
            reasoning_effort: reasoning_effort.clone(),
            network_proxy: network_proxy.clone(),
            rollout_path: rollout_path.clone(),
        }
    }
}

impl ThreadStartupMetadata {
    pub(crate) fn to_session_configured_event(
        &self,
        initial_messages: Option<Vec<EventMsg>>,
    ) -> SessionConfiguredEvent {
        SessionConfiguredEvent {
            session_id: self.session_id,
            thread_id: self.thread_id,
            forked_from_id: self.forked_from_id,
            parent_thread_id: self.parent_thread_id,
            thread_source: self.thread_source.clone(),
            thread_name: self.thread_name.clone(),
            model: self.model.clone(),
            model_provider_id: self.model_provider_id.clone(),
            service_tier: self.service_tier.clone(),
            approval_policy: self.approval_policy,
            approvals_reviewer: self.approvals_reviewer,
            permission_profile: self.permission_profile.clone(),
            active_permission_profile: self.active_permission_profile.clone(),
            cwd: self.cwd.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
            initial_messages,
            network_proxy: self.network_proxy.clone(),
            rollout_path: self.rollout_path.clone(),
        }
    }
}
