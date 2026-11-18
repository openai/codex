//! Restricted editing retains drafts and allows local warnings, configuration, and quit commands.
//! Unavailable threads also allow recovery and other local commands.
//! Connected unavailable threads retain the empty-prompt agents shortcut.
//! Paste Enter handling is shared with normal submission so buffered newlines survive both paths.
//! In the draft-only handoff, a recent paste prefix can preserve a raw Enter as a newline.
//! The submit path still requires an established paste before treating Enter as a newline.
//! Recovery commands must occupy one line and be visible before the submit key expands pastes.
//! Offline draft edits also consume the Astra sparkle opportunity before rendering.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestrictedInputMode {
    Disconnected,
    UnavailableThread,
    DraftHandoff,
}

impl ChatComposer {
    pub(crate) fn set_restricted_input_mode(&mut self, mode: Option<RestrictedInputMode>) {
        self.popups.restricted_input_mode = mode;
        // Reconcile a popup opened before the routing mode changed, before Tab can accept it.
        if matches!(self.popups.active, ActivePopup::Argument(_)) {
            self.sync_slash_argument_popup(/*allow*/ true);
        }
    }

    /// Preserve Enter inside a paste burst without attempting submission.
    pub(crate) fn handle_paste_enter(&mut self, now: Instant) -> bool {
        let in_slash_context = self.slash_commands_enabled()
            && !self.draft.is_bash_mode
            && (matches!(self.popups.active, ActivePopup::Command(_))
                || self
                    .draft
                    .textarea
                    .text()
                    .lines()
                    .next()
                    .unwrap_or("")
                    .starts_with('/'));
        !in_slash_context && self.append_paste_newline(now)
    }

    fn append_paste_newline(&mut self, now: Instant) -> bool {
        if self.draft.disable_paste_burst {
            return false;
        }
        if self.draft.paste_burst.is_active()
            && self
                .draft
                .paste_burst
                .append_control_char_if_active('\n', now)
        {
            self.begin_promoted_paste_edit();
            return true;
        }
        if self
            .draft
            .paste_burst
            .newline_should_insert_instead_of_submit(now)
        {
            self.draft.textarea.insert_str("\n");
            self.draft.paste_burst.extend_window(now);
            return true;
        }

        false
    }

