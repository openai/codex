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
        if mode == RestrictedInputMode::DraftHandoff {
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                let now = tokio::time::Instant::now().into_std();
                self.flush_handoff_paste(now);
                // Completion does not own keys during the handoff.
                self.popups.active = ActivePopup::None;
                if self.draft.textarea.wants_vim_search_key(key) {
                    self.edit_restricted_draft(key);
                } else if self.handle_vim_history_key(key) {
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
        self.popups.active = ActivePopup::None;
        self.set_disable_paste_burst(/*disabled*/ true);
        // Expand in reverse order so remaining ranges stay valid. Textarea replacement
        // preserves the cursor and other elements, including images and mentions.
        let pending_pastes = std::mem::take(&mut self.draft.pending_pastes);
        if !pending_pastes.is_empty() {
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
                if matches!(command, SlashCommand::Warnings | SlashCommand::Config | SlashCommand::Quit | SlashCommand::Exit)
                    || mode == RestrictedInputMode::UnavailableThread && command.available_when_thread_unavailable())
            {
                return self
                    .try_dispatch_bare_slash_command()
                    .or_else(|| self.try_dispatch_slash_command_with_args())
                    .unwrap_or(InputResult::None);
            }
        }

        // Other commands and prompts stay in the draft while offline.
        // The basic editor reconciles attachments without invoking composer-level shortcuts.
        if !matches!(key.code, KeyCode::Enter | KeyCode::Tab)
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            self.edit_restricted_draft(key);
        }
        InputResult::None
    }

    fn edit_restricted_draft(&mut self, key: KeyEvent) {
        // Null is sent internally to clean up on disconnect or expand a paste.
        let before = if key.code == KeyCode::Null {
            None
        } else {
            self.draft.textarea_state.get_mut().follow_cursor();
            self.before_sparkle_editor_key(key)
        };
        let (result, _) = self.handle_input_basic(key);
        self.after_sparkle_key(before, &result);
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
