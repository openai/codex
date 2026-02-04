//! UI-specific state.
//!
//! This module contains state that is local to the TUI and not
//! synchronized with the agent.

use std::time::Duration;
use std::time::Instant;

use cocode_protocol::ApprovalRequest;

use crate::theme::Theme;
use crate::widgets::Toast;

/// UI-specific state.
#[derive(Debug, Clone, Default)]
pub struct UiState {
    /// The current input state.
    pub input: InputState,

    /// Scroll offset in the chat history.
    pub scroll_offset: i32,

    /// Current focus target.
    pub focus: FocusTarget,

    /// Active overlay (modal dialog).
    pub overlay: Option<Overlay>,

    /// Streaming content state.
    pub streaming: Option<StreamingState>,

    /// File autocomplete state (shown when typing @path).
    pub file_suggestions: Option<FileSuggestionState>,

    /// Skill autocomplete state (shown when typing /command).
    pub skill_suggestions: Option<SkillSuggestionState>,

    /// Whether to show thinking content in chat messages.
    pub show_thinking: bool,

    /// Whether the user has manually scrolled (disables auto-scroll).
    pub user_scrolled: bool,

    /// When thinking started (for duration calculation).
    pub thinking_started_at: Option<Instant>,

    /// Duration of the last completed thinking phase.
    pub last_thinking_duration: Option<Duration>,

    /// Whether the terminal window is focused.
    pub terminal_focused: bool,

    /// Current theme.
    pub theme: Theme,

    /// Active toast notifications.
    pub toasts: Vec<Toast>,

    /// Animation frame counter (0-7 cycle) for animated elements.
    pub animation_frame: u8,

    /// Toast ID counter for generating unique IDs.
    toast_id_counter: i32,
}

impl UiState {
    /// Set the overlay.
    pub fn set_overlay(&mut self, overlay: Overlay) {
        self.overlay = Some(overlay);
    }

    /// Clear the overlay.
    pub fn clear_overlay(&mut self) {
        self.overlay = None;
    }

    /// Start streaming.
    pub fn start_streaming(&mut self, turn_id: String) {
        self.streaming = Some(StreamingState::new(turn_id));
    }

    /// Stop streaming.
    pub fn stop_streaming(&mut self) {
        self.streaming = None;
    }

    /// Append to streaming content.
    pub fn append_streaming(&mut self, delta: &str) {
        if let Some(ref mut streaming) = self.streaming {
            streaming.content.push_str(delta);
        }
    }

    /// Append to streaming thinking content.
    pub fn append_streaming_thinking(&mut self, delta: &str) {
        if let Some(ref mut streaming) = self.streaming {
            streaming.thinking.push_str(delta);
        }
    }

    /// Check if file suggestions are active.
    pub fn has_file_suggestions(&self) -> bool {
        self.file_suggestions.is_some()
    }

    /// Start showing file suggestions.
    pub fn start_file_suggestions(&mut self, query: String, start_pos: i32) {
        self.file_suggestions = Some(FileSuggestionState::new(query, start_pos));
    }

    /// Clear file suggestions.
    pub fn clear_file_suggestions(&mut self) {
        self.file_suggestions = None;
    }

    /// Update file suggestions with search results.
    pub fn update_file_suggestions(&mut self, suggestions: Vec<FileSuggestionItem>) {
        if let Some(ref mut state) = self.file_suggestions {
            state.update_suggestions(suggestions);
        }
    }

    /// Toggle display of thinking content.
    pub fn toggle_thinking(&mut self) {
        self.show_thinking = !self.show_thinking;
        tracing::debug!(
            show_thinking = self.show_thinking,
            "Toggled thinking display"
        );
    }

    /// Check if skill suggestions are active.
    pub fn has_skill_suggestions(&self) -> bool {
        self.skill_suggestions.is_some()
    }

    /// Start showing skill suggestions.
    pub fn start_skill_suggestions(&mut self, query: String, start_pos: i32) {
        self.skill_suggestions = Some(SkillSuggestionState::new(query, start_pos));
    }

    /// Clear skill suggestions.
    pub fn clear_skill_suggestions(&mut self) {
        self.skill_suggestions = None;
    }

    /// Update skill suggestions with search results.
    pub fn update_skill_suggestions(&mut self, suggestions: Vec<SkillSuggestionItem>) {
        if let Some(ref mut state) = self.skill_suggestions {
            state.update_suggestions(suggestions);
        }
    }

    /// Mark that the user has manually scrolled.
    pub fn mark_user_scrolled(&mut self) {
        self.user_scrolled = true;
    }

    /// Reset scroll state for auto-scroll (e.g., when user sends a message).
    pub fn reset_user_scrolled(&mut self) {
        self.user_scrolled = false;
    }

