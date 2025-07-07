use codex_core::protocol::TokenUsage;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tui_textarea::Input;
use tui_textarea::Key;
use tui_textarea::TextArea;

use super::chat_composer_history::ChatComposerHistory;
use super::command_popup::CommandPopup;
use super::file_search_popup::FileSearchPopup;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_file_search::FileMatch;

/// Minimum number of visible text rows inside the textarea.
const MIN_TEXTAREA_ROWS: usize = 1;
/// Rows consumed by the border.
const BORDER_LINES: u16 = 2;

const BASE_PLACEHOLDER_TEXT: &str = "send a message";

/// Result returned when the user interacts with the text area.
pub enum InputResult {
    Submitted(String),
    None,
}

pub(crate) struct ChatComposer<'a> {
    textarea: TextArea<'a>,
    active_popup: ActivePopup,
    app_event_tx: AppEventSender,
    history: ChatComposerHistory,
    ctrl_c_quit_hint: bool,
    dismissed_file_popup_token: Option<String>,
    current_file_query: Option<String>,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
}

impl ChatComposer<'_> {
    pub fn new(has_input_focus: bool, app_event_tx: AppEventSender) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text(BASE_PLACEHOLDER_TEXT);
        textarea.set_cursor_line_style(ratatui::style::Style::default());

        let mut this = Self {
            textarea,
            active_popup: ActivePopup::None,
            app_event_tx,
            history: ChatComposerHistory::new(),
            ctrl_c_quit_hint: false,
            dismissed_file_popup_token: None,
            current_file_query: None,
        };
        this.update_border(has_input_focus);
        this
    }

    /// Update the cached *context-left* percentage and refresh the placeholder
    /// text. The UI relies on the placeholder to convey the remaining
    /// context when the composer is empty.
    pub(crate) fn set_token_usage(
        &mut self,
        token_usage: TokenUsage,
        model_context_window: Option<u64>,
    ) {
        let placeholder = match (token_usage.total_tokens, model_context_window) {
            (total_tokens, Some(context_window)) => {
                let percent_remaining: u8 = if context_window > 0 {
                    // Calculate the percentage of context left.
                    let percent = 100.0 - (total_tokens as f32 / context_window as f32 * 100.0);
                    percent.clamp(0.0, 100.0) as u8
                } else {
                    // If we don't have a context window, we cannot compute the
                    // percentage.
                    100
                };
                if percent_remaining > 25 {
                    format!("{BASE_PLACEHOLDER_TEXT} — {percent_remaining}% context left")
                } else {
                    format!(
                        "{BASE_PLACEHOLDER_TEXT} — {percent_remaining}% context left (consider /compact)"
                    )
                }
            }
            (total_tokens, None) => {
                format!("{BASE_PLACEHOLDER_TEXT} — {total_tokens} tokens used")
            }
        };

        self.textarea.set_placeholder_text(placeholder);
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
        self.history
            .on_entry_response(log_id, offset, entry, &mut self.textarea)
    }

    pub fn set_input_focus(&mut self, has_focus: bool) {
        self.update_border(has_focus);
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
        self.update_border(has_focus);
    }

    /// Handle a key event coming from the main UI.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let result = match &mut self.active_popup {
            ActivePopup::Command(_) => self.handle_key_event_with_slash_popup(key_event),
            ActivePopup::File(_) => self.handle_key_event_with_file_popup(key_event),
            ActivePopup::None => self.handle_key_event_without_popup(key_event),
        };

        // Update (or hide/show) popup after processing the key.
        self.sync_command_popup();
        if matches!(self.active_popup, ActivePopup::Command(_)) {
            self.dismissed_file_popup_token = None;
        } else {
            self.sync_file_search_popup();
        }

        result
    }

    /// Handle key event when the slash-command popup is visible.
    fn handle_key_event_with_slash_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::Command(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event.into() {
            Input { key: Key::Up, .. } => {
                popup.move_up();
                (InputResult::None, true)
            }
            Input { key: Key::Down, .. } => {
                popup.move_down();
                (InputResult::None, true)
            }
            Input { key: Key::Tab, .. } => {
                if let Some(cmd) = popup.selected_command() {
                    let first_line = self
                        .textarea
                        .lines()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("");

                    let starts_with_cmd = first_line
                        .trim_start()
                        .starts_with(&format!("/{}", cmd.command()));

                    if !starts_with_cmd {
                        self.textarea.select_all();
                        self.textarea.cut();
                        let _ = self.textarea.insert_str(format!("/{} ", cmd.command()));
                    }
                }
                (InputResult::None, true)
            }
            Input {
                key: Key::Enter,
                shift: false,
                alt: false,
                ctrl: false,
            } => {
                if let Some(cmd) = popup.selected_command() {
                    // Send command to the app layer.
                    self.app_event_tx.send(AppEvent::DispatchCommand(*cmd));

                    // Clear textarea so no residual text remains.
                    self.textarea.select_all();
                    self.textarea.cut();

                    // Hide popup since the command has been dispatched.
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                }
                // Fallback to default newline handling if no command selected.
                self.handle_key_event_without_popup(key_event)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle key events when file search popup is visible.
    fn handle_key_event_with_file_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::File(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event.into() {
            Input { key: Key::Up, .. } => {
                popup.move_up();
                (InputResult::None, true)
            }
            Input { key: Key::Down, .. } => {
                popup.move_down();
                (InputResult::None, true)
            }
            Input { key: Key::Esc, .. } => {
                // Hide popup without modifying text, remember token to avoid immediate reopen.
                if let Some(tok) = Self::current_at_token(&self.textarea) {
                    self.dismissed_file_popup_token = Some(tok.to_string());
                }
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            Input { key: Key::Tab, .. }
            | Input {
                key: Key::Enter,
                ctrl: false,
                alt: false,
                shift: false,
            } => {
                if let Some(sel) = popup.selected_match() {
                    let sel_path = sel.to_string();
                    // Drop popup borrow before using self mutably again.
                    self.insert_selected_path(&sel_path);
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                }
                (InputResult::None, false)
            }
            input => self.handle_input_basic(input),
        }
    }

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
    fn current_at_token(textarea: &tui_textarea::TextArea) -> Option<String> {
        let (row, col) = textarea.cursor();

        // Guard against out-of-bounds rows.
        let line = textarea.lines().get(row)?.as_str();

        // Calculate byte offset for cursor position
        let cursor_byte_offset = line.chars().take(col).map(|c| c.len_utf8()).sum::<usize>();

        // Split the line at the cursor position so we can search for word
        // boundaries on both sides.
        let before_cursor = &line[..cursor_byte_offset];
        let after_cursor = &line[cursor_byte_offset..];

        // Find start index (first character **after** the previous multi-byte whitespace).
        let start_idx = before_cursor
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);

        // Find end index (first multi-byte whitespace **after** the cursor position).
        let end_rel_idx = after_cursor
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(idx, _)| idx)
            .unwrap_or(after_cursor.len());
        let end_idx = cursor_byte_offset + end_rel_idx;

        if start_idx >= end_idx {
            return None;
        }

        let token = &line[start_idx..end_idx];

        if token.starts_with('@') && token.len() > 1 {
            Some(token[1..].to_string())
        } else {
            None
        }
    }

    /// Replace the active `@token` (the one under the cursor) with `path`.
    ///
    /// The algorithm mirrors `current_at_token` so replacement works no matter
    /// where the cursor is within the token and regardless of how many
    /// `@tokens` exist in the line.
    fn insert_selected_path(&mut self, path: &str) {
        let (row, col) = self.textarea.cursor();

        // Materialize the textarea lines so we can mutate them easily.
        let mut lines: Vec<String> = self.textarea.lines().to_vec();

        if let Some(line) = lines.get_mut(row) {
            // Calculate byte offset for cursor position
            let cursor_byte_offset = line.chars().take(col).map(|c| c.len_utf8()).sum::<usize>();

            let before_cursor = &line[..cursor_byte_offset];
            let after_cursor = &line[cursor_byte_offset..];

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
            let end_idx = cursor_byte_offset + end_rel_idx;

            // Replace the slice `[start_idx, end_idx)` with the chosen path and a trailing space.
            let mut new_line =
                String::with_capacity(line.len() - (end_idx - start_idx) + path.len() + 1);
            new_line.push_str(&line[..start_idx]);
            new_line.push_str(path);
            new_line.push(' ');
            new_line.push_str(&line[end_idx..]);

            *line = new_line;

            // Re-populate the textarea.
            let new_text = lines.join("\n");
            self.textarea.select_all();
            self.textarea.cut();
            let _ = self.textarea.insert_str(new_text);

            // Note: tui-textarea currently exposes only relative cursor
            // movements. Leaving the cursor position unchanged is acceptable
            // as subsequent typing will move the cursor naturally.
        }
    }

    /// Handle key event when no popup is visible.
    fn handle_key_event_without_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let input: Input = key_event.into();
        match input {
            // -------------------------------------------------------------
            // History navigation (Up / Down) – only when the composer is not
            // empty or when the cursor is at the correct position, to avoid
            // interfering with normal cursor movement.
            // -------------------------------------------------------------
            Input { key: Key::Up, .. } => {
                if self.history.should_handle_navigation(&self.textarea) {
                    let consumed = self
                        .history
                        .navigate_up(&mut self.textarea, &self.app_event_tx);
                    if consumed {
                        return (InputResult::None, true);
                    }
                }
                self.handle_input_basic(input)
            }
            Input { key: Key::Down, .. } => {
                if self.history.should_handle_navigation(&self.textarea) {
                    let consumed = self
                        .history
                        .navigate_down(&mut self.textarea, &self.app_event_tx);
                    if consumed {
                        return (InputResult::None, true);
                    }
                }
                self.handle_input_basic(input)
            }
            Input {
                key: Key::Enter,
                shift: false,
                alt: false,
                ctrl: false,
            } => {
                let text = self.textarea.lines().join("\n");
                self.textarea.select_all();
                self.textarea.cut();

                if text.is_empty() {
                    (InputResult::None, true)
                } else {
                    self.history.record_local_submission(&text);
                    (InputResult::Submitted(text), true)
                }
            }
            Input {
                key: Key::Enter, ..
            }
            | Input {
                key: Key::Char('j'),
                ctrl: true,
                alt: false,
                shift: false,
            } => {
                self.textarea.insert_newline();
                (InputResult::None, true)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle generic Input events that modify the textarea content.
    fn handle_input_basic(&mut self, input: Input) -> (InputResult, bool) {
        self.textarea.input(input);
        (InputResult::None, true)
    }

    /// Synchronize `self.command_popup` with the current text in the
    /// textarea. This must be called after every modification that can change
    /// the text so the popup is shown/updated/hidden as appropriate.
    fn sync_command_popup(&mut self) {
        // Inspect only the first line to decide whether to show the popup. In
        // the common case (no leading slash) we avoid copying the entire
        // textarea contents.
        let first_line = self
            .textarea
            .lines()
            .first()
            .map(|s| s.as_str())
            .unwrap_or("");

        let input_starts_with_slash = first_line.starts_with('/');
        match &mut self.active_popup {
            ActivePopup::Command(popup) => {
                if input_starts_with_slash {
                    popup.on_composer_text_change(first_line.to_string());
                } else {
                    self.active_popup = ActivePopup::None;
                }
            }
            _ => {
                if input_starts_with_slash {
                    let mut command_popup = CommandPopup::new();
                    command_popup.on_composer_text_change(first_line.to_string());
                    self.active_popup = ActivePopup::Command(command_popup);
                }
            }
        }
    }

    /// Synchronize `self.file_search_popup` with the current text in the textarea.
    /// Note this is only called when self.active_popup is NOT Command.
    fn sync_file_search_popup(&mut self) {
        // Determine if there is an @token underneath the cursor.
        let query = match Self::current_at_token(&self.textarea) {
            Some(token) => token,
            None => {
                self.active_popup = ActivePopup::None;
                self.dismissed_file_popup_token = None;
                return;
            }
        };

        // If user dismissed popup for this exact query, don't reopen until text changes.
        if self.dismissed_file_popup_token.as_ref() == Some(&query) {
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
        self.dismissed_file_popup_token = None;
    }

    pub fn calculate_required_height(&self, area: &Rect) -> u16 {
        let rows = self.textarea.lines().len().max(MIN_TEXTAREA_ROWS);
        let num_popup_rows = match &self.active_popup {
            ActivePopup::Command(popup) => popup.calculate_required_height(area),
            ActivePopup::File(popup) => popup.calculate_required_height(area),
            ActivePopup::None => 0,
        };

        rows as u16 + BORDER_LINES + num_popup_rows
    }

    fn update_border(&mut self, has_focus: bool) {
        struct BlockState {
            right_title: Line<'static>,
            border_style: Style,
        }

        let bs = if has_focus {
            if self.ctrl_c_quit_hint {
                BlockState {
                    right_title: Line::from("Ctrl+C to quit").alignment(Alignment::Right),
                    border_style: Style::default(),
                }
            } else {
                BlockState {
                    right_title: Line::from("Enter to send | Ctrl+D to quit | Ctrl+J for newline")
                        .alignment(Alignment::Right),
                    border_style: Style::default(),
                }
            }
        } else {
            BlockState {
                right_title: Line::from(""),
                border_style: Style::default().dim(),
            }
        };

        self.textarea.set_block(
            ratatui::widgets::Block::default()
                .title_bottom(bs.right_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(bs.border_style),
        );
    }

    pub(crate) fn is_popup_visible(&self) -> bool {
        match self.active_popup {
            ActivePopup::Command(_) | ActivePopup::File(_) => true,
            ActivePopup::None => false,
        }
    }
}

impl WidgetRef for &ChatComposer<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match &self.active_popup {
            ActivePopup::Command(popup) => {
                let popup_height = popup.calculate_required_height(&area);

                // Split the provided rect so that the popup is rendered at the
                // *top* and the textarea occupies the remaining space below.
                let popup_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: popup_height.min(area.height),
                };

                let textarea_rect = Rect {
                    x: area.x,
                    y: area.y + popup_rect.height,
                    width: area.width,
                    height: area.height.saturating_sub(popup_rect.height),
                };

                popup.render(popup_rect, buf);
                self.textarea.render(textarea_rect, buf);
            }
            ActivePopup::File(popup) => {
                let popup_height = popup.calculate_required_height(&area);

                let popup_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: popup_height.min(area.height),
                };

                let textarea_rect = Rect {
                    x: area.x,
                    y: area.y + popup_rect.height,
                    width: area.width,
                    height: area.height.saturating_sub(popup_height),
                };

                popup.render(popup_rect, buf);
                self.textarea.render(textarea_rect, buf);
            }
            ActivePopup::None => {
                self.textarea.render(area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bottom_pane::ChatComposer;
    use tui_textarea::TextArea;

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
            let mut textarea = TextArea::default();
            textarea.insert_str(input);
            textarea.move_cursor(tui_textarea::CursorMove::Jump(0, cursor_pos));

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for case: {} - input: '{}', cursor: {}",
                description, input, cursor_pos
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
            let mut textarea = TextArea::default();
            textarea.insert_str(input);
            textarea.move_cursor(tui_textarea::CursorMove::Jump(0, cursor_pos));

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
                6,
                Some("İstanbul".to_string()),
                "@ token after full-width space",
            ),
            (
                "@ЙЦУ　@诶",
                6,
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
            let mut textarea = TextArea::default();
            textarea.insert_str(input);
            textarea.move_cursor(tui_textarea::CursorMove::Jump(0, cursor_pos));

            let result = ChatComposer::current_at_token(&textarea);
            assert_eq!(
                result, expected,
                "Failed for whitespace boundary case: {description} - input: '{input}', cursor: {cursor_pos}",
            );
        }
    }
}
