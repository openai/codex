//! Slash-command input parsing, cursor detection, and completion helpers.

use std::ops::Range;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use crate::bottom_pane::command_popup::CommandItem;
use crate::bottom_pane::command_popup::CommandPopup;
use crate::bottom_pane::command_popup::CommandPopupFlags;
use crate::bottom_pane::prompt_args::parse_slash_name;
use crate::bottom_pane::slash_commands::BuiltinCommandFlags;
use crate::bottom_pane::slash_commands::ServiceTierCommand;
use crate::bottom_pane::slash_commands::SlashCommandItem;
use crate::bottom_pane::slash_commands::find_slash_command;
use crate::bottom_pane::slash_commands::has_slash_command_prefix;
use crate::slash_command::SlashCommand;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;

use super::super::footer::esc_hint_mode;
use super::super::footer::reset_mode_after_activity;
use super::ActivePopup;
use super::ChatComposer;
use super::InputResult;
use super::QueuedInputAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SlashValidation {
    Immediate,
    Deferred,
}

pub(super) enum SubmissionValidation {
    Valid,
    UnknownCommand(String),
}

pub(super) struct InlineCommand<'a> {
    pub(super) command: SlashCommandItem,
    pub(super) rest: &'a str,
    pub(super) rest_offset: usize,
}

pub(super) struct SlashInput<'a> {
    enabled: bool,
    is_bash_mode: bool,
    command_flags: BuiltinCommandFlags,
    service_tier_commands: &'a [ServiceTierCommand],
}

impl<'a> SlashInput<'a> {
    pub(super) fn new(
        enabled: bool,
        is_bash_mode: bool,
        command_flags: BuiltinCommandFlags,
        service_tier_commands: &'a [ServiceTierCommand],
    ) -> Self {
        Self {
            enabled,
            is_bash_mode,
            command_flags,
            service_tier_commands,
        }
    }

    pub(super) fn validate_submission(
        &self,
        text: &str,
        input_starts_with_space: bool,
    ) -> SubmissionValidation {
        if !self.enabled {
            return SubmissionValidation::Valid;
        }
        let Some((name, _rest, _rest_offset)) = parse_slash_name(text) else {
            return SubmissionValidation::Valid;
        };
        if input_starts_with_space || name.contains('/') {
            return SubmissionValidation::Valid;
        }
        if self.command(name).is_some() {
            SubmissionValidation::Valid
        } else {
            SubmissionValidation::UnknownCommand(name.to_string())
        }
    }

    pub(super) fn bare_command(&self, text: &str) -> Option<SlashCommandItem> {
        if !self.enabled || self.is_bash_mode {
            return None;
        }
        let first_line = text.lines().next().unwrap_or("");
        let (name, rest, _rest_offset) = parse_slash_name(first_line)?;
        if !rest.is_empty() {
            return None;
        }
        let command = self.command(name)?;
        if command.supports_inline_args()
            && parse_slash_name(text).is_some_and(|(_, full_rest, _)| !full_rest.is_empty())
        {
            return None;
        }
        Some(command)
    }