    /// Start the thinking timer.
    pub fn start_thinking(&mut self) {
        if self.thinking_started_at.is_none() {
            self.thinking_started_at = Some(Instant::now());
        }
    }

    /// Stop the thinking timer and record the duration.
    pub fn stop_thinking(&mut self) {
        if let Some(started_at) = self.thinking_started_at.take() {
            self.last_thinking_duration = Some(started_at.elapsed());
        }
    }

    /// Get the current thinking duration (either elapsed or last completed).
    pub fn thinking_duration(&self) -> Option<Duration> {
        if let Some(started_at) = self.thinking_started_at {
            Some(started_at.elapsed())
        } else {
            self.last_thinking_duration
        }
    }

    /// Check if currently thinking.
    pub fn is_thinking(&self) -> bool {
        self.thinking_started_at.is_some()
    }

    /// Clear the last thinking duration (e.g., when starting a new turn).
    pub fn clear_thinking_duration(&mut self) {
        self.last_thinking_duration = None;
    }

    /// Set terminal focus state.
    pub fn set_terminal_focused(&mut self, focused: bool) {
        self.terminal_focused = focused;
        tracing::debug!(focused, "Terminal focus changed");
    }

    // ========== Toast Management ==========

    /// Add a toast notification.
    pub fn add_toast(&mut self, toast: Toast) {
        // Limit to max 5 toasts
        const MAX_TOASTS: usize = 5;
        if self.toasts.len() >= MAX_TOASTS {
            self.toasts.remove(0);
        }
        self.toasts.push(toast);
    }

    /// Add an info toast.
    pub fn toast_info(&mut self, message: impl Into<String>) {
        self.toast_id_counter += 1;
        let toast = Toast::info(format!("toast-{}", self.toast_id_counter), message);
        self.add_toast(toast);
    }

    /// Add a success toast.
    pub fn toast_success(&mut self, message: impl Into<String>) {
        self.toast_id_counter += 1;
        let toast = Toast::success(format!("toast-{}", self.toast_id_counter), message);
        self.add_toast(toast);
    }

    /// Add a warning toast.
    pub fn toast_warning(&mut self, message: impl Into<String>) {
        self.toast_id_counter += 1;
        let toast = Toast::warning(format!("toast-{}", self.toast_id_counter), message);
        self.add_toast(toast);
    }

    /// Add an error toast.
    pub fn toast_error(&mut self, message: impl Into<String>) {
        self.toast_id_counter += 1;
        let toast = Toast::error(format!("toast-{}", self.toast_id_counter), message);
        self.add_toast(toast);
    }

    /// Remove expired toasts.
    pub fn expire_toasts(&mut self) {
        self.toasts.retain(|toast| !toast.is_expired());
    }

    /// Check if there are any active toasts.
    pub fn has_toasts(&self) -> bool {
        !self.toasts.is_empty()
    }

    // ========== Animation ==========

    /// Increment the animation frame.
    pub fn tick_animation(&mut self) {
        self.animation_frame = (self.animation_frame + 1) % 8;
    }

    /// Get the current animation frame (0-7).
    pub fn animation_frame(&self) -> u8 {
        self.animation_frame
    }
}

/// State for the input field.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// The current input text.
    pub text: String,

    /// Cursor position (character index).
    pub cursor: i32,

    /// Selection start (if any).
    pub selection_start: Option<i32>,

    /// History of previous inputs with frecency scores.
    pub history: Vec<HistoryEntry>,

    /// Current history index (for up/down navigation).
    pub history_index: Option<i32>,
}

/// A history entry with frecency scoring.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// The command text.
    pub text: String,
    /// Number of times this command was used.
    pub frequency: i32,
    /// Unix timestamp of last use.
    pub last_used: i64,
}

impl HistoryEntry {
    /// Create a new history entry.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            frequency: 1,
            last_used: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }

    /// Calculate the frecency score for this entry.
    ///
    /// Higher scores indicate more relevant entries.
    /// Combines frequency with recency decay.
    pub fn frecency_score(&self) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Time decay: entries older than a day get penalized
        let age_hours = ((now - self.last_used) as f64 / 3600.0).max(0.0);
        let recency_factor = 1.0 / (1.0 + age_hours / 24.0);

        // Frequency boost with diminishing returns
        let frequency_factor = (self.frequency as f64).ln() + 1.0;

        frequency_factor * recency_factor
    }

    /// Update the entry for a new use.
    pub fn mark_used(&mut self) {
        self.frequency += 1;
        self.last_used = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
    }
}

