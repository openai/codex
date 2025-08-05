use codex_core::protocol::TokenUsage;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::WidgetRef;

use super::chat_composer_history::ChatComposerHistory;
use super::command_popup::CommandPopup;
use super::file_search_popup::FileSearchPopup;
use super::selection_popup::SelectionKind;
use super::selection_popup::SelectionPopup;
use super::selection_popup::SelectionValue;
use crate::command_utils::parse_execution_mode_token;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;
use crate::slash_command::ParsedSlash;
use crate::slash_command::SlashCommand;
use crate::slash_command::parse_slash_line;
use codex_file_search::FileMatch;
use std::cell::RefCell;

const BASE_PLACEHOLDER_TEXT: &str = "...";
/// If the pasted content exceeds this number of characters, replace it with a
/// placeholder in the UI.
const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

/// Result returned when the user interacts with the text area.
pub enum InputResult {
    Submitted(String),
    None,
}

struct TokenUsageInfo {
    token_usage: TokenUsage,
    model_context_window: Option<u64>,
}

pub(crate) struct ChatComposer {
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    active_popup: ActivePopup,
    app_event_tx: AppEventSender,
    history: ChatComposerHistory,
    ctrl_c_quit_hint: bool,
    use_shift_enter_hint: bool,
    dismissed: Dismissed,
    current_file_query: Option<String>,
    pending_pastes: Vec<(String, String)>,
    token_usage_info: Option<TokenUsageInfo>,
    has_focus: bool,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
    Selection(SelectionPopup),
}

/// Tracks tokens for which the user explicitly dismissed a popup to avoid
/// reopening it immediately unless the input changes meaningfully.
struct Dismissed {
    slash: Option<String>,
    file: Option<String>,
}

impl ChatComposer {
    #[inline]
    fn first_line(&self) -> &str {
        self.textarea.text().lines().next().unwrap_or("")
    }

    #[inline]
    fn sync_popups(&mut self) {
        self.sync_command_popup();
        if matches!(self.active_popup, ActivePopup::Command(_)) {
            self.dismissed.file = None;
        } else {
            self.sync_file_search_popup();
        }
    }
    pub fn new(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
    ) -> Self {
        let use_shift_enter_hint = enhanced_keys_supported;

        Self {
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            active_popup: ActivePopup::None,
            app_event_tx,
            history: ChatComposerHistory::new(),
            ctrl_c_quit_hint: false,
            use_shift_enter_hint,
            dismissed: Dismissed {
                slash: None,
                file: None,
            },
            current_file_query: None,
            pending_pastes: Vec::new(),
            token_usage_info: None,
            has_focus: has_input_focus,
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.textarea.desired_height(width - 1)
            + match &self.active_popup {
                ActivePopup::None => 1u16,
                ActivePopup::Command(c) => c.calculate_required_height(),
                ActivePopup::File(c) => c.calculate_required_height(),
                ActivePopup::Selection(c) => c.calculate_required_height(),
            }
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let popup_height = match &self.active_popup {
            ActivePopup::Command(popup) => popup.calculate_required_height(),
            ActivePopup::File(popup) => popup.calculate_required_height(),
            ActivePopup::Selection(popup) => popup.calculate_required_height(),
            ActivePopup::None => 1,
        };
        let [textarea_rect, _] =
            Layout::vertical([Constraint::Min(0), Constraint::Max(popup_height)]).areas(area);
        let mut textarea_rect = textarea_rect;
        textarea_rect.width = textarea_rect.width.saturating_sub(1);
        textarea_rect.x += 1;
        let state = self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, &state)
    }

    /// Returns true if the composer currently contains no user input.
    pub(crate) fn is_empty(&self) -> bool {
        self.textarea.is_empty()
    }

    /// Update the cached *context-left* percentage and refresh the placeholder
    /// text. The UI relies on the placeholder to convey the remaining
    /// context when the composer is empty.
    pub(crate) fn set_token_usage(
        &mut self,
        token_usage: TokenUsage,
        model_context_window: Option<u64>,
    ) {
        self.token_usage_info = Some(TokenUsageInfo {
            token_usage,
            model_context_window,
        });
    }