    pub(super) fn inline_command<'text>(&self, text: &'text str) -> Option<InlineCommand<'text>> {
        if !self.enabled || self.is_bash_mode || text.starts_with(' ') {
            return None;
        }

        let (name, rest, rest_offset) = parse_slash_name(text)?;
        if rest.is_empty() || name.contains('/') {
            return None;
        }

        let command = self.command(name)?;
        command.supports_inline_args().then_some(InlineCommand {
            command,
            rest,
            rest_offset,
        })
    }

    pub(super) fn should_parse_on_dequeue(&self, text: &str) -> bool {
        self.enabled && !text.starts_with(' ') && text.trim().starts_with('/')
    }

    pub(super) fn command_element_range(
        &self,
        first_line: &str,
        cursor: usize,
    ) -> Option<Range<usize>> {
        if self.is_bash_mode {
            return None;
        }
        let (name, _rest, _rest_offset) = parse_slash_name(first_line)?;
        if name.contains('/') {
            return None;
        }
        let element_end = 1 + name.len();
        // A draft tail can make an in-progress prefix look complete ("/re" + "view").
        // Keep it editable until the cursor leaves the command name.
        if cursor <= first_line.len() && (1..element_end).contains(&cursor) {
            return None;
        }
        let has_space_after = first_line
            .get(element_end..)
            .and_then(|tail| tail.chars().next())
            .is_some_and(char::is_whitespace);
        if !has_space_after {
            return None;
        }
        self.command(name).is_some().then_some(0..element_end)
    }

    pub(super) fn is_editing_command_name(&self, first_line: &str, cursor: usize) -> bool {
        let Some((name, rest)) = command_under_cursor(first_line, cursor) else {
            return false;
        };
        if !self.enabled {
            return false;
        }
        if name.is_empty() {
            return rest.is_empty();
        }

        has_slash_command_prefix(name, self.command_flags, self.service_tier_commands)
    }

    pub(super) fn command_popup(&self, filter_text: &str) -> CommandPopup {
        let mut command_popup = CommandPopup::new(
            CommandPopupFlags {
                collaboration_modes_enabled: self.command_flags.collaboration_modes_enabled,
                connectors_enabled: self.command_flags.connectors_enabled,
                plugins_command_enabled: self.command_flags.plugins_command_enabled,
                token_activity_command_enabled: self.command_flags.token_activity_command_enabled,
                service_tier_commands_enabled: self.command_flags.service_tier_commands_enabled,
                goal_command_enabled: self.command_flags.goal_command_enabled,
                personality_command_enabled: self.command_flags.personality_command_enabled,
                realtime_conversation_enabled: self.command_flags.realtime_conversation_enabled,
                audio_device_selection_enabled: self.command_flags.audio_device_selection_enabled,
                windows_degraded_sandbox_active: self.command_flags.allow_elevate_sandbox,
                side_conversation_active: self.command_flags.side_conversation_active,
            },
            self.service_tier_commands.to_vec(),
        );
        command_popup.on_composer_text_change(filter_text.to_string());
        command_popup
    }

    fn command(&self, name: &str) -> Option<SlashCommandItem> {
        find_slash_command(name, self.command_flags, self.service_tier_commands)
    }
}

pub(super) fn queued_input_action(
    prepared_text: &str,
    defer_slash_validation: bool,
) -> QueuedInputAction {
    if defer_slash_validation && prepared_text.starts_with('/') {
        QueuedInputAction::ParseSlash
    } else if prepared_text.starts_with('!') {
        QueuedInputAction::RunShell
    } else {
        QueuedInputAction::Plain
    }
}