impl InputState {
    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        let cursor = self.cursor as usize;
        if cursor >= self.text.len() {
            self.text.push(c);
        } else {
            self.text.insert(cursor, c);
        }
        self.cursor += 1;
    }

    /// Delete the character before the cursor.
    pub fn delete_backward(&mut self) {
        if self.cursor > 0 {
            let cursor = (self.cursor - 1) as usize;
            if cursor < self.text.len() {
                self.text.remove(cursor);
            }
            self.cursor -= 1;
        }
    }

    /// Delete the character at the cursor.
    pub fn delete_forward(&mut self) {
        let cursor = self.cursor as usize;
        if cursor < self.text.len() {
            self.text.remove(cursor);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if (self.cursor as usize) < self.text.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn move_end(&mut self) {
        self.cursor = self.text.len() as i32;
    }

    /// Move cursor to the start of the previous word.
    pub fn move_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let text = &self.text;
        let mut pos = (self.cursor - 1) as usize;

        // Skip any whitespace before cursor
        while pos > 0 && text.chars().nth(pos).is_some_and(|c| c.is_whitespace()) {
            pos -= 1;
        }

        // Skip to start of current word
        while pos > 0
            && text
                .chars()
                .nth(pos - 1)
                .is_some_and(|c| !c.is_whitespace())
        {
            pos -= 1;
        }

        self.cursor = pos as i32;
    }

    /// Move cursor to the start of the next word.
    pub fn move_word_right(&mut self) {
        let text = &self.text;
        let len = text.len();
        let mut pos = self.cursor as usize;

        if pos >= len {
            return;
        }

        // Skip current word
        while pos < len && text.chars().nth(pos).is_some_and(|c| !c.is_whitespace()) {
            pos += 1;
        }

        // Skip whitespace
        while pos < len && text.chars().nth(pos).is_some_and(|c| c.is_whitespace()) {
            pos += 1;
        }

        self.cursor = pos as i32;
    }

    /// Delete the word before the cursor.
    pub fn delete_word_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let original_cursor = self.cursor as usize;
        let text = &self.text;
        let mut pos = original_cursor;

        // Skip whitespace before cursor
        while pos > 0 && text.chars().nth(pos - 1).is_some_and(|c| c.is_whitespace()) {
            pos -= 1;
        }

        // Skip to start of word
        while pos > 0
            && text
                .chars()
                .nth(pos - 1)
                .is_some_and(|c| !c.is_whitespace())
        {
            pos -= 1;
        }

        // Remove the text between pos and original cursor
        self.text = format!("{}{}", &text[..pos], &text[original_cursor..]);
        self.cursor = pos as i32;
    }

    /// Delete the word after the cursor.
    pub fn delete_word_forward(&mut self) {
        let text = &self.text;
        let len = text.len();
        let start_pos = self.cursor as usize;

        if start_pos >= len {
            return;
        }

        let mut pos = start_pos;

        // Skip current word
        while pos < len && text.chars().nth(pos).is_some_and(|c| !c.is_whitespace()) {
            pos += 1;
        }

        // Skip whitespace after word
        while pos < len && text.chars().nth(pos).is_some_and(|c| c.is_whitespace()) {
            pos += 1;
        }

        // Remove the text between start_pos and pos
        self.text = format!("{}{}", &text[..start_pos], &text[pos..]);
        // Cursor stays at same position
    }

    /// Insert a newline.
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Clear the input and return the text.
    pub fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.selection_start = None;
        text
    }

    /// Check if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get the current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set the text (e.g., from history or paste).
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len() as i32;
    }

    /// Add text to history with frecency tracking.
    ///
    /// If the text already exists in history, updates its frecency.
    /// Otherwise, adds a new entry.
    pub fn add_to_history(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }

        // Check if entry already exists
        if let Some(entry) = self.history.iter_mut().find(|e| e.text == text) {
            entry.mark_used();
        } else {
            self.history.push(HistoryEntry::new(text));
        }

        // Sort by frecency (highest first)
        self.history
            .sort_by(|a, b| b.frecency_score().partial_cmp(&a.frecency_score()).unwrap());

        // Limit history size
        const MAX_HISTORY: usize = 100;
        if self.history.len() > MAX_HISTORY {
            self.history.truncate(MAX_HISTORY);
        }
    }

    /// Get a history entry by index.
    pub fn history_text(&self, index: usize) -> Option<&str> {
        self.history.get(index).map(|e| e.text.as_str())
    }

    /// Get the number of history entries.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Detect an @mention at or before the cursor and return the query.
    ///
    /// Returns `Some((start_pos, query))` if there's an @mention being typed,
    /// where `start_pos` is the position of the @ character and `query` is
    /// the text after @.
    ///
    /// An @mention is detected when:
    /// - There's an @ character before the cursor
    /// - The @ is either at the start or preceded by whitespace
    /// - There's no space between @ and the cursor
    pub fn current_at_token(&self) -> Option<(i32, String)> {
        let text = &self.text;
        let cursor = self.cursor as usize;

        if cursor == 0 || text.is_empty() {
            return None;
        }

        // Look backwards from cursor for @
        let before_cursor = &text[..cursor.min(text.len())];

        // Find the last @ before cursor that isn't followed by a space
        let mut at_pos = None;
        for (i, c) in before_cursor.char_indices().rev() {
            if c == ' ' || c == '\n' || c == '\t' {
                // Hit whitespace without finding @, no active mention
                break;
            }
            if c == '@' {
                // Check if @ is at start or preceded by whitespace
                if i == 0 {
                    at_pos = Some(i);
                } else {
                    let prev_char = before_cursor[..i].chars().last();
                    if prev_char.is_some_and(|c| c.is_whitespace()) {
                        at_pos = Some(i);
                    }
                }
                break;
            }
        }

        at_pos.map(|pos| {
            let query = before_cursor[pos + 1..].to_string();
            (pos as i32, query)
        })
    }

    /// Detect a /command token at cursor position.
    ///
    /// Returns `Some((start_pos, query))` if there's a slash command being typed,
    /// where `start_pos` is the position of the / character and `query` is
    /// the text after /.
    ///
    /// A slash command is detected when:
    /// - There's a / character before the cursor
    /// - The / is at the start of input or preceded by whitespace
    /// - There's no space between / and the cursor
    pub fn current_slash_token(&self) -> Option<(i32, String)> {
        let text = &self.text;
        let cursor = self.cursor as usize;

        if cursor == 0 || text.is_empty() {
            return None;
        }

        // Look backwards from cursor for /
        let before_cursor = &text[..cursor.min(text.len())];

        // Find the last / before cursor that isn't followed by a space
        let mut slash_pos = None;
        for (i, c) in before_cursor.char_indices().rev() {
            if c == ' ' || c == '\n' || c == '\t' {
                // Hit whitespace without finding /, no active command
                break;
            }
            if c == '/' {
                // Check if / is at start or preceded by whitespace
                if i == 0 {
                    slash_pos = Some(i);
                } else {
                    let prev_char = before_cursor[..i].chars().last();
                    if prev_char.is_some_and(|c| c.is_whitespace()) {
                        slash_pos = Some(i);
                    }
                }
                break;
            }
        }

        slash_pos.map(|pos| {
            let query = before_cursor[pos + 1..].to_string();
            (pos as i32, query)
        })
    }

    /// Insert a selected skill name, replacing the current /query.
    ///
    /// The `start_pos` is the position of the / character, and `name` is
    /// the skill name to insert (without the /).
    pub fn insert_selected_skill(&mut self, start_pos: i32, name: &str) {
        let start = start_pos as usize;
        let cursor = self.cursor as usize;

        if start >= self.text.len() || cursor > self.text.len() {
            return;
        }

        // Build new text: before / + /name + space + after cursor
        let before = &self.text[..start];
        let after = &self.text[cursor..];
        let new_text = format!("{before}/{name} {after}");

        // Calculate new cursor position: after the inserted skill name and space
        let new_cursor = start + 1 + name.len() + 1;

        self.text = new_text;
        self.cursor = new_cursor as i32;
    }

    /// Insert a selected file path, replacing the current @query.
    ///
    /// The `start_pos` is the position of the @ character, and `path` is
    /// the path to insert (without the @).
    pub fn insert_selected_path(&mut self, start_pos: i32, path: &str) {
        let start = start_pos as usize;
        let cursor = self.cursor as usize;

        if start >= self.text.len() || cursor > self.text.len() {
            return;
        }

        // Build new text: before @ + @path + after cursor
        let before = &self.text[..start];
        let after = &self.text[cursor..];
        let new_text = format!("{before}@{path} {after}");

        // Calculate new cursor position: after the inserted path and space
        let new_cursor = start + 1 + path.len() + 1;

        self.text = new_text;
        self.cursor = new_cursor as i32;
    }
}

