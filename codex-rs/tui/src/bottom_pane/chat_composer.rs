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
use ratatui::style::Modifier;
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
use crate::slash_command::SlashCommand;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;
use codex_file_search::FileMatch;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

// Heuristic thresholds for detecting paste-like input bursts.
const PASTE_BURST_MIN_CHARS: u16 = 3;
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(8);
const PASTE_ENTER_SUPPRESS_WINDOW: Duration = Duration::from_millis(120);

/// If the pasted content exceeds this number of characters, replace it with a
/// placeholder in the UI.
const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

/// Result returned when the user interacts with the text area.
pub enum InputResult {
    Submitted(String),
    Command(SlashCommand),
    None,
}

#[derive(Clone, Debug, PartialEq)]
struct AttachedImage {
    placeholder: String,
    path: PathBuf,
}

struct TokenUsageInfo {
    total_token_usage: TokenUsage,
    last_token_usage: TokenUsage,
    model_context_window: Option<u64>,
    /// Baseline token count present in the context before the user's first
    /// message content is considered. This is used to normalize the
    /// "context left" percentage so it reflects the portion the user can
    /// influence rather than fixed prompt overhead (system prompt, tool
    /// instructions, etc.).
    ///
    /// Preferred source is `cached_input_tokens` from the first turn (when
    /// available), otherwise we fall back to 0.
    initial_prompt_tokens: u64,
}

pub(crate) struct ChatComposer {
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    active_popup: ActivePopup,
    app_event_tx: AppEventSender,
    history: ChatComposerHistory,
    ctrl_c_quit_hint: bool,
    esc_backtrack_hint: bool,
    use_shift_enter_hint: bool,
    dismissed_file_popup_token: Option<String>,
    current_file_query: Option<String>,
    pending_pastes: Vec<(String, String)>,
    token_usage_info: Option<TokenUsageInfo>,
    has_focus: bool,
    attached_images: Vec<AttachedImage>,
    placeholder_text: String,
    // Heuristic state to detect non-bracketed paste bursts.
    last_plain_char_time: Option<Instant>,
    consecutive_plain_char_burst: u16,
    paste_burst_until: Option<Instant>,
    // Buffer to accumulate characters during a detected non-bracketed paste burst.
    paste_burst_buffer: String,
    in_paste_burst_mode: bool,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
}