impl ChatComposer {
    /// Handle key event when the slash-command popup is visible.
    pub(super) fn handle_key_event_with_slash_popup(
        &mut self,
        key_event: KeyEvent,
    ) -> (InputResult, bool) {
        if self.handle_shortcut_overlay_key(&key_event) {
            return (InputResult::None, true);
        }
        if key_event.code == KeyCode::Esc {
            let next_mode = esc_hint_mode(self.footer.mode, self.is_task_running);
            if next_mode != self.footer.mode {
                self.footer.mode = next_mode;
                return (InputResult::None, true);
            }
        } else {
            self.footer.mode = reset_mode_after_activity(self.footer.mode);
        }
        let ActivePopup::Command(popup) = &mut self.popups.active else {
            unreachable!();
        };

        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Dismiss the slash popup; keep the current input untouched.
                self.popups.active = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                // Ensure popup filtering/selection reflects the latest composer text
                // before applying completion.
                let text = self.draft.textarea.text();
                let first_line = text.lines().next().unwrap_or("").to_owned();
                let cursor = self.draft.textarea.cursor();
                let filter_text = command_popup_filter_text(&first_line, cursor)
                    .unwrap_or_else(|| first_line.clone());
                popup.on_composer_text_change(filter_text);
                if let Some(selected_cmd) = popup.selected_item() {
                    if selected_command_dispatches_immediately_on_tab(&selected_cmd)
                        && let CommandItem::Builtin(cmd) = &selected_cmd
                    {
                        self.stage_selected_slash_command_history(&selected_cmd);
                        self.draft.textarea.set_text_clearing_elements("");
                        self.draft.is_bash_mode = false;
                        return (InputResult::Command(*cmd), true);
                    }

                    if self
                        .complete_selected_slash_command_preserving_existing_draft_tail_as_inline_args(
                            &selected_cmd,
                        )
                    {
                        return (InputResult::None, true);
                    }

                    if let Some(completed_text) =
                        selected_command_completion(&first_line, &selected_cmd)
                    {
                        self.draft
                            .textarea
                            .set_text_clearing_elements(&completed_text);
                        if !self.draft.textarea.text().is_empty() {
                            self.draft
                                .textarea
                                .set_cursor(self.draft.textarea.text().len());
                        }
                        return (InputResult::None, true);
                    }
                }
                if self.is_task_running {
                    return self.handle_submission(/*should_queue*/ true);
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Treat "/" as accepting the highlighted command as text completion
                // while the slash-command popup is active.
                let text = self.draft.textarea.text();
                let first_line = text.lines().next().unwrap_or("").to_owned();
                let cursor = self.draft.textarea.cursor();
                let filter_text = command_popup_filter_text(&first_line, cursor)
                    .unwrap_or_else(|| first_line.clone());
                popup.on_composer_text_change(filter_text);
                if let Some(selected_cmd) = popup.selected_item() {
                    if self
                        .complete_selected_slash_command_preserving_existing_draft_tail_as_inline_args(
                            &selected_cmd,
                        )
                    {
                        return (InputResult::None, true);
                    }

                    if let Some(completed_text) =
                        selected_command_completion(&first_line, &selected_cmd)
                    {
                        self.draft
                            .textarea
                            .set_text_clearing_elements(&completed_text);
                        self.draft.is_bash_mode = false;
                    }
                    if !self.draft.textarea.text().is_empty() {
                        self.draft
                            .textarea
                            .set_cursor(self.draft.textarea.text().len());
                    }
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(sel) = popup.selected_item() {
                    if self
                        .complete_selected_slash_command_preserving_existing_draft_tail_as_inline_args(
                            &sel,
                        )
                        && let Some(result) = self.try_dispatch_slash_command_with_args()
                    {
                        return (result, true);
                    }

                    self.stage_selected_slash_command_history(&sel);
                    self.draft.textarea.set_text_clearing_elements("");
                    self.draft.is_bash_mode = false;
                    return (
                        match sel {
                            CommandItem::Builtin(cmd) => InputResult::Command(cmd),
                            CommandItem::ServiceTier(command) => {
                                InputResult::ServiceTierCommand(command)
                            }
                        },
                        true,
                    );
                }
                // Fallback to default newline handling if no command selected.
                self.handle_key_event_without_popup(key_event)
            }
            input => self.handle_input_basic(input),
        }
    }

