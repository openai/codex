//! Key routing and composer-adjacent UI interaction for `ChatWidget`.

use super::*;

impl ChatWidget {
    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.bottom_pane.has_active_view()
            && !matches!(
                key_event,
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers,
                    kind: KeyEventKind::Press,
                    ..
                } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'c')
            )
            && !key_hint::ctrl(KeyCode::Char('r')).is_press(key_event)
            && !key_hint::ctrl(KeyCode::Char('u')).is_press(key_event)
        {
            self.bottom_pane.handle_key_event(key_event);
            if self.bottom_pane.no_modal_or_popup_active() {
                self.maybe_send_next_queued_input();
            }
            return;
        }

        if self.handle_reasoning_shortcut(key_event) {
            self.bottom_pane.clear_quit_shortcut_hint();
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            return;
        }

        if key_event.kind == KeyEventKind::Press
            && self.copy_last_response_binding.is_pressed(key_event)
        {
            self.bottom_pane.clear_quit_shortcut_hint();
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.copy_last_agent_markdown();
            return;
        }

        match key_event {
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'c') => {
                self.on_ctrl_c();
                return;
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'d') => {
                if self.on_ctrl_d() {
                    return;
                }
                self.bottom_pane.clear_quit_shortcut_hint();
                self.quit_shortcut_expires_at = None;
                self.quit_shortcut_key = None;
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                kind: KeyEventKind::Press,
                ..
            } if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && c.eq_ignore_ascii_case(&'v') =>
            {
                match paste_image_to_temp_png() {
                    Ok((path, info)) => {
                        tracing::debug!(
                            "pasted image size={}x{} format={}",
                            info.width,
                            info.height,
                            info.encoded_format.label()
                        );
                        self.attach_image(path);
                    }
                    Err(err) => {
                        tracing::warn!("failed to paste image: {err}");
                        self.add_to_history(history_cell::new_error_event(format!(
                            "Failed to paste image: {err}",
                        )));
                    }
                }
                return;
            }
            other if other.kind == KeyEventKind::Press => {
                self.bottom_pane.clear_quit_shortcut_hint();
                self.quit_shortcut_expires_at = None;
                self.quit_shortcut_key = None;
            }
            _ => {}
        }

        if key_event.kind == KeyEventKind::Press
            && self.chat_keymap.edit_queued_message.is_pressed(key_event)
            && self.has_queued_follow_up_messages()
            && self.bottom_pane.no_modal_or_popup_active()
        {
            if let Some(user_message) = self.pop_latest_queued_user_message() {
                self.restore_user_message_to_composer(user_message);
                self.refresh_pending_input_preview();
                self.request_redraw();
            }
            return;
        }

        if matches!(key_event.code, KeyCode::Esc)
            && matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && !self.input_queue.pending_steers.is_empty()
            && self.bottom_pane.is_task_running()
            && self.bottom_pane.no_modal_or_popup_active()
            && !self.should_handle_vim_insert_escape(key_event)
        {
            self.input_queue.submit_pending_steers_after_interrupt = true;
            if !self.submit_op(AppCommand::interrupt()) {
                self.input_queue.submit_pending_steers_after_interrupt = false;
            }
            return;
        }

        if matches!(key_event.code, KeyCode::Esc)
            && key_event.kind == KeyEventKind::Press
            && self.should_show_plan_mode_nudge()
        {
            self.dismiss_plan_mode_nudge();
            return;
        }

        if self.handle_plugins_popup_key_event(key_event) {
            return;
        }

        match key_event {
            KeyEvent {
                code: KeyCode::BackTab,
                kind: KeyEventKind::Press,
                ..
            } if self.collaboration_modes_enabled()
                && !self.bottom_pane.is_task_running()
                && self.bottom_pane.no_modal_or_popup_active() =>
            {
                self.cycle_collaboration_mode();
                self.refresh_plan_mode_nudge();
            }
            _ => {
                let had_modal_or_popup = !self.bottom_pane.no_modal_or_popup_active();
                let input_result = self.bottom_pane.handle_key_event(key_event);
                self.handle_composer_input_result(input_result, had_modal_or_popup);
            }
        }
    }

    /// Attach a local image to the composer when the active model supports image inputs.
    ///
    /// When the model does not advertise image support, we keep the draft unchanged and surface a
    /// warning event so users can switch models or remove attachments.
    pub(crate) fn attach_image(&mut self, path: PathBuf) {
        if !self.current_model_supports_images() {
            self.add_to_history(history_cell::new_warning_event(
                self.image_inputs_not_supported_message(),
            ));
            self.request_redraw();
            return;
        }
        tracing::info!("attach_image path={path:?}");
        self.bottom_pane.attach_image(path);
        self.request_redraw();
    }

    pub(crate) fn composer_text_with_pending(&self) -> String {
        self.bottom_pane.composer_text_with_pending()
    }

    pub(crate) fn apply_external_edit(&mut self, text: String) {
        self.bottom_pane.apply_external_edit(text);
        self.refresh_plan_mode_nudge();
        self.request_redraw();
    }

    pub(crate) fn external_editor_state(&self) -> ExternalEditorState {
        self.external_editor_state
    }

    pub(crate) fn set_external_editor_state(&mut self, state: ExternalEditorState) {
        self.external_editor_state = state;
    }

    pub(crate) fn set_footer_hint_override(&mut self, items: Option<Vec<(String, String)>>) {
        self.bottom_pane.set_footer_hint_override(items);
    }

    pub(crate) fn show_selection_view(&mut self, params: SelectionViewParams) {
        self.bottom_pane.show_selection_view(params);
        self.refresh_plan_mode_nudge();
        self.request_redraw();
    }

    pub(crate) fn no_modal_or_popup_active(&self) -> bool {
        self.bottom_pane.no_modal_or_popup_active()
    }

    pub(crate) fn can_launch_external_editor(&self) -> bool {
        self.bottom_pane.can_launch_external_editor()
    }

    pub(crate) fn can_run_ctrl_l_clear_now(&mut self) -> bool {
        // Ctrl+L is not a slash command, but it follows /clear's current rule:
        // block while a task is running.
        if !self.bottom_pane.is_task_running() {
            return true;
        }

        let message = "Ctrl+L is disabled while a task is in progress.".to_string();
        self.add_to_history(history_cell::new_error_event(message));
        self.request_redraw();
        false
    }

    /// Copy the last agent response (raw markdown) to the system clipboard.
    pub(crate) fn copy_last_agent_markdown(&mut self) {
        self.copy_last_agent_markdown_with(crate::clipboard_copy::copy_to_clipboard);
    }

    pub(crate) fn truncate_agent_copy_history_to_user_turn_count(
        &mut self,
        user_turn_count: usize,
    ) {
        self.transcript
            .truncate_copy_history_to_user_turn_count(user_turn_count);
    }

    /// Inner implementation with an injectable clipboard backend for testing.
    pub(super) fn copy_last_agent_markdown_with(
        &mut self,
        copy_fn: impl FnOnce(&str) -> Result<Option<crate::clipboard_copy::ClipboardLease>, String>,
    ) {
        match self.transcript.last_agent_markdown.clone() {
            Some(markdown) if !markdown.is_empty() => match copy_fn(&markdown) {
                Ok(lease) => {
                    self.clipboard_lease = lease;
                    self.add_to_history(history_cell::new_info_event(
                        "Copied last message to clipboard".into(),
                        /*hint*/ None,
                    ));
                }
                Err(error) => self.add_to_history(history_cell::new_error_event(format!(
                    "Copy failed: {error}"
                ))),
            },
            _ if self.transcript.copy_history_evicted_by_rollback => {
                self.add_to_history(history_cell::new_error_event(format!(
                    "Cannot copy that response after rewinding. Only the most recent {MAX_AGENT_COPY_HISTORY} responses are available to /copy."
                )));
            }
            _ => self.add_to_history(history_cell::new_error_event(
                "No agent response to copy".into(),
            )),
        }
        self.request_redraw();
    }

    #[cfg(test)]
    pub(crate) fn last_agent_markdown_text(&self) -> Option<&str> {
        self.transcript.last_agent_markdown.as_deref()
    }

    pub(super) fn show_rename_prompt(&mut self) {
        if !self.ensure_thread_rename_allowed() {
            return;
        }
        let tx = self.app_event_tx.clone();
        let existing_name = self.thread_name.as_deref().filter(|name| !name.is_empty());
        let title = if existing_name.is_some() {
            "Rename thread"
        } else {
            "Name thread"
        };
        let view = CustomPromptView::new(
            title.to_string(),
            "Type a name and press Enter".to_string(),
            /*initial_text*/ existing_name.unwrap_or_default().to_string(),
            /*context_label*/ None,
            Box::new(move |name: String| {
                let Some(name) = crate::legacy_core::util::normalize_thread_name(&name) else {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event("Thread name cannot be empty.".to_string()),
                    )));
                    return;
                };
                tx.set_thread_name(name);
            }),
        );

        self.bottom_pane.show_view(Box::new(view));
    }

    pub(super) fn ensure_thread_rename_allowed(&mut self) -> bool {
        match self.thread_rename_block_message.clone() {
            Some(message) => {
                self.add_error_message(message);
                false
            }
            None => true,
        }
    }

    pub(crate) fn handle_paste(&mut self, text: String) {
        self.bottom_pane.handle_paste(text);
        self.refresh_plan_mode_nudge();
    }

    // Returns true if caller should skip rendering this frame (a future frame is scheduled).
    pub(crate) fn handle_paste_burst_tick(&mut self, frame_requester: FrameRequester) -> bool {
        if self.bottom_pane.flush_paste_burst_if_due() {
            self.refresh_plan_mode_nudge();
            // A paste just flushed; request an immediate redraw and skip this frame.
            self.request_redraw();
            true
        } else if self.bottom_pane.is_in_paste_burst() {
            // While capturing a burst, schedule a follow-up tick and skip this frame
            // to avoid redundant renders between ticks.
            frame_requester.schedule_frame_in(
                crate::bottom_pane::ChatComposer::recommended_paste_flush_delay(),
            );
            true
        } else {
            false
        }
    }

    /// Handles a Ctrl+C press at the chat-widget layer.
    ///
    /// The first press arms a time-bounded quit shortcut and shows a footer hint via the bottom
    /// pane. If cancellable work is active, Ctrl+C also submits `Op::Interrupt` after the shortcut
    /// is armed.
    ///
    /// Active realtime conversations take precedence over bottom-pane Ctrl+C handling so the
    /// first press always stops live voice, even when the composer contains the recording meter.
    ///
    /// When the double-press quit shortcut is enabled, pressing the same shortcut again before
    /// expiry requests a shutdown-first quit.
    fn on_ctrl_c(&mut self) {
        let key = key_hint::ctrl(KeyCode::Char('c'));
        if self.realtime_conversation.is_live() {
            self.bottom_pane.clear_quit_shortcut_hint();
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.stop_realtime_conversation_from_ui();
            return;
        }
        let modal_or_popup_active = !self.bottom_pane.no_modal_or_popup_active();
        if self.bottom_pane.on_ctrl_c() == CancellationEvent::Handled {
            if DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
                if modal_or_popup_active {
                    self.quit_shortcut_expires_at = None;
                    self.quit_shortcut_key = None;
                    self.bottom_pane.clear_quit_shortcut_hint();
                } else {
                    self.arm_quit_shortcut(key);
                }
            }
            return;
        }

        if !DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
            if self.is_cancellable_work_active() {
                self.quit_shortcut_expires_at = None;
                self.quit_shortcut_key = None;
                self.bottom_pane.clear_quit_shortcut_hint();
                self.submit_op(AppCommand::interrupt());
            } else {
                self.request_quit_without_confirmation();
            }
            return;
        }

        if self.quit_shortcut_active_for(key) {
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.request_quit_without_confirmation();
            return;
        }

        self.arm_quit_shortcut(key);

        if self.is_cancellable_work_active() {
            self.submit_op(AppCommand::interrupt());
        }
    }

    /// Handles a Ctrl+D press at the chat-widget layer.
    ///
    /// Ctrl-D only participates in quit when the composer is empty and no modal/popup is active.
    /// Otherwise it should be routed to the active view and not attempt to quit.
    fn on_ctrl_d(&mut self) -> bool {
        let key = key_hint::ctrl(KeyCode::Char('d'));
        if !DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED {
            if !self.bottom_pane.composer_is_empty() || !self.bottom_pane.no_modal_or_popup_active()
            {
                return false;
            }

            self.request_quit_without_confirmation();
            return true;
        }

        if self.quit_shortcut_active_for(key) {
            self.quit_shortcut_expires_at = None;
            self.quit_shortcut_key = None;
            self.request_quit_without_confirmation();
            return true;
        }

        if !self.bottom_pane.composer_is_empty() || !self.bottom_pane.no_modal_or_popup_active() {
            return false;
        }

        self.arm_quit_shortcut(key);
        true
    }

    /// True if `key` matches the armed quit shortcut and the window has not expired.
    fn quit_shortcut_active_for(&self, key: KeyBinding) -> bool {
        self.quit_shortcut_key == Some(key)
            && self
                .quit_shortcut_expires_at
                .is_some_and(|expires_at| Instant::now() < expires_at)
    }

    /// Arm the double-press quit shortcut and show the footer hint.
    ///
    /// This keeps the state machine (`quit_shortcut_*`) in `ChatWidget`, since
    /// it is the component that interprets Ctrl+C vs Ctrl+D and decides whether
    /// quitting is currently allowed, while delegating rendering to `BottomPane`.
    pub(super) fn arm_quit_shortcut(&mut self, key: KeyBinding) {
        self.quit_shortcut_expires_at = Instant::now()
            .checked_add(QUIT_SHORTCUT_TIMEOUT)
            .or_else(|| Some(Instant::now()));
        self.quit_shortcut_key = Some(key);
        self.bottom_pane.show_quit_shortcut_hint(key);
    }

    // Review mode counts as cancellable work so Ctrl+C interrupts instead of quitting.
    fn is_cancellable_work_active(&self) -> bool {
        self.bottom_pane.is_task_running() || self.review.is_review_mode
    }
}
