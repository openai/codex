//! Creates empty sessions from the command center without interrupting other agents.
//! Worktrees start at the repository default branch and bind only the new session.

use super::*;
use crate::startup_draft::StartupDraftInitialScreen;
use crate::startup_draft::StartupDraftPump;
use crate::startup_draft::StartupDraftSessionAction;

/// Owns a freshly created checkout until the UI accepts its completion event.
/// Dropping a cancelled worker or undelivered event removes only a clean checkout.
#[derive(Debug)]
pub(crate) struct PendingWorktree {
    pub(in crate::app) manager: codex_worktree::WorktreeManager,
    pub(in crate::app) checkout: Option<codex_worktree::ManagedWorktree>,
}

impl Drop for PendingWorktree {
    fn drop(&mut self) {
        if let Some(checkout) = &self.checkout {
            let result = dunce::canonicalize(&checkout.root)
                .map_err(anyhow::Error::from)
                .and_then(|root| self.manager.remove(&checkout.source_cwd, &root));
            if let Err(error) = result {
                tracing::error!(path = %checkout.root.display(), %error, "Could not remove unclaimed worktree");
            }
        }
    }
}

impl App {
    fn agents_overview_retained_worktree_error(
        &mut self,
        checkout: &codex_worktree::ManagedWorktree,
        reason: impl std::fmt::Display,
    ) {
        self.add_agents_overview_error(format!("{reason} A checkout was retained at {}; remove it with `git worktree remove <checkout-path>` from the source repository if it is no longer needed.", checkout.root.display()));
    }