    fn complete_selected_slash_command_preserving_existing_draft_tail_as_inline_args(
        &mut self,
        selected_cmd: &CommandItem,
    ) -> bool {
        let CommandItem::Builtin(cmd) = selected_cmd else {
            return false;
        };
        if !cmd.supports_inline_args() {
            return false;
        }

        let text = self.draft.textarea.text();
        let first_line_end = text.find('\n').unwrap_or(text.len());
        let cursor = self.draft.textarea.cursor();
        if cursor > first_line_end || !text.starts_with('/') || !text.is_char_boundary(cursor) {
            return false;
        }

        let command_token_end = text[1..first_line_end]
            .find(char::is_whitespace)
            .map(|idx| 1 + idx)
            .unwrap_or(first_line_end);
        let typed_command_name = &text[1..command_token_end];
        let rest_after_token_is_empty = text[command_token_end..].trim().is_empty();
        if rest_after_token_is_empty && (cursor <= 1 || cursor >= command_token_end) {
            return false;
        }
        let replace_end =
            if cursor <= 1 || (typed_command_name == cmd.command() && rest_after_token_is_empty) {
                command_token_end
            } else {
                cursor
            };
        let tail = &text[replace_end..];
        let tail_starts_with_whitespace = tail.chars().next().is_some_and(char::is_whitespace);
        let selected_command_text = format!("/{}", cmd.command());
        let replacement = if tail_starts_with_whitespace {
            selected_command_text
        } else {
            format!("{selected_command_text} ")
        };

        let ranges_to_unmark = self
            .draft
            .textarea
            .text_elements()
            .into_iter()
            .filter_map(|element| {
                let range = element.byte_range.start..element.byte_range.end;
                (range.start < replace_end && replace_end < range.end).then_some(range)
            })
            .collect::<Vec<_>>();
        for range in ranges_to_unmark {
            self.draft.textarea.remove_element_range(range);
        }
        self.draft
            .textarea
            .replace_range(0..replace_end, &replacement);
        self.draft.is_bash_mode = false;
        self.draft
            .textarea
            .set_cursor(self.draft.textarea.text().len());
        true
    }

    /// Keep slash command elements aligned with the current first line.
    pub(super) fn sync_slash_command_elements(&mut self) {
        if !self.slash_commands_enabled() {
            return;
        }
        let text = self.draft.textarea.text();
        let first_line_end = text.find('\n').unwrap_or(text.len());
        let first_line = &text[..first_line_end];
        let cursor = self.draft.textarea.cursor();
        let desired_range = self.slash_input().command_element_range(first_line, cursor);
        // Slash commands are only valid at byte 0 of the first line.
        // Any slash-shaped element not matching the current desired prefix is stale.
        let mut has_desired = false;
        let mut stale_ranges = Vec::new();
        for elem in self.draft.textarea.text_elements() {
            let Some(payload) = elem.placeholder(text) else {
                continue;
            };
            if payload.strip_prefix('/').is_none() {
                continue;
            }
            let range = elem.byte_range.start..elem.byte_range.end;
            if desired_range.as_ref() == Some(&range) {
                has_desired = true;
            } else {
                stale_ranges.push(range);
            }
        }

        for range in stale_ranges {
            self.draft.textarea.remove_element_range(range);
        }

        if let Some(range) = desired_range
            && !has_desired
        {
            self.draft.textarea.add_element_range(range);
        }
    }
}

pub(super) fn selected_command_dispatches_immediately_on_tab(command: &CommandItem) -> bool {
    matches!(command, CommandItem::Builtin(SlashCommand::Skills))
}

pub(super) fn selected_command_completion(
    first_line: &str,
    command: &CommandItem,
) -> Option<String> {
    let selected_command_text = format!("/{}", command.command());
    (!first_line.trim_start().starts_with(&selected_command_text))
        .then(|| format!("{selected_command_text} "))
}

pub(super) fn prepared_args(prepared_text: &str) -> Option<(&str, usize)> {
    let (_, prepared_rest, prepared_rest_offset) = parse_slash_name(prepared_text)?;
    Some((prepared_rest, prepared_rest_offset))
}

/// Translate full-text element ranges into command-argument ranges.
///
/// `rest_offset` is the byte offset where `rest` begins in the full text.
pub(super) fn args_elements(
    rest: &str,
    rest_offset: usize,
    text_elements: &[TextElement],
) -> Vec<TextElement> {
    if rest.is_empty() || text_elements.is_empty() {
        return Vec::new();
    }
    text_elements
        .iter()
        .filter_map(|elem| {
            if elem.byte_range.end <= rest_offset {
                return None;
            }
            let start = elem.byte_range.start.saturating_sub(rest_offset);
            let mut end = elem.byte_range.end.saturating_sub(rest_offset);
            if start >= rest.len() {
                return None;
            }
            end = end.min(rest.len());
            (start < end).then_some(elem.map_range(|_| ByteRange { start, end }))
        })
        .collect()
}

