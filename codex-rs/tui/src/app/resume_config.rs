//! Folder-entry consent and configuration for command-center dispatch and resume.
//! Keeps CLI/runtime cwd precedence, remote-workspace checks, and interactive prompts aligned.
//! Carries local preferences alongside the resolved configuration for session replacement.

use super::agents_overview_view::AgentsOverviewFocus;
use super::*;
use crate::onboarding::onboarding_screen::check_directory_trust;
use crate::startup_hooks_review::StartupHooksReviewOutcome;
use crate::startup_hooks_review::load_startup_hooks_review_entry;
use crate::startup_hooks_review::maybe_run_startup_hooks_review;
use codex_config::types::ResumeCwdMode;

impl App {
    pub(super) async fn resume_config_for_target(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        target_session: &SessionTarget,
    ) -> std::result::Result<(Config, crate::local_settings::LocalSettings), AppRunControl> {
        self.refresh_in_memory_config_from_disk_best_effort("resuming a thread")
            .await;
        let cwd_override = self
            .runtime_working_directory_override
            .as_deref()
            .or(self.harness_overrides.cwd.as_deref())
            .or_else(|| app_server.remote_cwd_override())
            .map(Path::to_path_buf);
        let cwd_override = cwd_override.as_deref();
        let resume_cwd_mode = crate::session_resume::effective_resume_cwd_mode(
            self.local_settings.tui.resume_cwd,
            cwd_override,
        );
        let remembered_current_cwd = cwd_override.unwrap_or(self.launch_cwd.as_path());
        let current_cwd = if matches!(resume_cwd_mode, Some(ResumeCwdMode::Current)) {
            remembered_current_cwd.to_path_buf()
        } else {
            self.config.cwd.to_path_buf()
        };
        let uses_remote_workspace_or_environment = crate::uses_remote_workspace_or_environment(
            &self.app_server_target,
            &self.environment_manager,
        );
        if uses_remote_workspace_or_environment
            && self.harness_overrides.cwd.is_none()
            && app_server.remote_cwd_override().is_none()
            && matches!(resume_cwd_mode, Some(ResumeCwdMode::Current))
        {
            self.add_session_picker_error(
                "`tui.resume_cwd = \"current\"` requires `--cd` when using a remote workspace"
                    .to_string(),
            );
            return Err(AppRunControl::Continue);
        }
        let resume_cwd = if self.app_server_target.uses_remote_workspace() {
            current_cwd.clone()
        } else {
            let history_cwd = if matches!(resume_cwd_mode, Some(ResumeCwdMode::Current)) {
                None
            } else {
                crate::session_resume::read_session_cwd(app_server, target_session.thread_id)
                    .await
                    .or_else(|| target_session.cwd.clone())
            };
            let outcome = crate::session_resume::resolve_cwd_for_resume_or_fork(
                tui,
                &self.config,
                history_cwd,
                CwdPromptAction::Resume,
                crate::session_resume::ResumeCwdContext {
                    current_cwd: &current_cwd,
                    remembered_current_cwd,
                    allow_remember_current: !uses_remote_workspace_or_environment
                        || cwd_override.is_some(),
                    mode: resume_cwd_mode,
                },
            )
            .await;
            match outcome {
                Err(err) => {
                    self.add_session_picker_error(format!(
                        "Failed to determine working directory for resume: {err}"
                    ));
                    return Err(AppRunControl::Continue);
                }
                Ok(crate::session_resume::ResolveCwdOutcome::Continue(Some(cwd)))
                | Ok(crate::session_resume::ResolveCwdOutcome::ContinueAfterPrompt(cwd)) => cwd,
                Ok(crate::session_resume::ResolveCwdOutcome::Continue(None)) => current_cwd.clone(),
                Ok(crate::session_resume::ResolveCwdOutcome::Exit) => {
                    return Err(AppRunControl::Exit(ExitReason::UserRequested));
                }
            }
        };

        let (config_current_cwd, config_resume_cwd) =
            if self.app_server_target.uses_remote_workspace() {
                let local_config_cwd = self.config.cwd.to_path_buf();
                (local_config_cwd.clone(), local_config_cwd)
            } else {
                (current_cwd, resume_cwd)
            };
        let mut resume_config = match self
            .rebuild_config_for_resume_or_fallback(&config_current_cwd, config_resume_cwd)
            .await
        {
            Ok(cfg) => cfg,
            Err(err) => {
                self.add_session_picker_error(format!(
                    "Failed to rebuild configuration for resume: {err}"
                ));
                return Err(AppRunControl::Continue);
            }
        };
        if self.reject_remote_resume_permission_override(&resume_config.0) {
            return Err(AppRunControl::Continue);
        }
        let resumed_thread =
            if matches!(self.app_server_target, AppServerTarget::LocalDaemon { .. }) {
                Some(
                    app_server
                        .thread_read(target_session.thread_id, /*include_turns*/ false)
                        .await
                        .map_err(|error| {
                            self.add_session_picker_error(format!(
                                "Unable to check resumed folder: {error}"
                            ));
                            AppRunControl::Continue
                        })?,
                )
            } else {
                None
            };
        let trust_cwd = resume_config.0.cwd.to_path_buf();
        self.confirm_directory_trust(
            tui,
            app_server,
            &mut resume_config.0,
            &trust_cwd,
            resumed_thread.as_ref(),
        )
        .await?;
        resume_config.1 = crate::local_settings::LocalSettings::from(&resume_config.0);
        Ok(resume_config)
    }

