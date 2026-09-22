//! Keyboard input, external editor, and status-line dispatch for the TUI app.
//!
//! This module owns global key bindings that sit above ChatWidget, including transcript overlay
//! entry, Ctrl-L clear, external editor launch, and agent navigation shortcuts.

use super::*;
use crate::app_backtrack::SIDE_EDIT_PREVIOUS_UNAVAILABLE_MESSAGE;
use crate::keymap::bindings_for_action;
use crate::keymap::keymap_action_ids;

impl App {
    pub(super) fn should_recover_vim_insert_escape(&self, key_event: KeyEvent) -> bool {
        let active_contexts = self.active_keymap_contexts();
        // Legacy terminals encode Alt+character and Escape+character identically. Active
        // bindings and either stroke of a chord win; inactive bindings do not consume input.
        cfg!(unix)
            && !self.enhanced_keys_supported
            && self.overlay.is_none()
            && !self.transcript_view.is_search_active()
            && self.chat_widget.no_modal_or_popup_active()
            && matches!(key_event.code, KeyCode::Char(_))
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && (key_event.modifiers == KeyModifiers::ALT
                || key_event.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT))
            && self
                .chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            // ChatWidget's fixed image-paste shortcut is not in the configurable keymap.
            && !(key_event.kind == KeyEventKind::Press
                && matches!(key_event.code, KeyCode::Char('v' | 'V')))
            // Empty-draft agent navigation also has fixed legacy-terminal fallbacks.
            && !(self.chat_widget.composer_text_with_pending().is_empty()
                && (previous_agent_shortcut_matches(key_event, /*allow_word_motion_fallback*/ true)
                    || next_agent_shortcut_matches(key_event, /*allow_word_motion_fallback*/ true)))
            && !keymap_action_ids()
                .filter(|action| active_contexts.contains_action(*action))
                .any(|action| {
                    bindings_for_action(&self.keymap, action.context.config_name(), action.action)
                        .is_some_and(|bindings| bindings.is_pressed(key_event))
                })
            && !matches!(
                self.key_chord_matcher.clone().advance(
                    key_event,
                    &self.keymap.chords,
                    active_contexts),
                crate::keymap::KeyChordMatch::Pending(_)
                    | crate::keymap::KeyChordMatch::Completed(_)
            )
    }

    pub(super) fn route_key_chord_event(
        &mut self,
        tui: &mut tui::Tui,
        key_event: KeyEvent,
    ) -> Option<KeyEvent> {
        self.transcript_view.set_keymap_bindings(&self.keymap);
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.set_keymap_bindings(&self.keymap);
        }
        if matches!(&self.overlay, Some(Overlay::Transcript(overlay)) if overlay.owns_interaction_key(key_event))
            || (tui.is_owned_screen()
                && self.overlay.is_none()
                && self.chat_widget.no_modal_or_popup_active()
                && self.transcript_view.owns_interaction_key(key_event)
                && (self.transcript_view.has_active_interaction()
                    || self.backtrack.overlay_preview_active
                    || crate::transcript_view::JumpTarget::from_key(key_event).is_none()))
        {
            let close_chord = tui.is_owned_screen()
                && self.overlay.is_none()
                && self.transcript_view.is_detailed()
                && !self.transcript_view.has_active_interaction()
                && !self.backtrack.overlay_preview_active
                && match self.key_chord_matcher.clone().advance(
                    key_event,
                    &self.keymap.chords,
                    self.active_keymap_contexts(),
                ) {
                    crate::keymap::KeyChordMatch::Completed(event) => {
                        self.keymap.pager.close_transcript.is_pressed(event)
                    }
                    crate::keymap::KeyChordMatch::Pending(prefix) => {
                        self.keymap.chords.bindings.iter().any(|binding| {
                            binding.action.context == crate::keymap::KeymapContext::Pager
                                && binding.action.action == "close_transcript"
                                && binding.chord.prefix == prefix
                        })
                    }
                    crate::keymap::KeyChordMatch::PassThrough
                    | crate::keymap::KeyChordMatch::Cancelled
                    | crate::keymap::KeyChordMatch::Ignored => false,
                };
            if !close_chord {
                self.cancel_pending_key_chord();
                return Some(key_event);
            }
        }
        let contexts = self.active_keymap_contexts();
        let was_pending = self.key_chord_matcher.is_pending();
        if !was_pending
            && contexts.is_warnings()
            && !crate::key_hint::is_plain_text_key_event(key_event)
            && self
                .keymap
                .list
                .action_for(key_event)
                .is_some_and(|action| action != crate::keymap::ListAction::Accept)
        {
            return Some(key_event);
        }
        if !was_pending
            && contexts.contains(crate::keymap::KeymapContext::Agents)
            && self
                .agents_overview
                .view_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .editing_metadata()
            && crate::key_hint::is_plain_text_key_event(key_event)
        {
            return Some(key_event);
        }
        match self
            .key_chord_matcher
            .advance(key_event, &self.keymap.chords, contexts)
        {
            crate::keymap::KeyChordMatch::PassThrough => {
                if was_pending && !self.key_chord_matcher.is_pending() {
                    self.set_key_chord_hint_override(/*items*/ None);
                }
                Some(key_event)
            }
            crate::keymap::KeyChordMatch::Pending(_) => {
                if self.backtrack.primed && !self.backtrack.overlay_preview_active {
                    self.reset_backtrack_state();
                }
                self.set_key_chord_hint_override(self.key_chord_matcher.pending_hint_items(
                    &self.keymap.chords,
                    tui.terminal.last_known_screen_size.width,
                ));
                tui.frame_requester()
                    .schedule_frame_in(crate::keymap::KEY_CHORD_TIMEOUT);
                None
            }
            crate::keymap::KeyChordMatch::Completed(dispatch_event) => {
                self.set_key_chord_hint_override(/*items*/ None);
                Some(dispatch_event)
            }
            crate::keymap::KeyChordMatch::Cancelled => {
                self.set_key_chord_hint_override(/*items*/ None);
                None
            }
            crate::keymap::KeyChordMatch::Ignored => None,
        }
    }

    pub(super) fn expire_pending_key_chord(&mut self) {
        let contexts = self.active_keymap_contexts();
        if self.key_chord_matcher.expire(contexts) {
            self.set_key_chord_hint_override(/*items*/ None);
        }
    }

    pub(super) fn cancel_pending_key_chord(&mut self) {
        if self.key_chord_matcher.cancel() {
            self.set_key_chord_hint_override(/*items*/ None);
        }
    }

    fn set_key_chord_hint_override(&mut self, items: Option<Vec<(String, String)>>) {
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.key_chord_hint = items.clone();
        }
        self.agents_overview
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .key_chord_hint = items.clone();
        self.chat_widget.set_footer_hint_override(items);
    }

    pub(super) fn active_keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        use crate::keymap::KeymapContext;
        use crate::keymap::KeymapContextSet;

        if let Some(overlay) = &self.overlay {
            let context = if matches!(overlay, Overlay::Transcript(view) if view.is_search_active())
            {
                KeymapContext::Editor
            } else {
                KeymapContext::Pager
            };
            let contexts = KeymapContextSet::new(context).with_voice_toggle(&self.keymap);
            return if self.backtrack.overlay_preview_active && context == KeymapContext::Pager {
                KeymapContextSet::browsing().with_voice_toggle(&self.keymap)
            } else {
                contexts
            };
        }
        if self.transcript_view.is_search_active() && self.chat_widget.no_modal_or_popup_active() {
            return KeymapContextSet::new(KeymapContext::Editor).with_voice_toggle(&self.keymap);
        }
        if self.transcript_view.is_activity_focused() && self.chat_widget.no_modal_or_popup_active()
        {
            return KeymapContextSet::activity().with_voice_toggle(&self.keymap);
        }
        if self.backtrack.overlay_preview_active && self.chat_widget.no_modal_or_popup_active() {
            return KeymapContextSet::browsing().with_voice_toggle(&self.keymap);
        }
        let voice_available = self.chat_widget.realtime_microphone_shortcut_available()
            || self.voice_owner_thread_id().is_some();
        let contexts = self.chat_widget.keymap_contexts();
        let contexts = if self.chat_widget.no_modal_or_popup_active() {
            let contexts = contexts
                .with(KeymapContext::Global)
                .with(KeymapContext::Chat);
            let contexts = if self.transcript_view.is_detailed() {
                contexts.with_transcript_close()
            } else {
                contexts
            };
            if voice_available {
                contexts.with(KeymapContext::Voice)
            } else {
                contexts
            }
        } else {
            contexts
        };
        contexts.with_voice_toggle(&self.keymap)
    }

    pub(super) async fn launch_external_editor(&mut self, tui: &mut tui::Tui) {
        self.chat_widget
            .empty_state_animation
            .borrow_mut()
            .pause_clock();
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
            .empty_state_animation
            .borrow_mut()
            .pause_clock();
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
        if tui.is_owned_screen() {
            self.transcript_view.set_presentation(
                self.transcript_view.is_detailed(),
                self.chat_widget.history_render_mode(),
            );
            tui.frame_requester().schedule_frame();
            return;
        }
        if self.overlay.is_some() {
            self.schedule_immediate_resize_reflow(tui);
            return;
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
        if self.chat_widget.fork_in_progress {
            return;
        }
        if self.chat_widget.is_external_writer_view()
            && self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
            && key_event.kind == KeyEventKind::Press
        {
            let modifiers = key_event.modifiers;
            if key_event.code == KeyCode::Esc
                && modifiers == KeyModifiers::NONE
                && !matches!(self.app_server_target, AppServerTarget::Embedded)
            {
                self.open_agents_overview(app_server);
                return;
            }
            let quit = match key_event.code {
                KeyCode::Esc => modifiers == KeyModifiers::NONE,
                KeyCode::Char('q' | 'Q') => {
                    modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT
                }
                KeyCode::Char('c' | 'C') => {
                    modifiers == KeyModifiers::CONTROL
                        || modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                }
                _ => false,
            };
            if quit {
                self.app_event_tx.send(AppEvent::Exit(ExitMode::Immediate));
                return;
            }
            if matches!(key_event.code, KeyCode::Char('f' | 'F'))
                && (modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT)
            {
                self.chat_widget.fork_in_progress = true;
                self.app_event_tx
                    .send(AppEvent::ForkCurrentSession { name: None });
                return;
            }
            if matches!(key_event.code, KeyCode::Char('r' | 'R'))
                && (modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT)
                && let Some(thread_id) = self.current_displayed_thread_id()
            {
                let target = SessionTarget {
                    path: self
                        .primary_session_configured
                        .as_ref()
                        .and_then(|session| session.rollout_path.clone()),
                    thread_id,
                    cwd: None,
                    history_mode: None,
                };
                if let Ok(AppRunControl::Exit(_)) =
                    self.resume_target_session(tui, app_server, target).await
                {
                    self.app_event_tx.send(AppEvent::Exit(ExitMode::Immediate));
                }
                return;
            }
        }
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
                    ..SelectionViewParams::picker()
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
        if app_keymap_shortcuts_available
            && self
                .handle_shared_app_keymap_action(tui, app_server, key_event)
                .await
        {
            return;
        }

        let find_transcript = self.keymap.app.find_transcript.is_pressed(key_event);
        if app_keymap_shortcuts_available
            && (self.keymap.app.open_transcript.is_pressed(key_event) || find_transcript)
        {
            self.scrollback_has_older_history = self
                .chat_widget
                .thread_id()
                .is_some_and(|thread_id| app_server.has_older_history(thread_id));
            self.open_transcript_overlay(tui);
            if find_transcript && let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
                overlay.set_keymap_bindings(&self.keymap);
                overlay.begin_search();
            }
            return;
        }

        if self.chat_widget.is_external_writer_view()
            && self.overlay.is_none()
            && self.chat_widget.no_modal_or_popup_active()
        {
            return;
        }

        if self.should_handle_unavailable_thread_key(key_event) {
            self.chat_widget
                .handle_restricted_key(key_event, RestrictedInputMode::UnavailableThread);
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
                self.chat_widget.prepare_composer_sparkle_key(key_event);
                if key_event.kind == KeyEventKind::Press {
                    self.handle_backtrack_esc_key(tui);
                }
            } else if self.should_reject_side_backtrack_esc(key_event) {
                self.reject_side_backtrack_esc();
            } else {
                self.chat_widget.handle_key_event(key_event);
            }
            return;
        }

        match key_event {
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
                    if self.backtrack.overlay_preview_active {
                        self.cancel_transcript_browsing(tui);
                    } else {
                        self.reset_backtrack_state();
                    }
                }
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                self.chat_widget.handle_key_event(key_event);
            }
        };
    }

    pub(crate) async fn handle_shared_app_keymap_action(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        key_event: KeyEvent,
    ) -> bool {
        if self.keymap.app.open_warnings.is_pressed(key_event) {
            self.chat_widget.open_warnings(&self.transcript_cells);
            return true;
        }
        let side_toggle_bindings = &self.keymap.app.toggle_side_conversation;
        if side_toggle_bindings.is_pressed(key_event)
            || side_toggle_bindings.contains(&crate::key_hint::ctrl(KeyCode::Char('/')))
                && crate::key_hint::ctrl(KeyCode::Char('7')).is_press(key_event)
        {
            if let Err(err) = self.toggle_side_conversation(tui, app_server).await {
                self.chat_widget
                    .add_error_message(format!("Failed to switch side conversation: {err}"));
            }
            return true;
        }

        if self.keymap.app.toggle_vim_mode.is_pressed(key_event) {
            self.chat_widget.toggle_vim_mode_and_notify();
            return true;
        }

        if self.keymap.app.toggle_fast_mode.is_pressed(key_event)
            && self.chat_widget.can_toggle_fast_mode_from_keybinding()
        {
            self.chat_widget.toggle_fast_mode_from_ui();
            return true;
        }

        if self.keymap.app.toggle_raw_output.is_pressed(key_event) {
            let enabled = !self.chat_widget.raw_output_mode();
            self.apply_raw_output_mode(tui, enabled, /*notify*/ false);
            return true;
        }

        if self.keymap.app.open_agents.is_pressed(key_event) {
            self.open_agents_overview(app_server);
            return true;
        }

        if self.keymap.app.open_external_editor.is_pressed(key_event) {
            if self.overlay.is_none()
                && self.chat_widget.can_launch_external_editor()
                && self.chat_widget.external_editor_state() == ExternalEditorState::Closed
            {
                self.request_external_editor_launch(tui);
            }
            return true;
        }

        if self.keymap.app.clear_terminal.is_pressed(key_event) {
            // Leave cached history intact and let the unavailable-thread input path handle this key.
            if self.should_handle_unavailable_thread_key(key_event) {
                return false;
            }
            if !self.chat_widget.can_run_ctrl_l_clear_now() {
                return true;
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
            return true;
        }

        false
    }

    fn should_handle_unavailable_thread_key(&self, key_event: KeyEvent) -> bool {
        !self.chat_widget.has_active_view()
            && self
                .current_displayed_thread_id()
                .is_some_and(|id| self.thread_unavailable(id))
            && !(key_event.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key_event.code, KeyCode::Char('c' | 'd')))
    }

    pub(crate) fn should_handle_backtrack_esc(&self, key_event: KeyEvent) -> bool {
        !self.chat_widget.is_external_writer_view()
            && !self.chat_widget.side_conversation_active()
            && !self.chat_widget.shortcut_overlay_visible()
            && self.chat_widget.is_normal_backtrack_mode()
            && self.chat_widget.composer_is_empty()
            && !self.chat_widget.should_handle_vim_insert_escape(key_event)
    }

    pub(super) fn should_reject_side_backtrack_esc(&self, key_event: KeyEvent) -> bool {
        self.chat_widget.side_conversation_active()
            && !self.chat_widget.shortcut_overlay_visible()
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
#[path = "input_ownership_tests.rs"]
mod ownership_tests;

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