    /// Record the history metadata advertised by `SessionConfiguredEvent` so
    /// that the composer can navigate cross-session history.
    pub(crate) fn set_history_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.history.set_metadata(log_id, entry_count);
    }

    /// Integrate an asynchronous response to an on-demand history lookup. If
    /// the entry is present and the offset matches the current cursor we
    /// immediately populate the textarea.
    pub(crate) fn on_history_entry_response(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
    ) -> bool {
        let Some(text) = self.history.on_entry_response(log_id, offset, entry) else {
            return false;
        };
        self.textarea.set_text(&text);
        self.textarea.set_cursor(0);
        true
    }

    pub fn handle_paste(&mut self, pasted: String) -> bool {
        let char_count = pasted.chars().count();
        if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = format!("[Pasted Content {char_count} chars]");
            self.textarea.insert_str(&placeholder);
            self.pending_pastes.push((placeholder, pasted));
        } else {
            self.textarea.insert_str(&pasted);
        }
        self.sync_popups();
        true
    }

    /// Integrate results from an asynchronous file search.
    pub(crate) fn on_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        // Only apply if user is still editing a token starting with `query`.
        let current_opt = Self::current_at_token(&self.textarea);
        let Some(current_token) = current_opt else {
            return;
        };

        if !current_token.starts_with(&query) {
            return;
        }

        if let ActivePopup::File(popup) = &mut self.active_popup {
            popup.set_matches(&query, matches);
        }
    }

    pub fn set_ctrl_c_quit_hint(&mut self, show: bool, has_focus: bool) {
        self.ctrl_c_quit_hint = show;
        self.set_has_focus(has_focus);
    }

    /// Open or update the model-selection popup with the provided options.
    pub(crate) fn open_model_selector(&mut self, current_model: &str, options: Vec<String>) {
        match &mut self.active_popup {
            ActivePopup::Selection(popup) if popup.kind() == SelectionKind::Model => {
                *popup = SelectionPopup::new_model(current_model, options);
            }
            _ => {
                self.active_popup =
                    ActivePopup::Selection(SelectionPopup::new_model(current_model, options));
            }
        }
        let first_line_owned = self.first_line().to_string();
        if let ParsedSlash::Command { cmd, args } = parse_slash_line(&first_line_owned) {
            if cmd == SlashCommand::Model {
                if let ActivePopup::Selection(popup) = &mut self.active_popup {
                    popup.set_query(args);
                }
            }
        }
    }

    /// Open or update the execution-mode selection popup with the provided options.
    pub(crate) fn open_execution_selector(
        &mut self,
        current_approval: codex_core::protocol::AskForApproval,
        current_sandbox: &codex_core::protocol::SandboxPolicy,
    ) {
        match &mut self.active_popup {
            ActivePopup::Selection(popup) if popup.kind() == SelectionKind::Execution => {
                *popup = SelectionPopup::new_execution_modes(current_approval, current_sandbox);
            }
            _ => {
                self.active_popup = ActivePopup::Selection(SelectionPopup::new_execution_modes(
                    current_approval,
                    current_sandbox,
                ));
            }
        }
        let first_line_owned = self.first_line().to_string();
        if let ParsedSlash::Command { cmd, args } = parse_slash_line(&first_line_owned) {
            if cmd == SlashCommand::Approvals {
                if let ActivePopup::Selection(popup) = &mut self.active_popup {
                    popup.set_query(args);
                }
            }
        }
    }

    /// Handle a key event coming from the main UI.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let result = match &mut self.active_popup {
            ActivePopup::Command(_) => self.handle_key_event_with_slash_popup(key_event),
            ActivePopup::File(_) => self.handle_key_event_with_file_popup(key_event),
            ActivePopup::Selection(_) => self.handle_key_event_with_selection_popup(key_event),
            ActivePopup::None => self.handle_key_event_without_popup(key_event),
        };

        // Update (or hide/show) popup after processing the key.
        match &self.active_popup {
            ActivePopup::Selection(_) => {
                self.sync_selection_popup();
            }
            ActivePopup::Command(_) => {
                self.sync_command_popup();
                // When slash popup active, suppress file popup.
                self.dismissed.file = None;
            }
            _ => {
                self.sync_command_popup();
                if !matches!(self.active_popup, ActivePopup::Command(_)) {
                    self.sync_file_search_popup();
                }
            }
        }

        result
    }

    /// Handle key event when the slash-command popup is visible.
    fn handle_key_event_with_slash_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let first_line_owned = self.first_line().to_string();
        let ActivePopup::Command(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Remember the dismissed token to avoid immediate reopen until input changes.
                let token = match parse_slash_line(&first_line_owned) {
                    ParsedSlash::Command { cmd, .. } => Some(cmd.command().to_string()),
                    ParsedSlash::Incomplete { token } => Some(token.to_string()),
                    ParsedSlash::None => None,
                };
                if let Some(tok) = token {
                    self.dismissed.slash = Some(tok);
                }
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                if let Some(cmd) = popup.selected_command() {
                    let first_line = self.textarea.text().lines().next().unwrap_or("");
                    let starts_with_cmd = first_line
                        .trim_start()
                        .starts_with(&format!("/{}", cmd.command()));

                    if !starts_with_cmd {
                        self.textarea.set_text(&format!("/{} ", cmd.command()));
                    }
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(cmd) = popup.selected_command() {
                    let args_opt = match parse_slash_line(&first_line_owned) {
                        ParsedSlash::Command {
                            cmd: parsed_cmd,
                            args,
                        } if parsed_cmd == *cmd => {
                            let a = args.trim().to_string();
                            if a.is_empty() { None } else { Some(a) }
                        }
                        _ => None,
                    };

                    self.app_event_tx.send(AppEvent::DispatchCommand {
                        cmd: *cmd,
                        args: args_opt,
                    });
                    self.textarea.set_text("");
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                }
                let invalid_token = match parse_slash_line(&first_line_owned) {
                    ParsedSlash::Command { cmd, .. } => cmd.command().to_string(),
                    ParsedSlash::Incomplete { token } => token.to_string(),
                    ParsedSlash::None => String::new(),
                };
                self.dismissed.slash = Some(invalid_token.clone());
                self.active_popup = ActivePopup::None;
                {
                    use crate::history_cell::HistoryCell;
                    let message = if invalid_token.is_empty() {
                        "Invalid command".to_string()
                    } else {
                        format!("Invalid command: /{invalid_token}")
                    };
                    let lines = HistoryCell::new_error_event(message).plain_lines();
                    self.app_event_tx.send(AppEvent::InsertHistory(lines));
                }

                (InputResult::None, true)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle key events when file search popup is visible.
    fn handle_key_event_with_file_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let _first_line_owned = self.first_line().to_string();
        let ActivePopup::File(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                if let Some(tok) = Self::current_at_token(&self.textarea) {
                    self.dismissed.file = Some(tok.to_string());
                }
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(sel) = popup.selected_match() {
                    let sel_path = sel.to_string();
                    self.insert_selected_path(&sel_path);
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                }
                (InputResult::None, false)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle key events when model selection popup is visible.
    fn handle_key_event_with_selection_popup(
        &mut self,
        key_event: KeyEvent,
    ) -> (InputResult, bool) {
        let first_line_owned = self.first_line().to_string();
        let ActivePopup::Selection(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                if let Some(value) = popup.selected_value() {
                    match value {
                        SelectionValue::Model(m) => {
                            self.app_event_tx.send(AppEvent::SelectModel(m))
                        }
                        SelectionValue::Execution { approval, sandbox } => self
                            .app_event_tx
                            .send(AppEvent::SelectExecutionMode { approval, sandbox }),
                    }
                    // Clear composer input and close the popup.
                    self.textarea.set_text("");
                    self.pending_pastes.clear();
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                }
                // No selection in the list: attempt to parse typed arguments for the appropriate kind.
                if let ParsedSlash::Command { cmd, args } = parse_slash_line(&first_line_owned) {
                    let args = args.trim().to_string();
                    if !args.is_empty() {
                        match popup.kind() {
                            SelectionKind::Model if cmd == SlashCommand::Model => {
                                self.app_event_tx.send(AppEvent::DispatchCommand {
                                    cmd: SlashCommand::Model,
                                    args: Some(args),
                                });
                                self.textarea.set_text("");
                                self.pending_pastes.clear();
                                self.active_popup = ActivePopup::None;
                                return (InputResult::None, true);
                            }
                            SelectionKind::Execution if cmd == SlashCommand::Approvals => {
                                if let Some((approval, sandbox)) = parse_execution_mode_token(&args)
                                {
                                    self.app_event_tx
                                        .send(AppEvent::SelectExecutionMode { approval, sandbox });
                                    self.textarea.set_text("");
                                    self.pending_pastes.clear();
                                    self.active_popup = ActivePopup::None;
                                    return (InputResult::None, true);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                (InputResult::None, false)
            }
            input => self.handle_input_basic(input),
        }
    }

    // Approval-specific handler removed; unified selection handler is used.

    /// Extract the `@token` that the cursor is currently positioned on, if any.
    ///
    /// The returned string **does not** include the leading `@`.
    ///
    /// Behavior:
    /// - The cursor may be anywhere *inside* the token (including on the
    ///   leading `@`). It does **not** need to be at the end of the line.
    /// - A token is delimited by ASCII whitespace (space, tab, newline).
    /// - If the token under the cursor starts with `@` and contains at least
    ///   one additional character, that token (without `@`) is returned.
    fn current_at_token(textarea: &TextArea) -> Option<String> {
        let cursor_offset = textarea.cursor();
        let text = textarea.text();

        // Adjust the provided byte offset to the nearest valid char boundary at or before it.
        let mut safe_cursor = cursor_offset.min(text.len());
        // If we're not on a char boundary, move back to the start of the current char.
        if safe_cursor < text.len() && !text.is_char_boundary(safe_cursor) {
            // Find the last valid boundary <= cursor_offset.
            safe_cursor = text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= cursor_offset)
                .last()
                .unwrap_or(0);
        }

        // Split the line around the (now safe) cursor position.
        let before_cursor = &text[..safe_cursor];
        let after_cursor = &text[safe_cursor..];

        // Detect whether we're on whitespace at the cursor boundary.
        let at_whitespace = if safe_cursor < text.len() {
            text[safe_cursor..]
                .chars()
                .next()
                .map(|c| c.is_whitespace())
                .unwrap_or(false)
        } else {
            false
        };

        // Left candidate: token containing the cursor position.
        let start_left = before_cursor
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);
        let end_left_rel = after_cursor
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(after_cursor.len());
        let end_left = safe_cursor + end_left_rel;
        let token_left = if start_left < end_left {
            Some(&text[start_left..end_left])
        } else {
            None
        };

        // Right candidate: token immediately after any whitespace from the cursor.
        let ws_len_right: usize = after_cursor
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| c.len_utf8())
            .sum();
        let start_right = safe_cursor + ws_len_right;
        let end_right_rel = text[start_right..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(text.len() - start_right);
        let end_right = start_right + end_right_rel;
        let token_right = if start_right < end_right {
            Some(&text[start_right..end_right])
        } else {
            None
        };

        let left_at = token_left
            .filter(|t| t.starts_with('@') && t.len() > 1)
            .map(|t| t[1..].to_string());
        let right_at = token_right
            .filter(|t| t.starts_with('@') && t.len() > 1)
            .map(|t| t[1..].to_string());

        if at_whitespace {
            return right_at.or(left_at);
        }
        if after_cursor.starts_with('@') {
            return right_at.or(left_at);
        }
        left_at.or(right_at)
    }

    /// Replace the active `@token` (the one under the cursor) with `path`.
    ///
    /// The algorithm mirrors `current_at_token` so replacement works no matter
    /// where the cursor is within the token and regardless of how many
    /// `@tokens` exist in the line.
    fn insert_selected_path(&mut self, path: &str) {
        let cursor_offset = self.textarea.cursor();
        let text = self.textarea.text();

        let before_cursor = &text[..cursor_offset];
        let after_cursor = &text[cursor_offset..];

        // Determine token boundaries.
        let start_idx = before_cursor
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);

        let end_rel_idx = after_cursor
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(after_cursor.len());
        let end_idx = cursor_offset + end_rel_idx;

        let mut new_text =
            String::with_capacity(text.len() - (end_idx - start_idx) + path.len() + 1);
        new_text.push_str(&text[..start_idx]);
        new_text.push_str(path);
        new_text.push(' ');
        new_text.push_str(&text[end_idx..]);

        self.textarea.set_text(&new_text);
    }

    /// Handle key event when no popup is visible.
    fn handle_key_event_without_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up | KeyCode::Down,
                ..
            } => {
                if self
                    .history
                    .should_handle_navigation(self.textarea.text(), self.textarea.cursor())
                {
                    let replace_text = match key_event.code {
                        KeyCode::Up => self.history.navigate_up(&self.app_event_tx),
                        KeyCode::Down => self.history.navigate_down(&self.app_event_tx),
                        _ => unreachable!(),
                    };
                    if let Some(text) = replace_text {
                        self.textarea.set_text(&text);
                        self.textarea.set_cursor(0);
                        return (InputResult::None, true);
                    }
                }
                self.handle_input_basic(key_event)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let mut text = self.textarea.text().to_string();
                self.textarea.set_text("");

                for (placeholder, actual) in &self.pending_pastes {
                    if text.contains(placeholder) {
                        text = text.replace(placeholder, actual);
                    }
                }
                self.pending_pastes.clear();

                if text.is_empty() {
                    (InputResult::None, true)
                } else {
                    self.history.record_local_submission(&text);
                    (InputResult::Submitted(text), true)
                }
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle generic Input events that modify the textarea content.
    fn handle_input_basic(&mut self, input: KeyEvent) -> (InputResult, bool) {
        if let KeyEvent {
            code: KeyCode::Backspace,
            ..
        } = input
        {
            if self.try_remove_placeholder_at_cursor() {
                return (InputResult::None, true);
            }
        }
        self.textarea.input(input);
        let text_after = self.textarea.text();
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));

        (InputResult::None, true)
    }

    /// Attempts to remove a placeholder if the cursor is at the end of one.
    /// Returns true if a placeholder was removed.
    fn try_remove_placeholder_at_cursor(&mut self) -> bool {
        let p = self.textarea.cursor();
        let text = self.textarea.text();

        // Find any placeholder that ends at the cursor position
        let placeholder_to_remove = self.pending_pastes.iter().find_map(|(ph, _)| {
            if p < ph.len() {
                return None;
            }
            let potential_ph_start = p - ph.len();
            if text[potential_ph_start..p] == *ph {
                Some(ph.clone())
            } else {
                None
            }
        });

        if let Some(placeholder) = placeholder_to_remove {
            self.textarea.replace_range(p - placeholder.len()..p, "");
            self.pending_pastes.retain(|(ph, _)| ph != &placeholder);
            true
        } else {
            false
        }
    }

    fn sync_command_popup(&mut self) {
        let first_line = self.textarea.text().lines().next().unwrap_or("");
        let input_starts_with_slash = first_line.starts_with('/');
        if !input_starts_with_slash {
            self.dismissed.slash = None;
        }
        let current_cmd_token: Option<String> = match parse_slash_line(first_line) {
            ParsedSlash::Command { cmd, .. } => Some(cmd.command().to_string()),
            ParsedSlash::Incomplete { token } => Some(token.to_string()),
            ParsedSlash::None => None,
        };

        match &mut self.active_popup {
            ActivePopup::Command(popup) => {
                if input_starts_with_slash {
                    popup.on_composer_text_change(first_line.to_string());
                } else {
                    self.active_popup = ActivePopup::None;
                    self.dismissed.slash = None;
                }
            }
            _ => {
                if input_starts_with_slash {
                    if self
                        .dismissed
                        .slash
                        .as_ref()
                        .is_some_and(|d| Some(d) == current_cmd_token.as_ref())
                    {
                        return;
                    }
                    let mut command_popup = CommandPopup::new();
                    command_popup.on_composer_text_change(first_line.to_string());
                    self.active_popup = ActivePopup::Command(command_popup);
                }
            }
        }
    }

    fn sync_file_search_popup(&mut self) {
        let query = match Self::current_at_token(&self.textarea) {
            Some(token) => token,
            None => {
                self.active_popup = ActivePopup::None;
                self.dismissed.file = None;
                return;
            }
        };
        if self.dismissed.file.as_ref() == Some(&query) {
            return;
        }

        self.app_event_tx
            .send(AppEvent::StartFileSearch(query.clone()));

        match &mut self.active_popup {
            ActivePopup::File(popup) => {
                popup.set_query(&query);
            }
            _ => {
                let mut popup = FileSearchPopup::new();
                popup.set_query(&query);
                self.active_popup = ActivePopup::File(popup);
            }
        }

        self.current_file_query = Some(query);
        self.dismissed.file = None;
    }

    fn sync_selection_popup(&mut self) {
        let first_line_owned = self.first_line().to_string();
        match (&mut self.active_popup, parse_slash_line(&first_line_owned)) {
            (ActivePopup::Selection(popup), ParsedSlash::Command { cmd, args }) => match popup
                .kind()
            {
                SelectionKind::Model if cmd == SlashCommand::Model => popup.set_query(args),
                SelectionKind::Execution if cmd == SlashCommand::Approvals => popup.set_query(args),
                _ => {
                    popup.set_query(first_line_owned.trim());
                }
            },
            (ActivePopup::Selection(popup), _no_slash_cmd) => {
                popup.set_query(first_line_owned.trim());
            }
            _ => {}
        }
    }

    fn set_has_focus(&mut self, has_focus: bool) {
        self.has_focus = has_focus;
    }
}

impl WidgetRef for &ChatComposer {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let popup_height = match &self.active_popup {
            ActivePopup::Command(popup) => popup.calculate_required_height(),
            ActivePopup::File(popup) => popup.calculate_required_height(),
            ActivePopup::Selection(popup) => popup.calculate_required_height(),
            ActivePopup::None => 1,
        };
        let [textarea_rect, popup_rect] =
            Layout::vertical([Constraint::Min(0), Constraint::Max(popup_height)]).areas(area);
        match &self.active_popup {
            ActivePopup::Command(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::File(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::Selection(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::None => {
                let bottom_line_rect = popup_rect;
                let key_hint_style = Style::default().fg(Color::Cyan);
                let hint = if self.ctrl_c_quit_hint {
                    vec![
                        Span::from(" "),
                        "Ctrl+C again".set_style(key_hint_style),
                        Span::from(" to quit"),
                    ]
                } else {
                    let newline_hint_key = if self.use_shift_enter_hint {
                        "Shift+⏎"
                    } else {
                        "Ctrl+J"
                    };
                    vec![
                        Span::from(" "),
                        "⏎".set_style(key_hint_style),
                        Span::from(" send   "),
                        newline_hint_key.set_style(key_hint_style),
                        Span::from(" newline   "),
                        "Ctrl+C".set_style(key_hint_style),
                        Span::from(" quit"),
                    ]
                };
                Line::from(hint)
                    .style(Style::default().dim())
                    .render_ref(bottom_line_rect, buf);
            }
        }
        Block::default()
            .border_style(Style::default().dim())
            .borders(Borders::LEFT)
            .border_type(BorderType::QuadrantOutside)
            .border_style(Style::default().fg(if self.has_focus {
                Color::Cyan
            } else {
                Color::Gray
            }))
            .render_ref(
                Rect::new(textarea_rect.x, textarea_rect.y, 1, textarea_rect.height),
                buf,
            );
        let mut textarea_rect = textarea_rect;
        textarea_rect.width = textarea_rect.width.saturating_sub(1);
        textarea_rect.x += 1;
        let mut state = self.textarea_state.borrow_mut();
        StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
        if self.textarea.text().is_empty() {
            let placeholder = if let Some(token_usage_info) = &self.token_usage_info {
                let token_usage = &token_usage_info.token_usage;
                let model_context_window = token_usage_info.model_context_window;
                match (token_usage.total_tokens, model_context_window) {
                    (total_tokens, Some(context_window)) => {
                        let percent_remaining: u8 = if context_window > 0 {
                            // Calculate the percentage of context left.
                            let percent =
                                100.0 - (total_tokens as f32 / context_window as f32 * 100.0);
                            percent.clamp(0.0, 100.0) as u8
                        } else {
                            // If we don't have a context window, we cannot compute the
                            // percentage.
                            100
                        };
                        // When https://github.com/openai/codex/issues/1257 is resolved,
                        // check if `percent_remaining < 25`, and if so, recommend
                        // /compact.
                        format!("{BASE_PLACEHOLDER_TEXT} — {percent_remaining}% context left")
                    }
                    (total_tokens, None) => {
                        format!("{BASE_PLACEHOLDER_TEXT} — {total_tokens} tokens used")
                    }
                }
            } else {
                BASE_PLACEHOLDER_TEXT.to_string()
            };
            Line::from(placeholder)
                .style(Style::default().dim())
                .render_ref(textarea_rect.inner(Margin::new(1, 0)), buf);
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::bottom_pane::AppEventSender;
    use crate::bottom_pane::ChatComposer;
    use crate::bottom_pane::InputResult;
    use crate::bottom_pane::chat_composer::LARGE_PASTE_CHAR_THRESHOLD;
    use crate::bottom_pane::textarea::TextArea;

    #[test]
    fn test_current_at_token_basic_cases() {
        let test_cases = vec![
            // Valid @ tokens
            ("@hello", 3, Some("hello".to_string()), "Basic ASCII token"),
            (
                "@file.txt",
                4,
                Some("file.txt".to_string()),
                "ASCII with extension",
            ),
            (
                "hello @world test",
                8,
                Some("world".to_string()),
                "ASCII token in middle",
            ),
            (
                "@test123",
                5,
                Some("test123".to_string()),
                "ASCII with numbers",
            ),
            // Unicode examples
            ("@İstanbul", 3, Some("İstanbul".to_string()), "Turkish text"),
            (
                "@testЙЦУ.rs",
                8,
                Some("testЙЦУ.rs".to_string()),
                "Mixed ASCII and Cyrillic",
            ),
            ("@诶", 2, Some("诶".to_string()), "Chinese character"),
            ("@👍", 2, Some("👍".to_string()), "Emoji token"),
            // Invalid cases (should return None)
            ("hello", 2, None, "No @ symbol"),
            ("@", 1, None, "Only @ symbol"),
            ("@ hello", 2, None, "@ followed by space"),
            ("test @ world", 6, None, "@ with spaces around"),
        ];

        for (input, cursor_pos, expected, description) in test_cases {
            let mut textarea = TextArea::new();
            textarea.insert_str(input);
            textarea.set_cursor(cursor_pos);

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for case: {description} - input: '{input}', cursor: {cursor_pos}"
            );
        }
    }

    #[test]
    fn test_current_at_token_cursor_positions() {
        let test_cases = vec![
            // Different cursor positions within a token
            ("@test", 0, Some("test".to_string()), "Cursor at @"),
            ("@test", 1, Some("test".to_string()), "Cursor after @"),
            ("@test", 5, Some("test".to_string()), "Cursor at end"),
            // Multiple tokens - cursor determines which token
            ("@file1 @file2", 0, Some("file1".to_string()), "First token"),
            (
                "@file1 @file2",
                8,
                Some("file2".to_string()),
                "Second token",
            ),
            // Edge cases
            ("@", 0, None, "Only @ symbol"),
            ("@a", 2, Some("a".to_string()), "Single character after @"),
            ("", 0, None, "Empty input"),
        ];

        for (input, cursor_pos, expected, description) in test_cases {
            let mut textarea = TextArea::new();
            textarea.insert_str(input);
            textarea.set_cursor(cursor_pos);

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for cursor position case: {description} - input: '{input}', cursor: {cursor_pos}",
            );
        }
    }

    #[test]
    fn test_current_at_token_whitespace_boundaries() {
        let test_cases = vec![
            // Space boundaries
            (
                "aaa@aaa",
                4,
                None,
                "Connected @ token - no completion by design",
            ),
            (
                "aaa @aaa",
                5,
                Some("aaa".to_string()),
                "@ token after space",
            ),
            (
                "test @file.txt",
                7,
                Some("file.txt".to_string()),
                "@ token after space",
            ),
            // Full-width space boundaries
            (
                "test　@İstanbul",
                8,
                Some("İstanbul".to_string()),
                "@ token after full-width space",
            ),
            (
                "@ЙЦУ　@诶",
                10,
                Some("诶".to_string()),
                "Full-width space between Unicode tokens",
            ),
            // Tab and newline boundaries
            (
                "test\t@file",
                6,
                Some("file".to_string()),
                "@ token after tab",
            ),
        ];

        for (input, cursor_pos, expected, description) in test_cases {
            let mut textarea = TextArea::new();
            textarea.insert_str(input);
            textarea.set_cursor(cursor_pos);

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for whitespace boundary case: {description} - input: '{input}', cursor: {cursor_pos}",
            );
        }
    }

    #[test]
    fn handle_paste_small_inserts_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let needs_redraw = composer.handle_paste("hello".to_string());
        assert!(needs_redraw);
        assert_eq!(composer.textarea.text(), "hello");
        assert!(composer.pending_pastes.is_empty());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, "hello"),
            _ => panic!("expected Submitted"),
        }
    }

    #[test]
    fn handle_paste_large_uses_placeholder_and_replaces_on_submit() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let large = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 10);
        let needs_redraw = composer.handle_paste(large.clone());
        assert!(needs_redraw);
        let placeholder = format!("[Pasted Content {} chars]", large.chars().count());
        assert_eq!(composer.textarea.text(), placeholder);
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, placeholder);
        assert_eq!(composer.pending_pastes[0].1, large);

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, large),
            _ => panic!("expected Submitted"),
        }
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn edit_clears_pending_paste() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let large = "y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1);
        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        composer.handle_paste(large);
        assert_eq!(composer.pending_pastes.len(), 1);

        // Any edit that removes the placeholder should clear pending_paste
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn ui_snapshots() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use insta::assert_snapshot;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut terminal = match Terminal::new(TestBackend::new(100, 10)) {
            Ok(t) => t,
            Err(e) => {
                // Avoid printing directly to stderr/stdout (clippy::print_stderr).
                // Log a warning instead and skip the snapshot test.
                tracing::warn!("Skipping ui_snapshots: failed to create terminal: {e}");
                return;
            }
        };

        let test_cases = vec![
            ("empty", None),
            ("small", Some("short".to_string())),
            ("large", Some("z".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5))),
            ("multiple_pastes", None),
            ("backspace_after_pastes", None),
        ];

        for (name, input) in test_cases {
            // Create a fresh composer for each test case
            let mut composer = ChatComposer::new(true, sender.clone(), false);

            if let Some(text) = input {
                composer.handle_paste(text);
            } else if name == "multiple_pastes" {
                // First large paste
                composer.handle_paste("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 3));
                // Second large paste
                composer.handle_paste("y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 7));
                // Small paste
                composer.handle_paste(" another short paste".to_string());
            } else if name == "backspace_after_pastes" {
                // Three large pastes
                composer.handle_paste("a".repeat(LARGE_PASTE_CHAR_THRESHOLD + 2));
                composer.handle_paste("b".repeat(LARGE_PASTE_CHAR_THRESHOLD + 4));
                composer.handle_paste("c".repeat(LARGE_PASTE_CHAR_THRESHOLD + 6));
                // Move cursor to end and press backspace
                composer.textarea.set_cursor(composer.textarea.text().len());
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }

            let draw_res = terminal.draw(|f| f.render_widget_ref(&composer, f.area()));
            assert!(draw_res.is_ok(), "Failed to draw {name} composer");

            assert_snapshot!(name, terminal.backend());
        }
    }

    #[test]
    fn esc_dismiss_slash_popup_reopen_on_token_change() {
        use crate::bottom_pane::chat_composer::ActivePopup;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        composer.handle_paste("/".to_string());
        assert!(matches!(composer.active_popup, ActivePopup::Command(_)));

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(composer.active_popup, ActivePopup::None));

        composer.handle_paste("c".to_string());
        assert!(matches!(composer.active_popup, ActivePopup::Command(_)));
    }

    #[test]
    fn esc_dismiss_then_delete_and_retype_slash_reopens_popup() {
        use crate::bottom_pane::chat_composer::ActivePopup;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        composer.handle_paste("/".to_string());
        assert!(matches!(composer.active_popup, ActivePopup::Command(_)));

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(composer.active_popup, ActivePopup::None));

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(matches!(composer.active_popup, ActivePopup::None));

        composer.handle_paste("/".to_string());
        assert!(matches!(composer.active_popup, ActivePopup::Command(_)));
    }

    // removed tests tied to auto-opening selectors and composer-owned error messages

    #[test]
    fn slash_popup_filters_as_user_types() {
        use crate::bottom_pane::chat_composer::ActivePopup;
        use crate::slash_command::SlashCommand;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        // Open the slash popup.
        composer.handle_paste("/".to_string());
        assert!(matches!(composer.active_popup, ActivePopup::Command(_)));

        // Type 'mo' and ensure the top selection corresponds to /model.
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));

        if let ActivePopup::Command(popup) = &composer.active_popup {
            let selected = popup.selected_command();
            assert_eq!(selected, Some(SlashCommand::Model).as_ref());
        } else {
            panic!("expected Command popup");
        }
    }

    #[test]
    fn enter_with_invalid_slash_token_shows_error_and_closes_popup() {
        use crate::app_event::AppEvent;
        use crate::bottom_pane::chat_composer::ActivePopup;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use std::sync::mpsc::TryRecvError;

        let (tx, rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        composer.handle_paste("/zzz".to_string());
        assert!(matches!(composer.active_popup, ActivePopup::Command(_)));

        let (result, _redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, InputResult::None));

        // Popup should be closed.
        assert!(matches!(composer.active_popup, ActivePopup::None));

        // We should receive an InsertHistory with an error message.
        let mut saw_error = false;
        loop {
            match rx.try_recv() {
                Ok(AppEvent::InsertHistory(lines)) => {
                    let joined: String = lines
                        .iter()
                        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
                        .collect::<Vec<_>>()
                        .join("");
                    if joined.to_lowercase().contains("invalid command") {
                        saw_error = true;
                        break;
                    }
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        assert!(saw_error, "expected an error InsertHistory entry");
    }

    #[test]
    fn enter_on_model_selector_selects_current_row() {
        use crate::app_event::AppEvent;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use std::sync::mpsc::TryRecvError;

        let (tx, rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        // Open the model selector directly with a few options and a current model.
        let options = vec![
            "codex-mini-latest".to_string(),
            "o3".to_string(),
            "gpt-4o".to_string(),
        ];
        composer.open_model_selector("o3", options);

        // Press Enter to select the currently highlighted row (should default to first visible).
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // We should receive a SelectModel event.
        let mut saw_select = false;
        loop {
            match rx.try_recv() {
                Ok(AppEvent::SelectModel(_m)) => {
                    saw_select = true;
                    break;
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        assert!(
            saw_select,
            "Enter on model selector should emit SelectModel"
        );
    }

    #[test]
    fn model_selector_stays_open_on_up_down() {
        use crate::bottom_pane::chat_composer::ActivePopup;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let options = vec![
            "codex-mini-latest".to_string(),
            "o3".to_string(),
            "gpt-4o".to_string(),
        ];
        composer.open_model_selector("o3", options);

        // Press Down; popup should remain visible
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(composer.active_popup, ActivePopup::Selection(_)));

        // Press Up; popup should remain visible
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert!(matches!(composer.active_popup, ActivePopup::Selection(_)));
    }

    #[test]
    fn model_selector_filters_with_free_text_typing() {
        use crate::app_event::AppEvent;
        use crate::bottom_pane::chat_composer::ActivePopup;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use std::sync::mpsc::TryRecvError;

        let (tx, rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let options = vec![
            "codex-mini-latest".to_string(),
            "o3".to_string(),
            "gpt-4o".to_string(),
        ];
        composer.open_model_selector("o3", options);
        assert!(matches!(composer.active_popup, ActivePopup::Selection(_)));

        // Type a free‑form query (without leading /model) and ensure it filters.
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        // Press Enter to select the (filtered) current row.
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // We should receive a SelectModel for the filtered option.
        let mut selected: Option<String> = None;
        loop {
            match rx.try_recv() {
                Ok(AppEvent::SelectModel(m)) => {
                    selected = Some(m);
                    break;
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        assert_eq!(selected.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn test_multiple_pastes_submission() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let test_cases = [
            ("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 3), true),
            (" and ".to_string(), false),
            ("y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 7), true),
        ];

        let mut expected_text = String::new();
        let mut expected_pending_count = 0;

        let states: Vec<_> = test_cases
            .iter()
            .map(|(content, is_large)| {
                composer.handle_paste(content.clone());
                if *is_large {
                    let placeholder = format!("[Pasted Content {} chars]", content.chars().count());
                    expected_text.push_str(&placeholder);
                    expected_pending_count += 1;
                } else {
                    expected_text.push_str(content);
                }
                (expected_text.clone(), expected_pending_count)
            })
            .collect();

        assert_eq!(
            states,
            vec![
                (
                    format!("[Pasted Content {} chars]", test_cases[0].0.chars().count()),
                    1
                ),
                (
                    format!(
                        "[Pasted Content {} chars] and ",
                        test_cases[0].0.chars().count()
                    ),
                    1
                ),
                (
                    format!(
                        "[Pasted Content {} chars] and [Pasted Content {} chars]",
                        test_cases[0].0.chars().count(),
                        test_cases[2].0.chars().count()
                    ),
                    2
                ),
            ]
        );

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        if let InputResult::Submitted(text) = result {
            assert_eq!(text, format!("{} and {}", test_cases[0].0, test_cases[2].0));
        } else {
            panic!("expected Submitted");
        }
    }

    // Note: slash command with args is usually handled via the selection popup.

    #[test]
    fn approvals_selection_full_yolo_emits_select_execution_mode() {
        use crate::app_event::AppEvent;
        use codex_core::protocol::AskForApproval;
        use codex_core::protocol::SandboxPolicy;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use std::sync::mpsc::TryRecvError;

        let (tx, rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        // Open the execution selector popup with a benign current mode.
        composer.open_execution_selector(
            AskForApproval::OnFailure,
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                network_access: false,
                include_default_writable_roots: true,
            },
        );

        // Immediately move selection up once to wrap to the last item (Full yolo), then Enter.
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Expect a SelectExecutionMode with DangerFullAccess.
        let mut saw = false;
        loop {
            match rx.try_recv() {
                Ok(AppEvent::SelectExecutionMode { approval, sandbox }) => {
                    assert_eq!(approval, AskForApproval::Never);
                    assert!(matches!(sandbox, SandboxPolicy::DangerFullAccess));
                    saw = true;
                    break;
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        assert!(saw, "expected SelectExecutionMode for Full yolo");
    }

    #[test]
    fn test_placeholder_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let test_cases = [
            ("a".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5), true),
            (" and ".to_string(), false),
            ("b".repeat(LARGE_PASTE_CHAR_THRESHOLD + 6), true),
        ];

        let mut current_pos = 0;
        let states: Vec<_> = test_cases
            .iter()
            .map(|(content, is_large)| {
                composer.handle_paste(content.clone());
                if *is_large {
                    let placeholder = format!("[Pasted Content {} chars]", content.chars().count());
                    current_pos += placeholder.len();
                } else {
                    current_pos += content.len();
                }
                (
                    composer.textarea.text().to_string(),
                    composer.pending_pastes.len(),
                    current_pos,
                )
            })
            .collect();

        let mut deletion_states = vec![];

        // First deletion
        composer.textarea.set_cursor(states[0].2);
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        deletion_states.push((
            composer.textarea.text().to_string(),
            composer.pending_pastes.len(),
        ));

        // Second deletion
        composer.textarea.set_cursor(composer.textarea.text().len());
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        deletion_states.push((
            composer.textarea.text().to_string(),
            composer.pending_pastes.len(),
        ));

        assert_eq!(
            deletion_states,
            vec![
                (" and [Pasted Content 1006 chars]".to_string(), 1),
                (" and ".to_string(), 0),
            ]
        );
    }

    #[test]
    fn test_partial_placeholder_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        let test_cases = [
            5, // Delete from middle - should clear tracking
            0, // Delete from end - should clear tracking
        ];

        let paste = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 4);
        let placeholder = format!("[Pasted Content {} chars]", paste.chars().count());

        let states: Vec<_> = test_cases
            .into_iter()
            .map(|pos_from_end| {
                composer.handle_paste(paste.clone());
                composer
                    .textarea
                    .set_cursor((placeholder.len() - pos_from_end) as usize);
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                let result = (
                    composer.textarea.text().contains(&placeholder),
                    composer.pending_pastes.len(),
                );
                composer.textarea.set_text("");
                result
            })
            .collect();

        assert_eq!(
            states,
            vec![
                (false, 0), // After deleting from middle
                (false, 0), // After deleting from end
            ]
        );
    }

    // removed test tied to composer opening approvals selector

    #[test]
    fn enter_on_approvals_selector_selects_current_row() {
        use crate::app_event::AppEvent;
        use codex_core::protocol::AskForApproval;
        use codex_core::protocol::SandboxPolicy;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use std::sync::mpsc::TryRecvError;

        let (tx, rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        // Open the execution selector directly (current: Read only)
        composer.open_execution_selector(AskForApproval::Never, &SandboxPolicy::ReadOnly);

        // Press Enter to select the currently highlighted row (first visible)
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // We should receive a SelectExecutionMode event.
        let mut saw_select = false;
        loop {
            match rx.try_recv() {
                Ok(AppEvent::SelectExecutionMode { .. }) => {
                    saw_select = true;
                    break;
                }
                Ok(_) => continue,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        assert!(
            saw_select,
            "Enter on approvals selector should emit SelectApprovalPolicy"
        );
    }
}