pub(super) fn command_popup_filter_text(first_line: &str, cursor: usize) -> Option<String> {
    let (name, _rest) = command_under_cursor(first_line, cursor)?;
    Some(format!("/{name}"))
}

/// If the cursor is currently within a slash command on the first line,
/// extract the command fragment before the cursor and the rest of the line after it.
fn command_under_cursor(first_line: &str, cursor: usize) -> Option<(&str, &str)> {
    if !first_line.starts_with('/') {
        return None;
    }
    if cursor > first_line.len() || !first_line.is_char_boundary(cursor) {
        return None;
    }

    let name_start = 1usize;
    let name_end = first_line[name_start..]
        .find(char::is_whitespace)
        .map(|idx| name_start + idx)
        .unwrap_or_else(|| first_line.len());

    let cursor = if cursor <= name_start {
        name_end
    } else {
        cursor
    };
    if cursor > name_end {
        return None;
    }

    let name = &first_line[name_start..cursor];
    let rest = &first_line[cursor..];

    Some((name, rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::bottom_pane::AppEventSender;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_composer() -> ChatComposer {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        ChatComposer::new(
            /*has_input_focus*/ true,
            AppEventSender::new(tx),
            /*enhanced_keys_supported*/ false,
            "Ask Codex to do anything".to_string(),
            /*disable_paste_burst*/ false,
        )
    }

    fn press(composer: &mut ChatComposer, code: KeyCode) -> InputResult {
        composer
            .handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
            .0
    }

    fn composer_with_text_at_cursor(text: &str, cursor: usize) -> ChatComposer {
        let mut composer = test_composer();
        composer.draft.textarea.set_text_clearing_elements(text);
        composer.draft.textarea.set_cursor(cursor);
        composer.sync_popups();
        composer
    }

    fn composer_with_draft_tail(prefix: &str, draft: &str) -> ChatComposer {
        composer_with_text_at_cursor(&format!("{prefix}{draft}"), prefix.len())
    }

    #[test]
    fn slash_completion_preserves_existing_draft_tail_for_inline_arg_commands() {
        let draft = "view the diff";
        let expected_text = "/review view the diff";

        let mut composer = composer_with_draft_tail("/re", draft);
        assert_eq!(press(&mut composer, KeyCode::Tab), InputResult::None);
        assert_eq!(composer.draft.textarea.text(), expected_text);
        assert_eq!(composer.draft.textarea.cursor(), expected_text.len());

        let mut composer = composer_with_draft_tail("/re", draft);
        assert_eq!(
            press(&mut composer, KeyCode::Enter),
            InputResult::CommandWithArgs(SlashCommand::Review, draft.to_string(), Vec::new())
        );
        assert_eq!(composer.draft.textarea.text(), expected_text);
    }

    #[test]
    fn slash_completion_does_not_preserve_existing_draft_tail_for_other_commands() {
        let mut composer = composer_with_draft_tail(
            "/mo",
            "preserve this draft only for opted-in slash commands",
        );

        assert_eq!(press(&mut composer, KeyCode::Tab), InputResult::None);
        assert_eq!(composer.draft.textarea.text(), "/model ");
        assert_eq!(composer.draft.textarea.cursor(), "/model ".len());
    }

    #[test]
    fn slash_completion_does_not_turn_command_suffix_into_args() {
        let mut composer = composer_with_text_at_cursor("/review", "/re".len());
        assert_eq!(press(&mut composer, KeyCode::Tab), InputResult::None);
        assert_eq!(composer.draft.textarea.text(), "/review ");

        let mut composer = composer_with_text_at_cursor("/review", "/re".len());
        assert_eq!(
            press(&mut composer, KeyCode::Enter),
            InputResult::Command(SlashCommand::Review)
        );
        assert!(composer.draft.textarea.is_empty());
    }
}
