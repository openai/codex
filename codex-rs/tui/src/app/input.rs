//! Keyboard input, external editor, and status-line dispatch for the TUI app.
//!
//! This module owns global key bindings that sit above ChatWidget, including transcript overlay
//! entry, Ctrl-L clear, external editor launch, and agent navigation shortcuts.

use super::*;
use crate::app_backtrack::SIDE_EDIT_PREVIOUS_UNAVAILABLE_MESSAGE;

impl App {
    pub(super) fn route_key_chord_event(
        &mut self,
        tui: &mut tui::Tui,
        key_event: KeyEvent,
    ) -> Option<KeyEvent> {
        let contexts = self.active_keymap_contexts();
        let was_pending = self.key_chord_matcher.is_pending();
        match self.key_chord_matcher.advance(
            key_event,
            &self.keymap.chords,
            contexts,
            tokio::time::Instant::now(),
        ) {
            crate::keymap::KeyChordMatch::PassThrough => {
                if was_pending && !self.key_chord_matcher.is_pending() {
                    self.chat_widget.set_footer_hint_override(/*items*/ None);
                }
                Some(key_event)
            }
            crate::keymap::KeyChordMatch::Pending(prefix) => {
                if self.backtrack.primed {
                    self.reset_backtrack_state();
                }
                self.chat_widget.set_footer_hint_override(Some(vec![
                    (
                        format!("{} …", prefix.display_label()),
                        "waiting for next key".to_string(),
                    ),
                    ("esc".to_string(), "cancel".to_string()),
                ]));
                tui.frame_requester()
                    .schedule_frame_in(crate::keymap::KEY_CHORD_TIMEOUT);
                None
            }
            crate::keymap::KeyChordMatch::Completed(dispatch_event) => {
                self.chat_widget.set_footer_hint_override(/*items*/ None);
                Some(dispatch_event)
            }
            crate::keymap::KeyChordMatch::Cancelled => {
                self.chat_widget.set_footer_hint_override(/*items*/ None);
                None
            }
            crate::keymap::KeyChordMatch::Ignored => None,
        }
    }

    pub(super) fn expire_pending_key_chord(&mut self) {
        let contexts = self.active_keymap_contexts();
        if self
            .key_chord_matcher
            .expire(contexts, tokio::time::Instant::now())
        {
            self.chat_widget.set_footer_hint_override(/*items*/ None);
        }
    }

    pub(super) fn cancel_pending_key_chord(&mut self) {
        if self.key_chord_matcher.cancel() {
            self.chat_widget.set_footer_hint_override(/*items*/ None);
        }
    }