    pub(in crate::app) async fn new_agents_overview_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        cwd: Option<AbsolutePathBuf>,
    ) -> Result<AppRunControl> {
        if self.reconnect.offline || self.windows_sandbox_blocks_thread_switch() {
            return Ok(AppRunControl::Continue);
        }
        let previous_thread = self.current_displayed_thread_id();
        let mut draft = self
            .agents_overview
            .new_session_draft
            .take()
            .unwrap_or_else(|| {
                Box::new(StartupDraftPump::new(
                    tui,
                    StartupDraftInitialScreen::Composer,
                    StartupDraftSessionAction::NewFromCommandCenter,
                ))
            });
        let mut display_config = self.chat_widget.config_ref().clone();
        if let Some(cwd) = cwd.as_ref() {
            display_config.cwd = cwd.clone();
        }
        draft.apply_config(&display_config);
        tui.terminal.clear()?;
        // Keep the large session-start future off the TUI's stack in dev builds.
        let result = Box::pin(self.start_agents_overview_session(
            tui,
            app_server,
            cwd,
            /*managed_worktree*/ None,
            Some(&mut draft),
        ))
        .await;
        if self.current_displayed_thread_id() != previous_thread {
            draft.flush_pending_paste_newline(tui).await?;
            self.chat_widget.restore_startup_draft(draft.take_draft());
        } else {
            // Retain edits if setup fails so retrying `n` does not lose the draft.
            draft.flush_pending_events(tui).await?;
            self.agents_overview.new_session_draft = Some(draft);
        }
        tui.frame_requester().schedule_frame();
        result
    }

    pub(in crate::app) async fn start_agents_overview_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        cwd: Option<AbsolutePathBuf>,
        managed_worktree: Option<(
            codex_worktree::WorktreeManager,
            codex_worktree::ManagedWorktree,
        )>,
        mut startup_draft: Option<&mut StartupDraftPump>,
    ) -> Result<AppRunControl> {
        if self.reconnect.offline || self.windows_sandbox_blocks_thread_switch() {
            if let Some((_, checkout)) = &managed_worktree {
                self.agents_overview_retained_worktree_error(
                    checkout,
                    "Could not start the session.",
                );
            }
            return Ok(AppRunControl::Continue);
        }
        let Some((config, remote_cwd)) = self
            .agents_overview_session_config(tui, app_server, cwd, startup_draft.as_deref_mut())
            .await
        else {
            if let Some((_, checkout)) = &managed_worktree {
                self.agents_overview_retained_worktree_error(
                    checkout,
                    "Could not load the new session settings.",
                );
            }
            return Ok(AppRunControl::Continue);
        };
        let selected_profile = self.chat_widget.thread_id().and_then(|thread_id| {
            let source = self.chat_widget.config_ref();
            let active = source.permissions.active_permission_profile()?;
            (self
                .agents_overview
                .selected_permission_profiles
                .get(&thread_id)
                == Some(&active.id))
            .then(|| PermissionProfileSelection {
                profile_id: active.id.clone(),
                approval_policy: Some(source.permissions.approval_policy.value().into()),
                approvals_reviewer: Some(source.approvals_reviewer),
                display_label: active.id,
            })
        });
        let local_settings = crate::local_settings::LocalSettings::from(&config);
        if let Some(draft) = startup_draft.as_deref_mut() {
            draft.apply_config(&config);
        }
        let result = StartupDraftPump::run_with_optional_draft(
            startup_draft.as_deref_mut(),
            tui,
            Box::pin(app_server.start_thread_with_session_start_source(
                &local_settings,
                &config,
                /*session_start_source*/ None,
                remote_cwd.as_deref(),
                selected_profile.as_ref(),
            )),
        )
        .await;
        let started = match result {
            Ok(started) => started,
            Err(error) => {
                if let Some((_, checkout)) = &managed_worktree {
                    self.agents_overview_retained_worktree_error(
                        checkout,
                        format!("Failed to start session: {error}"),
                    );
                } else {
                    self.add_agents_overview_error(format!("Failed to start session: {error}"));
                }
                return Ok(AppRunControl::Continue);
            }
        };
        let thread_id = started.session.thread_id;
        if let Some(selected) = selected_profile {
            self.agents_overview
                .selected_permission_profiles
                .insert(thread_id, selected.profile_id);
        }
        if let Some((manager, checkout)) = &managed_worktree {
            let result =
                if crate::session_resume::cwds_differ(started.session.cwd.as_path(), &checkout.cwd)
                {
                    Err(anyhow::anyhow!(
                        "The server did not apply the worktree directory."
                    ))
                } else {
                    manager.bind_thread(&checkout.root, &thread_id.to_string())
                };
            if let Err(error) = result {
                let _ = app_server.thread_unsubscribe(thread_id).await;
                self.agents_overview_retained_worktree_error(checkout, error);
                return Ok(AppRunControl::Continue);
            }
        }
        self.agents_overview
            .blank_sessions
            .insert(thread_id, started.clone());
        // Use the dashboard's existing attachment path, which preserves running agents
        // and unsent input in the previous session. Do not send an initial turn.
        let control = Box::pin(self.attach_agents_overview_thread(
            tui,
            app_server,
            thread_id,
            Some((config, started)),
            startup_draft,
        ))
        .await?;
        if self.current_displayed_thread_id() != Some(thread_id) {
            self.agents_overview.blank_sessions.remove(&thread_id);
            let _ = app_server.thread_unsubscribe(thread_id).await;
            if let Some((_, checkout)) = &managed_worktree {
                self.agents_overview_retained_worktree_error(
                    checkout,
                    "Could not open the new session.",
                );
            }
        }
        Ok(control)
    }

    pub(in crate::app) async fn new_agents_overview_worktree(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        cwd: Option<AbsolutePathBuf>,
    ) {
        if self.reconnect.offline
            || self.pending_managed_worktree_creation
            || self.windows_sandbox_blocks_thread_switch()
        {
            return;
        }
        if !self.config.features.enabled(Feature::Worktrees)
            || crate::uses_remote_workspace_or_environment(
                &self.app_server_target,
                self.environment_manager.as_ref(),
            )
        {
            self.add_agents_overview_error(
                "Managed worktrees require local worktree support.".to_string(),
            );
            return;
        }
        let Some((config, _)) = self
            .agents_overview_session_config(tui, app_server, cwd, /*startup_draft*/ None)
            .await
        else {
            return;
        };
        let setup = async {
            anyhow::ensure!(
                !config.active_project.is_untrusted(),
                "The source project is not trusted."
            );
            let host = crate::legacy_core::config::load_config_toml_with_layer_stack(
                &self.config.codex_home,
                /*cwd*/ None,
                Vec::new(),
                codex_config::ConfigLoadOptions::default(),
            )
            .await?;
            let settings = codex_worktree::WorktreeSettings::for_cli(
                &self.config.codex_home,
                host.config_toml.desktop.as_ref(),
            )?;
            anyhow::Ok(codex_worktree::WorktreeManager::new(settings))
        }
        .await;
        let manager = match setup {
            Ok(value) => value,
            Err(error) => return self.add_agents_overview_error(error.to_string()),
        };
        self.pending_managed_worktree_creation = true;
        self.agents_overview
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .creating_worktree = true;
        let sender = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                codex_worktree::default_worktree_base(config.cwd.as_path())
                    .and_then(|base| {
                        manager.create(&codex_worktree::CreateWorktree {
                            source_cwd: config.cwd.to_path_buf(),
                            base: Some(base),
                        })
                    })
                    .map(|checkout| PendingWorktree {
                        manager,
                        checkout: Some(checkout),
                    })
                    .map_err(|error| error.to_string())
            })
            .await
            .unwrap_or_else(|error| Err(format!("Worktree creation task failed: {error}")));
            sender.send(AppEvent::AgentsOverviewWorktreeCreated(result));
        });
    }
}
