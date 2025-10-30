use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, StatefulWidgetRef, WidgetRef};
use code_core::protocol::TokenUsage;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

use super::chat_composer_history::ChatComposerHistory;
use super::command_popup::CommandItem;
use super::command_popup::CommandPopup;
use super::file_search_popup::FileSearchPopup;
use super::paste_burst::PasteBurst;
use crate::slash_command::{built_in_slash_commands, SlashCommand};
use code_protocol::custom_prompts::CustomPrompt;
use code_protocol::custom_prompts::PROMPTS_CMD_PREFIX;

use crate::app_event_sender::AppEventSender;
use crate::auto_drive_style::{BorderGradient, ComposerStyle};
use crate::thread_spawner;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;
use crate::clipboard_paste::normalize_pasted_path;
use crate::clipboard_paste::paste_image_to_temp_png;
use crate::clipboard_paste::try_decode_base64_image_to_temp_png;
use code_file_search::FileMatch;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::Instant;

// Dynamic placeholder rendered when the composer is empty.
/// If the pasted content exceeds this number of characters, replace it with a
/// placeholder in the UI.
const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

struct PostPasteSpaceGuard {
    expires_at: Instant,
    cursor_pos: usize,
}

fn parse_slash_name(line: &str) -> Option<(&str, &str)> {
    let stripped = line.strip_prefix('/')?;
    let mut name_end = stripped.len();
    for (idx, ch) in stripped.char_indices() {
        if ch.is_whitespace() {
            name_end = idx;
            break;
        }
    }
    if name_end == 0 {
        return None;
    }
    let name = &stripped[..name_end];
    let rest = stripped[name_end..].trim_start();
    Some((name, rest))
}

/// Result returned when the user interacts with the text area.
#[derive(Debug, PartialEq)]
pub enum InputResult {
    Submitted(String),
    Command(SlashCommand),
    ScrollUp,
    ScrollDown,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComposerRenderMode {
    Full,
    FooterOnly,
}

struct TokenUsageInfo {
    _total_token_usage: TokenUsage,
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

// Format an integer with thousands separators (e.g., 125,654).
fn format_with_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let mut count = 0usize;
    for ch in s.chars().rev() {
        if count != 0 && count % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
        count += 1;
    }
    out.chars().rev().collect()
}

pub(crate) struct ChatComposer {
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    active_popup: ActivePopup,
    app_event_tx: AppEventSender,
    history: ChatComposerHistory,
    ctrl_c_quit_hint: bool,
    #[allow(dead_code)]
    use_shift_enter_hint: bool,
    dismissed_file_popup_token: Option<String>,
    current_file_query: Option<String>,
    // Tracks a one-off Tab-triggered file search. When set, we will only
    // create/show a popup if the results are non-empty to avoid flicker.
    pending_tab_file_query: Option<String>,
    file_popup_origin: Option<FilePopupOrigin>,
    pending_pastes: Vec<(String, String)>,
    token_usage_info: Option<TokenUsageInfo>,
    has_focus: bool,
    has_chat_history: bool,
    /// Tracks whether the user has typed or pasted any content since startup.
    typed_anything: bool,
    is_task_running: bool,
    // Current status message to display when task is running
    status_message: String,
    // Animation thread for spinning icon when task is running
    animation_running: Option<Arc<AtomicBool>>,
    using_chatgpt_auth: bool,
    // Ephemeral footer notice and its expiry
    footer_notice: Option<(String, std::time::Instant)>,
    // Persistent hint for specific modes (e.g., standard terminal mode)
    standard_terminal_hint: Option<String>,
    // Persistent/ephemeral access-mode indicator shown on the left
    access_mode_label: Option<String>,
    access_mode_label_expiry: Option<std::time::Instant>,
    access_mode_hint_expiry: Option<std::time::Instant>,
    // Footer hint visibility flags
    show_reasoning_hint: bool,
    show_diffs_hint: bool,
    reasoning_shown: bool,
    // Sticky flag: after a chat ScrollUp, make the very next Down trigger
    // chat ScrollDown instead of moving within the textarea, unless another
    // key is pressed in between.
    next_down_scrolls_history: bool,
    // Detect and coalesce paste bursts for smoother UX
    paste_burst: PasteBurst,
    post_paste_space_guard: Option<PostPasteSpaceGuard>,
    footer_hint_override: Option<Vec<(String, String)>>,
    embedded_mode: bool,
    render_mode: ComposerRenderMode,
    auto_drive_active: bool,
    auto_drive_style: Option<ComposerStyle>,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
}

enum FilePopupOrigin {
    Auto,
    Manual { token: String },
}

impl ChatComposer {
    pub fn new(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
        using_chatgpt_auth: bool,
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
            dismissed_file_popup_token: None,
            current_file_query: None,
            pending_tab_file_query: None,
            file_popup_origin: None,
            pending_pastes: Vec::new(),
            token_usage_info: None,
            has_focus: has_input_focus,
            has_chat_history: false,
            typed_anything: false,
            // no double‑Esc handling here; App manages Esc policy
            is_task_running: false,
            status_message: String::from("coding"),
            animation_running: None,
            using_chatgpt_auth,
            footer_notice: None,
            standard_terminal_hint: None,
            access_mode_label: None,
            access_mode_label_expiry: None,
            access_mode_hint_expiry: None,
            show_reasoning_hint: false,
            show_diffs_hint: false,
            reasoning_shown: false,
            next_down_scrolls_history: false,
            paste_burst: PasteBurst::default(),
            post_paste_space_guard: None,
            footer_hint_override: None,
            embedded_mode: false,
            render_mode: ComposerRenderMode::Full,
            auto_drive_active: false,
            auto_drive_style: None,
        }
    }

    pub fn set_using_chatgpt_auth(&mut self, using: bool) {
        self.using_chatgpt_auth = using;
    }

    /// Returns true if the input starts with a slash command and the cursor
    /// is positioned within the command head (i.e., before the first
    /// whitespace on the first line). Used to decide whether to keep the
    /// slash-command popup active and to suppress file completion.
    fn is_cursor_in_slash_command_head(&self) -> bool {
        let text = self.textarea.text();
        if text.is_empty() { return false; }
        let cursor = self.textarea.cursor();
        let first_line_end = text.find('\n').unwrap_or(text.len());
        let first_line = &text[..first_line_end];
        if !first_line.starts_with('/') { return false; }
        let head_end = first_line
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(first_line_end);
        cursor <= head_end
    }

    pub fn set_has_chat_history(&mut self, has_history: bool) {
        self.has_chat_history = has_history;
    }

    pub fn set_task_running(&mut self, running: bool) {
        self.is_task_running = running;

        if running {
            // Start animation thread if not already running
            if self.animation_running.is_none() {
                let animation_flag = Arc::new(AtomicBool::new(true));
                let animation_flag_clone = Arc::clone(&animation_flag);
                let app_event_tx_clone = self.app_event_tx.clone();

                // Drive redraws at the spinner's native cadence with a
                // phase‑aligned, monotonic scheduler to minimize drift and
                // reduce perceived frame skipping under load. We purposely
                // avoid very small intervals to keep CPU impact low.
                let fallback_tx = self.app_event_tx.clone();
                if thread_spawner::spawn_lightweight("composer-anim", move || {
                    use std::time::Instant;
                    // Default to ~120ms if spinner state is not yet initialized
                    let default_ms: u64 = 120;
                    // Clamp to a sane floor so we never busy loop if a custom spinner
                    // has an extremely small interval configured.
                    let min_ms: u64 = 60; // ~16 FPS upper bound for this thread

                    // Determine the target period. If the user changes the spinner
                    // while running, we'll still get correct visual output because
                    // frames are time‑based at render; this cadence simply requests
                    // redraws.
                    let period_ms = crate::spinner::current_spinner()
                        .interval_ms
                        .max(min_ms)
                        .max(1);
                    let period = Duration::from_millis(period_ms); // fallback uses default below if needed

                    let mut next = Instant::now()
                        .checked_add(if period_ms == 0 { Duration::from_millis(default_ms) } else { period })
                        .unwrap_or_else(Instant::now);

                    while animation_flag_clone.load(Ordering::Relaxed) {
                        let now = Instant::now();
                        if now < next {
                            let sleep_dur = next - now;
                            thread::sleep(sleep_dur);
                        } else {
                            // If we're late (system busy), request a redraw immediately.
                            app_event_tx_clone.send(crate::app_event::AppEvent::RequestRedraw);
                            // Step the schedule forward by whole periods to avoid
                            // bursty catch‑up redraws.
                            let mut target = next;
                            while target <= now {
                                if let Some(t) = target.checked_add(period) { target = t; } else { break; }
                            }
                            next = target;
                        }
                    }
                })
                .is_none()
                {
                    animation_flag.store(false, Ordering::Relaxed);
                    fallback_tx.send(crate::app_event::AppEvent::RequestRedraw);
                } else {
                    self.animation_running = Some(animation_flag);
                }
            }
        } else {
            // Stop animation thread
            if let Some(animation_flag) = self.animation_running.take() {
                animation_flag.store(false, Ordering::Relaxed);
            }
        }
    }

    pub fn update_status_message(&mut self, message: String) {
        self.status_message = Self::map_status_message(&message);
    }

