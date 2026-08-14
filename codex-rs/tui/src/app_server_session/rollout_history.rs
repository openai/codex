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
use crate::legacy_core::config::Config;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_features::Feature;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;

impl AppServerSession {
    /// Captures the server's startup migration policy before workspace config can change.
    ///
    /// Sessions without a recorded startup config conservatively assume migration is enabled.
    pub(crate) fn with_startup_config(mut self, config: &Config) -> Self {
        self.background_rollout_migration_enabled = config
            .features
            .enabled(Feature::BackgroundPaginatedRolloutMigration);
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
        config: Config,
        thread_id: ThreadId,
        model_settings: ResumeModelSettings,
    ) -> Result<AppServerStartedThread> {
        let session_config = if model_settings == ResumeModelSettings::RestoreFromThread {
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
        let mut rollout_maintenance_guard = None;
        params.exclude_turns = if self.history_support == ThreadHistorySupport::Paginated {
            let known_legacy_history = self
                .history_pagination
                .get(&thread_id)
                .is_some_and(|state| state.history_mode == ThreadHistoryMode::Legacy)
                && (!self.uses_embedded_app_server()
                    || (!self.background_rollout_migration_enabled
                        && !config
                            .features
                            .enabled(Feature::BackgroundPaginatedRolloutMigration)
                        && {
                            rollout_maintenance_guard =
                                codex_rollout::try_acquire_rollout_maintenance_lock(
                                    config.codex_home.as_path(),
                                )
                                .ok()
                                .flatten();
                            rollout_maintenance_guard.is_some()
                        }
                        && self
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
            HistoryHydrationScope::Initial,
        )
        .await?;
        let fork_parent_title = self
            .fork_parent_title_from_app_server(response.thread.forked_from_id.as_deref())
            .await;
        let mut started =
            started_thread_from_resume_response(response, &config, self.thread_params_mode())
                .await?;
        started.session.fork_parent_title = fork_parent_title;
        Ok(started)
    }
}

#[cfg(test)]
#[path = "rollout_history_tests.rs"]
mod tests;
