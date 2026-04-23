use super::App;
use crate::app_server_session::ThreadSessionState;
use crate::read_session_model;
use codex_app_server_protocol::Thread;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;

impl App {
    pub(super) async fn sync_active_thread_permission_settings_to_cached_session(&mut self) {
        let Some(active_thread_id) = self.active_thread_id else {
            return;
        };

        let approval_policy = self.config.permissions.approval_policy.value();
        let approvals_reviewer = self.config.approvals_reviewer;
        let sandbox_policy = self.config.permissions.sandbox_policy.get().clone();
        let update_session = |session: &mut ThreadSessionState| {
            session.approval_policy = approval_policy;
            session.approvals_reviewer = approvals_reviewer;
            session.sandbox_policy = sandbox_policy.clone();
            session.permission_profile =
                if matches!(sandbox_policy, SandboxPolicy::ExternalSandbox { .. }) {
                    None
                } else {
                    Some(PermissionProfile::from_legacy_sandbox_policy(
                        &sandbox_policy,
                        &session.cwd,
                    ))
                };
        };

        if self.primary_thread_id == Some(active_thread_id)
            && let Some(session) = self.primary_session_configured.as_mut()
        {
            update_session(session);
        }

        if let Some(channel) = self.thread_event_channels.get(&active_thread_id) {
            let mut store = channel.store.lock().await;
            if let Some(session) = store.session.as_mut() {
                update_session(session);
            }
        }
    }

    pub(super) async fn session_state_for_thread_read(
        &self,
        thread_id: ThreadId,
        thread: &Thread,
    ) -> ThreadSessionState {
        let sandbox_policy = self.config.permissions.sandbox_policy.get().clone();
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
                permission_profile: None,
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
        session.permission_profile = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::side::SideThreadState;
    use crate::app::test_support::make_test_app;
    use crate::app::thread_events::ThreadEventChannel;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use codex_config::types::ApprovalsReviewer;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn test_thread_session(thread_id: ThreadId, cwd: PathBuf) -> ThreadSessionState {
        ThreadSessionState {
            thread_id,
            forked_from_id: None,
            fork_parent_title: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: cwd.abs(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }
    }

    #[tokio::test]
    async fn permission_settings_sync_updates_active_snapshot_without_rewriting_side_thread() {
        let mut app = make_test_app().await;
        let main_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000401").expect("valid thread");
        let side_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000402").expect("valid thread");
        let main_session = test_thread_session(main_thread_id, test_path_buf("/tmp/main"));
        let side_session = ThreadSessionState {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
            ..test_thread_session(side_thread_id, test_path_buf("/tmp/side"))
        };

        app.primary_thread_id = Some(main_thread_id);
        app.active_thread_id = Some(main_thread_id);
        app.primary_session_configured = Some(main_session.clone());
        app.thread_event_channels.insert(
            main_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                main_session.clone(),
                Vec::new(),
            ),
        );
        app.thread_event_channels.insert(
            side_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                side_session.clone(),
                Vec::new(),
            ),
        );
        app.side_threads
            .insert(side_thread_id, SideThreadState::new(main_thread_id));
        app.config.permissions.approval_policy =
            codex_config::Constrained::allow_any(AskForApproval::OnRequest);
        app.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        let expected_sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        let expected_permission_profile = PermissionProfile::from_legacy_sandbox_policy(
            &expected_sandbox_policy,
            &main_session.cwd,
        );
        app.config.permissions.sandbox_policy =
            codex_config::Constrained::allow_any(expected_sandbox_policy.clone());

        app.sync_active_thread_permission_settings_to_cached_session()
            .await;

        let expected_main_session = ThreadSessionState {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::AutoReview,
            sandbox_policy: expected_sandbox_policy,
            permission_profile: Some(expected_permission_profile),
            ..main_session
        };
        assert_eq!(
            app.primary_session_configured,
            Some(expected_main_session.clone())
        );

        let main_store_session = app
            .thread_event_channels
            .get(&main_thread_id)
            .expect("main thread channel")
            .store
            .lock()
            .await
            .session
            .clone();
        assert_eq!(main_store_session, Some(expected_main_session));

        let side_store_session = app
            .thread_event_channels
            .get(&side_thread_id)
            .expect("side thread channel")
            .store
            .lock()
            .await
            .session
            .clone();
        assert_eq!(side_store_session, Some(side_session));
    }
}