    pub fn status_message(&self) -> Option<&str> {
        let trimmed = self.status_message.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub fn flash_footer_notice(&mut self, text: String) {
        let expiry = std::time::Instant::now() + std::time::Duration::from_secs(2);
        self.footer_notice = Some((text, expiry));
    }

    pub(crate) fn set_embedded_mode(&mut self, enabled: bool) {
        if self.embedded_mode != enabled {
            self.embedded_mode = enabled;
        }
    }

    pub(crate) fn set_render_mode(&mut self, mode: ComposerRenderMode) {
        if self.render_mode != mode {
            self.render_mode = mode;
        }
    }

    pub(crate) fn set_auto_drive_active(&mut self, active: bool) {
        self.auto_drive_active = active;
    }

    pub(crate) fn set_auto_drive_style(&mut self, style: Option<ComposerStyle>) {
        self.auto_drive_style = style;
    }

    /// Override the footer hint line with a simple key/label list.
    /// When set, we skip the standard reasoning/diff/help hints and render the
    /// provided items using our theme colors. Inspired by upstream's
    /// `FooterProps`, but routed through our single-line composer footer to
    /// preserve custom fork styling.
    pub(crate) fn set_footer_hint_override(
        &mut self,
        items: Option<Vec<(String, String)>>,
    ) {
        self.footer_hint_override = items.map(|values| {
            values
                .into_iter()
                .map(|(key, label)| (key.trim().to_string(), label.trim().to_string()))
                .collect()
        });
    }

    pub(crate) fn has_footer_hint_override(&self) -> bool {
        self.footer_hint_override.is_some()
    }

    /// Show a footer notice for a specific duration.
    pub fn flash_footer_notice_for(&mut self, text: String, dur: std::time::Duration) {
        let expiry = std::time::Instant::now() + dur;
        self.footer_notice = Some((text, expiry));
    }

    // Control footer hint visibility
    pub fn set_show_reasoning_hint(&mut self, show: bool) {
        if self.show_reasoning_hint != show {
            self.show_reasoning_hint = show;
        }
    }

    pub fn set_show_diffs_hint(&mut self, show: bool) {
        if self.show_diffs_hint != show {
            self.show_diffs_hint = show;
        }
    }

    pub fn set_access_mode_label(&mut self, label: Option<String>) {
        self.access_mode_label = label;
        self.access_mode_label_expiry = None;
        self.access_mode_hint_expiry = None;
    }
    pub fn set_access_mode_label_ephemeral(&mut self, label: String, dur: std::time::Duration) {
        self.access_mode_label = Some(label);
        let expiry = std::time::Instant::now() + dur;
        self.access_mode_label_expiry = Some(expiry);
        self.access_mode_hint_expiry = Some(expiry);
    }
    pub fn set_access_mode_hint_for(&mut self, dur: std::time::Duration) {
        self.access_mode_hint_expiry = Some(std::time::Instant::now() + dur);
    }

    pub fn set_reasoning_state(&mut self, shown: bool) {
        self.reasoning_shown = shown;
    }

    // Map technical status messages to user-friendly ones
    pub(crate) fn map_status_message(technical_message: &str) -> String {
        if technical_message.trim().is_empty() {
            return String::new();
        }

        let lower = technical_message.to_lowercase();

        // Auto Drive manual edit indicator
        if lower.contains("auto drive goal") {
            "Auto Drive Goal".to_string()
        } else if lower.contains("auto drive") {
            "Auto Drive".to_string()
        }
        // Thinking/reasoning patterns
        else if lower.contains("reasoning")
            || lower.contains("thinking")
            || lower.contains("planning")
            || lower.contains("waiting for model")
            || lower.contains("model")
        {
            "Thinking".to_string()
        }
        // Tool/command execution patterns
        else if lower.contains("tool")
            || lower.contains("command")
            || lower.contains("running command")
            || lower.contains("running")
            || lower.contains("bash")
            || lower.contains("shell")
        {
            "Using tools".to_string()
        }
        // Browser activity
        else if lower.contains("browser")
            || lower.contains("chrome")
            || lower.contains("cdp")
            || lower.contains("navigate")
            || lower.contains("url")
            || lower.contains("screenshot")
        {
            "Browsing".to_string()
        }
        // Multi-agent orchestration
        else if lower.contains("agent")
            || lower.contains("agents")
            || lower.contains("orchestrating")
            || lower.contains("coordinating")
        {
            "Agents".to_string()
        }
        // Response generation patterns
        else if lower.contains("generating")
            || lower.contains("responding")
            || lower.contains("streaming")
            || lower.contains("writing response")
            || lower.contains("assistant")
            || lower.contains("chat completions")
            || lower.contains("completion")
        {
            "Responding".to_string()
        }
        // Transient network/stream retry patterns → keep spinner visible with a
        // clear reconnecting message so the user knows we are still working.
        else if lower.contains("retrying")
            || lower.contains("reconnecting")
            || lower.contains("disconnected")
            || lower.contains("stream error")
            || lower.contains("stream closed")
            || lower.contains("timeout")
        {
            "Reconnecting".to_string()
        }
        // File/code editing patterns
        else if lower.contains("editing")
            || lower.contains("writing")
            || lower.contains("modifying")
            || lower.contains("creating file")
            || lower.contains("updating")
            || lower.contains("patch")
        {
            "Coding".to_string()
        }
        // Catch some common technical terms
        else if lower.contains("processing") || lower.contains("analyzing") {
            "Thinking".to_string()
        } else if lower.contains("reading") || lower.contains("searching") {
            "Reading".to_string()
        } else {
            // Default fallback - use "working" for unknown status
            "Working".to_string()
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        // Calculate hint/popup height
        let hint_height = match (&self.active_popup, self.embedded_mode) {
            (ActivePopup::Command(c), _) => c.calculate_required_height(),
            (ActivePopup::File(c), _) => c.calculate_required_height(),
            (ActivePopup::None, true) => 0,
            (ActivePopup::None, false) => 1,
        };

        // IMPORTANT: `width` here is the full BottomPane width. Subtract the
        // configured outer padding, border, and inner padding to match wrapping.
        let content_width = width.saturating_sub(crate::layout_consts::COMPOSER_CONTENT_WIDTH_OFFSET);
        let content_lines = self.textarea.desired_height(content_width).max(1); // At least 1 line

        // Total input height: content + border (2) only, no vertical padding
        // Minimum of 3 ensures at least 1 visible line with border
        let input_height = (content_lines + 2).max(3);

        input_height + hint_height
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        // Split area: textarea with border at top, hints/popup at bottom
        let hint_height = match (&self.active_popup, self.embedded_mode) {
            (ActivePopup::Command(popup), _) => popup.calculate_required_height(),
            (ActivePopup::File(popup), _) => popup.calculate_required_height(),
            (ActivePopup::None, true) => 0,
            (ActivePopup::None, false) => 1,
        };
        // Calculate dynamic height based on content
        let content_width = area.width.saturating_sub(4); // Account for border and padding
        let content_lines = self.textarea.desired_height(content_width).max(1);
        let desired_input_height = (content_lines + 2).max(3); // Parent layout enforces max

        // Use desired height but don't exceed available space
        let input_height = desired_input_height.min(area.height.saturating_sub(hint_height));
        let [input_area, _] = Layout::vertical([
            Constraint::Length(input_height),
            Constraint::Length(hint_height),
        ])
        .areas(area);

        // Get inner area of the bordered input box
        let input_block = Block::default().borders(Borders::ALL);
        let textarea_rect = input_block.inner(input_area);

        // Apply the same inner padding as in render (horizontal only).
        let padded_textarea_rect = textarea_rect.inner(Margin::new(crate::layout_consts::COMPOSER_INNER_HPAD.into(), 0));

        let state = self.textarea_state.borrow();
        self.textarea
            .cursor_pos_with_state(padded_textarea_rect, *state)
    }

    /// Returns true if the composer currently contains no user input.
    #[allow(dead_code)]
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
            .unwrap_or_else(|| last_token_usage.cached_input_tokens);

        self.token_usage_info = Some(TokenUsageInfo {
            _total_token_usage: total_token_usage,
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
        self.post_paste_space_guard = None;
        let mut pasted = pasted;
        if pasted.contains('\r') {
            pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        }
        let char_count = pasted.chars().count();
        // If the pasted text looks like a base64/data-URI image, decode it and insert as a path.
        if let Ok((path, info)) = try_decode_base64_image_to_temp_png(&pasted) {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image.png");
            let placeholder = format!("[image: {}]", filename);
            // Insert placeholder and notify chat widget about the mapping.
            self.textarea.insert_str(&placeholder);
            self.textarea.insert_str(" ");
            self.typed_anything = true; // Mark that user has interacted via paste
            self.app_event_tx
                .send(crate::app_event::AppEvent::RegisterPastedImage { placeholder: placeholder.clone(), path });
            self.flash_footer_notice(format!("Added image {}x{} (PNG)", info.width, info.height));
        } else if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = format!("[Pasted Content {char_count} chars]");
            self.textarea.insert_str(&placeholder);
            self.pending_pastes.push((placeholder, pasted));
            self.typed_anything = true; // Mark that user has interacted via paste
        } else if self.handle_paste_image_path(pasted.clone()) {
            self.textarea.insert_str(" ");
            self.typed_anything = true; // Mark that user has interacted via paste
        } else if pasted.trim().is_empty() {
            // No textual content pasted — try reading an image directly from the OS clipboard.
            match paste_image_to_temp_png() {
                Ok((path, info)) => {
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("image.png");
                    let placeholder = format!("[image: {}]", filename);
                    self.textarea.insert_str(&placeholder);
                    self.textarea.insert_str(" ");
                    self.typed_anything = true; // Mark that user has interacted via paste
                    self.app_event_tx
                        .send(crate::app_event::AppEvent::RegisterPastedImage { placeholder: placeholder.clone(), path });
                    // Give a small visual confirmation in the footer.
                    self.flash_footer_notice(format!(
                        "Added image {}x{} (PNG)",
                        info.width, info.height
                    ));
                }
                Err(_) => {
                    // Fall back to doing nothing special; keep composer unchanged.
                }
            }
        } else {
            self.textarea.insert_str(&pasted);
            self.typed_anything = true; // Mark that user has interacted via paste
            self.maybe_start_post_paste_space_guard(&pasted);
        }
        self.sync_command_popup();
        if matches!(self.active_popup, ActivePopup::Command(_)) {
            self.dismissed_file_popup_token = None;
        } else {
            self.sync_file_search_popup();
        }
        true
    }

    /// Heuristic handling for pasted paths: if the pasted text looks like a
    /// filesystem path (including file:// URLs and Windows paths), insert the
    /// normalized path directly into the composer and return true. The caller
    /// will add a trailing space to separate from subsequent input.
    fn handle_paste_image_path(&mut self, pasted: String) -> bool {
        if let Some(path) = normalize_pasted_path(&pasted) {
            // Insert the normalized path verbatim. We don't attempt to load the
            // file or special-case images here; higher layers handle attachments.
            self.textarea.insert_str(&path.to_string_lossy());
            return true;
        }
        false
    }

    fn maybe_start_post_paste_space_guard(&mut self, pasted: &str) {
        if pasted.chars().last() != Some(' ') {
            return;
        }
        let cursor_pos = self.textarea.cursor();
        // Ensure the character immediately before the cursor is a literal space.
        if cursor_pos == 0 {
            return;
        }
        if let Some(slice) = self
            .textarea
            .text()
            .as_bytes()
            .get(cursor_pos - 1)
        {
            if *slice == b' ' {
                self.post_paste_space_guard = Some(PostPasteSpaceGuard {
                    expires_at: Instant::now() + Duration::from_secs(2),
                    cursor_pos,
                });
            }
        }
    }

    fn should_suppress_post_paste_space(&mut self, event: &KeyEvent) -> bool {
        if event.kind != KeyEventKind::Press {
            return false;
        }
        if event.code != KeyCode::Char(' ') {
            return false;
        }
        let unshifted_space = event.modifiers == KeyModifiers::NONE
            || event.modifiers == KeyModifiers::SHIFT;
        if !unshifted_space {
            return false;
        }
        let Some(guard) = &self.post_paste_space_guard else {
            return false;
        };
        let now = Instant::now();
        if now > guard.expires_at {
            self.post_paste_space_guard = None;
            return false;
        }
        if self.textarea.cursor() != guard.cursor_pos {
            self.post_paste_space_guard = None;
            return false;
        }
        let text = self.textarea.text();
        if guard.cursor_pos == 0 || guard.cursor_pos > text.len() {
            self.post_paste_space_guard = None;
            return false;
        }
        if text.as_bytes()[guard.cursor_pos - 1] != b' ' {
            self.post_paste_space_guard = None;
            return false;
        }
        self.post_paste_space_guard = None;
        true
    }


    /// Clear all composer input and reset transient state like pending pastes
    /// and history navigation.
    pub(crate) fn clear_text(&mut self) {
        self.textarea.set_text("");
        self.pending_pastes.clear();
        self.history.reset_navigation();
        self.post_paste_space_guard = None;
    }

    #[allow(dead_code)]
    pub(crate) fn flush_paste_burst_if_due(&mut self) -> bool {
        let now = Instant::now();
        if let Some(pasted) = self.paste_burst.flush_if_due(now) {
            let _ = self.handle_paste(pasted);
            return true;
        }
        false
    }

    pub(crate) fn is_in_paste_burst(&self) -> bool {
        self.paste_burst.is_active()
    }

    pub(crate) fn recommended_paste_flush_delay() -> Duration {
        PasteBurst::recommended_flush_delay()
    }

    /// Integrate results from an asynchronous file search.
    pub(crate) fn on_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        // Handle one-off Tab-triggered case first: only open if matches exist.
        if self.pending_tab_file_query.as_ref() == Some(&query) {
            // If the user kept typing while the search was in-flight, resync to the
            // latest token before applying stale results.
            if let Some(current_token) = Self::current_generic_token(&self.textarea) {
                if Self::current_completion_token(&self.textarea).is_some() {
                    // A new auto-triggerable token (e.g., @ or ./) should be handled by the
                    // standard auto completion path instead of hijacking the manual popup.
                    self.pending_tab_file_query = None;
                    self.file_popup_origin = None;
                    self.current_file_query = None;
                    return;
                }
                if !current_token.is_empty() && current_token != query {
                    self.pending_tab_file_query = Some(current_token.clone());
                    if let ActivePopup::File(popup) = &mut self.active_popup {
                        popup.set_query(&current_token);
                    } else {
                        let mut popup = FileSearchPopup::new();
                        popup.set_query(&current_token);
                        self.active_popup = ActivePopup::File(popup);
                    }
                    self.current_file_query = Some(current_token.clone());
                    self.file_popup_origin = Some(FilePopupOrigin::Manual { token: current_token.clone() });
                    self.app_event_tx
                        .send(crate::app_event::AppEvent::StartFileSearch(current_token));
                    return;
                }
            }

            // Clear pending regardless of result to avoid repeats.
            self.pending_tab_file_query = None;

            if matches.is_empty() {
                if let ActivePopup::File(popup) = &mut self.active_popup {
                    // Clear the waiting state so the popup shows "no matches" instead of spinning forever.
                    popup.set_matches(&query, Vec::new());
                }
                return; // do not open popup when no matches to avoid flicker
            }

            match &mut self.active_popup {
                ActivePopup::File(popup) => popup.set_matches(&query, matches),
                _ => {
                    let mut popup = FileSearchPopup::new();
                    popup.set_query(&query);
                    popup.set_matches(&query, matches);
                    self.active_popup = ActivePopup::File(popup);
                }
            }
            self.current_file_query = Some(query.clone());
            self.file_popup_origin = Some(FilePopupOrigin::Manual { token: query });
            self.dismissed_file_popup_token = None;
            return;
        }

        if matches!(self.file_popup_origin, Some(FilePopupOrigin::Manual { .. }))
            && self.current_file_query.as_ref() == Some(&query)
        {
            if let ActivePopup::File(popup) = &mut self.active_popup {
                popup.set_matches(&query, matches);
            }
            return;
        }

        // Otherwise, only apply if user is still editing a token matching the query
        // and that token qualifies for auto-trigger (i.e., @ or ./).
        let current_opt = Self::current_completion_token(&self.textarea);
        let Some(current_token) = current_opt else { return; };
        if !current_token.starts_with(&query) { return; }

        if let ActivePopup::File(popup) = &mut self.active_popup {
            popup.set_matches(&query, matches);
        }
    }

    pub fn set_ctrl_c_quit_hint(&mut self, show: bool, has_focus: bool) {
        self.ctrl_c_quit_hint = show;
        self.set_has_focus(has_focus);
    }

    pub fn set_standard_terminal_hint(&mut self, hint: Option<String>) {
        self.standard_terminal_hint = hint;
    }

    pub fn standard_terminal_hint(&self) -> Option<&str> {
        self.standard_terminal_hint.as_deref()
    }

    pub fn set_text_content(&mut self, text: String) {
        self.textarea.set_text(&text);
        *self.textarea_state.borrow_mut() = TextAreaState::default();
        if !text.is_empty() {
            self.typed_anything = true;
        }
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.textarea.insert_str(text);
        self.typed_anything = true; // Mark that user has interacted via programmatic insertion
        self.sync_command_popup();
        self.sync_file_search_popup();
    }

    pub(crate) fn text(&self) -> &str {
        self.textarea.text()
    }

    /// Close the file-search popup if it is currently active. Returns true if closed.
    pub(crate) fn close_file_popup_if_active(&mut self) -> bool {
        match self.active_popup {
            ActivePopup::File(_) => {
                self.active_popup = ActivePopup::None;
                self.file_popup_origin = None;
                self.current_file_query = None;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn file_popup_visible(&self) -> bool {
        matches!(self.active_popup, ActivePopup::File(_))
    }

    /// Handle a key event coming from the main UI.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        // Any non-Down key clears the sticky flag; handled before popup routing
        if !matches!(key_event.code, KeyCode::Down) {
            self.next_down_scrolls_history = false;
        }
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

    // popup_active removed; callers use explicit state or rely on App policy.

    /// Handle key event when the slash-command popup is visible.
    fn handle_key_event_with_slash_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::Command(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            // Allow Shift+Up to navigate history even when slash popup is active.
            KeyEvent { code: KeyCode::Up, modifiers, .. } => {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    return self.handle_key_event_without_popup(key_event);
                }
                // If there are 0 or 1 items, let Up behave normally (cursor/history/scroll)
                if popup.match_count() <= 1 {
                    return self.handle_key_event_without_popup(key_event);
                }
                popup.move_up();
                (InputResult::None, true)
            }
            // Allow Shift+Down to navigate history even when slash popup is active.
            KeyEvent { code: KeyCode::Down, modifiers, .. } => {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    return self.handle_key_event_without_popup(key_event);
                }
                // If there are 0 or 1 items, let Down behave normally (cursor/history/scroll)
                if popup.match_count() <= 1 {
                    return self.handle_key_event_without_popup(key_event);
                }
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Dismiss the slash popup; keep the current input untouched.
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                if let Some(sel) = popup.selected_item() {
                    let first_line = self.textarea.text().lines().next().unwrap_or("");

                    match sel {
                        CommandItem::Builtin(cmd) => {
                            let starts_with_cmd = first_line
                                .trim_start()
                                .starts_with(&format!("/{}", cmd.command()));
                            if !starts_with_cmd {
                                self.textarea.set_text(&format!("/{} ", cmd.command()));
                            }
                        }
                        CommandItem::UserPrompt(idx) => {
                            if let Some(prompt) = popup.prompt(idx) {
                                let name = prompt.name.clone();
                                let starts_with_cmd = first_line
                                    .trim_start()
                                    .starts_with(format!("/{PROMPTS_CMD_PREFIX}:{name}").as_str());
                                if !starts_with_cmd {
                                    self.textarea.set_text(
                                        format!("/{PROMPTS_CMD_PREFIX}:{name} ").as_str(),
                                    );
                                }
                            }
                        }
                        CommandItem::Subagent(i) => {
                            if let Some(name) = popup.subagent_name(i) {
                                let starts_with_cmd = first_line
                                    .trim_start()
                                    .starts_with(&format!("/{}", name));
                                if !starts_with_cmd {
                                    self.textarea.set_text(&format!("/{} ", name));
                                }
                            }
                        }
                    }
                    // After completing, place the cursor at the end of the
                    // slash command so the user can immediately type args.
                    let new_cursor = self.textarea.text().len();
                    self.textarea.set_cursor(new_cursor);
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(sel) = popup.selected_item() {
                    let command_text = self.textarea.text().to_string();
                    self.history.record_local_submission(&command_text);

                    match sel {
                        CommandItem::Builtin(cmd) => {
                            if cmd.is_prompt_expanding() {
                                self.app_event_tx.send(crate::app_event::AppEvent::PrepareAgents);
                            }
                            self.app_event_tx
                                .send(crate::app_event::AppEvent::DispatchCommand(cmd, command_text.clone()));
                            self.textarea.set_text("");
                            self.active_popup = ActivePopup::None;
                            return (InputResult::Command(cmd), true);
                        }
                        CommandItem::UserPrompt(idx) => {
                            let prompt_content = popup
                                .prompt(idx)
                                .map(|p| p.content.clone());
                            self.textarea.set_text("");
                            self.active_popup = ActivePopup::None;
                            if let Some(contents) = prompt_content {
                                return (InputResult::Submitted(contents), true);
                            }
                            return (InputResult::None, true);
                        }
                        CommandItem::Subagent(i) => {
                            if let Some(name) = popup.subagent_name(i) {
                                let first_line = command_text.lines().next().unwrap_or("");
                                let starts_with = first_line
                                    .trim_start()
                                    .starts_with(&format!("/{}", name));
                                if starts_with {
                                    self.active_popup = ActivePopup::None;
                                    return self
                                        .handle_key_event_without_popup(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                                }
                                self.textarea.set_text(&format!("/{} ", name));
                                let new_cursor = self.textarea.text().len();
                                self.textarea.set_cursor(new_cursor);
                                return (InputResult::None, true);
                            }
                            return (InputResult::None, true);
                        }
                    }
                }
                self.handle_key_event_without_popup(key_event)
            }
            input => self.handle_input_basic(input),
        }
    }

    #[inline]
    fn clamp_to_char_boundary(text: &str, pos: usize) -> usize {
        let mut p = pos.min(text.len());
        if p < text.len() && !text.is_char_boundary(p) {
            p = text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= p)
                .last()
                .unwrap_or(0);
        }
        p
    }

    #[inline]
    #[allow(dead_code)]
    fn handle_non_ascii_char(&mut self, input: KeyEvent) -> (InputResult, bool) {
        if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
            self.handle_paste(pasted);
        }
        self.textarea.input(input);
        let text_after = self.textarea.text();
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));
        (InputResult::None, true)
    }

    /// Handle key events when file search popup is visible.
    fn handle_key_event_with_file_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        let ActivePopup::File(popup) = &mut self.active_popup else {
            unreachable!();
        };

        match key_event {
            KeyEvent { code: KeyCode::Up, modifiers, .. } => {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    return self.handle_key_event_without_popup(key_event);
                }
                // If there are 0 or 1 items, let Up behave normally (cursor/history/scroll)
                if popup.match_count() <= 1 {
                    return self.handle_key_event_without_popup(key_event);
                }
                popup.move_up();
                (InputResult::None, true)
            }
            KeyEvent { code: KeyCode::Down, modifiers, .. } => {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    return self.handle_key_event_without_popup(key_event);
                }
                // If there are 0 or 1 items, let Down behave normally (cursor/history/scroll)
                if popup.match_count() <= 1 {
                    return self.handle_key_event_without_popup(key_event);
                }
                popup.move_down();
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Hide popup without modifying text, remember token to avoid immediate reopen.
                if let Some(tok) = Self::current_completion_token(&self.textarea) {
                    self.dismissed_file_popup_token = Some(tok.to_string());
                }
                self.active_popup = ActivePopup::None;
                self.file_popup_origin = None;
                self.current_file_query = None;
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
                    // Drop popup borrow before using self mutably again.
                    self.insert_selected_path(&sel_path);
                    self.active_popup = ActivePopup::None;
                    self.file_popup_origin = None;
                    self.current_file_query = None;
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
    /// - The cursor may be anywhere inside the token (including on the
    ///   leading `@`). It does not need to be at the end of the line.
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

    /// Extract the completion token under the cursor for auto file search.
    ///
    /// Auto-trigger only for:
    /// - explicit @tokens (without the leading '@' in the return value)
    /// - tokens starting with "./" (relative paths)
    ///
    /// Returns the token text (without a leading '@' if present). Any other
    /// tokens should not auto-trigger completion; they may be handled on Tab.
    fn current_completion_token(textarea: &TextArea) -> Option<String> {
        // Prefer explicit @tokens when present.
        if let Some(tok) = Self::current_at_token(textarea) {
            return Some(tok);
        }

        // Otherwise, consider the generic token under the cursor, but only
        // auto-trigger for tokens starting with "./".
        let cursor_offset = textarea.cursor();
        let text = textarea.text();

        let mut safe_cursor = cursor_offset.min(text.len());
        if safe_cursor < text.len() && !text.is_char_boundary(safe_cursor) {
            safe_cursor = text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= cursor_offset)
                .last()
                .unwrap_or(0);
        }

        let before_cursor = &text[..safe_cursor];
        let after_cursor = &text[safe_cursor..];

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
        let end_idx = safe_cursor + end_rel_idx;

        if start_idx >= end_idx {
            return None;
        }

        let token = &text[start_idx..end_idx];

        // Strip a leading '@' if the user typed it but we didn't catch it
        // (paranoia; current_at_token should have handled this case).
        let token_stripped = token.strip_prefix('@').unwrap_or(token);

        if token_stripped.starts_with("./") {
            return Some(token_stripped.to_string());
        }

        None
    }

    /// Extract the generic token under the cursor (no special rules).
    /// Used for Tab-triggered one-off file searches.
    fn current_generic_token(textarea: &TextArea) -> Option<String> {
        let cursor_offset = textarea.cursor();
        let text = textarea.text();

        let mut safe_cursor = cursor_offset.min(text.len());
        if safe_cursor < text.len() && !text.is_char_boundary(safe_cursor) {
            safe_cursor = text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= cursor_offset)
                .last()
                .unwrap_or(0);
        }

        let before_cursor = &text[..safe_cursor];
        let after_cursor = &text[safe_cursor..];

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
        let end_idx = safe_cursor + end_rel_idx;

        if start_idx >= end_idx { return None; }

        Some(text[start_idx..end_idx].trim().to_string())
    }

    /// Replace the active `@token` (the one under the cursor) with `path`.
    ///
    /// The algorithm mirrors `current_at_token` so replacement works no matter
    /// where the cursor is within the token and regardless of how many
    /// `@tokens` exist in the line.
    fn insert_selected_path(&mut self, path: &str) {
        let cursor_offset = self.textarea.cursor();
        let text = self.textarea.text();
        // Clamp to a valid char boundary to avoid panics when slicing.
        let safe_cursor = Self::clamp_to_char_boundary(text, cursor_offset);

        let before_cursor = &text[..safe_cursor];
        let after_cursor = &text[safe_cursor..];

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
        let end_idx = safe_cursor + end_rel_idx;

        // If the path contains whitespace, wrap it in double quotes so the
        // local prompt arg parser treats it as a single argument. Avoid adding
        // quotes when the path already contains one to keep behavior simple.
        let needs_quotes = path.chars().any(char::is_whitespace);
        let inserted = if needs_quotes && !path.contains('"') {
            format!("\"{path}\"")
        } else {
            path.to_string()
        };

        // Replace the slice `[start_idx, end_idx)` with the chosen path and a trailing space.
        let mut new_text =
            String::with_capacity(text.len() - (end_idx - start_idx) + inserted.len() + 1);
        new_text.push_str(&text[..start_idx]);
        new_text.push_str(&inserted);
        new_text.push(' ');
        new_text.push_str(&text[end_idx..]);

        self.textarea.set_text(&new_text);
        let new_cursor = start_idx.saturating_add(inserted.len()).saturating_add(1);
        self.textarea.set_cursor(new_cursor);
    }

    /// Handle key event when no popup is visible.
    fn handle_key_event_without_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } if self.is_empty() => {
                self.app_event_tx.send(crate::app_event::AppEvent::ExitRequest);
                (InputResult::None, true)
            }
            // -------------------------------------------------------------
            // Shift+Tab — rotate access preset (Read Only → Write with Approval → Full Access)
            // -------------------------------------------------------------
            KeyEvent { code: KeyCode::BackTab, .. } => {
                self.app_event_tx.send(crate::app_event::AppEvent::CycleAccessMode);
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } if self.auto_drive_active && self.has_focus => {
                self
                    .app_event_tx
                    .send(crate::app_event::AppEvent::CycleAutoDriveVariant);
                (InputResult::None, true)
            }
            // -------------------------------------------------------------
            // Tab-press file search when not using @ or ./ and not in slash cmd
            // -------------------------------------------------------------
            KeyEvent { code: KeyCode::Tab, .. } => {
                // Suppress Tab completion only while the cursor is within the
                // slash command head (before the first space). Allow Tab-based
                // file search in the arguments of /plan, /solve, etc.
                if self.is_cursor_in_slash_command_head() {
                    return (InputResult::None, false);
                }

                // If already showing a file popup, let the dedicated handler manage Tab.
                if matches!(self.active_popup, ActivePopup::File(_)) {
                    return (InputResult::None, false);
                }

                // If an @ token is present or token starts with ./, rely on auto-popup.
                if Self::current_completion_token(&self.textarea).is_some() {
                    return (InputResult::None, false);
                }

                // Use the generic token under cursor for a one-off search.
                if let Some(tok) = Self::current_generic_token(&self.textarea) {
                    if !tok.is_empty() {
                        self.pending_tab_file_query = Some(tok.clone());
                        self.app_event_tx.send(crate::app_event::AppEvent::StartFileSearch(tok));
                        // Do not show a popup yet; wait for results and only
                        // show if there are matches to avoid flicker.
                        return (InputResult::None, true);
                    }
                }
                (InputResult::None, false)
            }
            // -------------------------------------------------------------
            // Handle Esc key — leave to App-level policy (clear/stop/backtrack)
            // -------------------------------------------------------------
            KeyEvent { code: KeyCode::Esc, .. } => {
                // Do nothing here so App can implement global Esc ordering.
                (InputResult::None, false)
            }
            // -------------------------------------------------------------
            // Up/Down key handling - check modifiers to determine action
            // -------------------------------------------------------------
            KeyEvent {
                code: KeyCode::Up | KeyCode::Down,
                modifiers,
                ..
            } => {
                // Check if Shift is held for history navigation
                if modifiers.contains(KeyModifiers::SHIFT) {
                    // History navigation with Shift+Up/Down
                    if self
                        .history
                        .should_handle_navigation(self.textarea.text(), self.textarea.cursor())
                    {
                        let replace_text = match key_event.code {
                            KeyCode::Up => self
                                .history
                                .navigate_up(self.textarea.text(), &self.app_event_tx),
                            KeyCode::Down => self.history.navigate_down(&self.app_event_tx),
                            _ => None,
                        };
                        if let Some(text) = replace_text {
                            self.textarea.set_text(&text);
                            self.textarea.set_cursor(0);
                            return (InputResult::None, true);
                        }
                    }
                    // If history navigation didn't happen, just ignore the key
                    (InputResult::None, false)
                } else {
                    // No Shift modifier — move cursor within the input first.
                    // Only when already at the top-left/bottom-right should Up/Down scroll chat.
                    if self.textarea.is_empty() {
                        return match key_event.code {
                            KeyCode::Up => (InputResult::ScrollUp, false),
                            KeyCode::Down => (InputResult::ScrollDown, false),
                            _ => (InputResult::None, false),
                        };
                    }

                    let before = self.textarea.cursor();
                    let len = self.textarea.text().len();
                    match key_event.code {
                        KeyCode::Up => {
                            if before == 0 {
                                (InputResult::ScrollUp, false)
                            } else {
                                // Move up a visual/logical line; if already on first line, TextArea moves to start.
                                self.textarea.input(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
                                (InputResult::None, true)
                            }
                        }
                        KeyCode::Down => {
                            // If sticky is set, prefer chat ScrollDown once
                            if self.next_down_scrolls_history {
                                self.next_down_scrolls_history = false;
                                return (InputResult::ScrollDown, false);
                            }
                            if before == len {
                                (InputResult::ScrollDown, false)
                            } else {
                                // Move down a visual/logical line; if already on last line, TextArea moves to end.
                                self.textarea.input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
                                (InputResult::None, true)
                            }
                        }
                        _ => (InputResult::None, false),
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.handle_backslash_continuation() {
                    return (InputResult::None, true);
                }
                let command_text = self.textarea.text().to_string();
                let first_line = command_text.lines().next().unwrap_or("");
                if let Some((name, rest)) = parse_slash_name(first_line)
                    && rest.is_empty()
                    && let Some((_label, cmd)) = built_in_slash_commands()
                        .into_iter()
                        .find(|(n, _)| *n == name)
                {
                    if cmd.is_prompt_expanding() {
                        self.app_event_tx.send(crate::app_event::AppEvent::PrepareAgents);
                    }
                    self.history.record_local_submission(&command_text);
                    self.app_event_tx
                        .send(crate::app_event::AppEvent::DispatchCommand(cmd, command_text));
                    self.textarea.set_text("");
                    self.active_popup = ActivePopup::None;
                    return (InputResult::Command(cmd), true);
                }

                // Record the exact text that was typed (before replacement)
                let original_text = self.textarea.text().to_string();

                let mut text = self.textarea.text().to_string();
                self.textarea.set_text("");

                // Replace all pending pastes in the text
                for (placeholder, actual) in &self.pending_pastes {
                    if text.contains(placeholder) {
                        text = text.replace(placeholder, actual);
                    }
                }
                self.pending_pastes.clear();

                if text.is_empty() {
                    (InputResult::None, true)
                } else {
                    // Check if this is a prompt-expanding command that will trigger agents
                    let trimmed = original_text.trim();
                    if trimmed.starts_with("/plan ")
                        || trimmed.starts_with("/solve ")
                        || trimmed.starts_with("/code ")
                    {
                        self.app_event_tx.send(crate::app_event::AppEvent::PrepareAgents);
                    }

                    self.history.record_local_submission(&original_text);
                    (InputResult::Submitted(text), true)
                }
            }
            input => self.handle_input_basic(input),
        }
    }

    fn handle_backslash_continuation(&mut self) -> bool {
        let text = self.textarea.text();
        if text.is_empty() {
            return false;
        }

        let mut iter = text.char_indices().rev();
        let Some((last_idx, last_char)) = iter.next() else {
            return false;
        };
        if matches!(last_char, ' ' | '\t') {
            return false;
        }
        if last_char != '\\' {
            return false;
        }

        let trailing_backslashes = text[..last_idx]
            .chars()
            .rev()
            .take_while(|c| *c == '\\')
            .count()
            + 1;
        if trailing_backslashes % 2 == 0 {
            return false;
        }

        let line_start = text[..last_idx].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
        let line_before = &text[line_start..last_idx];
        let indentation: String = line_before
            .chars()
            .take_while(|c| matches!(*c, ' ' | '\t'))
            .collect();
        let replacement = if indentation.is_empty() {
            String::from("\n")
        } else {
            format!("\n{indentation}")
        };
        let backslash_end = last_idx + '\\'.len_utf8();
        self.textarea.replace_range(last_idx..backslash_end, &replacement);

        self.history.reset_navigation();
        self.post_paste_space_guard = None;
        self.pending_pastes
            .retain(|(placeholder, _)| self.textarea.text().contains(placeholder));
        self.typed_anything = true;
        true
    }

    /// Handle generic Input events that modify the textarea content.
    fn handle_input_basic(&mut self, input: KeyEvent) -> (InputResult, bool) {
        let text_before = self.textarea.text().to_string();

        if self.should_suppress_post_paste_space(&input) {
            return (InputResult::None, false);
        }

        // Special handling for backspace on placeholders
        if let KeyEvent {
            code: KeyCode::Backspace,
            ..
        } = input
        {
            if self.try_remove_placeholder_at_cursor() {
                // Text was modified, reset history navigation
                self.history.reset_navigation();
                return (InputResult::None, true);
            }
        }

        // Normal input handling
        self.textarea.input(input);
        let text_after = self.textarea.text();

        if text_before != text_after {
            self.post_paste_space_guard = None;
        } else if self
            .post_paste_space_guard
            .as_ref()
            .map(|guard| self.textarea.cursor() != guard.cursor_pos)
            .unwrap_or(false)
        {
            self.post_paste_space_guard = None;
        }

        // If text changed, reset history navigation state
        if text_before != text_after {
            self.history.reset_navigation();
            if !text_after.is_empty() { self.typed_anything = true; }
        }

        // Check if any placeholders were removed and remove their corresponding pending pastes
        self.pending_pastes
            .retain(|(placeholder, _)| text_after.contains(placeholder));

        (InputResult::None, true)
    }

    /// Attempts to remove a placeholder if the cursor is at the end of one.
    /// Returns true if a placeholder was removed.
    fn try_remove_placeholder_at_cursor(&mut self) -> bool {
        let text = self.textarea.text();
        let p = Self::clamp_to_char_boundary(text, self.textarea.cursor());

        // Find any placeholder that ends at the cursor position
        let placeholder_to_remove = self.pending_pastes.iter().find_map(|(ph, _)| {
            if p < ph.len() {
                return None;
            }
            let potential_ph_start = p - ph.len();
            // Use `get` to avoid panicking on non-char-boundary indices.
            match text.get(potential_ph_start..p) {
                Some(slice) if slice == ph => Some(ph.clone()),
                _ => None,
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

    /// Synchronize `self.command_popup` with the current text in the
    /// textarea. This must be called after every modification that can change
    /// the text so the popup is shown/updated/hidden as appropriate.
    fn sync_command_popup(&mut self) {
        let first_line = self.textarea.text().lines().next().unwrap_or("");
        let input_starts_with_slash = first_line.starts_with('/');
        // Keep the slash popup only while the cursor is within the command head
        // (before the first space). This allows @-file completion for arguments
        // in commands like "/plan" and "/solve".
        let in_slash_head = self.is_cursor_in_slash_command_head();
        match &mut self.active_popup {
            ActivePopup::Command(popup) => {
                if input_starts_with_slash && in_slash_head {
                    popup.on_composer_text_change(first_line.to_string());
                } else {
                    self.active_popup = ActivePopup::None;
                }
            }
            _ => {
                if input_starts_with_slash && in_slash_head {
                    let mut command_popup = CommandPopup::new_with_filter(self.using_chatgpt_auth);
                    // Load saved subagent commands to include in autocomplete (exclude built-ins)
                    if let Ok(cfg) = code_core::config::Config::load_with_cli_overrides(vec![], code_core::config::ConfigOverrides::default()) {
                        let mut names: Vec<String> = cfg
                            .subagent_commands
                            .iter()
                            .map(|c| c.name.clone())
                            .filter(|n| {
                                let l = n.to_ascii_lowercase();
                                l != "plan" && l != "solve" && l != "code"
                            })
                            .collect();
                        // Stable sort for presentation
                        names.sort();
                        command_popup.set_subagent_commands(names);
                    }
                    command_popup.on_composer_text_change(first_line.to_string());
                    self.active_popup = ActivePopup::Command(command_popup);
                    // Notify app: composer expanded due to slash popup
                    self.app_event_tx.send(crate::app_event::AppEvent::ComposerExpanded);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_custom_prompts(&mut self, prompts: Vec<CustomPrompt>) {
        if let ActivePopup::Command(popup) = &mut self.active_popup {
            popup.set_prompts(prompts);
        }
    }

    /// Synchronize `self.file_search_popup` with the current text in the textarea.
    /// Note this is only called when self.active_popup is NOT Command.
    fn sync_file_search_popup(&mut self) {
        // Determine if there is a token underneath the cursor worth completing.
        match Self::current_completion_token(&self.textarea) {
            Some(query) => {
                if self.dismissed_file_popup_token.as_ref() == Some(&query) {
                    return;
                }

                if query.chars().count() >= 1 {
                    self.app_event_tx
                        .send(crate::app_event::AppEvent::StartFileSearch(query.clone()));
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
                self.file_popup_origin = Some(FilePopupOrigin::Auto);
                self.dismissed_file_popup_token = None;
            }
            None => {
                // Allow manually-triggered popups (via Tab) to stay open while the
                // cursor remains within the same generic token. When the token
                // changes, trigger a new search; otherwise keep the popup stable.
                if let Some(FilePopupOrigin::Manual { token }) = &mut self.file_popup_origin {
                    if let Some(generic) = Self::current_generic_token(&self.textarea) {
                        if generic.is_empty() {
                            self.active_popup = ActivePopup::None;
                            self.dismissed_file_popup_token = None;
                            self.file_popup_origin = None;
                            self.current_file_query = None;
                        } else {
                            if *token != generic {
                                *token = generic.clone();
                                if let ActivePopup::File(popup) = &mut self.active_popup {
                                    popup.set_query(&generic);
                                } else {
                                    let mut popup = FileSearchPopup::new();
                                    popup.set_query(&generic);
                                    self.active_popup = ActivePopup::File(popup);
                                }
                                self.current_file_query = Some(generic.clone());
                                self.app_event_tx
                                    .send(crate::app_event::AppEvent::StartFileSearch(generic));
                            }
                            // Keep popup visible while token is still active.
                        }
                        return;
                    }
                }

                self.active_popup = ActivePopup::None;
                self.dismissed_file_popup_token = None;
                self.file_popup_origin = None;
                self.current_file_query = None;
            }
        }
    }

    pub(crate) fn set_has_focus(&mut self, has_focus: bool) {
        self.has_focus = has_focus;
    }

    // -------------------------------------------------------------
    // History navigation helpers (used by ChatWidget at scroll boundaries)
    // -------------------------------------------------------------
    pub(crate) fn try_history_up(&mut self) -> bool {
        if !self
            .history
            .should_handle_navigation(self.textarea.text(), self.textarea.cursor())
        {
            return false;
        }
        if let Some(text) = self.history.navigate_up(self.textarea.text(), &self.app_event_tx) {
            self.textarea.set_text(&text);
            self.textarea.set_cursor(0);
        }
        true
    }

    pub(crate) fn try_history_down(&mut self) -> bool {
        // Only meaningful when browsing or when original text is recorded
        if !self
            .history
            .should_handle_navigation(self.textarea.text(), self.textarea.cursor())
        {
            return false;
        }
        if let Some(text) = self.history.navigate_down(&self.app_event_tx) {
            self.textarea.set_text(&text);
            self.textarea.set_cursor(0);
        }
        true
    }

    pub(crate) fn history_is_browsing(&self) -> bool { self.history.is_browsing() }

    pub(crate) fn mark_next_down_scrolls_history(&mut self) {
        self.next_down_scrolls_history = true;
    }

    pub(crate) fn token_usage_spans(&self, label_style: Style) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if let Some(token_usage_info) = &self.token_usage_info {
            let turn_usage = &token_usage_info.last_token_usage;
            let tokens_used = turn_usage.tokens_in_context_window();
            let used_str = format_with_thousands(tokens_used);
            spans.push(Span::from(used_str).style(label_style.add_modifier(Modifier::BOLD)));
            spans.push(Span::from(" tokens").style(label_style));
            if let Some(context_window) = token_usage_info.model_context_window {
                if context_window > 0 {
                    let percent_remaining = {
                        let percent = 100.0
                            - (tokens_used as f32 / context_window as f32 * 100.0);
                        percent.clamp(0.0, 100.0) as u8
                    };
                    spans.push(Span::from(" (").style(label_style));
                    spans.push(Span::from(percent_remaining.to_string()).style(label_style.add_modifier(Modifier::BOLD)));
                    spans.push(Span::from("% left)").style(label_style));
                }
            }
        }
        spans
    }

    fn build_auto_drive_hint_spans(
        text: &str,
        key_style: Style,
        label_style: Style,
    ) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let parts: Vec<String> = text
            .split('•')
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect();

        for (index, part) in parts.iter().enumerate() {
            if index > 0 {
                spans.push(Span::from("  •  ".to_string()).style(label_style));
            }
            let (key, label) = Self::split_auto_drive_key_label(part);
            if let Some(key) = key {
                spans.push(Span::from(key).style(key_style));
                if !label.is_empty() {
                    spans.push(Span::from(format!(" {}", label)).style(label_style));
                }
            } else if !label.is_empty() {
                spans.push(Span::from(label).style(label_style));
            }
        }

        spans
    }

    fn split_auto_drive_key_label(part: &str) -> (Option<String>, String) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return (None, String::new());
        }
        if let Some((first, rest)) = trimmed.split_once(' ') {
            let key = first.trim();
            let remainder = rest.trim_start();
            if Self::is_auto_drive_key(key) {
                return (Some(key.to_string()), remainder.to_string());
            }
        }
        (None, trimmed.to_string())
    }

    fn is_auto_drive_key(token: &str) -> bool {
        let normalized = token.trim();
        if normalized.is_empty() {
            return false;
        }
        if normalized.contains('+') {
            return true;
        }
        matches!(normalized, "Esc" | "Enter" | "Tab" | "Space" | "Backspace")
    }

    pub(crate) fn footer_height(&self) -> u16 {
        if self.render_mode == ComposerRenderMode::FooterOnly {
            return if self.standard_terminal_hint.is_some() { 1 } else { 0 };
        }

        match (&self.active_popup, self.embedded_mode) {
            (ActivePopup::Command(popup), _) => popup.calculate_required_height(),
            (ActivePopup::File(popup), _) => popup.calculate_required_height(),
            (ActivePopup::None, true) => 0,
            (ActivePopup::None, false) => 1,
        }
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        match &self.active_popup {
            ActivePopup::Command(popup) => {
                popup.render_ref(area, buf);
            }
            ActivePopup::File(popup) => {
                popup.render_ref(area, buf);
            }
            ActivePopup::None => {
                if self.embedded_mode {
                    return;
                }

                let key_hint_style = Style::default().fg(crate::colors::function());
                let label_style = Style::default().fg(crate::colors::text_dim());

                if let Some(hints) = &self.footer_hint_override {
                    let mut left_spans: Vec<Span<'static>> = vec![Span::from("  ")];
                    for (idx, (key, label)) in hints.iter().enumerate() {
                        if idx > 0 {
                            left_spans.push(Span::from("   ").style(label_style));
                        }
                        if !key.is_empty() {
                            left_spans.push(Span::from(key.clone()).style(key_hint_style));
                        }
                        if !label.is_empty() {
                            let prefix = if key.is_empty() { String::new() } else { String::from(" ") };
                            left_spans.push(Span::from(format!("{prefix}{label}")).style(label_style));
                        }
                    }

                    let token_spans: Vec<Span<'static>> = self.token_usage_spans(label_style);
                    let left_len: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();
                    let right_len: usize = token_spans.iter().map(|s| s.content.chars().count()).sum();
                    let total_width = area.width as usize;
                    let trailing_pad = 1usize;
                    let spacer = if total_width > left_len + right_len + trailing_pad {
                        " ".repeat(total_width - left_len - right_len - trailing_pad)
                    } else {
                        String::from(" ")
                    };

                    let mut line_spans = left_spans;
                    line_spans.push(Span::from(spacer));
                    line_spans.extend(token_spans);
                    line_spans.push(Span::from(" "));

                    Line::from(line_spans)
                        .style(
                            Style::default()
                                .fg(crate::colors::text_dim())
                                .add_modifier(Modifier::DIM),
                        )
                        .render_ref(area, buf);
                    return;
                }

                let mut left_spans: Vec<Span> = vec![Span::from("  ")];
                let mut right_spans: Vec<Span<'static>> = Vec::new();

                let show_access_label = if let Some(until) = self.access_mode_label_expiry {
                    std::time::Instant::now() <= until
                } else {
                    true
                };
                if show_access_label && !self.auto_drive_active {
                    if let Some(label) = &self.access_mode_label {
                        left_spans.push(Span::from(label.clone()).style(label_style));
                        let show_suffix = if let Some(until) = self.access_mode_hint_expiry {
                            std::time::Instant::now() <= until
                        } else {
                            self.access_mode_label_expiry.is_some()
                        };
                        if show_suffix {
                            left_spans.push(Span::from("  (").style(label_style));
                            left_spans.push(Span::from("Shift+Tab").style(key_hint_style));
                            left_spans.push(Span::from(" change)").style(label_style));
                        }
                    }
                }

                if self.ctrl_c_quit_hint {
                    if self.access_mode_label.is_some() {
                        left_spans.push(Span::from("   "));
                    }
                    left_spans.push(Span::from("Ctrl+C").style(key_hint_style));
                    left_spans.push(Span::from(" again to quit").style(label_style));
                }

                if let Some(hint) = &self.standard_terminal_hint {
                    if self.auto_drive_active {
                        let (left_hint, right_hint) = match hint.split_once('\t') {
                            Some((left, right)) => (left.trim().to_string(), Some(right.trim().to_string())),
                            None => (hint.trim().to_string(), None),
                        };

                        let auto_label_style = Style::default().fg(crate::colors::text_dim());
                        let auto_key_style = Style::default().fg(crate::colors::info());

                        if !left_hint.is_empty() {
                            if left_spans.len() > 1 {
                                left_spans.push(Span::from("   ").style(auto_label_style));
                            }
                            let spans = Self::build_auto_drive_hint_spans(
                                &left_hint,
                                auto_key_style,
                                auto_label_style,
                            );
                            left_spans.extend(spans);
                        }

                        if let Some(right_hint) = right_hint {
                            if !right_hint.is_empty() {
                                let spans = Self::build_auto_drive_hint_spans(
                                    &right_hint,
                                    auto_key_style,
                                    auto_label_style,
                                );
                                if !spans.is_empty() && !right_spans.is_empty() {
                                    right_spans.push(Span::from("  •  ").style(auto_label_style));
                                }
                                right_spans.extend(spans);
                            }
                        }
                    } else {
                        if left_spans.len() > 1 {
                            left_spans.push(Span::from("   "));
                        }
                        left_spans.push(
                            Span::from(hint.clone())
                                .style(Style::default().fg(crate::colors::warning()).add_modifier(Modifier::BOLD)),
                        );
                    }
                }

                if let Some((msg, until)) = &self.footer_notice {
                    if std::time::Instant::now() <= *until {
                        if left_spans.len() > 1 {
                            left_spans.push(Span::from("   "));
                        }
                        left_spans.push(
                            Span::from(msg.clone()).style(Style::default().add_modifier(Modifier::DIM)),
                        );
                    }
                }

                let token_spans: Vec<Span<'static>> = self.token_usage_spans(label_style);

                let build_hints = |include_reasoning: bool,
                                   include_diff: bool|
                 -> Vec<Span<'static>> {
                    let mut spans: Vec<Span<'static>> = Vec::new();
                    if self.auto_drive_active {
                        return spans;
                    }
                    if !self.ctrl_c_quit_hint {
                        if self.show_reasoning_hint && include_reasoning {
                            if !spans.is_empty() {
                                spans.push(Span::from("  •  ").style(label_style));
                            }
                            spans.push(Span::from("Ctrl+R").style(key_hint_style));
                            let label = if self.reasoning_shown {
                                " hide reasoning"
                            } else {
                                " show reasoning"
                            };
                            spans.push(Span::from(label).style(label_style));
                        }
                        if self.show_diffs_hint && include_diff {
                            if !spans.is_empty() {
                                spans.push(Span::from("  •  ").style(label_style));
                            }
                            spans.push(Span::from("Ctrl+D").style(key_hint_style));
                            spans.push(Span::from(" diff viewer").style(label_style));
                        }
                        if !spans.is_empty() {
                            spans.push(Span::from("  •  ").style(label_style));
                        }
                        spans.push(Span::from("Ctrl+H").style(key_hint_style));
                        spans.push(Span::from(" help").style(label_style));
                    }
                    spans
                };

                let mut include_reasoning = true;
                let mut include_diff = true;
                let mut hint_spans = build_hints(include_reasoning, include_diff);

                if self.auto_drive_active {
                    hint_spans.clear();

                    let mut auto_drive_hint_spans: Vec<Span<'static>> = Vec::new();
                    auto_drive_hint_spans.push(Span::from("Ctrl+S").style(key_hint_style));
                    auto_drive_hint_spans.push(Span::from(" settings").style(label_style));
                    auto_drive_hint_spans.push(Span::from(" • ").style(label_style));
                    auto_drive_hint_spans.push(Span::from("Esc").style(key_hint_style));
                    auto_drive_hint_spans.push(Span::from(" stop Auto Drive").style(label_style));

                    if !auto_drive_hint_spans.is_empty() {
                        if !right_spans.is_empty() {
                            right_spans.push(Span::from("  •  ").style(label_style));
                        }
                        right_spans.extend(auto_drive_hint_spans);
                    }
                }

                let measure = |spans: &[Span<'static>]| -> usize {
                    spans.iter().map(|s| s.content.chars().count()).sum()
                };

                let base_right_len = measure(&right_spans);
                let has_base_right = !right_spans.is_empty();
                let total_width = area.width as usize;
                let trailing_pad = 1usize;

                let mut auth_spans: Vec<Span<'static>> = if self.using_chatgpt_auth {
                    Vec::new()
                } else {
                    vec![Span::from("API key").style(label_style)]
                };

                let separator_len = "  •  ".chars().count();
                let combined_len =
                    |h: &[Span<'static>], t: &[Span<'static>], a: &[Span<'static>]| -> usize {
                        let mut len = base_right_len;
                        let mut sections = if has_base_right { 1 } else { 0 };

                        if !h.is_empty() {
                            if sections > 0 {
                                len += separator_len;
                            }
                            len += measure(h);
                            sections += 1;
                        }
                        if !t.is_empty() {
                            if sections > 0 {
                                len += separator_len;
                            }
                            len += measure(t);
                            sections += 1;
                        }
                        if !a.is_empty() {
                            if sections > 0 {
                                len += separator_len;
                            }
                            len += measure(a);
                        }

                        len
                    };

                let mut left_len: usize = left_spans.iter().map(|s| s.content.chars().count()).sum();

                while left_len + combined_len(&hint_spans, &token_spans, &auth_spans) + trailing_pad
                    > total_width
                {
                    if include_reasoning {
                        include_reasoning = false;
                    } else if include_diff {
                        include_diff = false;
                    } else if !auth_spans.is_empty() {
                        auth_spans.clear();
                    } else {
                        break;
                    }
                    hint_spans = build_hints(include_reasoning, include_diff);
                }

                if !hint_spans.is_empty() {
                    if !right_spans.is_empty() {
                        right_spans.push(Span::from("  •  ").style(label_style));
                    }
                    right_spans.extend(hint_spans);
                }
                if !token_spans.is_empty() {
                    if !right_spans.is_empty() {
                        right_spans.push(Span::from("  •  ").style(label_style));
                    }
                    right_spans.extend(token_spans);
                }
                if !auth_spans.is_empty() {
                    if !right_spans.is_empty() {
                        right_spans.push(Span::from("  •  ").style(label_style));
                    }
                    right_spans.extend(auth_spans);
                }

                let right_len: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
                if left_len + right_len + trailing_pad > total_width {
                    let mut remaining = total_width.saturating_sub(right_len + trailing_pad);
                    if remaining == 0 {
                        left_spans.clear();
                    } else {
                        let mut truncated: Vec<Span> = Vec::new();
                        for span in left_spans.iter() {
                            if remaining == 0 {
                                break;
                            }
                            let span_len = span.content.chars().count();
                            if span_len <= remaining {
                                truncated.push(span.clone());
                                remaining -= span_len;
                                continue;
                            }

                            if span.content.trim().is_empty() {
                                truncated.push(Span::from(" ".repeat(remaining)).style(span.style));
                                remaining = 0;
                            } else if remaining <= 1 {
                                truncated.push(Span::from("…").style(span.style));
                                remaining = 0;
                            } else {
                                let mut collected = span
                                    .content
                                    .chars()
                                    .take(remaining)
                                    .collect::<String>();
                                if !collected.is_empty() {
                                    collected.pop();
                                }
                                collected.push('…');
                                truncated.push(Span::from(collected).style(span.style));
                                remaining = 0;
                            }
                        }
                        if truncated.is_empty() {
                            truncated.push(Span::from("  "));
                        }
                        left_spans = truncated;
                    }
                    left_len = left_spans.iter().map(|s| s.content.chars().count()).sum();
                }

                let spacer = if total_width > left_len + right_len + trailing_pad {
                    " ".repeat(total_width - left_len - right_len - trailing_pad)
                } else {
                    String::from(" ")
                };

                let mut line_spans = left_spans;
                line_spans.push(Span::from(spacer));
                line_spans.extend(right_spans);
                line_spans.push(Span::from(" "));

                Line::from(line_spans)
                    .style(
                        Style::default()
                            .fg(crate::colors::text_dim())
                            .add_modifier(Modifier::DIM),
                    )
                    .render_ref(area, buf);
            }
        }
    }
}

impl WidgetRef for ChatComposer {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if self.render_mode == ComposerRenderMode::FooterOnly {
            let footer_height = self.footer_height();
            if footer_height == 0 {
                return;
            }
            let footer_area = Rect {
                x: area.x,
                y: area.y + area.height.saturating_sub(footer_height),
                width: area.width,
                height: footer_height,
            };
            self.render_footer(footer_area, buf);
            return;
        }

        let footer_height = self.footer_height();

        let content_width = area.width.saturating_sub(4);
        let content_lines = self.textarea.desired_height(content_width).max(1);
        let desired_input_height = (content_lines + 2).max(3);

        let available_height = area.height.saturating_sub(footer_height);
        if available_height == 0 {
            return;
        }
        let input_height = desired_input_height.min(available_height);

        let (input_area, footer_area) = if footer_height == 0 {
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: available_height,
                },
                None,
            )
        } else {
            let [input_area, footer_area] = Layout::vertical([
                Constraint::Length(input_height),
                Constraint::Length(footer_height),
            ])
            .areas(area);
            (input_area, Some(footer_area))
        };

        if let Some(area) = footer_area {
            self.render_footer(area, buf);
        }
        // Draw border around input area with optional variant title when task is running
        let mut input_block = Block::default().borders(Borders::ALL);
        let mut auto_drive_border_gradient = None;
        if let Some(style) = self
            .auto_drive_style
            .as_ref()
            .filter(|_| self.auto_drive_active)
        {
            auto_drive_border_gradient = style.border_gradient;
            input_block = input_block
                .border_style(style.border_style.clone())
                .border_type(style.border_type)
                .style(style.background_style.clone());
        } else {
            input_block = input_block
                .border_style(Style::default().fg(crate::colors::border()))
                .border_type(BorderType::Plain)
                .style(Style::default().bg(crate::colors::background()));
        }

        if self.is_task_running && !self.embedded_mode {
            if self.auto_drive_active {
                if let Some(style) = self.auto_drive_style.as_ref() {
                    if self
                        .status_message
                        .eq_ignore_ascii_case("auto drive goal")
                    {
                        let title_text = format!(
                            "{}Auto Drive Goal{}",
                            style.goal_title_prefix, style.goal_title_suffix
                        );
                        let title_line =
                            Line::from(Span::styled(title_text, style.title_style.clone()));
                        input_block = input_block.title(title_line);
                    }
                }
            } else {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let def = crate::spinner::current_spinner();
                let spinner_str = crate::spinner::frame_at_time(def, now_ms);

                let title_line = Line::from(vec![
                    Span::raw(" "),
                    Span::styled(spinner_str, Style::default().fg(crate::colors::info())),
                    Span::styled(
                        format!(" {}... ", self.status_message),
                        Style::default().fg(crate::colors::info()),
                    ),
                ])
                .centered();
                input_block = input_block.title(title_line);
            }
        }

        let textarea_rect = input_block.inner(input_area);
        input_block.render_ref(input_area, buf);
        if let Some(gradient) = auto_drive_border_gradient {
            apply_auto_drive_border_gradient(buf, input_area, gradient);
        }

        // Add padding inside the text area (1 char horizontal only, no vertical padding)
        let padded_textarea_rect = textarea_rect.inner(Margin::new(1, 0));

        let mut state = self.textarea_state.borrow_mut();
        StatefulWidgetRef::render_ref(&(&self.textarea), padded_textarea_rect, buf, &mut state);
        // Only show placeholder if there's no chat history AND no text typed
        if !self.typed_anything && self.textarea.text().is_empty() {
            let placeholder = crate::greeting::greeting_placeholder();
            Line::from(placeholder)
                .style(Style::default().dim())
                .render_ref(padded_textarea_rect, buf);
        }

        // Draw a high-contrast cursor overlay under the terminal cursor using the theme's
        // `cursor` color. This improves visibility on dark themes where the terminal's own
        // cursor color may be hard to see or user-defined.
        //
        // Implementation notes:
        // - We compute the visible cursor position using the same `state` (scroll) used to
        //   render the textarea so the overlay aligns with wrapped lines.
        // - We paint the underlying cell with bg=theme.cursor and fg=theme.background.
        //   This provides contrast regardless of light/dark theme.
        // - The hardware cursor is still positioned via `frame.set_cursor_position` at the
        //   app layer; this overlay ensures visibility independent of terminal settings.
        drop(state); // release the borrow before computing position again
        if let Some((cx, cy)) = self
            .textarea
            .cursor_pos_with_state(padded_textarea_rect, *self.textarea_state.borrow())
        {
            let cursor_bg = crate::theme::current_theme().cursor;
            if cx < buf.area.width.saturating_add(buf.area.x)
                && cy < buf.area.height.saturating_add(buf.area.y)
            {
                let cell = &mut buf[(cx, cy)];
                // Only tint the background so the foreground glyph stays intact. Some
                // terminals (e.g. GNOME Terminal/VTE) temporarily hide the hardware
                // cursor while processing arrow keys; preserving the foreground color
                // keeps the caret location visible instead of flashing blank cells.
                cell.set_bg(cursor_bg);
            }
        }
    }
}

