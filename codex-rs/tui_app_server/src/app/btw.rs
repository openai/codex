use super::*;

const BTW_RENAME_BLOCK_MESSAGE: &str = "BTW threads are ephemeral and cannot be renamed.";

#[derive(Clone, Debug)]
pub(super) struct BtwThreadState {
    /// Thread to return to when the current BTW thread is dismissed.
    pub(super) parent_thread_id: ThreadId,
    /// Pretty parent label for the next synthetic fork banner, consumed on first attach.
    pub(super) next_fork_banner_parent_label: Option<String>,
}

impl App {
    pub(super) fn sync_btw_thread_ui(&mut self) {
        let clear_btw_ui = |chat_widget: &mut crate::chatwidget::ChatWidget| {
            chat_widget.set_thread_footer_hint_override(None);
            chat_widget.clear_thread_rename_block();
        };
        let Some(active_thread_id) = self.current_displayed_thread_id() else {
            clear_btw_ui(&mut self.chat_widget);
            return;
        };
        let Some(mut parent_thread_id) = self
            .btw_threads
            .get(&active_thread_id)
            .map(|state| state.parent_thread_id)
        else {
            clear_btw_ui(&mut self.chat_widget);
            return;
        };

        self.chat_widget
            .set_thread_rename_block_message(BTW_RENAME_BLOCK_MESSAGE);
        let mut depth = 1usize;
        while let Some(next_parent_thread_id) = self
            .btw_threads
            .get(&parent_thread_id)
            .map(|state| state.parent_thread_id)
        {
            depth += 1;
            parent_thread_id = next_parent_thread_id;
        }
        let repeated_prefix = "BTW from ".repeat(depth.saturating_sub(1));
        let label = if self.primary_thread_id == Some(parent_thread_id) {
            format!("from {repeated_prefix}main thread · Esc to return")
        } else {
            let parent_label = self.thread_label(parent_thread_id);
            format!("from {repeated_prefix}parent thread ({parent_label}) · Esc to return")
        };
        self.chat_widget
            .set_thread_footer_hint_override(Some(vec![("BTW".to_string(), label)]));
    }

    pub(super) fn active_btw_parent_thread_id(&self) -> Option<ThreadId> {
        self.current_displayed_thread_id()
            .and_then(|thread_id| self.btw_threads.get(&thread_id))
            .map(|state| state.parent_thread_id)
    }

