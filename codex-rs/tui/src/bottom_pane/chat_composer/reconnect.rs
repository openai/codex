//! Offline editing and event-channel rebinding retain the draft in place.
//! Paste Enter handling is shared with normal submission so buffered newlines survive both paths.

use super::*;

impl ChatComposer {
    /// Rebind retained editors after the app replaces its event channel.
    pub(crate) fn set_app_event_sender(&mut self, sender: AppEventSender) {
        self.app_event_tx = sender;
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
        if !self.draft.disable_paste_burst
            && self.draft.paste_burst.is_active()
            && !in_slash_context
            && self.draft.paste_burst.append_newline_if_active(now)
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

    pub(crate) fn handle_disconnected_key(&mut self, key: KeyEvent) {
        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && !matches!(key.code, KeyCode::Null)
        {
            self.interact_with_astra_sparkle();
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
        // Enter/Tab and configured submit bindings must never consume the draft offline.
        // The basic editor reconciles attachments without invoking composer-level shortcuts.
        if !matches!(key.code, KeyCode::Enter | KeyCode::Tab)
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            self.handle_input_basic(key);
        }
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