/// The current focus target in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    /// Input field is focused.
    #[default]
    Input,
    /// Chat history is focused (for scrolling).
    Chat,
    /// Tool panel is focused.
    ToolPanel,
}

/// An active overlay (modal dialog).
#[derive(Debug, Clone)]
pub enum Overlay {
    /// Permission approval prompt.
    Permission(PermissionOverlay),
    /// Model picker.
    ModelPicker(ModelPickerOverlay),
    /// Command palette.
    CommandPalette(CommandPaletteOverlay),
    /// Session browser.
    SessionBrowser(SessionBrowserOverlay),
    /// Help screen.
    Help,
    /// Error message.
    Error(String),
}

/// Permission approval overlay state.
#[derive(Debug, Clone)]
pub struct PermissionOverlay {
    /// The approval request.
    pub request: ApprovalRequest,
    /// Selected option index (0 = approve, 1 = deny, 2 = approve all).
    pub selected: i32,
}

impl PermissionOverlay {
    /// Create a new permission overlay.
    pub fn new(request: ApprovalRequest) -> Self {
        Self {
            request,
            selected: 0,
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        if self.selected < 2 {
            self.selected += 1;
        }
    }
}

/// Model picker overlay state.
#[derive(Debug, Clone)]
pub struct ModelPickerOverlay {
    /// Available models.
    pub models: Vec<String>,
    /// Currently selected index.
    pub selected: i32,
    /// Search filter.
    pub filter: String,
}

impl ModelPickerOverlay {
    /// Create a new model picker.
    pub fn new(models: Vec<String>) -> Self {
        Self {
            models,
            selected: 0,
            filter: String::new(),
        }
    }