    pub(crate) fn handle_restricted_key(
        &mut self,
        key: KeyEvent,
        mode: RestrictedInputMode,
    ) -> InputResult {
        self.set_restricted_input_mode(Some(mode));
        if mode == RestrictedInputMode::DraftHandoff {
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                let now = tokio::time::Instant::now().into_std();
                self.flush_handoff_paste(now);
                // Completion does not own keys during the handoff.
                self.popups.active = ActivePopup::None;
                if self.draft.textarea.wants_vim_search_key(key) {
                    self.edit_restricted_draft(key);
                } else if self.handle_edit_history_key(key) {
                    self.draft.textarea_state.get_mut().follow_cursor();
                } else {
                    // Enter cannot submit here. Preserve a possible legacy paste
                    // newline after a directly inserted prefix, as we do for Tab.
                    let paste_separator = (key.code == KeyCode::Enter
                        && key.modifiers.is_empty()
                        && (self.append_paste_newline(now)
                            || self.append_recent_paste_separator('\n', now)))
                        || self.handle_paste_tab(key, now);
                    if !paste_separator
                        && !self.submit_keys.is_pressed(key)
                        && !self.queue_keys.is_pressed(key)
                    {
                        self.edit_restricted_draft(key);
                    }
                }
                // A paste flush can open a popup before the edit changes its text.
                // Dismiss it before ordinary input resumes, even without a redraw.
                self.popups.active = ActivePopup::None;
            }
            return InputResult::None;
        }
        if mode == RestrictedInputMode::UnavailableThread && self.handle_agents_navigation(key) {
            return InputResult::None;
        }
        self.cancel_history_search();
        self.attachments.clear_remote_image_selection();
        if mode != RestrictedInputMode::UnavailableThread
            || !matches!(
                self.popups.active,
                ActivePopup::Command(_) | ActivePopup::Argument(_)
            )
        {
            self.popups.active = ActivePopup::None;
        }
        self.set_disable_paste_burst(/*disabled*/ true);
        let history_handled = matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && self.handle_edit_history_key(key);
        // Expand in reverse order so remaining ranges stay valid. Textarea replacement
        // preserves the cursor and other elements, including images and mentions.
        let pending_pastes = std::mem::take(&mut self.draft.pending_pastes);
        if !pending_pastes.is_empty() {
            self.popups.active = ActivePopup::None;
            for element in self.draft.textarea.text_elements().into_iter().rev() {
                if let Some((_, text)) = pending_pastes.iter().find(|(placeholder, _)| {
                    element.placeholder(self.draft.textarea.text()) == Some(placeholder.as_str())
                }) {
                    self.draft
                        .textarea
                        .replace_range(element.byte_range.start..element.byte_range.end, text);
                }
            }
        }
        if history_handled {
            self.draft.textarea_state.get_mut().follow_cursor();
            return InputResult::None;
        }
        if mode == RestrictedInputMode::UnavailableThread
            && self.draft.textarea.wants_vim_search_key(key)
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            self.popups.active = ActivePopup::None;
            self.edit_restricted_draft(key);
            return InputResult::None;
        }
        if mode == RestrictedInputMode::UnavailableThread
            && pending_pastes.is_empty()
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && matches!(self.popups.active, ActivePopup::Argument(_))
            && !self.submit_keys.is_pressed(key)
            && (matches!(
                key.code,
                KeyCode::Tab | KeyCode::Up | KeyCode::Down | KeyCode::Esc
            ) || key.modifiers == KeyModifiers::CONTROL
                && matches!(key.code, KeyCode::Char('p' | 'n')))
        {
            let vim_escape = self.should_handle_vim_insert_escape(key);
            let result = self.handle_slash_argument_key(key).0;
            if vim_escape {
                let first_line = self.draft.textarea.text().lines().next().unwrap_or("");
                self.popups.dismissed_command_token =
                    slash_input::command_popup_filter_text(first_line, /*cursor*/ 0);
                self.edit_restricted_draft(key);
            }
            self.sync_unavailable_slash_popup();
            return result;
        }
        if mode == RestrictedInputMode::UnavailableThread
            && pending_pastes.is_empty()
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && let ActivePopup::Command(popup) = &self.popups.active
        {
            let submit = self.submit_keys.is_pressed(key);
            if (submit || key.code == KeyCode::Tab)
                && let Some(selected) = popup.selected_item()
            {
                let text = self.draft.textarea.text();
                if !matches!(selected, CommandItem::Builtin(command)
                    if (command == SlashCommand::Config || command.available_when_thread_unavailable())
                        && text.trim().lines().count() == 1
                        && (command.supports_inline_args()
                            || text.split_once(char::is_whitespace).is_none_or(|(_, args)| args.trim().is_empty())))
                {
                    return InputResult::None;
                }
                let popup_key = if submit { KeyCode::Enter.into() } else { key };
                let result = self
                    .handle_key_event_with_slash_popup(
                        popup_key,
                        slash_input::SlashPopupInputMode::UnavailableThread,
                    )
                    .0;
                self.reset_vim_mode_after_successful_dispatch(&result);
                self.sync_unavailable_slash_popup();
                return result;
            }
            if !submit
                && (matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Esc)
                    || matches!(
                        key,
                        KeyEvent {
                            code: KeyCode::Char('p' | 'n'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        }
                    ))
            {
                let vim_escape = self.should_handle_vim_insert_escape(key);
                let result = self
                    .handle_key_event_with_slash_popup(
                        key,
                        slash_input::SlashPopupInputMode::UnavailableThread,
                    )
                    .0;
                if vim_escape {
                    self.edit_restricted_draft(key);
                }
                return result;
            }
        }
        if pending_pastes.is_empty()
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && self.submit_keys.is_pressed(key)
        {
            let input = self.slash_input();
            let text = self.draft.textarea.text();
            let command = input
                .bare_command(text)
                .or_else(|| input.inline_command(text).map(|command| command.command))
                .filter(|_| text.trim().lines().count() == 1);
            if matches!(command, Some(SlashCommandItem::Builtin(command))
                if matches!(command, SlashCommand::Warnings | SlashCommand::Config | SlashCommand::Rc | SlashCommand::Quit | SlashCommand::Exit)
                    || mode == RestrictedInputMode::UnavailableThread && command.available_when_thread_unavailable())
                && let Some(result) = self
                    .try_dispatch_bare_slash_command()
                    .or_else(|| self.try_dispatch_slash_command_with_args())
            {
                self.reset_vim_mode_after_successful_dispatch(&result);
                self.popups.active = ActivePopup::None;
                return result;
            }
        }

        // Other commands and prompts stay in the draft while offline.
        // The basic editor reconciles attachments without invoking composer-level shortcuts.
        if !matches!(key.code, KeyCode::Enter | KeyCode::Tab)
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            self.edit_restricted_draft(key);
            if mode == RestrictedInputMode::UnavailableThread {
                self.sync_unavailable_slash_popup();
            }
        }
        InputResult::None
    }

    fn sync_unavailable_slash_popup(&mut self) {
        let allow = self.slash_commands_enabled()
            && !self.draft.is_bash_mode
            && self.draft.textarea.vim_query().is_none();
        if !self.sync_slash_argument_popup(allow) {
            self.sync_command_popup(allow);
        }
    }

    fn edit_restricted_draft(&mut self, key: KeyEvent) {
        // Null is sent internally to clean up on disconnect or expand a paste.
        let before = if key.code == KeyCode::Null {
            None
        } else {
            self.draft.textarea_state.get_mut().follow_cursor();
            self.before_sparkle_editor_key(key)
        };
        self.begin_composer_key_edit(key);
        let (result, _) = self.handle_input_basic(key);
        if !self.draft.textarea.is_vim_enabled() {
            self.finish_edit();
        }
        self.after_sparkle_key(before, &result);
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
