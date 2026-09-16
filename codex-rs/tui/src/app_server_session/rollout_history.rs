//! Resume history policy for local rollouts that may migrate between formats.

use super::AppServerSession;
use super::AppServerStartedThread;
use super::HistoryHydrationScope;
use super::ResumeModelSettings;
use super::ThreadHistorySupport;
use super::bootstrap_request_error;
use super::is_history_pagination_unsupported;
use super::started_thread_from_resume_response;
use super::thread_resume_params_from_config;
use super::thread_session_state_from_thread_response;
use crate::legacy_core::config::Config;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use color_eyre::eyre::Result;

impl AppServerSession {
    /// Read a conflicting thread without taking its writer lease. This is a snapshot, not an
    /// attachment: the caller must not route turns through this session.
    pub(crate) async fn read_thread_for_viewing(
        &mut self,
        config: &Config,
        local_settings: &crate::local_settings::LocalSettings,
        thread_id: ThreadId,
    ) -> Result<AppServerStartedThread> {
        let mut thread = self.thread_read(thread_id, /*include_turns*/ false).await?;
        self.hydrate_initial_thread_history(
            &mut thread,
            /*turn_cursor*/ None,
            /*item_cursor*/ None,
            Some(config),
            Some(local_settings),
            HistoryHydrationScope::Initial,
        )
        .await?;
        let session = thread_session_state_from_thread_response(
            &thread.id,
            crate::windows_sandbox::host_from_environments(thread.environments.as_deref()),
            thread.forked_from_id.clone(),
            thread.name.clone(),
            thread.path.clone(),
            thread.model.clone().unwrap_or_default(),
            thread.model_provider.clone(),
            config.service_tier.clone(),
            AskForApproval::from(config.permissions.approval_policy.value()),
            config.approvals_reviewer,
            config.permissions.permission_profile().clone(),
            config.permissions.active_permission_profile(),
            thread.cwd.clone(),
            config.workspace_roots.clone(),
            Vec::new(),
            thread.reasoning_effort,
            config.personality,
            local_settings,
        )
        .await
        .map_err(color_eyre::eyre::Report::msg)?;
        Ok(AppServerStartedThread {
            session,
            turns: thread.turns,
            blocks_direct_input: false,
            task_tools_available: false,
        })
    }

    pub(crate) fn with_local_codex_home(mut self, codex_home: &AbsolutePathBuf) -> Self {
        self.task_tool_capabilities_dir = (!self.uses_embedded_app_server())
            .then(|| codex_home.join("tui-thread-reference-capabilities"));
        self
    }

    /// Retains trusted embedded-server history metadata for a later resume.
    ///
    /// Remote hints are ignored because another process can migrate the thread before resume.
    pub(crate) fn remember_thread_history_mode(
        &mut self,
        thread_id: ThreadId,
        history_mode: ThreadHistoryMode,
    ) {
        if !self.uses_embedded_app_server() {
            return;
        }
        self.history_pagination
            .entry(thread_id)
            .or_default()
            .history_mode = history_mode;
    }

    pub(crate) async fn resume_thread(
        &mut self,
        local_settings: &crate::local_settings::LocalSettings,
        config: Config,
        thread_id: ThreadId,
        model_settings: ResumeModelSettings,
    ) -> Result<AppServerStartedThread> {
        let session_config = if matches!(
            model_settings,
            ResumeModelSettings::RestoreFromThread | ResumeModelSettings::PreserveExistingThread
        ) {
            config.clone()
        } else {
            self.session_config_with_effective_service_tier(&config)
        };
        let mut params = thread_resume_params_from_config(
            session_config,
            thread_id,
            self.thread_params_mode(),
            self.remote_cwd_override.as_deref(),
            model_settings,
        );
        self.thread_tool_transport()
            .configure_mcp(&mut params.config);
        let mut rollout_maintenance_guard = None;
        params.exclude_turns = if self.history_support == ThreadHistorySupport::Paginated {
            let known_legacy_history = self
                .history_pagination
                .get(&thread_id)
                .is_some_and(|state| state.history_mode == ThreadHistoryMode::Legacy)
                && (!self.uses_embedded_app_server()
                    || ({
                        // The guard prevents migration through the full resume,
                        // regardless of the server's migration feature settings.
                        rollout_maintenance_guard =
                            codex_rollout::try_acquire_rollout_maintenance_lock(
                                config.codex_home.as_path(),
                            )
                            .ok()
                            .flatten();
                        rollout_maintenance_guard.is_some()
                    } && self
                        .thread_read(thread_id, /*include_turns*/ false)
                        .await
                        .is_ok_and(|thread| thread.history_mode == ThreadHistoryMode::Legacy)));
            !known_legacy_history
        } else {
            false
        };
        if params.exclude_turns {
            rollout_maintenance_guard = None;
        }
        let request_id = self.next_request_id();
        let resume_response = self
            .client
            .request_typed(ClientRequest::ThreadResume {
                request_id,
                params: params.clone(),
            })
            .await;
        drop(rollout_maintenance_guard);
        let mut response: ThreadResumeResponse = match resume_response {
            Ok(response) => response,
            Err(TypedRequestError::Server { source, .. })
                if params.exclude_turns && is_history_pagination_unsupported(&source) =>
            {
                self.history_support = ThreadHistorySupport::LegacyOnly;
                params.exclude_turns = false;
                let request_id = self.next_request_id();
                self.client
                    .request_typed(ClientRequest::ThreadResume { request_id, params })
                    .await
                    .map_err(|err| {
                        bootstrap_request_error("thread/resume failed during TUI bootstrap", err)
                    })?
            }
            Err(err) => {
                return Err(bootstrap_request_error(
                    "thread/resume failed during TUI bootstrap",
                    err,
                ));
            }
        };
        self.hydrate_initial_thread_history(
            &mut response.thread,
            response.turns_backwards_cursor.clone(),
            response.items_backwards_cursor.clone(),
            Some(&config),
            Some(local_settings),
            HistoryHydrationScope::Initial,
        )
        .await?;
        let fork_parent_title = self
            .fork_parent_title_from_app_server(response.thread.forked_from_id.as_deref())
            .await;
        let mut started = started_thread_from_resume_response(
            response,
            local_settings,
            &config,
            self.thread_params_mode(),
        )
        .await?;
        started.session.fork_parent_title = fork_parent_title;
        if self.task_tools_available(thread_id) {
            self.remember_task_tool_thread(thread_id);
            started.task_tools_available = true;
        }
        Ok(started)
    }
}

#[cfg(test)]
#[path = "rollout_history_tests.rs"]
mod tests;