    fn active_keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        if self.overlay.is_some() {
            return crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::Pager);
        }

        let contexts = self.chat_widget.keymap_contexts();
        if self.chat_widget.no_modal_or_popup_active() {
            contexts
                .with(crate::keymap::KeymapContext::Global)
                .with(crate::keymap::KeymapContext::Chat)
        } else {
            contexts
        }
    }

    pub(super) async fn launch_external_editor(&mut self, tui: &mut tui::Tui) {
        let editor_cmd = match external_editor::resolve_editor_command() {
            Ok(cmd) => cmd,
            Err(external_editor::EditorError::MissingEditor) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(
                    "Cannot open external editor: set $VISUAL or $EDITOR before starting Codex."
                        .to_string(),
                ));
                self.reset_external_editor_state(tui);
                return;
            }
            Err(err) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(format!(
                        "Failed to open editor: {err}",
                    )));
                self.reset_external_editor_state(tui);
                return;
            }
        };

        let seed = self.chat_widget.composer_text_with_pending();
        let config = self.chat_widget.config_ref();
        let file_system_policy = config.permissions.file_system_sandbox_policy();
        let editor_result = tui
            .with_restored(|| async {
                external_editor::run_editor(
                    &seed,
                    &editor_cmd,
                    config.codex_home.as_path(),
                    &file_system_policy,
                    config.cwd.as_path(),
                )
                .await
            })
            .await;
        self.reset_external_editor_state(tui);

        match editor_result {
            Ok(new_text) => {
                // Trim trailing whitespace
                let cleaned = new_text.trim_end().to_string();
                self.chat_widget.apply_external_edit(cleaned);
            }
            Err(err) => {
                self.chat_widget
                    .add_to_history(history_cell::new_error_event(format!(
                        "Failed to open editor: {err}",
                    )));
            }
        }
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn request_external_editor_launch(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .set_external_editor_state(ExternalEditorState::Requested);
        self.chat_widget.set_footer_hint_override(Some(vec![(
            EXTERNAL_EDITOR_HINT.to_string(),
            String::new(),
        )]));
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn reset_external_editor_state(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .set_external_editor_state(ExternalEditorState::Closed);
        self.chat_widget.set_footer_hint_override(/*items*/ None);
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn apply_raw_output_mode(
        &mut self,
        tui: &mut tui::Tui,
        enabled: bool,
        notify: bool,
    ) {
        if notify {
            self.chat_widget.set_raw_output_mode_and_notify(enabled);
        } else {
            self.chat_widget.set_raw_output_mode(enabled);
        }
        let terminal_width = tui.terminal.last_known_screen_size.into();
        if let Err(err) = self.reflow_transcript_now(tui, terminal_width) {
            tracing::warn!(error = %err, "failed to reflow transcript after raw output mode toggle");
            self.chat_widget
                .add_error_message(format!("Failed to redraw transcript: {err}"));
        }
        tui.frame_requester().schedule_frame();
    }

    pub(super) async fn handle_key_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key_event: KeyEvent,
    ) {
        // Some terminals, especially on macOS, encode Option+Left/Right as Option+b/f unless
        // enhanced keyboard reporting is available. We only treat those word-motion fallbacks as
        // agent-switch shortcuts when the composer is empty so we never steal the expected
        // editing behavior for moving across words inside a draft.
        let allow_agent_word_motion_fallback = !self.enhanced_keys_supported
            && self.chat_widget.composer_text_with_pending().is_empty();
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            // Alt+Left/Right are also natural word-motion keys in the composer. Keep agent
            // fast-switch available only once the draft is empty so editing behavior wins whenever
            // there is text on screen.
            && self.chat_widget.composer_text_with_pending().is_empty()
            && previous_agent_shortcut_matches(key_event, allow_agent_word_motion_fallback)
        {
            if let Some(thread_id) = self
                .adjacent_thread_id_with_backfill(app_server, AgentNavigationDirection::Previous)
                .await
            {
                let _ = self
                    .select_agent_thread_and_discard_side(tui, app_server, thread_id)
                    .await;
            }
            return;
        }
        if self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            // Mirror the previous-agent rule above: empty drafts may use these keys for thread
            // switching, but non-empty drafts keep them for expected word-wise cursor motion.
            && self.chat_widget.composer_text_with_pending().is_empty()
            && next_agent_shortcut_matches(key_event, allow_agent_word_motion_fallback)
        {
            if let Some(thread_id) = self
                .adjacent_thread_id_with_backfill(app_server, AgentNavigationDirection::Next)
                .await
            {
                let _ = self
                    .select_agent_thread_and_discard_side(tui, app_server, thread_id)
                    .await;
            }
            return;
        }
        if matches!(self.app_server_target, AppServerTarget::LocalDaemon { .. })
            && self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            && self.chat_widget.composer_is_empty()
            && self.active_side_parent_thread_id().is_none()
            && matches!(
                key_event,
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers,
                    kind: KeyEventKind::Press,
                    ..
                } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'c')
            )
        {
            let mut running_thread_id = if self.chat_widget.is_agent_turn_running() {
                self.chat_widget.thread_id()
            } else {
                None
            };
            let mut running_side_thread_id =
                running_thread_id.filter(|thread_id| self.side_threads.contains_key(thread_id));
            if running_side_thread_id.is_none() {
                for thread_id in self.side_threads.keys().copied() {
                    if self.active_turn_id_for_thread(thread_id).await.is_some() {
                        running_side_thread_id = Some(thread_id);
                        break;
                    }
                }
            }
            if running_thread_id.is_none() {
                running_thread_id = running_side_thread_id;
            }

            if let Some(thread_id) = running_thread_id {
                let allow_background = running_side_thread_id.is_none()
                    && !self.chat_widget.has_queued_follow_up_messages();
                self.chat_widget.show_selection_view(SelectionViewParams {
                    title: Some("Task is still running".to_string()),
                    subtitle: Some("Choose what happens to the current task.".to_string()),
                    footer_hint: Some(standard_popup_hint_line()),
                    items: [
                        (
                            "Cancel task",
                            "Stop the current task and stay in Codex",
                            RunningTaskExitAction::CancelTask,
                        ),
                        (
                            "Run in background",
                            "Exit Codex and leave the task running",
                            RunningTaskExitAction::RunInBackground,
                        ),
                        (
                            "Exit",
                            "Stop the current task and exit Codex",
                            RunningTaskExitAction::Exit,
                        ),
                    ]
                    .into_iter()
                    .filter(|(_, _, action)| {
                        allow_background || *action != RunningTaskExitAction::RunInBackground
                    })
                    .map(|(name, description, action)| SelectionItem {
                        name: name.to_string(),
                        description: Some(description.to_string()),
                        actions: vec![Box::new(move |tx| {
                            tx.send(AppEvent::RunningTaskExit { action, thread_id });
                        })],
                        dismiss_on_select: true,
                        ..Default::default()
                    })
                    .collect(),
                    ..Default::default()
                });
                return;
            }
        }

        if side_return_shortcut_matches(key_event)
            && self.maybe_return_from_side(tui, app_server).await
        {
            return;
        }

        let app_keymap_shortcuts_available = self.app_keymap_shortcuts_available();

        let side_toggle_bindings = &self.keymap.app.toggle_side_conversation;
        if app_keymap_shortcuts_available
            && (side_toggle_bindings.is_pressed(key_event)
                || side_toggle_bindings.contains(&crate::key_hint::ctrl(KeyCode::Char('/')))
                    && crate::key_hint::ctrl(KeyCode::Char('7')).is_press(key_event))
        {
            if let Err(err) = self.toggle_side_conversation(tui, app_server).await {
                self.chat_widget
                    .add_error_message(format!("Failed to switch side conversation: {err}"));
            }
            return;
        }

        if app_keymap_shortcuts_available && self.keymap.app.toggle_vim_mode.is_pressed(key_event) {
            self.chat_widget.toggle_vim_mode_and_notify();
            return;
        }

        if app_keymap_shortcuts_available
            && self.keymap.app.toggle_fast_mode.is_pressed(key_event)
            && self.chat_widget.can_toggle_fast_mode_from_keybinding()
        {
            self.chat_widget.toggle_fast_mode_from_ui();
            return;
        }

        if app_keymap_shortcuts_available && self.keymap.app.toggle_raw_output.is_pressed(key_event)
        {
            let enabled = !self.chat_widget.raw_output_mode();
            self.apply_raw_output_mode(tui, enabled, /*notify*/ false);
            return;
        }

        if app_keymap_shortcuts_available && self.keymap.app.open_agents.is_pressed(key_event) {
            self.open_agents_overview(app_server);
            return;
        }

        if app_keymap_shortcuts_available && self.keymap.app.open_transcript.is_pressed(key_event) {
            self.scrollback_has_older_history = self
                .chat_widget
                .thread_id()
                .is_some_and(|thread_id| app_server.has_older_history(thread_id));
            self.open_transcript_overlay(tui);
            return;
        }

        if app_keymap_shortcuts_available
            && self.keymap.app.open_external_editor.is_pressed(key_event)
        {
            // Only launch the external editor if there is no overlay and the bottom pane is not in use.
            // Note that it can be launched while a task is running to enable editing while the previous turn is ongoing.
            if self.overlay.is_none()
                && self.chat_widget.can_launch_external_editor()
                && self.chat_widget.external_editor_state() == ExternalEditorState::Closed
            {
                self.request_external_editor_launch(tui);
            }
            return;
        }

        if matches!(key_event.code, KeyCode::Esc)
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            // Esc primes/advances backtracking only in normal (not working) mode
            // with the composer focused and empty. In any other state, forward
            // Esc so the active UI (e.g. status indicator, modals, popups)
            // handles it.
            if self.should_handle_backtrack_esc(key_event) {
                self.handle_backtrack_esc_key(tui);
            } else if self.should_reject_side_backtrack_esc(key_event) {
                self.reject_side_backtrack_esc();
            } else {
                self.chat_widget.handle_key_event(key_event);
            }
            return;
        }

        match key_event {
            _ if app_keymap_shortcuts_available
                && self.keymap.app.clear_terminal.is_pressed(key_event) =>
            {
                if !self.chat_widget.can_run_ctrl_l_clear_now() {
                    return;
                }
                if let Err(err) = self.clear_terminal_ui(tui, /*redraw_header*/ false) {
                    tracing::warn!(error = %err, "failed to clear terminal UI");
                    self.chat_widget
                        .add_error_message(format!("Failed to clear terminal UI: {err}"));
                } else {
                    self.reset_app_ui_state_after_clear();
                    self.queue_clear_ui_header(tui);
                    tui.frame_requester().schedule_frame();
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self.chat_widget.composer_is_empty() =>
            {
                if let Some(selection) = self.confirm_backtrack_from_main() {
                    self.apply_backtrack_selection(selection);
                    tui.frame_requester().schedule_frame();
                }
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                // Any non-Esc key press should cancel a primed backtrack.
                // This avoids stale "Esc-primed" state after the user starts typing
                // (even if they later backspace to empty).
                if key_event.code != KeyCode::Esc && self.backtrack.primed {
                    self.reset_backtrack_state();
                }
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                self.chat_widget.handle_key_event(key_event);
            }
        };
    }

    pub(super) fn should_handle_backtrack_esc(&self, key_event: KeyEvent) -> bool {
        !self.chat_widget.side_conversation_active()
            && self.chat_widget.is_normal_backtrack_mode()
            && self.chat_widget.composer_is_empty()
            && !self.chat_widget.should_handle_vim_insert_escape(key_event)
    }

    pub(super) fn should_reject_side_backtrack_esc(&self, key_event: KeyEvent) -> bool {
        self.chat_widget.side_conversation_active()
            && self.chat_widget.is_normal_backtrack_mode()
            && self.chat_widget.composer_is_empty()
            && !self.chat_widget.should_handle_vim_insert_escape(key_event)
    }

    pub(super) fn reject_side_backtrack_esc(&mut self) {
        self.reset_backtrack_state();
        self.chat_widget
            .add_error_message(SIDE_EDIT_PREVIOUS_UNAVAILABLE_MESSAGE.to_string());
    }

    fn app_keymap_shortcuts_available(&self) -> bool {
        self.overlay.is_none() && self.chat_widget.no_modal_or_popup_active()
    }

    pub(super) fn refresh_status_line(&mut self) {
        self.chat_widget.refresh_status_line();
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::make_test_app;

    #[tokio::test]
    async fn app_keymap_shortcuts_are_disabled_while_keymap_view_is_active() {
        let mut app = make_test_app().await;
        assert!(app.app_keymap_shortcuts_available());

        let keymap = app.keymap.clone();
        app.chat_widget.open_keymap_debug(&keymap);

        assert!(!app.app_keymap_shortcuts_available());
    }
}