    pub(super) async fn maybe_return_from_btw(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> bool {
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            && self.chat_widget.composer_is_empty()
            && let Some(parent_thread_id) = self.active_btw_parent_thread_id()
        {
            let _ = self
                .select_agent_thread_and_discard_btw_chain(tui, app_server, parent_thread_id)
                .await;
            true
        } else {
            false
        }
    }

    pub(super) fn btw_threads_to_discard_after_switch(
        &self,
        target_thread_id: ThreadId,
    ) -> Vec<ThreadId> {
        let Some(mut btw_thread_id) = self.current_displayed_thread_id() else {
            return Vec::new();
        };
        if !self.btw_threads.contains_key(&btw_thread_id)
            || self
                .btw_threads
                .get(&target_thread_id)
                .map(|state| state.parent_thread_id)
                == Some(btw_thread_id)
        {
            return Vec::new();
        }

        let mut btw_threads_to_discard = Vec::new();
        loop {
            btw_threads_to_discard.push(btw_thread_id);
            let Some(parent_thread_id) = self
                .btw_threads
                .get(&btw_thread_id)
                .map(|state| state.parent_thread_id)
            else {
                break;
            };
            if parent_thread_id == target_thread_id
                || !self.btw_threads.contains_key(&parent_thread_id)
            {
                break;
            }
            btw_thread_id = parent_thread_id;
        }
        btw_threads_to_discard
    }

    pub(super) fn take_next_btw_fork_banner_parent_label(
        &mut self,
        thread_id: ThreadId,
    ) -> Option<String> {
        self.btw_threads
            .get_mut(&thread_id)
            .and_then(|state| state.next_fork_banner_parent_label.take())
    }

    pub(super) async fn discard_btw_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
            tracing::warn!("failed to unsubscribe BTW thread {thread_id}: {err}");
        }
        self.abort_thread_event_listener(thread_id);
        self.thread_event_channels.remove(&thread_id);
        self.btw_threads.remove(&thread_id);
        if self.active_thread_id == Some(thread_id) {
            self.clear_active_thread().await;
        } else {
            self.refresh_pending_thread_approvals().await;
        }
        self.sync_active_agent_label();
    }

    async fn fork_banner_parent_label(&self, parent_thread_id: ThreadId) -> Option<String> {
        if self.chat_widget.thread_id() == Some(parent_thread_id) {
            return self
                .chat_widget
                .thread_name()
                .filter(|name| !name.trim().is_empty());
        }

        let channel = self.thread_event_channels.get(&parent_thread_id)?;
        let store = channel.store.lock().await;
        match store.session_configured.as_ref().map(|event| &event.msg) {
            Some(EventMsg::SessionConfigured(session)) => session
                .thread_name
                .clone()
                .filter(|name| !name.trim().is_empty()),
            _ => None,
        }
    }

    pub(super) async fn select_agent_thread_and_discard_btw_chain(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<()> {
        let btw_threads_to_discard = self.btw_threads_to_discard_after_switch(thread_id);
        self.select_agent_thread(tui, thread_id).await?;
        if self.active_thread_id == Some(thread_id) {
            for btw_thread_id in btw_threads_to_discard {
                self.discard_btw_thread(app_server, btw_thread_id).await;
            }
        }
        Ok(())
    }

    pub(super) async fn handle_start_btw(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        user_message: crate::chatwidget::UserMessage,
    ) -> Result<AppRunControl> {
        self.session_telemetry
            .counter("codex.thread.btw", 1, &[("source", "slash_command")]);
        self.refresh_in_memory_config_from_disk_best_effort("starting a BTW subagent")
            .await;

        let mut fork_config = self.config.clone();
        fork_config.ephemeral = true;
        let fork_result = match app_server.fork_thread(fork_config, parent_thread_id).await {
            Ok(forked) => Ok(forked),
            Err(err) => {
                if self.current_displayed_thread_id() != Some(parent_thread_id) {
                    self.chat_widget.add_error_message(format!(
                        "Failed to fork BTW thread from {parent_thread_id}: {err}"
                    ));
                    return Ok(AppRunControl::Continue);
                }
                let Some(parent_rollout_path) =
                    self.chat_widget.rollout_path().filter(|path| path.exists())
                else {
                    self.chat_widget.add_error_message(
                        "A thread must contain at least one turn before /btw can fork it."
                            .to_string(),
                    );
                    return Ok(AppRunControl::Continue);
                };
                let mut fork_config = self.config.clone();
                fork_config.ephemeral = true;
                app_server
                    .fork_thread_from_path(fork_config, parent_thread_id, parent_rollout_path)
                    .await
            }
        };

        match fork_result {
            Ok(forked) => {
                let child_thread_id = forked.session_configured.session_id;
                let next_fork_banner_parent_label =
                    self.fork_banner_parent_label(parent_thread_id).await;
                self.enqueue_thread_event(
                    child_thread_id,
                    Event {
                        id: String::new(),
                        msg: EventMsg::SessionConfigured(forked.session_configured),
                    },
                )
                .await?;
                self.btw_threads.insert(
                    child_thread_id,
                    BtwThreadState {
                        parent_thread_id,
                        next_fork_banner_parent_label,
                    },
                );
                if let Err(err) = self
                    .select_agent_thread_and_discard_btw_chain(tui, app_server, child_thread_id)
                    .await
                {
                    self.discard_btw_thread(app_server, child_thread_id).await;
                    return Err(err);
                }
                if self.active_thread_id == Some(child_thread_id) {
                    let _ = self
                        .chat_widget
                        .submit_user_message_as_plain_user_turn(user_message);
                } else {
                    self.discard_btw_thread(app_server, child_thread_id).await;
                    self.chat_widget.add_error_message(format!(
                        "Failed to switch into BTW thread {child_thread_id}."
                    ));
                }
            }
            Err(err) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to start BTW thread from {parent_thread_id}: {err}"
                ));
            }
        }

        Ok(AppRunControl::Continue)
    }
}
