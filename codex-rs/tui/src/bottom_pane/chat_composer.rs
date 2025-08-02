use codex_core::protocol::TokenUsage;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
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
use std::path::Path;

const BASE_PLACEHOLDER_TEXT: &str = "...";
/// If the pasted content exceeds this number of characters, replace it with a
/// placeholder in the UI.
const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

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
    use_shift_enter_hint: bool,
    dismissed_file_popup_token: Option<String>,
    current_file_query: Option<String>,
    pending_pastes: Vec<(String, String)>,
    attached_images: Vec<(String, std::path::PathBuf)>,
    recent_submission_images: Vec<std::path::PathBuf>,
    /// When true we are in an explicit file search session initiated via @file.
    file_search_mode: bool,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Slash(CommandPopup),
    File(FileSearchPopup),
}

impl ChatComposer<'_> {
    pub fn new(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
    ) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text(BASE_PLACEHOLDER_TEXT);
        textarea.set_cursor_line_style(ratatui::style::Style::default());

        let use_shift_enter_hint = enhanced_keys_supported;

        let mut this = Self {
            textarea,
            active_popup: ActivePopup::None,
            app_event_tx,
            history: ChatComposerHistory::new(),
            ctrl_c_quit_hint: false,
            use_shift_enter_hint,
            dismissed_file_popup_token: None,
            current_file_query: None,
            pending_pastes: Vec::new(),
            attached_images: Vec::new(),
            recent_submission_images: Vec::new(),
            file_search_mode: false,
        };
        this.update_border(has_input_focus);
        this
    }

    pub fn desired_height(&self) -> u16 {
        self.textarea.lines().len().max(1) as u16
            + match &self.active_popup {
                ActivePopup::None => 1u16,
                ActivePopup::Slash(c) => c.calculate_required_height(),
                ActivePopup::File(c) => c.calculate_required_height(),
            }
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
                // When https://github.com/openai/codex/issues/1257 is resolved,
                // check if `percent_remaining < 25`, and if so, recommend
                // /compact.
                format!("{BASE_PLACEHOLDER_TEXT} — {percent_remaining}% context left")
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

    pub fn handle_paste(&mut self, pasted: String) -> bool {
        let char_count = pasted.chars().count();
        if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = format!("[Pasted Content {char_count} chars]");
            self.textarea.insert_str(&placeholder);
            self.pending_pastes.push((placeholder, pasted));
        } else {
            self.textarea.insert_str(&pasted);
        }
        self.sync_slash_command_popup();
        self.sync_file_search_popup();
        true
    }

    pub fn attach_image(
        &mut self,
        path: std::path::PathBuf,
        width: u32,
        height: u32,
        format_label: &str,
    ) -> bool {
        let placeholder = format!("[image {width}x{height} {format_label}]");
        self.textarea.insert_str(&placeholder);
        self.attached_images.push((placeholder, path));
        true
    }

    pub fn take_recent_submission_images(&mut self) -> Vec<std::path::PathBuf> {
        std::mem::take(&mut self.recent_submission_images)
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
            ActivePopup::Slash(_) => self.handle_key_event_with_slash_popup(key_event),
            ActivePopup::File(_) => self.handle_key_event_with_file_popup(key_event),
            ActivePopup::None => self.handle_key_event_without_popup(key_event),
        };

        // Update (or hide/show) popup after processing the key.
        self.sync_slash_command_popup();
        if matches!(self.active_popup, ActivePopup::Slash(_)) {
            self.dismissed_file_popup_token = None;
        } else {
            self.sync_file_search_popup();
        }

        result
    }

    /// Handle key event when the slash-command popup is visible.
    fn handle_key_event_with_slash_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::Slash(popup) = &mut self.active_popup else {
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
                self.file_search_mode = false; // end session
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
                    // If selected path looks like an image (png/jpeg), attach as image instead of inserting text.
                    let is_image = {
                        let lower = sel_path.to_ascii_lowercase();
                        lower.ends_with(".png")
                            || lower.ends_with(".jpg")
                            || lower.ends_with(".jpeg")
                    };
                    if is_image {
                        // Determine dimensions; if that fails fall back to normal path insertion.
                        let path_buf = std::path::PathBuf::from(&sel_path);
                        match image::image_dimensions(&path_buf) {
                            Ok((w, h)) => {
                                // Remove the current @token (mirror logic from insert_selected_path without inserting text)
                                let (row, col) = self.textarea.cursor();
                                let mut lines: Vec<String> = self.textarea.lines().to_vec();
                                if let Some(line) = lines.get_mut(row) {
                                    let cursor_byte_offset = cursor_byte_offset(line, col);
                                    if let Some((start, end)) =
                                        at_token_bounds(line, cursor_byte_offset, true)
                                    {
                                        let mut new_line =
                                            String::with_capacity(line.len() - (end - start));
                                        new_line.push_str(&line[..start]);
                                        new_line.push_str(&line[end..]);
                                        *line = new_line;
                                        let new_text = lines.join("\n");
                                        self.textarea.select_all();
                                        self.textarea.cut();
                                        let _ = self.textarea.insert_str(new_text);
                                    }
                                }
                                let format_label = match Path::new(&sel_path)
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .map(|s| s.to_ascii_lowercase())
                                {
                                    Some(ext) if ext == "png" => "PNG",
                                    Some(ext) if ext == "jpg" || ext == "jpeg" => "JPEG",
                                    _ => "IMG",
                                };
                                self.app_event_tx.send(AppEvent::AttachImage {
                                    path: path_buf.clone(),
                                    width: w,
                                    height: h,
                                    format_label,
                                });
                                tracing::info!(
                                    "file_search_image selected path={:?} width={} height={} format={}",
                                    path_buf,
                                    w,
                                    h,
                                    format_label
                                );
                                // Optionally add a trailing space to keep typing fluid.
                                let _ = self.textarea.insert_str(" ");
                            }
                            Err(_) => {
                                // Fallback to plain path insertion if metadata read fails.
                                self.insert_selected_path(&sel_path);
                            }
                        }
                    } else {
                        // Non-image: original behavior.
                        self.insert_selected_path(&sel_path);
                    }
                    self.active_popup = ActivePopup::None;
                    self.file_search_mode = false; // end session on selection
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
        let line = textarea.lines().get(row)?.as_str();
        let cursor_byte_offset = cursor_byte_offset(line, col);
        let (start, end) = at_token_bounds(line, cursor_byte_offset, false)?;
        Some(line[start + 1..end].to_string())
    }

    /// Similar to `current_at_token` but returns Some("") if cursor is on a bare '@' token (no body yet).
    fn current_at_token_allow_empty(textarea: &tui_textarea::TextArea) -> Option<String> {
        let (row, col) = textarea.cursor();
        let line = textarea.lines().get(row)?.as_str();
        let cursor_byte_offset = cursor_byte_offset(line, col);
        let (start, end) = at_token_bounds(line, cursor_byte_offset, true)?;
        Some(line[start + 1..end].to_string()) // body may be empty
    }

    /// Replace the active `@token` (the one under the cursor) with `path`.
    /// Mirrors legacy logic using new shared helpers.
    fn insert_selected_path(&mut self, path: &str) {
        let (row, col) = self.textarea.cursor();
        let mut lines: Vec<String> = self.textarea.lines().to_vec();
        if let Some(line) = lines.get_mut(row) {
            let cursor_byte_offset = cursor_byte_offset(line, col);
            if let Some((start, end)) = token_bounds(line, cursor_byte_offset) {
                let mut new_line =
                    String::with_capacity(line.len() - (end - start) + path.len() + 1);
                new_line.push_str(&line[..start]);
                new_line.push_str(path);
                new_line.push(' ');
                new_line.push_str(&line[end..]);
                *line = new_line;
                let new_text = lines.join("\n");
                self.textarea.select_all();
                self.textarea.cut();
                let _ = self.textarea.insert_str(new_text);
            }
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
                let mut text = self.textarea.lines().join("\n");
                self.textarea.select_all();
                self.textarea.cut();

                // Replace all pending pastes in the text
                for (placeholder, actual) in &self.pending_pastes {
                    if text.contains(placeholder) {
                        text = text.replace(placeholder, actual);
                    }
                }
                self.pending_pastes.clear();

                // If removing all image placeholders leaves only whitespace, treat as empty (no submission).
                let mut content_without_images = text.clone();
                for (placeholder, _) in &self.attached_images {
                    content_without_images = content_without_images.replace(placeholder, "");
                }
                if content_without_images.trim().is_empty() {
                    return (InputResult::None, true);
                }

                // Consume image placeholders and stage their paths (text now guaranteed non-empty after removal).
                let mut attached_paths = Vec::new();
                for (placeholder, path) in &self.attached_images {
                    if text.contains(placeholder) {
                        text = text.replace(placeholder, "");
                        attached_paths.push(path.clone());
                    }
                }
                if !attached_paths.is_empty() {
                    self.recent_submission_images = attached_paths;
                    text = text.trim().to_string();
                }

                if text.is_empty() {
                    (InputResult::None, true)
                } else {
                    self.history.record_local_submission(&text);
                    self.attached_images.clear();
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
            Input {
                key: Key::Char('d'),
                ctrl: true,
                alt: false,
                shift: false,
            } => {
                self.textarea.input(Input {
                    key: Key::Delete,
                    ctrl: false,
                    alt: false,
                    shift: false,
                });
                (InputResult::None, true)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Handle generic Input events that modify the textarea content.
    fn handle_input_basic(&mut self, input: Input) -> (InputResult, bool) {
        // Special handling for backspace on placeholders
        if let Input {
            key: Key::Backspace,
            ..
        } = input
        {
            // First try image placeholders (any backspace inside one removes it entirely)
            if self.try_remove_image_placeholder_on_backspace() {
                return (InputResult::None, true);
            }
            // Then try pasted-content placeholders (only when at end)
            if self.try_remove_placeholder_at_cursor() {
                return (InputResult::None, true);
            }
        }

        if let Input {
            key: Key::Char('u'),
            ctrl: true,
            alt: false,
            ..
        } = input
        {
            self.textarea.delete_line_by_head();
            return (InputResult::None, true);
        }

        // Normal input handling
        self.textarea.input(input);
        let text_after = self.textarea.lines().join("\n");

        // Start/continue an explicit file-search session when the cursor is on an @token.
        if Self::current_at_token_allow_empty(&self.textarea).is_some() {
            self.file_search_mode = true;
            // Allow popup to show for this token.
            self.dismissed_file_popup_token = None;
        }

        // Check if any placeholders were removed and remove their corresponding pending pastes
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));

        (InputResult::None, true)
    }

    /// Attempts to remove a placeholder if the cursor is at the end of one.
    /// Returns true if a placeholder was removed.
    fn try_remove_placeholder_at_cursor(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();
        let line = self
            .textarea
            .lines()
            .get(row)
            .map(|s| s.as_str())
            .unwrap_or("");

        // Find any placeholder that ends at the cursor position
        let placeholder_to_remove = self.pending_pastes.iter().find_map(|(ph, _)| {
            if col < ph.len() {
                return None;
            }
            let potential_ph_start = col - ph.len();
            if line[potential_ph_start..col] == *ph {
                Some(ph.clone())
            } else {
                None
            }
        });

        if let Some(placeholder) = placeholder_to_remove {
            // Remove the entire placeholder from the text
            let placeholder_len = placeholder.len();
            for _ in 0..placeholder_len {
                self.textarea.input(Input {
                    key: Key::Backspace,
                    ctrl: false,
                    alt: false,
                    shift: false,
                });
            }
            // Remove from pending pastes
            self.pending_pastes.retain(|(ph, _)| ph != &placeholder);
            true
        } else {
            false
        }
    }

    /// Attempts to remove an attached image placeholder if a backspace occurs *anywhere* inside it.
    /// Returns true if a placeholder + image mapping was removed.
    fn try_remove_image_placeholder_on_backspace(&mut self) -> bool {
        if self.attached_images.is_empty() {
            return false;
        }

        // Materialize full text and compute global cursor + deletion indices.
        let lines: Vec<String> = self.textarea.lines().to_vec();
        let (cursor_row, cursor_col) = self.textarea.cursor();

        // Compute global char index of cursor (in characters, since placeholders are ASCII).
        let mut global_index: usize = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == cursor_row {
                global_index += cursor_col;
                break;
            } else {
                global_index += line.chars().count() + 1; // +1 for the newline that will be joined
            }
        }
        if global_index == 0 {
            return false;
        }
        let deletion_index = global_index - 1; // char that will be removed by backspace

        let text = lines.join("\n");

        // Iterate over attached images; search each placeholder occurrence.
        for idx in 0..self.attached_images.len() {
            let (placeholder, _path) = &self.attached_images[idx];
            let ph_len = placeholder.len();
            let mut search_from = 0;
            while let Some(rel_pos) = text[search_from..].find(placeholder) {
                let ph_start = search_from + rel_pos;
                let ph_end = ph_start + ph_len; // exclusive
                if deletion_index >= ph_start && deletion_index < ph_end {
                    // Deletion inside this placeholder: remove entire placeholder.
                    let mut new_text = String::with_capacity(text.len() - ph_len);
                    new_text.push_str(&text[..ph_start]);
                    new_text.push_str(&text[ph_end..]);

                    // Replace textarea contents.
                    self.textarea.select_all();
                    self.textarea.cut();
                    let _ = self.textarea.insert_str(new_text);

                    // Remove attached image entry.
                    self.attached_images.remove(idx);
                    return true;
                }
                search_from = ph_start + ph_len; // continue searching for additional occurrences
            }
        }
        false
    }

    /// Synchronize `self.command_popup` with the current text in the
    /// textarea. This must be called after every modification that can change
    /// the text so the popup is shown/updated/hidden as appropriate.
    fn sync_slash_command_popup(&mut self) {
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
            ActivePopup::Slash(popup) => {
                if input_starts_with_slash {
                    popup.on_composer_text_change(first_line.to_string());
                } else {
                    self.active_popup = ActivePopup::None;
                }
            }
            _ => {
                if input_starts_with_slash {
                    let mut command_popup = CommandPopup::slash();
                    command_popup.on_composer_text_change(first_line.to_string());
                    self.active_popup = ActivePopup::Slash(command_popup);
                }
            }
        }
    }

    /// Synchronize `self.file_search_popup` with the current text in the textarea.
    /// Note this is only called when self.active_popup is NOT Command.
    fn sync_file_search_popup(&mut self) {
        // Only active during an explicit @file initiated session.
        if !self.file_search_mode {
            return;
        }

        // Determine current query (may be empty if user just selected @file and hasn't typed yet).
        let query_opt = Self::current_at_token_allow_empty(&self.textarea);
        let Some(query) = query_opt else {
            // Token removed – end session.
            self.active_popup = ActivePopup::None;
            self.dismissed_file_popup_token = None;
            self.file_search_mode = false;
            return;
        };

        // If user dismissed popup for this exact query, don't reopen until text changes.
        if self.dismissed_file_popup_token.as_ref() == Some(&query) {
            return;
        }
        // Only trigger a search when query non-empty. (Empty shows an idle popup.)
        if !query.is_empty() {
            self.app_event_tx
                .send(AppEvent::StartFileSearch(query.clone()));
        }

        match &mut self.active_popup {
            ActivePopup::File(popup) => {
                if !query.is_empty() {
                    popup.set_query(&query);
                }
            }
            _ => {
                let mut popup = FileSearchPopup::new();
                if !query.is_empty() {
                    popup.set_query(&query);
                }
                self.active_popup = ActivePopup::File(popup);
            }
        }

        self.current_file_query = Some(query);
        self.dismissed_file_popup_token = None;
    }

    fn update_border(&mut self, has_focus: bool) {
        let border_style = if has_focus {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().dim()
        };

        self.textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(Borders::LEFT)
                .border_type(BorderType::QuadrantOutside)
                .border_style(border_style),
        );
    }
}