    pub(super) async fn confirm_directory_trust(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        config: &mut Config,
        cwd: &Path,
        resumed_thread: Option<&codex_app_server_protocol::Thread>,
    ) -> std::result::Result<(), AppRunControl> {
        // Keep the existing explicit remote --cd gate, including retries after cancellation.
        // Other remote destinations await authoritative trust-root metadata.
        let cwd = if self.app_server_target.uses_remote_workspace() {
            let Some(cwd) = app_server.remote_cwd_override() else {
                return Ok(());
            };
            cwd
        } else {
            cwd
        };
        let result = check_directory_trust(
            tui,
            app_server,
            config,
            &self.app_server_target,
            cwd,
            resumed_thread,
            /*startup_draft*/ None,
        )
        .await
        .map_err(|error| {
            self.add_session_picker_error(format!("Unable to check folder trust: {error}"));
            AppRunControl::Continue
        })?;
        if result.should_exit {
            if matches!(self.app_server_target, AppServerTarget::Embedded) {
                return Err(AppRunControl::Exit(ExitReason::UserRequested));
            }
            if self
                .chat_widget
                .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
                .is_none()
            {
                self.open_agents_overview(app_server, AgentsOverviewFocus::List);
            }
            return Err(AppRunControl::Continue);
        }
        if result.directory_trust_persisted && !app_server.uses_remote_workspace() {
            *config = self
                .rebuild_config_for_cwd(config.cwd.to_path_buf())
                .await
                .map_err(|error| {
                    self.add_session_picker_error(format!(
                        "Failed to reload trusted folder settings: {error}"
                    ));
                    AppRunControl::Continue
                })?;
            if resumed_thread.is_none() {
                let hooks = load_startup_hooks_review_entry(
                    app_server.request_handle(),
                    config.cwd.to_path_buf(),
                )
                .await;
                match maybe_run_startup_hooks_review(
                    app_server,
                    tui,
                    config,
                    config.bypass_hook_trust,
                    hooks,
                )
                .await
                .map_err(|error| {
                    self.add_session_picker_error(format!(
                        "Unable to review folder hooks: {error}"
                    ));
                    AppRunControl::Continue
                })? {
                    StartupHooksReviewOutcome::Continue => {}
                    StartupHooksReviewOutcome::OpenHooksBrowser(hooks) => {
                        self.chat_widget.open_hooks_browser(hooks);
                        return Err(AppRunControl::Continue);
                    }
                }
            }
        }
        Ok(())
    }
}