fn apply_auto_drive_border_gradient(
    buf: &mut Buffer,
    area: Rect,
    gradient: BorderGradient,
) {
    let width = area.width as usize;
    let height = area.height as usize;
    if width == 0 || height == 0 {
        return;
    }

    let horizontal_span = (width.saturating_sub(1)) as f32;
    for dx in 0..width {
        let ratio = if horizontal_span <= 0.0 {
            0.0
        } else {
            dx as f32 / horizontal_span
        };
        let color = lerp_gradient_color(gradient, ratio);
        let x = area.x + dx as u16;
        let top = &mut buf[(x, area.y)];
        top.set_fg(color);

        if height > 1 {
            let bottom_y = area.y + area.height.saturating_sub(1);
            let bottom = &mut buf[(x, bottom_y)];
            bottom.set_fg(color);
        }
    }

    if height <= 2 {
        return;
    }

    let left_x = area.x;
    let right_x = area.x + area.width.saturating_sub(1);
    for dy in 1..height.saturating_sub(1) {
        let y = area.y + dy as u16;
        buf[(left_x, y)].set_fg(gradient.left);
        buf[(right_x, y)].set_fg(gradient.right);
    }
}

fn lerp_gradient_color(gradient: BorderGradient, ratio: f32) -> Color {
    let clamped = ratio.clamp(0.0, 1.0);
    let (lr, lg, lb) = crate::colors::color_to_rgb(gradient.left);
    let (rr, rg, rb) = crate::colors::color_to_rgb(gradient.right);
    let mix = |a: u8, b: u8| -> u8 {
        let a = a as f32;
        let b = b as f32;
        (a + (b - a) * clamped).round().clamp(0.0, 255.0) as u8
    };
    Color::Rgb(mix(lr, rr), mix(lg, rg), mix(lb, rb))
}
