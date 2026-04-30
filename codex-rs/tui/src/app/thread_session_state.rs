use super::App;
use crate::session_resume::read_session_model;
use crate::session_state::ThreadSessionState;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::Thread;
use codex_protocol::ThreadId;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;

impl App {
    pub(super) async fn sync_active_thread_permission_settings_to_cached_session(&mut self) {
        let Some(active_thread_id) = self.active_thread_id else {
            return;
        };

        let approval_policy = AskForApproval::from(self.config.permissions.approval_policy.value());
        let approvals_reviewer = self.config.approvals_reviewer;
        let permission_profile = self
            .chat_widget
            .config_ref()
            .permissions
            .permission_profile();
        let active_permission_profile = self
            .chat_widget
            .config_ref()
            .permissions
            .active_permission_profile();
        let update_session = |session: &mut ThreadSessionState| {
            session.approval_policy = approval_policy;
            session.approvals_reviewer = approvals_reviewer;
            session.permission_profile = permission_profile.clone();
            session.active_permission_profile = active_permission_profile.clone();
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
        let permission_profile = self.current_permission_profile();
        let active_permission_profile = self.current_active_permission_profile();
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
                approval_policy: AskForApproval::from(
                    self.config.permissions.approval_policy.value(),
                ),
                approvals_reviewer: self.config.approvals_reviewer,
                permission_profile: permission_profile.clone(),
                active_permission_profile: active_permission_profile.clone(),
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
        session.permission_profile = permission_profile;
        session.active_permission_profile = active_permission_profile;
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

    fn current_permission_profile(&self) -> PermissionProfile {
        self.chat_widget
            .config_ref()
            .permissions
            .permission_profile()
    }

    fn current_active_permission_profile(&self) -> Option<ActivePermissionProfile> {
        self.chat_widget
            .config_ref()
            .permissions
            .active_permission_profile()
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
    use codex_app_server_protocol::AskForApproval;
    use codex_app_server_protocol::FileSystemAccessMode;
    use codex_app_server_protocol::FileSystemPath;
    use codex_app_server_protocol::FileSystemSandboxEntry;
    use codex_app_server_protocol::FileSystemSpecialPath;
    use codex_app_server_protocol::PermissionProfile as AppServerPermissionProfile;
    use codex_app_server_protocol::PermissionProfileFileSystemPermissions;
    use codex_app_server_protocol::PermissionProfileNetworkPermissions;
    use codex_config::types::ApprovalsReviewer;
    use codex_protocol::models::PermissionProfile;
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
            permission_profile: PermissionProfile::read_only(),
            active_permission_profile: None,
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
            permission_profile: PermissionProfile::workspace_write(),
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
            codex_config::Constrained::allow_any(AskForApproval::OnRequest.to_core());
        app.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        let expected_permission_profile = PermissionProfile::workspace_write();
        app.chat_widget.handle_thread_session(main_session.clone());
        app.chat_widget
            .set_permission_profile(expected_permission_profile.clone())
            .expect("set widget permission profile");
        app.config
            .permissions
            .set_permission_profile(expected_permission_profile.clone())
            .expect("set permission profile");

        app.sync_active_thread_permission_settings_to_cached_session()
            .await;

        let expected_main_session = ThreadSessionState {
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::AutoReview,
            permission_profile: expected_permission_profile,
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

    #[tokio::test]
    async fn permission_settings_sync_preserves_active_profile_only_rules() {
        let mut app = make_test_app().await;
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000403").expect("valid thread");
        let profile: PermissionProfile = AppServerPermissionProfile::Managed {
            network: PermissionProfileNetworkPermissions { enabled: false },
            file_system: PermissionProfileFileSystemPermissions::Restricted {
                entries: vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::GlobPattern {
                            pattern: "**/.env".to_string(),
                        },
                        access: FileSystemAccessMode::None,
                    },
                ],
                glob_scan_max_depth: None,
            },
        }
        .into();
        let session = ThreadSessionState {
            permission_profile: profile.clone(),
            ..test_thread_session(thread_id, test_path_buf("/tmp/main"))
        };

        app.primary_thread_id = Some(thread_id);
        app.active_thread_id = Some(thread_id);
        app.primary_session_configured = Some(session.clone());
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(/*capacity*/ 4, session.clone(), Vec::new()),
        );
        app.chat_widget.handle_thread_session(session.clone());
        app.config.permissions.approval_policy =
            codex_config::Constrained::allow_any(AskForApproval::OnRequest.to_core());

        app.sync_active_thread_permission_settings_to_cached_session()
            .await;

        let expected_session = ThreadSessionState {
            approval_policy: AskForApproval::OnRequest,
            permission_profile: profile,
            ..session
        };
        assert_eq!(
            app.primary_session_configured,
            Some(expected_session.clone())
        );

        let store_session = app
            .thread_event_channels
            .get(&thread_id)
            .expect("thread channel")
            .store
            .lock()
            .await
            .session
            .clone();
        assert_eq!(store_session, Some(expected_session));
    }

    #[tokio::test]
    async fn thread_read_fallback_uses_active_permission_settings() {
        let mut app = make_test_app().await;
        let primary_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000404").expect("valid thread");
        let read_thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000405").expect("valid thread");
        let primary_session = ThreadSessionState {
            permission_profile: PermissionProfile::workspace_write(),
            ..test_thread_session(primary_thread_id, test_path_buf("/tmp/primary"))
        };
        let read_thread = Thread {
            id: read_thread_id.to_string(),
            forked_from_id: None,
            preview: "read thread".to_string(),
            ephemeral: false,
            model_provider: "read-provider".to_string(),
            created_at: 1,
            updated_at: 2,
            status: codex_app_server_protocol::ThreadStatus::Idle,
            path: None,
            cwd: test_path_buf("/tmp/read").abs(),
            cli_version: "0.0.0".to_string(),
            source: codex_app_server_protocol::SessionSource::Unknown,
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: Some("read thread".to_string()),
            turns: Vec::new(),
        };

        app.primary_session_configured = Some(primary_session.clone());
        app.chat_widget.handle_thread_session(primary_session);

        let session = app
            .session_state_for_thread_read(read_thread_id, &read_thread)
            .await;

        let expected_permission_profile = app
            .chat_widget
            .config_ref()
            .permissions
            .permission_profile();
        assert_eq!(session.permission_profile, expected_permission_profile);
        assert_ne!(
            session.permission_profile,
            app.config.permissions.permission_profile(),
            "thread/read fallback must use the active widget permissions rather than stale app \
             config defaults"
        );
    }
}