    /// Get filtered models.
    pub fn filtered_models(&self) -> Vec<&str> {
        if self.filter.is_empty() {
            self.models.iter().map(String::as_str).collect()
        } else {
            let filter = self.filter.to_lowercase();
            self.models
                .iter()
                .filter(|m| m.to_lowercase().contains(&filter))
                .map(String::as_str)
                .collect()
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = self.filtered_models().len() as i32 - 1;
        if self.selected < max {
            self.selected += 1;
        }
    }
}

/// Command palette overlay state.
#[derive(Debug, Clone)]
pub struct CommandPaletteOverlay {
    /// Search query.
    pub query: String,
    /// All available commands.
    pub commands: Vec<CommandItem>,
    /// Indices of filtered commands.
    pub filtered: Vec<i32>,
    /// Currently selected index in filtered list.
    pub selected: i32,
}

impl CommandPaletteOverlay {
    /// Create a new command palette.
    pub fn new(commands: Vec<CommandItem>) -> Self {
        let filtered: Vec<i32> = (0..commands.len() as i32).collect();
        Self {
            query: String::new(),
            commands,
            filtered,
            selected: 0,
        }
    }

    /// Update the filter based on query.
    pub fn update_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.commands.len() as i32).collect();
        } else {
            let query = self.query.to_lowercase();
            self.filtered = self
                .commands
                .iter()
                .enumerate()
                .filter(|(_, cmd)| {
                    cmd.name.to_lowercase().contains(&query)
                        || cmd.description.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i as i32)
                .collect();
        }
        // Reset selection if out of bounds
        if self.selected >= self.filtered.len() as i32 {
            self.selected = 0;
        }
    }

    /// Get filtered commands.
    pub fn filtered_commands(&self) -> Vec<&CommandItem> {
        self.filtered
            .iter()
            .filter_map(|&i| self.commands.get(i as usize))
            .collect()
    }

    /// Get the currently selected command.
    pub fn selected_command(&self) -> Option<&CommandItem> {
        self.filtered
            .get(self.selected as usize)
            .and_then(|&i| self.commands.get(i as usize))
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = (self.filtered.len() as i32).saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }

    /// Add a character to the query.
    pub fn insert_char(&mut self, c: char) {
        self.query.push(c);
        self.update_filter();
    }

    /// Delete a character from the query.
    pub fn delete_char(&mut self) {
        self.query.pop();
        self.update_filter();
    }
}

/// A command item in the command palette.
#[derive(Debug, Clone)]
pub struct CommandItem {
    /// Command name/label.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Keyboard shortcut (if any).
    pub shortcut: Option<String>,
    /// The action to execute.
    pub action: CommandAction,
}

/// Action to execute when a command is selected.
#[derive(Debug, Clone)]
pub enum CommandAction {
    /// Toggle plan mode.
    TogglePlanMode,
    /// Cycle thinking level.
    CycleThinkingLevel,
    /// Show model picker.
    ShowModelPicker,
    /// Show help.
    ShowHelp,
    /// Show session browser.
    ShowSessionBrowser,
    /// Clear screen.
    ClearScreen,
    /// Interrupt.
    Interrupt,
    /// Quit.
    Quit,
}

/// Session browser overlay state.
#[derive(Debug, Clone)]
pub struct SessionBrowserOverlay {
    /// Available sessions.
    pub sessions: Vec<SessionSummary>,
    /// Currently selected index.
    pub selected: i32,
    /// Search filter.
    pub filter: String,
}

impl SessionBrowserOverlay {
    /// Create a new session browser.
    pub fn new(sessions: Vec<SessionSummary>) -> Self {
        Self {
            sessions,
            selected: 0,
            filter: String::new(),
        }
    }

    /// Get filtered sessions.
    pub fn filtered_sessions(&self) -> Vec<&SessionSummary> {
        if self.filter.is_empty() {
            self.sessions.iter().collect()
        } else {
            let filter = self.filter.to_lowercase();
            self.sessions
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&filter)
                        || s.id.to_lowercase().contains(&filter)
                })
                .collect()
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = (self.filtered_sessions().len() as i32).saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }

    /// Get the currently selected session.
    pub fn selected_session(&self) -> Option<&SessionSummary> {
        let filtered = self.filtered_sessions();
        filtered.get(self.selected as usize).copied()
    }

    /// Add a character to the filter.
    pub fn insert_char(&mut self, c: char) {
        self.filter.push(c);
    }

    /// Delete a character from the filter.
    pub fn delete_char(&mut self) {
        self.filter.pop();
    }
}

