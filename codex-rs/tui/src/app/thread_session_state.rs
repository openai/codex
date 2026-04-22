use super::App;
use crate::app_server_session::ThreadSessionState;
use crate::read_session_model;
use codex_app_server_protocol::Thread;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SandboxPolicy;

impl App {
    pub(super) async fn session_state_for_thread_read(
        &self,
        thread_id: ThreadId,
        thread: &Thread,
    ) -> ThreadSessionState {
        let sandbox_policy = self.config.permissions.sandbox_policy.get().clone();
        let permission_profile = match &sandbox_policy {
            SandboxPolicy::ExternalSandbox { .. } => None,
            SandboxPolicy::DangerFullAccess
            | SandboxPolicy::ReadOnly { .. }
            | SandboxPolicy::WorkspaceWrite { .. } => {
                Some(self.config.permissions.permission_profile())
            }
        };
        let mut session = self
            .primary_session_configured
            .clone()
            .unwrap_or(ThreadSessionState {
                thread_id,
                forked_from_id: None,
                fork_parent_title: None,
                thread_name: None,
                model: self.chat_widget.current_model().to_string(),
                model_provider_id: self.config.model_provider_id.clone(),
                service_tier: self.chat_widget.current_service_tier(),
                approval_policy: self.config.permissions.approval_policy.value(),
                approvals_reviewer: self.config.approvals_reviewer,
                sandbox_policy,
                permission_profile,
                cwd: thread.cwd.clone(),
                instruction_source_paths: Vec::new(),
                reasoning_effort: self.chat_widget.current_reasoning_effort(),
                history_log_id: 0,
                history_entry_count: 0,
                network_proxy: None,
                rollout_path: thread.path.clone(),
            });
        session.thread_id = thread_id;
        session.thread_name = thread.name.clone();
        session.model_provider_id = thread.model_provider.clone();
        session.cwd = thread.cwd.clone();
        session.instruction_source_paths = Vec::new();
        session.rollout_path = thread.path.clone();
        if let Some(model) =
            read_session_model(&self.config, thread_id, thread.path.as_deref()).await
        {
            session.model = model;
        } else if thread.path.is_some() {
            session.model.clear();
        }
        session.history_log_id = 0;
        session.history_entry_count = 0;
        session
    }
}