impl WidgetRef for &ChatComposer<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match &self.active_popup {
            ActivePopup::Slash(popup) => {
                let popup_height = popup.calculate_required_height();

                // Split the provided rect so that the popup is rendered at the
                // **bottom** and the textarea occupies the remaining space above.
                let popup_height = popup_height.min(area.height);
                let textarea_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height.saturating_sub(popup_height),
                };
                let popup_rect = Rect {
                    x: area.x,
                    y: area.y + textarea_rect.height,
                    width: area.width,
                    height: popup_height,
                };

                popup.render(popup_rect, buf);
                self.textarea.render(textarea_rect, buf);
            }
            ActivePopup::File(popup) => {
                let popup_height = popup.calculate_required_height();

                let popup_height = popup_height.min(area.height);
                let textarea_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height.saturating_sub(popup_height),
                };
                let popup_rect = Rect {
                    x: area.x,
                    y: area.y + textarea_rect.height,
                    width: area.width,
                    height: popup_height,
                };

                popup.render(popup_rect, buf);
                self.textarea.render(textarea_rect, buf);
            }
            ActivePopup::None => {
                let mut textarea_rect = area;
                textarea_rect.height = textarea_rect.height.saturating_sub(1);
                self.textarea.render(textarea_rect, buf);
                let mut bottom_line_rect = area;
                bottom_line_rect.y += textarea_rect.height;
                bottom_line_rect.height = 1;
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
    }
}