/// Summary of a saved session.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Session ID.
    pub id: String,
    /// Session title/description.
    pub title: String,
    /// Creation timestamp (Unix seconds).
    pub created_at: i64,
    /// Last modified timestamp (Unix seconds).
    pub updated_at: i64,
    /// Number of messages in the session.
    pub message_count: i32,
}

/// State for streaming content.
#[derive(Debug, Clone)]
pub struct StreamingState {
    /// Turn identifier.
    pub turn_id: String,
    /// Content being streamed.
    pub content: String,
    /// Thinking content being streamed.
    pub thinking: String,
    /// Whether currently streaming thinking content (before main content).
    pub is_thinking: bool,
}

impl StreamingState {
    /// Create new streaming state.
    pub fn new(turn_id: String) -> Self {
        Self {
            turn_id,
            content: String::new(),
            thinking: String::new(),
            is_thinking: false,
        }
    }

    /// Get estimated thinking token count (rough estimate: words * 1.3).
    pub fn thinking_tokens(&self) -> i32 {
        let word_count = self.thinking.split_whitespace().count();
        (word_count as f64 * 1.3) as i32
    }
}

/// State for file autocomplete suggestions.
#[derive(Debug, Clone)]
pub struct FileSuggestionState {
    /// The query extracted from @mention (without the @).
    pub query: String,
    /// Start position of the @mention in the input text.
    pub start_pos: i32,
    /// Current suggestions.
    pub suggestions: Vec<FileSuggestionItem>,
    /// Currently selected index in the dropdown.
    pub selected: i32,
    /// Whether a search is currently in progress.
    pub loading: bool,
}

