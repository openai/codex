//! Managed checkout creation and handoff from blocking Git work to the TUI event loop.
//!
//! Source-only blockers are checked before allocation. The completion event rechecks the
//! original session and reports a recovery path for every retained checkout.

use super::*;
use crate::app_event::ManagedWorktreeCreated;
use crate::app_event::ManagedWorktreeTransition;
use crate::history_cell::McpInventoryLoadingCell as LoadingCell;
use codex_app_server_protocol::ThreadBackgroundTerminalsListParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsListResponse as ListResponse;

pub(super) fn background_terminals_blocker(
    result: Result<ListResponse, TypedRequestError>,
    target: &AppServerTarget,
) -> Option<&'static str> {
    match result {
        Ok(response) if response.data.is_empty() => None,
        Err(TypedRequestError::Server { source, .. })
            if matches!(target, AppServerTarget::LocalDaemon { .. })
                && (source.code == -32601
                    || source.code == -32600
                        && source.message.contains("thread/backgroundTerminals/list")
                        && (source.message.contains("unknown variant")
                            || source.message.contains("unknown method"))) =>
        {
            Some(
                "The local Codex service cannot check background terminals. Run `codex app-server daemon update`, then restart Codex.",
            )
        }
        _ => Some("Active background terminals block /cd."),
    }
}

