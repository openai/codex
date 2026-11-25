//! Restricted editing retains drafts and limits slash dispatch to local commands.
//! A reconnect can replace the composer. Preserve the slash selection with the draft.
//! Unavailable threads also allow recovery and other local commands.
//! Connected unavailable threads retain the empty-prompt agents shortcut.
//! Paste Enter handling is shared with normal submission so buffered newlines survive both paths.
//! In the draft-only handoff, a recent paste prefix can preserve a raw Enter as a newline.
//! In an ordinary draft, a lone IME character does not turn Enter into a newline.
//! Recovery commands must occupy one line and be visible before the submit key expands pastes.
//! Offline draft edits also consume the Astra sparkle opportunity before rendering.

use super::*;
use crate::bottom_pane::BottomPane;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DisconnectedCommandState {
    cursor: usize,
    selected: Option<CommandItem>,
    dismissed: Option<String>,
}

impl BottomPane {
    pub(crate) fn disconnected_command_state(&self) -> Option<DisconnectedCommandState> {
        let composer = &self.composer;
        if composer.popups.restricted_input_mode != Some(RestrictedInputMode::Disconnected)
            || composer.draft.is_bash_mode
            || !composer.draft.textarea.text().starts_with('/')
        {
            return None;
        }
        Some(DisconnectedCommandState {
            cursor: composer.logical_cursor(),
            selected: match &composer.popups.active {
                ActivePopup::Command(popup) => popup.current_item(),
                _ => None,
            },
            dismissed: composer.popups.dismissed_command_token.clone(),
        })
    }

    pub(crate) fn restore_disconnected_command(&mut self, state: DisconnectedCommandState) {
        self.composer.popups.active = ActivePopup::None;
        self.composer.popups.dismissed_command_token = state.dismissed;
        self.set_composer_cursor(state.cursor);
        if let ActivePopup::Command(popup) = &mut self.composer.popups.active {
            // An absent selection matters too, e.g. when the cursor split the name offline.
            popup.restore_selection(state.selected);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestrictedInputMode {
    Disconnected,
    UnavailableThread,
    DraftHandoff,
}

impl RestrictedInputMode {
    fn allows_command(self, command: SlashCommand) -> bool {
        match self {
            Self::Disconnected => command.available_when_disconnected(),
            Self::UnavailableThread => {
                command == SlashCommand::Config || command.available_when_thread_unavailable()
            }
            Self::DraftHandoff => false,
        }
    }
}

impl ChatComposer {
    pub(crate) fn accepts_reconnect_shortcut(&self) -> bool {
        !self.is_vim_enabled() || self.draft.textarea.uses_vim_insert_cursor()
    }

    pub(crate) fn set_restricted_input_mode(&mut self, mode: Option<RestrictedInputMode>) {
        let was_disconnected =
            self.popups.restricted_input_mode == Some(RestrictedInputMode::Disconnected);
        self.popups.restricted_input_mode = mode;
        let disconnected = mode == Some(RestrictedInputMode::Disconnected);
        if was_disconnected != disconnected
            && let ActivePopup::Command(popup) = &mut self.popups.active
        {
            popup.set_disconnected(disconnected);
            self.sync_restricted_slash_popup();
        }
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
        !in_slash_context && self.append_recent_paste_separator('\n', now)
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
                self.handle_paste_burst_flush(now);
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
                        && self.append_recent_paste_separator('\n', now))
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
        if !matches!(
            self.popups.active,
            ActivePopup::Command(_) | ActivePopup::Argument(_)
        ) {
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
        if self.draft.textarea.wants_vim_search_key(key)
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
            self.sync_restricted_slash_popup();
            return result;
        }
        if pending_pastes.is_empty()
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && let ActivePopup::Command(popup) = &self.popups.active
        {
            let submit = self.submit_keys.is_pressed(key);
            if (submit || key.code == KeyCode::Tab)
                && let Some(selected) = popup.selected_item()
            {
                let text = self.draft.textarea.text();
                if !matches!(selected, CommandItem::Builtin(command)
                    if mode.allows_command(command)
                        && text.trim().lines().count() == 1
                        && (command.supports_inline_args()
                            || text.split_once(char::is_whitespace).is_none_or(|(_, args)| args.trim().is_empty())))
                {
                    return InputResult::None;
                }
                if submit
                    && let CommandItem::Builtin(command) = selected
                    && self.reject_slash_command_if_unavailable(&SlashCommandItem::Builtin(command))
                {
                    return InputResult::None;
                }
                let popup_key = if submit { KeyCode::Enter.into() } else { key };
                let result = self
                    .handle_key_event_with_slash_popup(
                        popup_key,
                        slash_input::SlashPopupInputMode::Restricted,
                    )
                    .0;
                self.reset_vim_mode_after_successful_dispatch(&result);
                self.sync_restricted_slash_popup();
                return result;
            }
            if submit && popup.blocks_submit_fallback() {
                return InputResult::None;
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
                        slash_input::SlashPopupInputMode::Restricted,
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
            // A partial name under the cursor hides the popup offline. Don't
            // dispatch the full draft in place of the previously highlighted row.
            if mode == RestrictedInputMode::Disconnected
                && let Some((_, tail)) =
                    slash_input::command_under_cursor(text, self.logical_cursor())
                && tail.starts_with(|ch: char| !ch.is_whitespace())
            {
                return InputResult::None;
            }
            let command = input
                .bare_command(text)
                .or_else(|| input.inline_command(text).map(|command| command.command))
                .filter(|_| text.trim().lines().count() == 1);
            if matches!(command, Some(SlashCommandItem::Builtin(command)) if mode.allows_command(command))
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
            self.sync_restricted_slash_popup();
        }
        InputResult::None
    }

    fn sync_restricted_slash_popup(&mut self) {
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