impl ChatComposer {
    pub fn new(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
        placeholder_text: String,
    ) -> Self {
        let use_shift_enter_hint = enhanced_keys_supported;

        Self {
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            active_popup: ActivePopup::None,
            app_event_tx,
            history: ChatComposerHistory::new(),
            ctrl_c_quit_hint: false,
            esc_backtrack_hint: false,
            use_shift_enter_hint,
            dismissed_file_popup_token: None,
            current_file_query: None,
            pending_pastes: Vec::new(),
            token_usage_info: None,
            has_focus: has_input_focus,
            attached_images: Vec::new(),
            placeholder_text,
            last_plain_char_time: None,
            consecutive_plain_char_burst: 0,
            paste_burst_until: None,
            paste_burst_buffer: String::new(),
            in_paste_burst_mode: false,
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.textarea.desired_height(width - 1)
            + match &self.active_popup {
                ActivePopup::None => 1u16,
                ActivePopup::Command(c) => c.calculate_required_height(),
                ActivePopup::File(c) => c.calculate_required_height(),
            }
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let popup_height = match &self.active_popup {
            ActivePopup::Command(popup) => popup.calculate_required_height(),
            ActivePopup::File(popup) => popup.calculate_required_height(),
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
        total_token_usage: TokenUsage,
        last_token_usage: TokenUsage,
        model_context_window: Option<u64>,
    ) {
        let initial_prompt_tokens = self
            .token_usage_info
            .as_ref()
            .map(|info| info.initial_prompt_tokens)
            .unwrap_or_else(|| last_token_usage.cached_input_tokens.unwrap_or(0));

        self.token_usage_info = Some(TokenUsageInfo {
            total_token_usage,
            last_token_usage,
            model_context_window,
            initial_prompt_tokens,
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
            self.textarea.insert_element(&placeholder);
            self.pending_pastes.push((placeholder, pasted));
        } else {
            self.textarea.insert_str(&pasted);
        }
        // Explicit paste events should not trigger Enter suppression.
        self.last_plain_char_time = None;
        self.consecutive_plain_char_burst = 0;
        self.paste_burst_until = None;
        self.sync_command_popup();
        self.sync_file_search_popup();
        true
    }

    pub fn attach_image(&mut self, path: PathBuf, width: u32, height: u32, format_label: &str) {
        let placeholder = format!("[image {width}x{height} {format_label}]");
        // Insert as an element to match large paste placeholder behavior:
        // styled distinctly and treated atomically for cursor/mutations.
        self.textarea.insert_element(&placeholder);
        self.attached_images
            .push(AttachedImage { placeholder, path });
    }

    pub fn take_recent_submission_images(&mut self) -> Vec<PathBuf> {
        let images = std::mem::take(&mut self.attached_images);
        images.into_iter().map(|img| img.path).collect()
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

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.textarea.insert_str(text);
        self.sync_command_popup();
        self.sync_file_search_popup();
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
                code: KeyCode::Tab, ..
            } => {
                if let Some(cmd) = popup.selected_command() {
                    let first_line = self.textarea.text().lines().next().unwrap_or("");

                    let starts_with_cmd = first_line
                        .trim_start()
                        .starts_with(&format!("/{}", cmd.command()));

                    if !starts_with_cmd {
                        self.textarea.set_text(&format!("/{} ", cmd.command()));
                        self.textarea.set_cursor(self.textarea.text().len());
                    }
                    // After completing the command, move cursor to the end.
                    if !self.textarea.text().is_empty() {
                        let end = self.textarea.text().len();
                        self.textarea.set_cursor(end);
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
                    // Clear textarea so no residual text remains.
                    self.textarea.set_text("");

                    let result = (InputResult::Command(*cmd), true);

                    // Hide popup since the command has been dispatched.
                    self.active_popup = ActivePopup::None;

                    return result;
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
                // Hide popup without modifying text, remember token to avoid immediate reopen.
                if let Some(tok) = Self::current_at_token(&self.textarea) {
                    self.dismissed_file_popup_token = Some(tok.to_string());
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
                let Some(sel) = popup.selected_match() else {
                    self.active_popup = ActivePopup::None;
                    return (InputResult::None, true);
                };

                let sel_path = sel.to_string();
                // If selected path looks like an image (png/jpeg), attach as image instead of inserting text.
                let is_image = Self::is_image_path(&sel_path);
                if is_image {
                    // Determine dimensions; if that fails fall back to normal path insertion.
                    let path_buf = PathBuf::from(&sel_path);
                    if let Ok((w, h)) = image::image_dimensions(&path_buf) {
                        // Remove the current @token (mirror logic from insert_selected_path without inserting text)
                        // using the flat text and byte-offset cursor API.
                        let cursor_offset = self.textarea.cursor();
                        let text = self.textarea.text();
                        let before_cursor = &text[..cursor_offset];
                        let after_cursor = &text[cursor_offset..];

                        // Determine token boundaries in the full text.
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

                        self.textarea.replace_range(start_idx..end_idx, "");
                        self.textarea.set_cursor(start_idx);

                        let format_label = match Path::new(&sel_path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_ascii_lowercase())
                        {
                            Some(ext) if ext == "png" => "PNG",
                            Some(ext) if ext == "jpg" || ext == "jpeg" => "JPEG",
                            _ => "IMG",
                        };
                        self.attach_image(path_buf.clone(), w, h, format_label);
                        // Add a trailing space to keep typing fluid.
                        self.textarea.insert_str(" ");
                    } else {
                        // Fallback to plain path insertion if metadata read fails.
                        self.insert_selected_path(&sel_path);
                    }
                } else {
                    // Non-image: inserting file path.
                    self.insert_selected_path(&sel_path);
                }
                // No selection: treat Enter as closing the popup/session.
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            input => self.handle_input_basic(input),
        }
    }

    fn is_image_path(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
    }

    /// Extract the `@token` that the cursor is currently positioned on, if any.
    ///
    /// The returned string **does not** include the leading `@`.
    ///
    /// Behavior:
    /// - The cursor may be anywhere *inside* the token (including on the
    ///   leading `@`). It does **not** need to be at the end of the line.
    /// - A token is delimited by ASCII whitespace (space, tab, newline).
    /// - If the token under the cursor starts with `@`, that token is
    ///   returned without the leading `@`. This includes the case where the
    ///   token is just "@" (empty query), which is used to trigger a UI hint
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
            .filter(|t| t.starts_with('@'))
            .map(|t| t[1..].to_string());
        let right_at = token_right
            .filter(|t| t.starts_with('@'))
            .map(|t| t[1..].to_string());

        if at_whitespace {
            if right_at.is_some() {
                return right_at;
            }
            if token_left.is_some_and(|t| t == "@") {
                return None;
            }
            return left_at;
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

        // Replace the slice `[start_idx, end_idx)` with the chosen path and a trailing space.
        let mut new_text =
            String::with_capacity(text.len() - (end_idx - start_idx) + path.len() + 1);
        new_text.push_str(&text[..start_idx]);
        new_text.push_str(path);
        new_text.push(' ');
        new_text.push_str(&text[end_idx..]);

        self.textarea.set_text(&new_text);
        let new_cursor = start_idx.saturating_add(path.len()).saturating_add(1);
        self.textarea.set_cursor(new_cursor);
    }

    /// Handle key event when no popup is visible.
    fn handle_key_event_without_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        match key_event {
            // -------------------------------------------------------------
            // History navigation (Up / Down) – only when the composer is not
            // empty or when the cursor is at the correct position, to avoid
            // interfering with normal cursor movement.
            // -------------------------------------------------------------
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
                // If we're in a paste-like burst capture, treat Enter as part of the burst
                // and accumulate it rather than submitting or inserting immediately.
                // Do not treat Enter as paste inside a slash-command context.
                let in_slash_context = matches!(self.active_popup, ActivePopup::Command(_))
                    || self
                        .textarea
                        .text()
                        .lines()
                        .next()
                        .unwrap_or("")
                        .starts_with('/');
                if (self.in_paste_burst_mode || !self.paste_burst_buffer.is_empty())
                    && !in_slash_context
                {
                    self.paste_burst_buffer.push('\n');
                    let now = Instant::now();
                    // Keep the window alive so subsequent lines are captured too.
                    self.paste_burst_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
                    return (InputResult::None, true);
                }
                // If we have pending placeholder pastes, submit immediately to expand them.
                if !self.pending_pastes.is_empty() {
                    let mut text = self.textarea.text().to_string();
                    self.textarea.set_text("");
                    for (placeholder, actual) in &self.pending_pastes {
                        if text.contains(placeholder) {
                            text = text.replace(placeholder, actual);
                        }
                    }
                    self.pending_pastes.clear();
                    if text.is_empty() {
                        return (InputResult::None, true);
                    }
                    self.history.record_local_submission(&text);
                    return (InputResult::Submitted(text), true);
                }

                // During a paste-like burst, treat Enter as a newline instead of submit.
                let now = Instant::now();
                let tight_after_char = self
                    .last_plain_char_time
                    .is_some_and(|t| now.duration_since(t) <= PASTE_BURST_CHAR_INTERVAL);
                let recent_after_char = self
                    .last_plain_char_time
                    .is_some_and(|t| now.duration_since(t) <= PASTE_ENTER_SUPPRESS_WINDOW);
                let burst_by_count =
                    recent_after_char && self.consecutive_plain_char_burst >= PASTE_BURST_MIN_CHARS;
                let in_burst_window = self.paste_burst_until.is_some_and(|until| now <= until);

                if tight_after_char || burst_by_count || in_burst_window {
                    self.textarea.insert_str("\n");
                    self.paste_burst_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
                    return (InputResult::None, true);
                }
                let mut text = self.textarea.text().to_string();
                self.textarea.set_text("");

                // Replace all pending pastes in the text
                for (placeholder, actual) in &self.pending_pastes {
                    if text.contains(placeholder) {
                        text = text.replace(placeholder, actual);
                    }
                }
                self.pending_pastes.clear();

                // Strip image placeholders from the submitted text; images are retrieved via take_recent_submission_images()
                for img in &self.attached_images {
                    if text.contains(&img.placeholder) {
                        text = text.replace(&img.placeholder, "");
                    }
                }

                text = text.trim().to_string();
                if !text.is_empty() {
                    self.history.record_local_submission(&text);
                }
                // Do not clear attached_images here; ChatWidget drains them via take_recent_submission_images().
                (InputResult::Submitted(text), true)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle generic Input events that modify the textarea content.
    fn handle_input_basic(&mut self, input: KeyEvent) -> (InputResult, bool) {
        // If we have a buffered non-bracketed paste burst and enough time has
        // elapsed since the last char, flush it before handling a new input.
        let now = Instant::now();
        let timed_out = self
            .last_plain_char_time
            .is_some_and(|t| now.duration_since(t) > PASTE_BURST_CHAR_INTERVAL);
        if timed_out && (!self.paste_burst_buffer.is_empty() || self.in_paste_burst_mode) {
            let pasted = std::mem::take(&mut self.paste_burst_buffer);
            self.in_paste_burst_mode = false;
            // Reuse normal paste path (handles large-paste placeholders).
            self.handle_paste(pasted);
        }

        // If we're capturing a burst and receive Enter, accumulate it instead of inserting.
        if matches!(input.code, KeyCode::Enter)
            && (self.in_paste_burst_mode || !self.paste_burst_buffer.is_empty())
        {
            self.paste_burst_buffer.push('\n');
            self.paste_burst_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            return (InputResult::None, true);
        }

        // Intercept plain Char inputs to optionally accumulate into a burst buffer.
        if let KeyEvent {
            code: KeyCode::Char(ch),
            modifiers,
            ..
        } = input
        {
            let has_ctrl_or_alt =
                modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::ALT);
            if !has_ctrl_or_alt {
                // Update burst heuristics.
                match self.last_plain_char_time {
                    Some(prev) if now.duration_since(prev) <= PASTE_BURST_CHAR_INTERVAL => {
                        self.consecutive_plain_char_burst =
                            self.consecutive_plain_char_burst.saturating_add(1);
                    }
                    _ => {
                        self.consecutive_plain_char_burst = 1;
                    }
                }
                self.last_plain_char_time = Some(now);

                // If we're already buffering, capture the char into the buffer.
                if self.in_paste_burst_mode {
                    self.paste_burst_buffer.push(ch);
                    // Keep the window alive while we receive the burst.
                    self.paste_burst_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
                    return (InputResult::None, true);
                } else if self.consecutive_plain_char_burst >= PASTE_BURST_MIN_CHARS {
                    // Do not start burst buffering while typing a slash command (first line starts with '/').
                    let first_line = self.textarea.text().lines().next().unwrap_or("");
                    if first_line.starts_with('/') {
                        // Keep heuristics but do not buffer.
                        self.paste_burst_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
                        // Insert normally.
                        self.textarea.input(input);
                        let text_after = self.textarea.text();
                        self.pending_pastes
                            .retain(|(placeholder, _)| text_after.contains(placeholder));
                        return (InputResult::None, true);
                    }
                    // Begin buffering from this character onward.
                    self.paste_burst_buffer.push(ch);
                    self.in_paste_burst_mode = true;
                    // Keep the window alive to continue capturing.
                    self.paste_burst_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
                    return (InputResult::None, true);
                }

                // Not buffering: insert normally and continue.
                self.textarea.input(input);
                let text_after = self.textarea.text();
                self.pending_pastes
                    .retain(|(placeholder, _)| text_after.contains(placeholder));
                return (InputResult::None, true);
            } else {
                // Modified char ends any burst: flush buffered content before applying.
                if !self.paste_burst_buffer.is_empty() || self.in_paste_burst_mode {
                    let pasted = std::mem::take(&mut self.paste_burst_buffer);
                    self.in_paste_burst_mode = false;
                    self.handle_paste(pasted);
                }
            }
        }

        // For non-char inputs (or after flushing), handle normally.
        // Special handling for backspace on placeholders
        if let KeyEvent {
            code: KeyCode::Backspace,
            ..
        } = input
            && self.try_remove_any_placeholder_at_cursor()
        {
            return (InputResult::None, true);
        }

        // Normal input handling
        self.textarea.input(input);
        let text_after = self.textarea.text();

        // Update paste-burst heuristic for plain Char (no Ctrl/Alt) events.
        let crossterm::event::KeyEvent {
            code, modifiers, ..
        } = input;
        match code {
            KeyCode::Char(_) => {
                let has_ctrl_or_alt = modifiers.contains(KeyModifiers::CONTROL)
                    || modifiers.contains(KeyModifiers::ALT);
                if has_ctrl_or_alt {
                    // Modified char: clear burst window.
                    self.consecutive_plain_char_burst = 0;
                    self.last_plain_char_time = None;
                    self.paste_burst_until = None;
                    self.in_paste_burst_mode = false;
                    self.paste_burst_buffer.clear();
                }
                // Plain chars handled above.
            }
            KeyCode::Enter => {
                // Keep burst window alive (supports blank lines in paste).
            }
            _ => {
                // Other keys: clear burst window and any buffer (after flushing earlier).
                self.consecutive_plain_char_burst = 0;
                self.last_plain_char_time = None;
                self.paste_burst_until = None;
                self.in_paste_burst_mode = false;
                // Do not clear paste_burst_buffer here; it should have been flushed above.
            }
        }

        // Check if any placeholders were removed and remove their corresponding pending pastes
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));

        // Keep attached images in proportion to how many matching placeholders exist in the text.
        // This handles duplicate placeholders that share the same visible label.
        if !self.attached_images.is_empty() {
            let mut needed: HashMap<String, usize> = HashMap::new();
            for img in &self.attached_images {
                needed
                    .entry(img.placeholder.clone())
                    .or_insert_with(|| text_after.matches(&img.placeholder).count());
            }

            let mut used: HashMap<String, usize> = HashMap::new();
            let mut kept: Vec<AttachedImage> = Vec::with_capacity(self.attached_images.len());
            for img in self.attached_images.drain(..) {
                let total_needed = *needed.get(&img.placeholder).unwrap_or(&0);
                let used_count = used.entry(img.placeholder.clone()).or_insert(0);
                if *used_count < total_needed {
                    kept.push(img);
                    *used_count += 1;
                }
            }
            self.attached_images = kept;
        }

        (InputResult::None, true)
    }

    /// Attempts to remove an image or paste placeholder if the cursor is at the end of one.
    /// Returns true if a placeholder was removed.
    fn try_remove_any_placeholder_at_cursor(&mut self) -> bool {
        let p = self.textarea.cursor();
        let text = self.textarea.text();

        // Try image placeholders first
        let mut out: Option<(usize, String)> = None;
        // Detect if the cursor is at the end of any image placeholder.
        // If duplicates exist, remove the specific occurrence's mapping.
        for (i, img) in self.attached_images.iter().enumerate() {
            let ph = &img.placeholder;
            if p < ph.len() {
                continue;
            }
            let start = p - ph.len();
            if text[start..p] != *ph {
                continue;
            }

            // Count the number of occurrences of `ph` before `start`.
            let mut occ_before = 0usize;
            let mut search_pos = 0usize;
            while search_pos < start {
                if let Some(found) = text[search_pos..start].find(ph) {
                    occ_before += 1;
                    search_pos += found + ph.len();
                } else {
                    break;
                }
            }

            // Remove the occ_before-th attached image that shares this placeholder label.
            out = if let Some((remove_idx, _)) = self
                .attached_images
                .iter()
                .enumerate()
                .filter(|(_, img2)| img2.placeholder == *ph)
                .nth(occ_before)
            {
                Some((remove_idx, ph.clone()))
            } else {
                Some((i, ph.clone()))
            };
            break;
        }
        if let Some((idx, placeholder)) = out {
            self.textarea.replace_range(p - placeholder.len()..p, "");
            self.attached_images.remove(idx);
            return true;
        }

        // Also handle when the cursor is at the START of an image placeholder.
        // let result = 'out: {
        let out: Option<(usize, String)> = 'out: {
            for (i, img) in self.attached_images.iter().enumerate() {
                let ph = &img.placeholder;
                if p + ph.len() > text.len() {
                    continue;
                }
                if &text[p..p + ph.len()] != ph {
                    continue;
                }

                // Count occurrences of `ph` before `p`.
                let mut occ_before = 0usize;
                let mut search_pos = 0usize;
                while search_pos < p {
                    if let Some(found) = text[search_pos..p].find(ph) {
                        occ_before += 1;
                        search_pos += found + ph.len();
                    } else {
                        break 'out None;
                    }
                }

                if let Some((remove_idx, _)) = self
                    .attached_images
                    .iter()
                    .enumerate()
                    .filter(|(_, img2)| img2.placeholder == *ph)
                    .nth(occ_before)
                {
                    break 'out Some((remove_idx, ph.clone()));
                } else {
                    break 'out Some((i, ph.clone()));
                }
            }
            None
        };

        if let Some((idx, placeholder)) = out {
            self.textarea.replace_range(p..p + placeholder.len(), "");
            self.attached_images.remove(idx);
            return true;
        }

        // Then try pasted-content placeholders
        if let Some(placeholder) = self.pending_pastes.iter().find_map(|(ph, _)| {
            if p < ph.len() {
                return None;
            }
            let start = p - ph.len();
            if text[start..p] == *ph {
                Some(ph.clone())
            } else {
                None
            }
        }) {
            self.textarea.replace_range(p - placeholder.len()..p, "");
            self.pending_pastes.retain(|(ph, _)| ph != &placeholder);
            return true;
        }

        // Also handle when the cursor is at the START of a pasted-content placeholder.
        if let Some(placeholder) = self.pending_pastes.iter().find_map(|(ph, _)| {
            if p + ph.len() > text.len() {
                return None;
            }
            if &text[p..p + ph.len()] == ph {
                Some(ph.clone())
            } else {
                None
            }
        }) {
            self.textarea.replace_range(p..p + placeholder.len(), "");
            self.pending_pastes.retain(|(ph, _)| ph != &placeholder);
            return true;
        }

        false
    }

    /// Synchronize `self.command_popup` with the current text in the
    /// textarea. This must be called after every modification that can change
    /// the text so the popup is shown/updated/hidden as appropriate.
    fn sync_command_popup(&mut self) {
        let first_line = self.textarea.text().lines().next().unwrap_or("");
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

        if !query.is_empty() {
            self.app_event_tx
                .send(AppEvent::StartFileSearch(query.clone()));
        }

        match &mut self.active_popup {
            ActivePopup::File(popup) => {
                if query.is_empty() {
                    popup.set_empty_prompt();
                } else {
                    popup.set_query(&query);
                }
            }
            _ => {
                let mut popup = FileSearchPopup::new();
                if query.is_empty() {
                    popup.set_empty_prompt();
                } else {
                    popup.set_query(&query);
                }
                self.active_popup = ActivePopup::File(popup);
            }
        }

        self.current_file_query = Some(query);
        self.dismissed_file_popup_token = None;
    }

    fn set_has_focus(&mut self, has_focus: bool) {
        self.has_focus = has_focus;
    }

    pub(crate) fn set_esc_backtrack_hint(&mut self, show: bool) {
        self.esc_backtrack_hint = show;
    }
}

impl WidgetRef for &ChatComposer {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let popup_height = match &self.active_popup {
            ActivePopup::Command(popup) => popup.calculate_required_height(),
            ActivePopup::File(popup) => popup.calculate_required_height(),
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
            ActivePopup::None => {
                let bottom_line_rect = popup_rect;
                let key_hint_style = Style::default().fg(Color::Cyan);
                let mut hint = if self.ctrl_c_quit_hint {
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
                        "Ctrl+T".set_style(key_hint_style),
                        Span::from(" transcript   "),
                        "Ctrl+C".set_style(key_hint_style),
                        Span::from(" quit"),
                    ]
                };

                if !self.ctrl_c_quit_hint && self.esc_backtrack_hint {
                    hint.push(Span::from("   "));
                    hint.push("Esc".set_style(key_hint_style));
                    hint.push(Span::from(" edit prev"));
                }

                // Append token/context usage info to the footer hints when available.
                if let Some(token_usage_info) = &self.token_usage_info {
                    let token_usage = &token_usage_info.total_token_usage;
                    hint.push(Span::from("   "));
                    hint.push(
                        Span::from(format!("{} tokens used", token_usage.blended_total()))
                            .style(Style::default().add_modifier(Modifier::DIM)),
                    );
                    let last_token_usage = &token_usage_info.last_token_usage;
                    if let Some(context_window) = token_usage_info.model_context_window {
                        let percent_remaining: u8 = if context_window > 0 {
                            last_token_usage.percent_of_context_window_remaining(
                                context_window,
                                token_usage_info.initial_prompt_tokens,
                            )
                        } else {
                            100
                        };
                        hint.push(Span::from("   "));
                        hint.push(
                            Span::from(format!("{percent_remaining}% context left"))
                                .style(Style::default().add_modifier(Modifier::DIM)),
                        );
                    }
                }

                Line::from(hint)
                    .style(Style::default().dim())
                    .render_ref(bottom_line_rect, buf);
            }
        }
        let border_style = if self.has_focus {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        Block::default()
            .borders(Borders::LEFT)
            .border_type(BorderType::QuadrantOutside)
            .border_style(border_style)
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
            Line::from(self.placeholder_text.as_str())
                .style(Style::default().dim())
                .render_ref(textarea_rect.inner(Margin::new(1, 0)), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::app_event::AppEvent;
    use crate::bottom_pane::AppEventSender;
    use crate::bottom_pane::ChatComposer;
    use crate::bottom_pane::InputResult;
    use crate::bottom_pane::chat_composer::AttachedImage;
    use crate::bottom_pane::chat_composer::LARGE_PASTE_CHAR_THRESHOLD;
    use crate::bottom_pane::textarea::TextArea;
    use tokio::sync::mpsc::unbounded_channel;

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
            (
                "@",
                1,
                Some("".to_string()),
                "Only @ symbol triggers empty query",
            ),
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
            ("@", 0, Some("".to_string()), "Only @ symbol"),
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

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

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

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

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
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

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

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut terminal = match Terminal::new(TestBackend::new(100, 10)) {
            Ok(t) => t,
            Err(e) => panic!("Failed to create terminal: {e}"),
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
            let mut composer = ChatComposer::new(
                true,
                sender.clone(),
                false,
                "Ask Codex to do anything".to_string(),
            );

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

            terminal
                .draw(|f| f.render_widget_ref(&composer, f.area()))
                .unwrap_or_else(|e| panic!("Failed to draw {name} composer: {e}"));

            assert_snapshot!(name, terminal.backend());
        }
    }

    #[test]
    fn slash_init_dispatches_command_and_does_not_submit_literal_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        // Type the slash command.
        for ch in [
            '/', 'i', 'n', 'i', 't', // "/init"
        ] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        // Press Enter to dispatch the selected command.
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // When a slash command is dispatched, the composer should return a
        // Command result (not submit literal text) and clear its textarea.
        match result {
            InputResult::Command(cmd) => {
                assert_eq!(cmd.command(), "init");
            }
            InputResult::Submitted(text) => {
                panic!("expected command dispatch, but composer submitted literal text: {text}")
            }
            InputResult::None => panic!("expected Command result for '/init'"),
        }
        assert!(composer.textarea.is_empty(), "composer should be cleared");
    }

    #[test]
    fn slash_tab_completion_moves_cursor_to_end() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        for ch in ['/', 'c'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        let (_result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(composer.textarea.text(), "/compact ");
        assert_eq!(composer.textarea.cursor(), composer.textarea.text().len());
    }

    #[test]
    fn slash_mention_dispatches_command_and_inserts_at() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        for ch in ['/', 'm', 'e', 'n', 't', 'i', 'o', 'n'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match result {
            InputResult::Command(cmd) => {
                assert_eq!(cmd.command(), "mention");
            }
            InputResult::Submitted(text) => {
                panic!("expected command dispatch, but composer submitted literal text: {text}")
            }
            InputResult::None => panic!("expected Command result for '/mention'"),
        }
        assert!(composer.textarea.is_empty(), "composer should be cleared");
        composer.insert_str("@");
        assert_eq!(composer.textarea.text(), "@");
    }

    #[test]
    fn test_multiple_pastes_submission() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        // Define test cases: (paste content, is_large)
        let test_cases = [
            ("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 3), true),
            (" and ".to_string(), false),
            ("y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 7), true),
        ];

        // Expected states after each paste
        let mut expected_text = String::new();
        let mut expected_pending_count = 0;

        // Apply all pastes and build expected state
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

        // Verify all intermediate states were correct
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

        // Submit and verify final expansion
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        if let InputResult::Submitted(text) = result {
            assert_eq!(text, format!("{} and {}", test_cases[0].0, test_cases[2].0));
        } else {
            panic!("expected Submitted");
        }
    }

    #[test]
    fn test_placeholder_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        // Define test cases: (content, is_large)
        let test_cases = [
            ("a".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5), true),
            (" and ".to_string(), false),
            ("b".repeat(LARGE_PASTE_CHAR_THRESHOLD + 6), true),
        ];

        // Apply all pastes
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

        // Delete placeholders one by one and collect states
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

        // Verify all states
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

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        // Define test cases: (cursor_position_from_end, expected_pending_count)
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

    // --- Image attachment tests ---
    #[test]
    fn attach_image_and_submit_includes_image_paths() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());
        let path = PathBuf::from("/tmp/image1.png");
        composer.attach_image(path.clone(), 32, 16, "PNG");
        composer.handle_paste(" hi".into());
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, "hi"),
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(vec![path], imgs);
    }

    #[test]
    fn attach_image_without_text_submits_empty_text_and_images() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());
        let path = PathBuf::from("/tmp/image2.png");
        composer.attach_image(path.clone(), 10, 5, "PNG");
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert!(text.is_empty()),
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0], path);
        assert!(composer.attached_images.is_empty());
    }

    #[test]
    fn image_placeholder_backspace_behaves_like_text_placeholder() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());
        let path = PathBuf::from("/tmp/image3.png");
        composer.attach_image(path.clone(), 20, 10, "PNG");
        let placeholder = composer.attached_images[0].placeholder.clone();

        // Case 1: backspace at end
        composer.textarea.move_cursor_to_end_of_line(false);
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(!composer.textarea.text().contains(&placeholder));
        assert!(composer.attached_images.is_empty());

        // Re-add and test backspace in middle: should break the placeholder string
        // and drop the image mapping (same as text placeholder behavior).
        composer.attach_image(path.clone(), 20, 10, "PNG");
        let placeholder2 = composer.attached_images[0].placeholder.clone();
        // Move cursor to roughly middle of placeholder
        if let Some(start_pos) = composer.textarea.text().find(&placeholder2) {
            let mid_pos = start_pos + (placeholder2.len() / 2);
            composer.textarea.set_cursor(mid_pos);
            composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            assert!(!composer.textarea.text().contains(&placeholder2));
            assert!(composer.attached_images.is_empty());
        } else {
            panic!("Placeholder not found in textarea");
        }
    }

    #[test]
    fn deleting_one_of_duplicate_image_placeholders_removes_matching_entry() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer =
            ChatComposer::new(true, sender, false, "Ask Codex to do anything".to_string());

        let path1 = PathBuf::from("/tmp/image_dup1.png");
        let path2 = PathBuf::from("/tmp/image_dup2.png");

        composer.attach_image(path1.clone(), 10, 5, "PNG");
        // separate placeholders with a space for clarity
        composer.handle_paste(" ".into());
        composer.attach_image(path2.clone(), 10, 5, "PNG");

        let ph = composer.attached_images[0].placeholder.clone();
        let text = composer.textarea.text().to_string();
        let start1 = text.find(&ph).expect("first placeholder present");
        let end1 = start1 + ph.len();
        composer.textarea.set_cursor(end1);

        // Backspace should delete the first placeholder and its mapping.
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        let new_text = composer.textarea.text().to_string();
        assert_eq!(1, new_text.matches(&ph).count(), "one placeholder remains");
        assert_eq!(
            vec![AttachedImage {
                path: path2,
                placeholder: "[image 10x5 PNG]".to_string()
            }],
            composer.attached_images,
            "one image mapping remains"
        );
    }
}
