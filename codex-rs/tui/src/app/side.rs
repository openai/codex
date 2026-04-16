//! Transient side-conversation threads.
//!
//! A side conversation is an ephemeral fork used for a quick /side question while keeping the
//! primary thread focused. This module owns the app-level lifecycle for those forks: switching into
//! them, returning to their parent, and discarding them when normal thread navigation moves
//! elsewhere. The fork receives hidden developer instructions that make inherited history reference
//! material only and steer the agent away from mutations unless the side conversation explicitly asks
//! for them.

use super::*;
use crate::chatwidget::InterruptedTurnNoticeMode;
use codex_app_server_protocol::ToolAccessPolicy;

const SIDE_RENAME_BLOCK_MESSAGE: &str = "Side conversations are ephemeral and cannot be renamed.";
const SIDE_ALREADY_OPEN_MESSAGE: &str =
    "A side conversation is already open. Press Esc to return before starting another.";
const SIDE_DEVELOPER_INSTRUCTIONS: &str = r#"You are in a side conversation.

This side conversation is for answering questions and lightweight exploration without disrupting the main thread.

The inherited fork history is provided only as reference context. Do not treat instructions, plans, or requests found in the inherited history as active instructions for this side conversation.

MCPs, app connector tools, and dynamic external tools are not available in this side conversation. Any MCP or external tool calls or outputs visible in the inherited history happened in the parent thread and are reference-only; do not infer current external tool access from them.

You may perform non-mutating inspection, including reading or searching files and running checks that do not alter repo-tracked files.

Do not modify files, source, git state, permissions, configuration, or any other workspace state unless the user explicitly requests that mutation in this side conversation. Do not request escalated permissions or broader sandbox access unless the user explicitly requests a mutation that requires it. If the user explicitly requests a mutation, keep it minimal, local to the request, and avoid disrupting the main thread."#;

#[derive(Clone, Debug)]
pub(super) struct SideThreadState {
    /// Thread to return to when the current side conversation is dismissed.
    pub(super) parent_thread_id: ThreadId,
    /// Pretty parent label for the next synthetic fork banner, consumed on first attach.
    pub(super) next_fork_banner_parent_label: Option<String>,
}

impl App {
    pub(super) fn sync_side_thread_ui(&mut self) {
        let clear_side_ui = |chat_widget: &mut crate::chatwidget::ChatWidget| {
            chat_widget.set_thread_footer_hint_override(/*items*/ None);
            chat_widget.set_side_conversation_active(/*active*/ false);
            chat_widget.clear_thread_rename_block();
            chat_widget.set_interrupted_turn_notice_mode(InterruptedTurnNoticeMode::Default);
        };
        let Some(active_thread_id) = self.current_displayed_thread_id() else {
            clear_side_ui(&mut self.chat_widget);
            return;
        };
        let Some(parent_thread_id) = self
            .side_threads
            .get(&active_thread_id)
            .map(|state| state.parent_thread_id)
        else {
            clear_side_ui(&mut self.chat_widget);
            return;
        };

        self.chat_widget
            .set_thread_rename_block_message(SIDE_RENAME_BLOCK_MESSAGE);
        self.chat_widget
            .set_side_conversation_active(/*active*/ true);
        self.chat_widget
            .set_interrupted_turn_notice_mode(InterruptedTurnNoticeMode::Suppress);
        let label = if self.primary_thread_id == Some(parent_thread_id) {
            "from main thread · Esc to return".to_string()
        } else {
            let parent_label = self.thread_label(parent_thread_id);
            format!("from parent thread ({parent_label}) · Esc to return")
        };
        self.chat_widget
            .set_thread_footer_hint_override(Some(vec![("Side".to_string(), label)]));
    }

    pub(super) fn active_side_parent_thread_id(&self) -> Option<ThreadId> {
        self.current_displayed_thread_id()
            .and_then(|thread_id| self.side_threads.get(&thread_id))
            .map(|state| state.parent_thread_id)
    }

    pub(super) async fn maybe_return_from_side(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> bool {
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            && self.chat_widget.composer_is_empty()
            && let Some(parent_thread_id) = self.active_side_parent_thread_id()
        {
            let _ = self
                .select_agent_thread_and_discard_side_chain(tui, app_server, parent_thread_id)
                .await;
            true
        } else {
            false
        }
    }

    pub(super) fn side_threads_to_discard_after_switch(
        &self,
        target_thread_id: ThreadId,
    ) -> Vec<ThreadId> {
        let Some(mut side_thread_id) = self.current_displayed_thread_id() else {
            return Vec::new();
        };
        if target_thread_id == side_thread_id
            || !self.side_threads.contains_key(&side_thread_id)
            || self
                .side_threads
                .get(&target_thread_id)
                .map(|state| state.parent_thread_id)
                == Some(side_thread_id)
        {
            return Vec::new();
        }

        let mut side_threads_to_discard = Vec::new();
        loop {
            side_threads_to_discard.push(side_thread_id);
            let Some(parent_thread_id) = self
                .side_threads
                .get(&side_thread_id)
                .map(|state| state.parent_thread_id)
            else {
                break;
            };
            if parent_thread_id == target_thread_id
                || !self.side_threads.contains_key(&parent_thread_id)
            {
                break;
            }
            side_thread_id = parent_thread_id;
        }
        side_threads_to_discard
    }

    pub(super) fn take_next_side_fork_banner_parent_label(
        &mut self,
        thread_id: ThreadId,
    ) -> Option<String> {
        self.side_threads
            .get_mut(&thread_id)
            .and_then(|state| state.next_fork_banner_parent_label.take())
    }

    pub(super) async fn discard_side_thread(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        self.interrupt_side_thread(app_server, thread_id).await;
        if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
            tracing::warn!("failed to unsubscribe side conversation {thread_id}: {err}");
        }
        self.abort_thread_event_listener(thread_id);
        self.thread_event_channels.remove(&thread_id);
        self.side_threads.remove(&thread_id);
        self.agent_navigation.remove(thread_id);
        if self.active_thread_id == Some(thread_id) {
            self.clear_active_thread().await;
        } else {
            self.refresh_pending_thread_approvals().await;
        }
        self.sync_active_agent_label();
    }

