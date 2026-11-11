//! Prepare saved-thread attachment before replacing the active conversation UI.
//!
//! Stored-backend preparation leaves active settings unchanged; active-backend resume retains its
//! eager preference refresh. Neither detaches the active thread or replaces its composer. The
//! prepared result retains the connection identity and any provider error for read-only history.

use super::*;
use codex_app_server_client::TypedRequestError;

/// Settings retained for a backend that is not currently displayed.
#[derive(Clone)]
pub(super) struct BackendSettings {
    pub(super) config: Config,
    pub(super) local_settings: crate::local_settings::LocalSettings,
    pub(super) target: AppServerTarget,
}

/// The settings source for a resume attempt.
///
/// The active source may refresh the displayed conversation's settings eagerly. A stored backend
/// is refreshed without adopting its settings until attachment succeeds.
#[derive(Clone, Copy)]
pub(super) enum ResumeSource<'a> {
    Active,
    Backend(&'a BackendSettings),
}

pub(super) struct PreparedResume {
    pub(super) backend_id: crate::backend_id::BackendId,
    pub(super) config: Config,
    pub(super) local_settings: crate::local_settings::LocalSettings,
    pub(super) thread: AppServerStartedThread,
    pub(super) restored_blank_session: bool,
    pub(super) read_only: bool,
    pub(super) history_notice: Option<&'static str>,
    pub(super) provider_error: Option<codex_app_server_protocol::ModelProviderError>,
}

impl App {
    pub(super) fn backend_settings(&self) -> BackendSettings {
        BackendSettings {
            config: self.config.clone(),
            local_settings: self.local_settings.clone(),
            target: self.app_server_target.clone(),
        }
    }

    pub(super) async fn live_blank_session(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        thread: &codex_app_server_protocol::Thread,
    ) -> Option<AppServerStartedThread> {
        if !matches!(thread.status, codex_app_server_protocol::ThreadStatus::Idle) {
            return None;
        }
        let mut blank = self.agents_overview.blank_sessions.get(&thread_id)?.clone();

        // A turn can start while this backend is parked, without the TUI seeing it.
        let paginated = matches!(
            thread.history_mode,
            codex_app_server_protocol::ThreadHistoryMode::Paginated
        ) && app_server.supports_paginated_history();
        let untouched = if paginated {
            match app_server.thread_turns_page(thread_id, /*cursor*/ None, /*limit*/ 1).await {
                Ok(page) => Some(page.data.is_empty() && page.next_cursor.is_none()),
                Err(error) if Self::can_fallback_from_include_turns_error(&error) => Some(true),
                Err(error) if error.chain().any(|cause| matches!(
                    cause.downcast_ref::<TypedRequestError>(),
                    Some(TypedRequestError::Server { source, .. })
                        if crate::app_server_session::is_history_pagination_unsupported(source)
                )) => None,
                Err(_) => Some(false),
            }
        } else {
            None
        };
        let untouched = match untouched {
            Some(untouched) => untouched,
            None => match app_server
                .thread_read(thread_id, /*include_turns*/ true)
                .await
            {
                Ok(thread) => {
                    thread.turns.is_empty()
                        && matches!(thread.status, codex_app_server_protocol::ThreadStatus::Idle)
                }
                Err(error) => Self::can_fallback_from_include_turns_error(&error),
            },
        };
        if !untouched {
            return None;
        }
        // An untouched task may have no rollout to resume yet.
        blank.session.thread_name.clone_from(&thread.name);
        Some(blank)
    }

    pub(super) async fn prepare_target_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        target_session: &SessionTarget,
        source: ResumeSource<'_>,
    ) -> std::result::Result<PreparedResume, AppRunControl> {
        if let Some(owner) = self.backend_registry.owner(target_session.thread_id)
            && let Err(error) = target_session.ensure_backend(owner)
        {
            self.add_session_picker_error(error.to_string());
            return Err(AppRunControl::Continue);
        }
        let (mut resume_config, local_settings) = self
            .resume_config_for_target(tui, app_server, target_session, source)
            .await?;
        if let Err(error) = self.apply_runtime_policy_overrides(
            &mut resume_config,
            RuntimePolicyOverrideScope::ExplicitOnly,
        ) {
            self.add_session_picker_error(format!("{error:#}"));
            return Err(AppRunControl::Continue);
        }
        let permission_overrides = self.resume_permission_overrides(&resume_config);
        let model_settings = match source {
            ResumeSource::Active => self.resume_model_settings(),
            ResumeSource::Backend(_) => {
                super::config_persistence::resume_model_settings_for_overrides(
                    &resume_config,
                    &self.harness_overrides,
                )
            }
        };

        if let Some(history_mode) = target_session.history_mode {
            app_server.remember_thread_history_mode(target_session.thread_id, history_mode);
        }
        let thread_id = target_session.thread_id;
        let live_blank = if matches!(source, ResumeSource::Backend(_))
            && self.backend_registry.owner(thread_id) == Some(app_server.backend_id())
            && self.agents_overview.blank_sessions.contains_key(&thread_id)
            && let Ok(thread) = app_server
                .thread_read(thread_id, /*include_turns*/ false)
                .await
        {
            self.live_blank_session(app_server, thread_id, &thread)
                .await
        } else {
            None
        };
        let restored_blank_session = live_blank.is_some();
        let resumed = match live_blank {
            Some(thread) => Ok(thread),
            None => {
                app_server
                    .resume_thread_with_permission_overrides(
                        &local_settings,
                        resume_config.clone(),
                        thread_id,
                        model_settings,
                        permission_overrides,
                    )
                    .await
            }
        };
        let mut history_notice = None;
        let mut provider_error = None;
        let (resumed, read_only) = match resumed {
            Ok(resumed) => (resumed, false),
            Err(err)
                if crate::app_server_session::is_active_writer_error(&err)
                    || crate::app_server_session::model_provider_error(&err).is_some() =>
            {
                provider_error = crate::app_server_session::model_provider_error(&err);
                match app_server
                    .read_thread_for_viewing(
                        &resume_config,
                        &local_settings,
                        target_session.thread_id,
                    )
                    .await
                {
                    Ok((thread, notice)) => {
                        history_notice = notice;
                        (thread, true)
                    }
                    Err(read_err) => {
                        self.add_session_picker_error(format!(
                            "Failed to load chat history: {read_err}"
                        ));
                        return Err(AppRunControl::Continue);
                    }
                }
            }
            Err(err) => {
                let path_display = target_session.display_label();
                self.add_session_picker_error(format!(
                    "Failed to resume session from {path_display}: {err}"
                ));
                return Err(AppRunControl::Continue);
            }
        };
        Ok(PreparedResume {
            backend_id: app_server.backend_id(),
            config: resume_config,
            local_settings,
            thread: resumed,
            restored_blank_session,
            read_only,
            history_notice,
            provider_error,
        })
    }
}