impl App {
    pub(super) async fn start_managed_worktree(
        &mut self,
        app_server: &mut AppServerSession,
        mode: crate::app_event::ManagedWorktreeMode,
        name: Option<String>,
    ) {
        if !self.config.features.enabled(Feature::Worktrees) {
            self.chat_widget.add_error_message(
                "Enable worktrees in your Codex configuration to create a worktree.".to_string(),
            );
        } else if self.config.active_project.is_untrusted() {
            self.chat_widget.add_error_message(
                "Cannot create a worktree from an explicitly untrusted source.".to_string(),
            );
        } else if crate::uses_remote_workspace_or_environment(
            &self.app_server_target,
            self.environment_manager.as_ref(),
        ) {
            self.chat_widget.add_error_message(
                "Managed worktrees are only supported for local sessions.".to_string(),
            );
        } else if self
            .primary_thread_id
            .is_none_or(|thread_id| !self.chat_widget.can_change_working_directory(thread_id))
        {
            self.chat_widget.add_error_message(
                "Creating a worktree requires an idle primary session without queued input."
                    .to_string(),
            );
        } else if self.pending_managed_worktree_creation {
            self.chat_widget
                .add_error_message("A worktree is already being created.".to_string());
        } else {
            // These source-only checks must precede allocation. The transition repeats them
            // after the background task, since the session can change while Git is running.
            if self
                .transcript_cells
                .iter()
                .any(|cell| cell.as_any().is::<LoadingCell>())
            {
                return self.working_directory_error("MCP inventory is still loading.");
            }
            let Some(thread_id) = self.primary_thread_id else {
                return self.working_directory_error(
                    "Creating a worktree requires an idle primary session without queued input.",
                );
            };
            let agents = self.agent_navigation.ordered_threads();
            let closed_agents: HashSet<_> = agents
                .iter()
                .filter_map(|(id, agent)| agent.is_closed.then_some(*id))
                .collect();
            let active = self.thread_event_channels.iter().any(|(id, channel)| {
                *id != thread_id
                    && !closed_agents.contains(id)
                    && !channel
                        .store
                        .try_lock()
                        .is_ok_and(|store| store.active_turn_id().is_none())
            });
            if active
                || agents
                    .iter()
                    .any(|(id, agent)| *id != thread_id && agent.is_running)
            {
                return self.working_directory_error("Cannot change: another agent is running.");
            }
            let rollout = self.chat_widget.rollout_path();
            let has_rollout = rollout.as_deref().is_some_and(rollout_path_is_resumable);
            if mode == crate::app_event::ManagedWorktreeMode::Fork
                && !has_rollout
                && (!self.chat_widget.token_usage().is_zero()
                    || self
                        .thread_event_channels
                        .get(&thread_id)
                        .is_some_and(|channel| {
                            channel.store.try_lock().map_or(/*default*/ true, |store| {
                                !store.turns.is_empty() || store.buffer.iter().any(|event| {
                                    matches!(event, ThreadBufferedEvent::Notification(notification)
                                        if matches!(notification.as_ref(),
                                            ServerNotification::TurnStarted(_)
                                            | ServerNotification::TurnCompleted(_)))
                                })
                            })
                        }))
            {
                return self.working_directory_error("Conversation history is not saved.");
            }
            let mut ids: HashSet<_> = self
                .thread_event_channels
                .keys()
                .copied()
                .filter(|id| !closed_agents.contains(id))
                .collect();
            ids.extend(
                agents
                    .iter()
                    .filter_map(|(id, agent)| (!agent.is_closed).then_some(*id)),
            );
            for tracked_id in ids {
                let request = ClientRequest::ThreadBackgroundTerminalsList {
                    request_id: app_server.next_request_id(),
                    params: ThreadBackgroundTerminalsListParams {
                        thread_id: tracked_id.to_string(),
                        cursor: None,
                        limit: Some(1),
                    },
                };
                let result = app_server
                    .request_handle()
                    .request_typed::<ListResponse>(request)
                    .await;
                if let Some(message) = background_terminals_blocker(result, &self.app_server_target)
                {
                    return self.working_directory_error(message);
                }
            }
            let setup = async {
                let source = self
                    .rebuild_config_for_cwd(self.config.cwd.to_path_buf())
                    .await
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                anyhow::ensure!(
                    !source.active_project.is_untrusted(),
                    "Cannot create a worktree from an explicitly untrusted source."
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
                let manager = codex_worktree::WorktreeManager::new(settings);
                anyhow::Ok(manager)
            }
            .await;
            match setup {
                Ok(manager) => {
                    self.pending_managed_worktree_creation = true;
                    let source_cwd = self.config.cwd.clone();
                    let sender = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let create_cwd = source_cwd.to_path_buf();
                        let result = tokio::task::spawn_blocking(move || {
                            manager
                                .create(&codex_worktree::CreateWorktree {
                                    source_cwd: create_cwd,
                                    base: None,
                                })
                                .map(|checkout| (manager, checkout))
                                .map_err(|error| error.to_string())
                        })
                        .await
                        .unwrap_or_else(|error| {
                            Err(format!("Worktree creation task failed: {error}"))
                        });
                        sender.send(AppEvent::ManagedWorktreeCreated(Box::new(
                            ManagedWorktreeCreated {
                                source_thread_id: thread_id,
                                source_cwd,
                                mode,
                                name,
                                result,
                            },
                        )));
                    });
                }
                Err(error) => self.chat_widget.add_error_message(error.to_string()),
            }
        }
    }

    pub(super) async fn finish_managed_worktree(&mut self, created: ManagedWorktreeCreated) {
        self.pending_managed_worktree_creation = false;
        let (manager, checkout) = match created.result {
            Ok(created) => created,
            Err(error) => return self.working_directory_error(error),
        };
        let can_continue = !self.reconnect.offline
            && !self.chat_widget.has_misalignment_policy_violation()
            && self.config.features.enabled(Feature::Worktrees)
            && !crate::uses_remote_workspace_or_environment(
                &self.app_server_target,
                self.environment_manager.as_ref(),
            )
            && self.primary_thread_id == Some(created.source_thread_id)
            && !crate::session_resume::cwds_differ(
                self.config.cwd.as_path(),
                created.source_cwd.as_path(),
            )
            && self
                .chat_widget
                .can_change_working_directory(created.source_thread_id);
        if !can_continue {
            return self.retained_worktree_error(
                &checkout,
                "Cannot continue into the new worktree while the source session changed or is unavailable.",
            );
        }
        match self
            .rebuild_config_for_cwd(created.source_cwd.to_path_buf())
            .await
        {
            Ok(config) if !config.active_project.is_untrusted() => {}
            Ok(_) | Err(_) => {
                return self.retained_worktree_error(
                    &checkout,
                    "Cannot continue into the new worktree because source configuration changed.",
                );
            }
        }
        let config = match self.rebuild_config_for_cwd(checkout.cwd.clone()).await {
            Ok(config) if config.active_project.trust_level.is_some() => config,
            Ok(_) => {
                return self.retained_worktree_error(
                    &checkout,
                    "The new worktree is not trusted; run Codex there.",
                );
            }
            Err(error) => {
                return self.retained_worktree_error(
                    &checkout,
                    format!("Cannot load the new worktree configuration: {error}"),
                );
            }
        };
        self.pending_managed_worktree_transition = Some(Box::new(ManagedWorktreeTransition {
            source_thread_id: created.source_thread_id,
            source_cwd: created.source_cwd,
            manager,
            checkout,
            config: Box::new(config),
            mode: created.mode,
            name: created.name,
        }));
    }

    fn retained_worktree_error(
        &mut self,
        checkout: &codex_worktree::ManagedWorktree,
        reason: impl std::fmt::Display,
    ) {
        self.working_directory_error(format!(
            "{reason} A checkout was retained at {}; remove it with `git worktree remove <checkout-path>` from the source repository if it is no longer needed.",
            checkout.root.display()
        ));
    }
}