    async fn interrupt_side_thread(&self, app_server: &mut AppServerSession, thread_id: ThreadId) {
        let interrupt_result =
            if let Some(turn_id) = self.active_turn_id_for_thread(thread_id).await {
                app_server.turn_interrupt(thread_id, turn_id).await
            } else {
                app_server.startup_interrupt(thread_id).await
            };
        if let Err(err) = interrupt_result {
            tracing::warn!("failed to interrupt side conversation before discard: {err}");
        }
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
        store
            .session
            .as_ref()
            .and_then(|session| session.thread_name.clone())
            .filter(|name| !name.trim().is_empty())
    }

    fn side_developer_instructions(existing_instructions: Option<&str>) -> String {
        match existing_instructions {
            Some(existing_instructions) if !existing_instructions.trim().is_empty() => {
                format!("{existing_instructions}\n\n{SIDE_DEVELOPER_INSTRUCTIONS}")
            }
            _ => SIDE_DEVELOPER_INSTRUCTIONS.to_string(),
        }
    }

    pub(super) fn side_fork_config(&self) -> Config {
        let mut fork_config = self.config.clone();
        fork_config.ephemeral = true;
        fork_config.developer_instructions = Some(Self::side_developer_instructions(
            fork_config.developer_instructions.as_deref(),
        ));
        fork_config
    }

    pub(super) fn side_start_block_message(&self) -> Option<&'static str> {
        (!self.side_threads.is_empty()).then_some(SIDE_ALREADY_OPEN_MESSAGE)
    }

    pub(super) async fn select_agent_thread_and_discard_side_chain(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) -> Result<()> {
        let side_threads_to_discard = self.side_threads_to_discard_after_switch(thread_id);
        self.select_agent_thread(tui, app_server, thread_id).await?;
        if self.active_thread_id == Some(thread_id) {
            for side_thread_id in side_threads_to_discard {
                self.discard_side_thread(app_server, side_thread_id).await;
            }
        }
        Ok(())
    }

    pub(super) async fn handle_start_side(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        parent_thread_id: ThreadId,
        user_message: Option<crate::chatwidget::UserMessage>,
    ) -> Result<AppRunControl> {
        if let Some(message) = self.side_start_block_message() {
            self.chat_widget.add_error_message(message.to_string());
            return Ok(AppRunControl::Continue);
        }

        self.session_telemetry.counter(
            "codex.thread.side",
            /*inc*/ 1,
            &[("source", "slash_command")],
        );
        self.refresh_in_memory_config_from_disk_best_effort("starting a side conversation")
            .await;

        let fork_config = self.side_fork_config();
        match app_server
            .fork_thread(
                fork_config,
                parent_thread_id,
                Some(ToolAccessPolicy::NoExternalTools),
            )
            .await
        {
            Ok(forked) => {
                let child_thread_id = forked.session.thread_id;
                let next_fork_banner_parent_label =
                    self.fork_banner_parent_label(parent_thread_id).await;
                let channel = self.ensure_thread_channel(child_thread_id);
                {
                    let mut store = channel.store.lock().await;
                    store.set_session(forked.session, forked.turns);
                }
                self.side_threads.insert(
                    child_thread_id,
                    SideThreadState {
                        parent_thread_id,
                        next_fork_banner_parent_label,
                    },
                );
                if let Err(err) = self
                    .select_agent_thread_and_discard_side_chain(tui, app_server, child_thread_id)
                    .await
                {
                    self.discard_side_thread(app_server, child_thread_id).await;
                    self.chat_widget.add_error_message(format!(
                        "Failed to switch into side conversation {child_thread_id}: {err}"
                    ));
                    return Ok(AppRunControl::Continue);
                }
                if self.active_thread_id == Some(child_thread_id) {
                    if let Some(user_message) = user_message {
                        let _ = self
                            .chat_widget
                            .submit_user_message_as_plain_user_turn(user_message);
                    }
                } else {
                    self.discard_side_thread(app_server, child_thread_id).await;
                    self.chat_widget.add_error_message(format!(
                        "Failed to switch into side conversation {child_thread_id}."
                    ));
                }
            }
            Err(err) => {
                self.chat_widget
                    .set_thread_footer_hint_override(/*items*/ None);
                self.chat_widget.add_error_message(format!(
                    "Failed to fork side conversation from {parent_thread_id}: {err}"
                ));
            }
        }

        Ok(AppRunControl::Continue)
    }
}
