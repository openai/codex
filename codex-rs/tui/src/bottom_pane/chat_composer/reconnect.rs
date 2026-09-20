//! Restricted editing retains drafts and allows the local warnings viewer.
//! Unavailable threads also allow recovery and other local commands.
//! Paste Enter handling is shared with normal submission so buffered newlines survive both paths.
//! Recovery commands must occupy one line and be visible before the submit key expands pastes.
//! Offline draft edits also consume the Astra sparkle opportunity before rendering.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestrictedInputMode {
    Disconnected,
    UnavailableThread,
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
        if !self.draft.disable_paste_burst
            && self.draft.paste_burst.is_active()
            && !in_slash_context
            && self
                .draft
                .paste_burst
                .append_control_char_if_active('\n', now)
        {
            return true;
        }
        if !in_slash_context
            && !self.draft.disable_paste_burst
            && self
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
                if command == SlashCommand::Warnings
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
            // Null is sent internally to clean up on disconnect or expand a paste; it isn't an edit.
            let before = if key.code == KeyCode::Null {
                None
            } else {
                self.before_sparkle_editor_key(key)
            };
            let (result, _) = self.handle_input_basic(key);
            self.after_sparkle_key(before, &result);
        }
        InputResult::None
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