// -----------------------------------------------------------------------------
// Shared helper functions for token boundary calculations.
// Centralizing these reduces subtle divergence between behaviors that rely on
// the exact same definition of a *token* (whitespace-delimited) and *@token*.
// -----------------------------------------------------------------------------

/// Convert a cursor column expressed in characters (as provided by tui-textarea)
/// to a byte offset into `line`.
fn cursor_byte_offset(line: &str, cursor_col_chars: usize) -> usize {
    line.chars()
        .take(cursor_col_chars)
        .map(|c| c.len_utf8())
        .sum()
}

/// Return (start_byte, end_byte) of the token (whitespace-delimited) containing
/// `cursor_byte_offset`. Returns None if there is no non-empty token.
fn token_bounds(line: &str, cursor_byte_offset: usize) -> Option<(usize, usize)> {
    if cursor_byte_offset > line.len() {
        return None;
    }
    let before = &line[..cursor_byte_offset];
    let after = &line[cursor_byte_offset..];
    let start = before
        .char_indices()
        .rfind(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let end_rel = after
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(after.len());
    let end = cursor_byte_offset + end_rel;
    if start >= end {
        None
    } else {
        Some((start, end))
    }
}

/// Like `token_bounds` but ensures the token starts with '@'. If `allow_empty`
/// is false, requires at least one character after '@'. Returns byte bounds.
fn at_token_bounds(
    line: &str,
    cursor_byte_offset: usize,
    allow_empty: bool,
) -> Option<(usize, usize)> {
    let (start, end) = token_bounds(line, cursor_byte_offset)?;
    let token = &line[start..end];
    if token.starts_with('@') && (allow_empty || token.len() > 1) {
        Some((start, end))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ActivePopup;
    use crate::bottom_pane::AppEventSender;
    use crate::bottom_pane::ChatComposer;
    use crate::bottom_pane::InputResult;
    use crate::bottom_pane::chat_composer::LARGE_PASTE_CHAR_THRESHOLD;
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
        assert_eq!(composer.textarea.lines(), ["hello"]);
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
        assert_eq!(composer.textarea.lines(), [placeholder.as_str()]);
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
                composer.textarea.move_cursor(tui_textarea::CursorMove::End);
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }

            terminal
                .draw(|f| f.render_widget_ref(&composer, f.area()))
                .unwrap_or_else(|e| panic!("Failed to draw {name} composer: {e}"));

            assert_snapshot!(name, terminal.backend());
        }
    }

    #[test]
    fn test_multiple_pastes_submission() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

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

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

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
                    composer.textarea.lines().join("\n"),
                    composer.pending_pastes.len(),
                    current_pos,
                )
            })
            .collect();

        // Delete placeholders one by one and collect states
        let mut deletion_states = vec![];

        // First deletion
        composer
            .textarea
            .move_cursor(tui_textarea::CursorMove::Jump(0, states[0].2 as u16));
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        deletion_states.push((
            composer.textarea.lines().join("\n"),
            composer.pending_pastes.len(),
        ));

        // Second deletion
        composer
            .textarea
            .move_cursor(tui_textarea::CursorMove::Jump(
                0,
                composer.textarea.lines().join("\n").len() as u16,
            ));
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        deletion_states.push((
            composer.textarea.lines().join("\n"),
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

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

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
                    .move_cursor(tui_textarea::CursorMove::Jump(
                        0,
                        (placeholder.len() - pos_from_end) as u16,
                    ));
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                let result = (
                    composer.textarea.lines().join("\n").contains(&placeholder),
                    composer.pending_pastes.len(),
                );
                composer.textarea.select_all();
                composer.textarea.cut();
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
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);
        let path = std::path::PathBuf::from("/tmp/image1.png");
        assert!(composer.attach_image(path.clone(), 32, 16, "PNG"));
        composer.handle_paste(" hi".into());
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted(text) => assert_eq!(text, "hi"),
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0], path);
    }

    #[test]
    fn attach_image_without_text_not_submitted() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);
        let path = std::path::PathBuf::from("/tmp/image2.png");
        assert!(composer.attach_image(path.clone(), 10, 5, "PNG"));
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, InputResult::None));
        assert!(composer.take_recent_submission_images().is_empty());
        assert_eq!(composer.attached_images.len(), 1); // still pending
    }

    #[test]
    fn image_placeholder_removed_on_backspace_anywhere() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);
        let path = std::path::PathBuf::from("/tmp/image3.png");
        assert!(composer.attach_image(path.clone(), 20, 10, "PNG"));
        let placeholder = composer.attached_images[0].0.clone();

        // Case 1: backspace at end
        composer.textarea.move_cursor(tui_textarea::CursorMove::End);
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(!composer.textarea.lines().join("\n").contains(&placeholder));
        assert!(composer.attached_images.is_empty());

        // Re-add and test backspace in middle
        assert!(composer.attach_image(path.clone(), 20, 10, "PNG"));
        let placeholder2 = composer.attached_images[0].0.clone();
        // Move cursor to roughly middle of placeholder
        let mid = (placeholder2.len() / 2) as u16;
        composer
            .textarea
            .move_cursor(tui_textarea::CursorMove::Jump(0, mid));
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(!composer.textarea.lines().join("\n").contains(&placeholder2));
        assert!(composer.attached_images.is_empty());
    }

    #[test]
    fn at_symbol_opens_file_popup_and_enter_closes_it() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = std::sync::mpsc::channel();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(true, sender, false);

        // Type '@' to open file search popup
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE));
        // Popup should be the File popup, and we should be in file_search_mode
        assert!(matches!(composer.active_popup, ActivePopup::File(_)));
        assert!(composer.file_search_mode);

        // Press Enter to select current item and close popup/session
        composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(composer.active_popup, ActivePopup::None));
        assert!(!composer.file_search_mode);
    }
}