impl FileSuggestionState {
    /// Create a new file suggestion state.
    pub fn new(query: String, start_pos: i32) -> Self {
        Self {
            query,
            start_pos,
            suggestions: Vec::new(),
            selected: 0,
            loading: true,
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = (self.suggestions.len() as i32).saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }

    /// Get the currently selected suggestion.
    pub fn selected_suggestion(&self) -> Option<&FileSuggestionItem> {
        self.suggestions.get(self.selected as usize)
    }

    /// Update suggestions from search results.
    pub fn update_suggestions(&mut self, suggestions: Vec<FileSuggestionItem>) {
        self.suggestions = suggestions;
        self.loading = false;
        // Reset selection if out of bounds
        if self.selected >= self.suggestions.len() as i32 {
            self.selected = 0;
        }
    }
}

/// A single file suggestion item for display.
#[derive(Debug, Clone)]
pub struct FileSuggestionItem {
    /// The file path (relative).
    pub path: String,
    /// Display text (may differ from path, e.g., with trailing / for dirs).
    pub display_text: String,
    /// Relevance score (higher = better match).
    pub score: u32,
    /// Character indices that matched the query (for highlighting).
    pub match_indices: Vec<i32>,
    /// Whether this is a directory.
    pub is_directory: bool,
}

/// State for skill autocomplete suggestions.
#[derive(Debug, Clone)]
pub struct SkillSuggestionState {
    /// The query extracted from /command (without the /).
    pub query: String,
    /// Start position of the /command in the input text.
    pub start_pos: i32,
    /// Current suggestions.
    pub suggestions: Vec<SkillSuggestionItem>,
    /// Currently selected index in the dropdown.
    pub selected: i32,
    /// Whether a search is currently in progress.
    pub loading: bool,
}

impl SkillSuggestionState {
    /// Create a new skill suggestion state.
    pub fn new(query: String, start_pos: i32) -> Self {
        Self {
            query,
            start_pos,
            suggestions: Vec::new(),
            selected: 0,
            loading: false,
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = (self.suggestions.len() as i32).saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }

    /// Get the currently selected suggestion.
    pub fn selected_suggestion(&self) -> Option<&SkillSuggestionItem> {
        self.suggestions.get(self.selected as usize)
    }

    /// Update suggestions from search results.
    pub fn update_suggestions(&mut self, suggestions: Vec<SkillSuggestionItem>) {
        self.suggestions = suggestions;
        self.loading = false;
        // Reset selection if out of bounds
        if self.selected >= self.suggestions.len() as i32 {
            self.selected = 0;
        }
    }
}

/// A single skill suggestion item for display.
#[derive(Debug, Clone)]
pub struct SkillSuggestionItem {
    /// Skill name (e.g., "commit").
    pub name: String,
    /// Short description.
    pub description: String,
    /// Fuzzy match score (lower = better match).
    pub score: i32,
    /// Character indices that matched the query (for highlighting).
    pub match_indices: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_insert() {
        let mut input = InputState::default();
        input.insert_char('H');
        input.insert_char('i');
        assert_eq!(input.text(), "Hi");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_input_state_delete() {
        let mut input = InputState::default();
        input.set_text("Hello");
        input.cursor = 3; // After "Hel"

        input.delete_backward();
        assert_eq!(input.text(), "Helo");
        assert_eq!(input.cursor, 2);

        input.delete_forward();
        assert_eq!(input.text(), "Heo");
    }

    #[test]
    fn test_input_state_navigation() {
        let mut input = InputState::default();
        input.set_text("Hello");

        input.move_home();
        assert_eq!(input.cursor, 0);

        input.move_right();
        assert_eq!(input.cursor, 1);

        input.move_end();
        assert_eq!(input.cursor, 5);

        input.move_left();
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn test_input_state_take() {
        let mut input = InputState::default();
        input.set_text("Hello");

        let text = input.take();
        assert_eq!(text, "Hello");
        assert!(input.is_empty());
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_streaming_state() {
        let mut ui = UiState::default();

        ui.start_streaming("turn-1".to_string());
        assert!(ui.streaming.is_some());

        ui.append_streaming("Hello ");
        ui.append_streaming("World");
        assert_eq!(
            ui.streaming.as_ref().map(|s| s.content.as_str()),
            Some("Hello World")
        );

        ui.stop_streaming();
        assert!(ui.streaming.is_none());
    }

    #[test]
    fn test_focus_target_default() {
        assert_eq!(FocusTarget::default(), FocusTarget::Input);
    }

    #[test]
    fn test_current_at_token_simple() {
        let mut input = InputState::default();
        input.set_text("@src/main");

        let result = input.current_at_token();
        assert_eq!(result, Some((0, "src/main".to_string())));
    }

    #[test]
    fn test_current_at_token_mid_text() {
        let mut input = InputState::default();
        input.set_text("read @src/lib.rs please");
        input.cursor = 16; // After "@src/lib.rs"

        let result = input.current_at_token();
        assert_eq!(result, Some((5, "src/lib.rs".to_string())));
    }

    #[test]
    fn test_current_at_token_no_mention() {
        let mut input = InputState::default();
        input.set_text("no mention here");

        let result = input.current_at_token();
        assert_eq!(result, None);
    }

    #[test]
    fn test_current_at_token_after_space() {
        let mut input = InputState::default();
        input.set_text("@file completed ");
        input.cursor = 16; // After space

        let result = input.current_at_token();
        assert_eq!(result, None); // Space breaks the mention
    }

    #[test]
    fn test_insert_selected_path() {
        let mut input = InputState::default();
        input.set_text("read @src/ please");
        input.cursor = 10; // After "@src/"

        input.insert_selected_path(5, "src/main.rs");

        assert_eq!(input.text(), "read @src/main.rs  please");
        assert_eq!(input.cursor, 18); // After "@src/main.rs "
    }

    #[test]
    fn test_file_suggestion_state() {
        let mut state = FileSuggestionState::new("src/".to_string(), 5);

        assert!(state.loading);
        assert!(state.suggestions.is_empty());
        assert_eq!(state.selected, 0);

        // Add suggestions
        state.update_suggestions(vec![
            FileSuggestionItem {
                path: "src/main.rs".to_string(),
                display_text: "src/main.rs".to_string(),
                score: 100,
                match_indices: vec![],
                is_directory: false,
            },
            FileSuggestionItem {
                path: "src/lib.rs".to_string(),
                display_text: "src/lib.rs".to_string(),
                score: 90,
                match_indices: vec![],
                is_directory: false,
            },
        ]);

        assert!(!state.loading);
        assert_eq!(state.suggestions.len(), 2);

        // Navigate
        state.move_down();
        assert_eq!(state.selected, 1);

        state.move_down(); // Should not go past last
        assert_eq!(state.selected, 1);

        state.move_up();
        assert_eq!(state.selected, 0);

        state.move_up(); // Should not go negative
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_move_word_left() {
        let mut input = InputState::default();
        input.set_text("hello world test");

        // Cursor at end
        assert_eq!(input.cursor, 16);

        input.move_word_left();
        assert_eq!(input.cursor, 12); // Before "test"

        input.move_word_left();
        assert_eq!(input.cursor, 6); // Before "world"

        input.move_word_left();
        assert_eq!(input.cursor, 0); // At start

        input.move_word_left(); // Should stay at 0
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_move_word_right() {
        let mut input = InputState::default();
        input.set_text("hello world test");
        input.cursor = 0;

        input.move_word_right();
        assert_eq!(input.cursor, 6); // After "hello "

        input.move_word_right();
        assert_eq!(input.cursor, 12); // After "world "

        input.move_word_right();
        assert_eq!(input.cursor, 16); // At end

        input.move_word_right(); // Should stay at end
        assert_eq!(input.cursor, 16);
    }

    #[test]
    fn test_delete_word_backward() {
        let mut input = InputState::default();
        input.set_text("hello world test");

        input.delete_word_backward();
        assert_eq!(input.text(), "hello world ");
        assert_eq!(input.cursor, 12);

        input.delete_word_backward();
        assert_eq!(input.text(), "hello ");
        assert_eq!(input.cursor, 6);

        input.delete_word_backward();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_delete_word_forward() {
        let mut input = InputState::default();
        input.set_text("hello world test");
        input.cursor = 0;

        input.delete_word_forward();
        assert_eq!(input.text(), "world test");
        assert_eq!(input.cursor, 0);

        input.delete_word_forward();
        assert_eq!(input.text(), "test");
        assert_eq!(input.cursor, 0);

        input.delete_word_forward();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_toggle_thinking() {
        let mut ui = UiState::default();
        assert!(!ui.show_thinking);

        ui.toggle_thinking();
        assert!(ui.show_thinking);

        ui.toggle_thinking();
        assert!(!ui.show_thinking);
    }

    #[test]
    fn test_user_scrolled() {
        let mut ui = UiState::default();
        assert!(!ui.user_scrolled);

        ui.mark_user_scrolled();
        assert!(ui.user_scrolled);

        ui.reset_user_scrolled();
        assert!(!ui.user_scrolled);
    }

    #[test]
    fn test_current_slash_token_simple() {
        let mut input = InputState::default();
        input.set_text("/commit");

        let result = input.current_slash_token();
        assert_eq!(result, Some((0, "commit".to_string())));
    }

    #[test]
    fn test_current_slash_token_mid_text() {
        let mut input = InputState::default();
        input.set_text("run /review file.rs");
        input.cursor = 11; // After "/review"

        let result = input.current_slash_token();
        assert_eq!(result, Some((4, "review".to_string())));
    }

    #[test]
    fn test_current_slash_token_no_command() {
        let mut input = InputState::default();
        input.set_text("no command here");

        let result = input.current_slash_token();
        assert_eq!(result, None);
    }

    #[test]
    fn test_current_slash_token_after_space() {
        let mut input = InputState::default();
        input.set_text("/commit completed ");
        input.cursor = 18; // After space

        let result = input.current_slash_token();
        assert_eq!(result, None); // Space breaks the command
    }

    #[test]
    fn test_insert_selected_skill() {
        let mut input = InputState::default();
        input.set_text("run /com please");
        input.cursor = 8; // After "/com"

        input.insert_selected_skill(4, "commit");

        assert_eq!(input.text(), "run /commit  please");
        assert_eq!(input.cursor, 12); // After "/commit "
    }

    #[test]
    fn test_skill_suggestion_state() {
        let mut state = SkillSuggestionState::new("com".to_string(), 0);

        assert!(!state.loading);
        assert!(state.suggestions.is_empty());
        assert_eq!(state.selected, 0);

        // Add suggestions
        state.update_suggestions(vec![
            SkillSuggestionItem {
                name: "commit".to_string(),
                description: "Generate a commit message".to_string(),
                score: -100,
                match_indices: vec![0, 1, 2],
            },
            SkillSuggestionItem {
                name: "config".to_string(),
                description: "Configure settings".to_string(),
                score: -98,
                match_indices: vec![0, 1],
            },
        ]);

        assert!(!state.loading);
        assert_eq!(state.suggestions.len(), 2);

        // Navigate
        state.move_down();
        assert_eq!(state.selected, 1);

        state.move_down(); // Should not go past last
        assert_eq!(state.selected, 1);

        state.move_up();
        assert_eq!(state.selected, 0);

        state.move_up(); // Should not go negative
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_thinking_duration() {
        let mut ui = UiState::default();

        // Initially not thinking
        assert!(!ui.is_thinking());
        assert!(ui.thinking_duration().is_none());

        // Start thinking
        ui.start_thinking();
        assert!(ui.is_thinking());
        assert!(ui.thinking_duration().is_some());

        // Duration should be small (just started)
        let duration = ui.thinking_duration().unwrap();
        assert!(duration.as_millis() < 1000);

        // Stop thinking
        ui.stop_thinking();
        assert!(!ui.is_thinking());
        assert!(ui.last_thinking_duration.is_some());

        // Clear thinking duration
        ui.clear_thinking_duration();
        assert!(ui.thinking_duration().is_none());
    }

    #[test]
    fn test_terminal_focused_default() {
        let ui = UiState::default();
        assert!(!ui.terminal_focused);
    }
}
