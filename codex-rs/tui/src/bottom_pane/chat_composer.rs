//! The chat composer is the bottom-pane text input state machine.
//!
//! It is responsible for:
//!
//! - Editing the input buffer (a [`TextArea`]), including placeholder "elements" for attachments.
//! - Routing keys to the active popup (slash commands, file search, skill/apps mentions).
//! - Handling submit vs newline on Enter.
//! - Turning raw key streams into explicit paste operations on platforms where terminals
//!   don't provide reliable bracketed paste (notably Windows).
//!
//! # Key Event Routing
//!
//! Most key handling goes through [`ChatComposer::handle_key_event`], which dispatches to a
//! popup-specific handler if a popup is visible and otherwise to
//! [`ChatComposer::handle_key_event_without_popup`]. After every handled key, we call
//! [`ChatComposer::sync_popups`] so UI state follows the latest buffer/cursor.
//!
//! # History Navigation (↑/↓)
//!
//! The Up/Down history path is managed by [`ChatComposerHistory`]. It merges:
//!
//! - Persistent cross-session history (text-only; no element ranges or attachments).
//! - Local in-session history (full text + text elements + local image paths).
//!
//! When recalling a local entry, the composer rehydrates text elements and image attachments.
//! When recalling a persistent entry, only the text is restored.
//!
//! # Submission and Prompt Expansion
//!
//! On submit/queue paths, the composer:
//!
//! - Expands pending paste placeholders so element ranges align with the final text.
//! - Trims whitespace and rebases text elements accordingly.
//! - Expands `/prompts:` custom prompts (named or numeric args), preserving text elements.
//! - Prunes attached images so only placeholders that survive expansion are sent.
//!
//! The numeric auto-submit path used by the slash popup performs the same pending-paste expansion
//! and attachment pruning, and clears pending paste state on success.
//!
//! # Non-bracketed Paste Bursts
//!
//! On some terminals (especially on Windows), pastes arrive as a rapid sequence of
//! `KeyCode::Char` and `KeyCode::Enter` key events instead of a single paste event.
//!
//! To avoid misinterpreting these bursts as real typing (and to prevent transient UI effects like
//! shortcut overlays toggling on a pasted `?`), we feed "plain" character events into
//! [`PasteBurst`](super::paste_burst::PasteBurst), which buffers bursts and later flushes them
//! through [`ChatComposer::handle_paste`].
//!
//! The burst detector intentionally treats ASCII and non-ASCII differently:
//!
//! - ASCII: we briefly hold the first fast char (flicker suppression) until we know whether the
//!   stream is paste-like.
//! - non-ASCII: we do not hold the first char (IME input would feel dropped), but we still allow
//!   burst detection for actual paste streams.
//!
//! The burst detector can also be disabled (`disable_paste_burst`), which bypasses the state
//! machine and treats the key stream as normal typing. When toggling from enabled → disabled, the
//! composer flushes/clears any in-flight burst state so it cannot leak into subsequent input.
//!
//! For the detailed burst state machine, see `codex-rs/tui/src/bottom_pane/paste_burst.rs`.
//! For a narrative overview of the combined state machine, see `docs/tui-chat-composer.md`.
//!
//! # PasteBurst Integration Points
//!
//! The burst detector is consulted in a few specific places:
//!
//! - [`ChatComposer::handle_input_basic`]: flushes any due burst first, then intercepts plain char
//!   input to either buffer it or insert normally.
//! - [`ChatComposer::handle_non_ascii_char`]: handles the non-ASCII/IME path without holding the
//!   first char, while still allowing paste detection via retro-capture.
//! - [`ChatComposer::flush_paste_burst_if_due`]/[`ChatComposer::handle_paste_burst_flush`]: called
//!   from UI ticks to turn a pending burst into either an explicit paste (`handle_paste`) or a
//!   normal typed character.
//!
//! # Input Disabled Mode
//!
//! The composer can be temporarily read-only (`input_enabled = false`). In that mode it ignores
//! edits and renders a placeholder prompt instead of the editable textarea. This is part of the
//! overall state machine, since it affects which transitions are even possible from a given UI
//! state.
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::has_ctrl_or_alt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::WidgetRef;

use super::chat_composer_history::ChatComposerHistory;
use super::chat_composer_history::HistoryEntry;
use super::command_popup::CommandItem;
use super::command_popup::CommandPopup;
use super::command_popup::CommandPopupFlags;
use super::file_search_popup::FileSearchPopup;
use super::footer::CollaborationModeIndicator;
use super::footer::FooterMode;
use super::footer::FooterProps;
use super::footer::SummaryLeft;
use super::footer::can_show_left_with_context;
use super::footer::context_window_line;
use super::footer::esc_hint_mode;
use super::footer::footer_height;
use super::footer::footer_hint_items_width;
use super::footer::footer_line_width;
use super::footer::inset_footer_hint_area;
use super::footer::render_context_right;
use super::footer::render_footer_from_props;
use super::footer::render_footer_hint_items;
use super::footer::render_footer_line;
use super::footer::reset_mode_after_activity;
use super::footer::single_line_footer_layout;
use super::footer::toggle_shortcut_mode;
use super::paste_burst::CharDecision;
use super::paste_burst::PasteBurst;
use super::skill_popup::MentionItem;
use super::skill_popup::SkillPopup;
use super::slash_commands;
use crate::bottom_pane::paste_burst::FlushResult;
use crate::bottom_pane::prompt_args::expand_custom_prompt;
use crate::bottom_pane::prompt_args::expand_if_numeric_with_positional_args;
use crate::bottom_pane::prompt_args::parse_slash_name;
use crate::bottom_pane::prompt_args::prompt_argument_names;
use crate::bottom_pane::prompt_args::prompt_command_with_arg_placeholders;
use crate::bottom_pane::prompt_args::prompt_has_numeric_placeholders;
use crate::render::Insets;
use crate::render::RectExt;
use crate::render::renderable::Renderable;
use crate::slash_command::SlashCommand;
use crate::style::user_message_style;
use codex_common::fuzzy_match::fuzzy_match;
use codex_protocol::custom_prompts::CustomPrompt;
use codex_protocol::custom_prompts::PROMPTS_CMD_PREFIX;
use codex_protocol::models::local_image_label_text;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;

use crate::app_event::AppEvent;
use crate::app_event::ConnectorsSnapshot;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::LocalImageAttachment;
use crate::bottom_pane::textarea::TextArea;
use crate::bottom_pane::textarea::TextAreaState;
use crate::clipboard_paste::normalize_pasted_path;
use crate::clipboard_paste::pasted_image_format;
use crate::history_cell;
use crate::ui_consts::LIVE_PREFIX_COLS;
use codex_chatgpt::connectors;
use codex_chatgpt::connectors::AppInfo;
use codex_core::skills::model::SkillMetadata;
use codex_file_search::FileMatch;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

/// If the pasted content exceeds this number of characters, replace it with a
/// placeholder in the UI.
const LARGE_PASTE_CHAR_THRESHOLD: usize = 1000;

/// Result returned when the user interacts with the text area.
#[derive(Debug, PartialEq)]
pub enum InputResult {
    Submitted {
        text: String,
        text_elements: Vec<TextElement>,
    },
    Queued {
        text: String,
        text_elements: Vec<TextElement>,
    },
    Command(SlashCommand),
    CommandWithArgs(SlashCommand, String),
    None,
}

#[derive(Clone, Debug, PartialEq)]
struct AttachedImage {
    placeholder: String,
    path: PathBuf,
}

enum PromptSelectionMode {
    Completion,
    Submit,
}

enum PromptSelectionAction {
    Insert {
        text: String,
        cursor: Option<usize>,
    },
    Submit {
        text: String,
        text_elements: Vec<TextElement>,
    },
}

/// Feature flags for reusing the chat composer in other bottom-pane surfaces.
///
/// The default keeps today's behavior intact. Other call sites can opt out of
/// specific behaviors by constructing a config with those flags set to `false`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChatComposerConfig {
    /// Whether command/file/skill popups are allowed to appear.
    pub(crate) popups_enabled: bool,
    /// Whether `/...` input is parsed and dispatched as slash commands.
    pub(crate) slash_commands_enabled: bool,
    /// Whether pasting a file path can attach local images.
    pub(crate) image_paste_enabled: bool,
}

impl Default for ChatComposerConfig {
    fn default() -> Self {
        Self {
            popups_enabled: true,
            slash_commands_enabled: true,
            image_paste_enabled: true,
        }
    }
}

impl ChatComposerConfig {
    /// A minimal preset for plain-text inputs embedded in other surfaces.
    ///
    /// This disables popups, slash commands, and image-path attachment behavior
    /// so the composer behaves like a simple notes field.
    pub(crate) const fn plain_text() -> Self {
        Self {
            popups_enabled: false,
            slash_commands_enabled: false,
            image_paste_enabled: false,
        }
    }
}
pub(crate) struct ChatComposer {
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    active_popup: ActivePopup,
    app_event_tx: AppEventSender,
    history: ChatComposerHistory,
    quit_shortcut_expires_at: Option<Instant>,
    quit_shortcut_key: KeyBinding,
    esc_backtrack_hint: bool,
    use_shift_enter_hint: bool,
    dismissed_file_popup_token: Option<String>,
    current_file_query: Option<String>,
    pending_pastes: Vec<(String, String)>,
    large_paste_counters: HashMap<usize, usize>,
    has_focus: bool,
    /// Invariant: attached images are labeled `[Image #1]..[Image #N]` in vec order.
    attached_images: Vec<AttachedImage>,
    placeholder_text: String,
    is_task_running: bool,
    /// When false, the composer is temporarily read-only (e.g. during sandbox setup).
    input_enabled: bool,
    input_disabled_placeholder: Option<String>,
    /// Non-bracketed paste burst tracker (see `bottom_pane/paste_burst.rs`).
    paste_burst: PasteBurst,
    // When true, disables paste-burst logic and inserts characters immediately.
    disable_paste_burst: bool,
    custom_prompts: Vec<CustomPrompt>,
    footer_mode: FooterMode,
    footer_hint_override: Option<Vec<(String, String)>>,
    footer_flash: Option<FooterFlash>,
    context_window_percent: Option<i64>,
    context_window_used_tokens: Option<i64>,
    skills: Option<Vec<SkillMetadata>>,
    connectors_snapshot: Option<ConnectorsSnapshot>,
    dismissed_mention_popup_token: Option<String>,
    mention_paths: HashMap<String, String>,
    /// When enabled, `Enter` submits immediately and `Tab` requests queuing behavior.
    steer_enabled: bool,
    collaboration_modes_enabled: bool,
    config: ChatComposerConfig,
    collaboration_mode_indicator: Option<CollaborationModeIndicator>,
    connectors_enabled: bool,
    personality_command_enabled: bool,
    windows_degraded_sandbox_active: bool,
}

#[derive(Clone, Debug)]
struct FooterFlash {
    line: Line<'static>,
    expires_at: Instant,
}

/// Popup state – at most one can be visible at any time.
enum ActivePopup {
    None,
    Command(CommandPopup),
    File(FileSearchPopup),
    Skill(SkillPopup),
}

const FOOTER_SPACING_HEIGHT: u16 = 0;

impl ChatComposer {
    pub fn new(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
        placeholder_text: String,
        disable_paste_burst: bool,
    ) -> Self {
        Self::new_with_config(
            has_input_focus,
            app_event_tx,
            enhanced_keys_supported,
            placeholder_text,
            disable_paste_burst,
            ChatComposerConfig::default(),
        )
    }

    /// Construct a composer with explicit feature gating.
    ///
    /// This enables reuse in contexts like request-user-input where we want
    /// the same visuals and editing behavior without slash commands or popups.
    pub(crate) fn new_with_config(
        has_input_focus: bool,
        app_event_tx: AppEventSender,
        enhanced_keys_supported: bool,
        placeholder_text: String,
        disable_paste_burst: bool,
        config: ChatComposerConfig,
    ) -> Self {
        let use_shift_enter_hint = enhanced_keys_supported;

        let mut this = Self {
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            active_popup: ActivePopup::None,
            app_event_tx,
            history: ChatComposerHistory::new(),
            quit_shortcut_expires_at: None,
            quit_shortcut_key: key_hint::ctrl(KeyCode::Char('c')),
            esc_backtrack_hint: false,
            use_shift_enter_hint,
            dismissed_file_popup_token: None,
            current_file_query: None,
            pending_pastes: Vec::new(),
            large_paste_counters: HashMap::new(),
            has_focus: has_input_focus,
            attached_images: Vec::new(),
            placeholder_text,
            is_task_running: false,
            input_enabled: true,
            input_disabled_placeholder: None,
            paste_burst: PasteBurst::default(),
            disable_paste_burst: false,
            custom_prompts: Vec::new(),
            footer_mode: FooterMode::ComposerEmpty,
            footer_hint_override: None,
            footer_flash: None,
            context_window_percent: None,
            context_window_used_tokens: None,
            skills: None,
            connectors_snapshot: None,
            dismissed_mention_popup_token: None,
            mention_paths: HashMap::new(),
            steer_enabled: false,
            collaboration_modes_enabled: false,
            config,
            collaboration_mode_indicator: None,
            connectors_enabled: false,
            personality_command_enabled: false,
            windows_degraded_sandbox_active: false,
        };
        // Apply configuration via the setter to keep side-effects centralized.
        this.set_disable_paste_burst(disable_paste_burst);
        this
    }

    pub fn set_skill_mentions(&mut self, skills: Option<Vec<SkillMetadata>>) {
        self.skills = skills;
    }

    pub fn set_connector_mentions(&mut self, connectors_snapshot: Option<ConnectorsSnapshot>) {
        self.connectors_snapshot = connectors_snapshot;
    }

    pub(crate) fn take_mention_paths(&mut self) -> HashMap<String, String> {
        std::mem::take(&mut self.mention_paths)
    }

    /// Enables or disables "Steer" behavior for submission keys.
    ///
    /// When steer is enabled, `Enter` produces [`InputResult::Submitted`] (send immediately) and
    /// `Tab` produces [`InputResult::Queued`] (eligible to queue if a task is running).
    /// When steer is disabled, `Enter` produces [`InputResult::Queued`], preserving the default
    /// "queue while a task is running" behavior.
    pub fn set_steer_enabled(&mut self, enabled: bool) {
        self.steer_enabled = enabled;
    }

    pub fn set_collaboration_modes_enabled(&mut self, enabled: bool) {
        self.collaboration_modes_enabled = enabled;
    }

    pub fn set_connectors_enabled(&mut self, enabled: bool) {
        self.connectors_enabled = enabled;
    }

    pub fn set_collaboration_mode_indicator(
        &mut self,
        indicator: Option<CollaborationModeIndicator>,
    ) {
        self.collaboration_mode_indicator = indicator;
    }

    pub fn set_personality_command_enabled(&mut self, enabled: bool) {
        self.personality_command_enabled = enabled;
    }
    /// Centralized feature gating keeps config checks out of call sites.
    fn popups_enabled(&self) -> bool {
        self.config.popups_enabled
    }

    fn slash_commands_enabled(&self) -> bool {
        self.config.slash_commands_enabled
    }

    fn image_paste_enabled(&self) -> bool {
        self.config.image_paste_enabled
    }
    #[cfg(target_os = "windows")]
    pub fn set_windows_degraded_sandbox_active(&mut self, enabled: bool) {
        self.windows_degraded_sandbox_active = enabled;
    }
    fn layout_areas(&self, area: Rect) -> [Rect; 3] {
        let footer_props = self.footer_props();
        let footer_hint_height = self
            .custom_footer_height()
            .unwrap_or_else(|| footer_height(footer_props));
        let footer_spacing = Self::footer_spacing(footer_hint_height);
        let footer_total_height = footer_hint_height + footer_spacing;
        let popup_constraint = match &self.active_popup {
            ActivePopup::Command(popup) => {
                Constraint::Max(popup.calculate_required_height(area.width))
            }
            ActivePopup::File(popup) => Constraint::Max(popup.calculate_required_height()),
            ActivePopup::Skill(popup) => {
                Constraint::Max(popup.calculate_required_height(area.width))
            }
            ActivePopup::None => Constraint::Max(footer_total_height),
        };
        let [composer_rect, popup_rect] =
            Layout::vertical([Constraint::Min(3), popup_constraint]).areas(area);
        let textarea_rect = composer_rect.inset(Insets::tlbr(1, LIVE_PREFIX_COLS, 1, 1));
        [composer_rect, textarea_rect, popup_rect]
    }

    fn footer_spacing(footer_hint_height: u16) -> u16 {
        if footer_hint_height == 0 {
            0
        } else {
            FOOTER_SPACING_HEIGHT
        }
    }

    /// Returns true if the composer currently contains no user input.
    pub(crate) fn is_empty(&self) -> bool {
        self.textarea.is_empty()
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
        let Some(entry) = self.history.on_entry_response(log_id, offset, entry) else {
            return false;
        };
        // Persistent ↑/↓ history is text-only (backwards-compatible and avoids persisting
        // attachments), but local in-session ↑/↓ history can rehydrate elements and image paths.
        self.set_text_content(entry.text, entry.text_elements, entry.local_image_paths);
        true
    }

    /// Integrate pasted text into the composer.
    ///
    /// Acts as the only place where paste text is integrated, both for:
    ///
    /// - Real/explicit paste events surfaced by the terminal, and
    /// - Non-bracketed "paste bursts" that [`PasteBurst`](super::paste_burst::PasteBurst) buffers
    ///   and later flushes here.
    ///
    /// Behavior:
    ///
    /// - If the paste is larger than `LARGE_PASTE_CHAR_THRESHOLD` chars, inserts a placeholder
    ///   element (expanded on submit) and stores the full text in `pending_pastes`.
    /// - Otherwise, if the paste looks like an image path, attaches the image and inserts a
    ///   trailing space so the user can keep typing naturally.
    /// - Otherwise, inserts the pasted text directly into the textarea.
    ///
    /// In all cases, clears any paste-burst Enter suppression state so a real paste cannot affect
    /// the next user Enter key, then syncs popup state.
    pub fn handle_paste(&mut self, pasted: String) -> bool {
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        let char_count = pasted.chars().count();
        if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = self.next_large_paste_placeholder(char_count);
            self.textarea.insert_element(&placeholder);
            self.pending_pastes.push((placeholder, pasted));
        } else if char_count > 1
            && self.image_paste_enabled()
            && self.handle_paste_image_path(pasted.clone())
        {
            self.textarea.insert_str(" ");
        } else {
            self.textarea.insert_str(&pasted);
        }
        // Explicit paste events should not trigger Enter suppression.
        self.paste_burst.clear_after_explicit_paste();
        self.sync_popups();
        true
    }

    pub fn handle_paste_image_path(&mut self, pasted: String) -> bool {
        let Some(path_buf) = normalize_pasted_path(&pasted) else {
            return false;
        };

        // normalize_pasted_path already handles Windows → WSL path conversion,
        // so we can directly try to read the image dimensions.
        match image::image_dimensions(&path_buf) {
            Ok((width, height)) => {
                tracing::info!("OK: {pasted}");
                tracing::debug!("image dimensions={}x{}", width, height);
                let format = pasted_image_format(&path_buf);
                tracing::debug!("attached image format={}", format.label());
                self.attach_image(path_buf);
                true
            }
            Err(err) => {
                tracing::trace!("ERR: {err}");
                false
            }
        }
    }

    /// Enable or disable paste-burst handling.
    ///
    /// `disable_paste_burst` is an escape hatch for terminals/platforms where the burst heuristic
    /// is unwanted or has already been handled elsewhere.
    ///
    /// When transitioning from enabled → disabled, we "defuse" any in-flight burst state so it
    /// cannot affect subsequent normal typing:
    ///
    /// - First, flush any held/buffered text immediately via
    ///   [`PasteBurst::flush_before_modified_input`], and feed it through `handle_paste(String)`.
    ///   This preserves user input and routes it through the same integration path as explicit
    ///   pastes (large-paste placeholders, image-path detection, and popup sync).
    /// - Then clear the burst timing and Enter-suppression window via
    ///   [`PasteBurst::clear_after_explicit_paste`].
    ///
    /// We intentionally do not use `clear_window_after_non_char()` here: it clears timing state
    /// without emitting any buffered text, which can leave a non-empty buffer unable to flush
    /// later (because `flush_if_due()` relies on `last_plain_char_time` to time out).
    pub(crate) fn set_disable_paste_burst(&mut self, disabled: bool) {
        let was_disabled = self.disable_paste_burst;
        self.disable_paste_burst = disabled;
        if disabled && !was_disabled {
            if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
                self.handle_paste(pasted);
            }
            self.paste_burst.clear_after_explicit_paste();
        }
    }

    /// Replace the composer content with text from an external editor.
    /// Clears pending paste placeholders and keeps only attachments whose
    /// placeholder labels still appear in the new text. Image placeholders
    /// are renumbered to `[Image #1]..[Image #N]`. Cursor is placed at the end
    /// after rebuilding elements.
    pub(crate) fn apply_external_edit(&mut self, text: String) {
        self.pending_pastes.clear();

        // Count placeholder occurrences in the new text.
        let mut placeholder_counts: HashMap<String, usize> = HashMap::new();
        for placeholder in self.attached_images.iter().map(|img| &img.placeholder) {
            if placeholder_counts.contains_key(placeholder) {
                continue;
            }
            let count = text.match_indices(placeholder).count();
            if count > 0 {
                placeholder_counts.insert(placeholder.clone(), count);
            }
        }

        // Keep attachments only while we have matching occurrences left.
        let mut kept_images = Vec::new();
        for img in self.attached_images.drain(..) {
            if let Some(count) = placeholder_counts.get_mut(&img.placeholder)
                && *count > 0
            {
                *count -= 1;
                kept_images.push(img);
            }
        }
        self.attached_images = kept_images;

        // Rebuild textarea so placeholders become elements again.
        self.textarea.set_text_clearing_elements("");
        let mut remaining: HashMap<&str, usize> = HashMap::new();
        for img in &self.attached_images {
            *remaining.entry(img.placeholder.as_str()).or_insert(0) += 1;
        }

        let mut occurrences: Vec<(usize, &str)> = Vec::new();
        for placeholder in remaining.keys() {
            for (pos, _) in text.match_indices(placeholder) {
                occurrences.push((pos, *placeholder));
            }
        }
        occurrences.sort_unstable_by_key(|(pos, _)| *pos);

        let mut idx = 0usize;
        for (pos, ph) in occurrences {
            let Some(count) = remaining.get_mut(ph) else {
                continue;
            };
            if *count == 0 {
                continue;
            }
            if pos > idx {
                self.textarea.insert_str(&text[idx..pos]);
            }
            self.textarea.insert_element(ph);
            *count -= 1;
            idx = pos + ph.len();
        }
        if idx < text.len() {
            self.textarea.insert_str(&text[idx..]);
        }

        // Keep image placeholders normalized to [Image #1].. in attachment order.
        self.relabel_attached_images_and_update_placeholders();
        self.textarea.set_cursor(self.textarea.text().len());
        self.sync_popups();
    }

    pub(crate) fn current_text_with_pending(&self) -> String {
        let mut text = self.textarea.text().to_string();
        for (placeholder, actual) in &self.pending_pastes {
            if text.contains(placeholder) {
                text = text.replace(placeholder, actual);
            }
        }
        text
    }

    pub(crate) fn pending_pastes(&self) -> Vec<(String, String)> {
        self.pending_pastes.clone()
    }

    pub(crate) fn set_pending_pastes(&mut self, pending_pastes: Vec<(String, String)>) {
        let text = self.textarea.text().to_string();
        self.pending_pastes = pending_pastes
            .into_iter()
            .filter(|(placeholder, _)| text.contains(placeholder))
            .collect();
    }

    /// Override the footer hint items displayed beneath the composer. Passing
    /// `None` restores the default shortcut footer.
    pub(crate) fn set_footer_hint_override(&mut self, items: Option<Vec<(String, String)>>) {
        self.footer_hint_override = items;
    }

    #[cfg(test)]
    pub(crate) fn show_footer_flash(&mut self, line: Line<'static>, duration: Duration) {
        let expires_at = Instant::now()
            .checked_add(duration)
            .unwrap_or_else(Instant::now);
        self.footer_flash = Some(FooterFlash { line, expires_at });
    }

    pub(crate) fn footer_flash_visible(&self) -> bool {
        self.footer_flash
            .as_ref()
            .is_some_and(|flash| Instant::now() < flash.expires_at)
    }

    /// Replace the entire composer content with `text` and reset cursor.
    /// This clears any pending paste payloads.
    pub(crate) fn set_text_content(
        &mut self,
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
    ) {
        // Clear any existing content, placeholders, and attachments first.
        self.textarea.set_text_clearing_elements("");
        self.pending_pastes.clear();
        self.attached_images.clear();
        self.mention_paths.clear();

        self.textarea.set_text_with_elements(&text, &text_elements);

        let image_placeholders: HashSet<String> = text_elements
            .iter()
            .filter_map(|elem| elem.placeholder(&text).map(str::to_string))
            .collect();
        for (idx, path) in local_image_paths.into_iter().enumerate() {
            let placeholder = local_image_label_text(idx + 1);
            if image_placeholders.contains(&placeholder) {
                self.attached_images
                    .push(AttachedImage { placeholder, path });
            }
        }

        self.textarea.set_cursor(0);
        self.sync_popups();
    }

    /// Update the placeholder text without changing input enablement.
    pub(crate) fn set_placeholder_text(&mut self, placeholder: String) {
        self.placeholder_text = placeholder;
    }

    /// Move the cursor to the end of the current text buffer.
    pub(crate) fn move_cursor_to_end(&mut self) {
        self.textarea.set_cursor(self.textarea.text().len());
    }

    pub(crate) fn clear_for_ctrl_c(&mut self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let previous = self.current_text();
        let text_elements = self.textarea.text_elements();
        let local_image_paths = self
            .attached_images
            .iter()
            .map(|img| img.path.clone())
            .collect();
        self.set_text_content(String::new(), Vec::new(), Vec::new());
        self.history.reset_navigation();
        self.history.record_local_submission(HistoryEntry {
            text: previous.clone(),
            text_elements,
            local_image_paths,
        });
        Some(previous)
    }

    /// Get the current composer text.
    pub(crate) fn current_text(&self) -> String {
        self.textarea.text().to_string()
    }

    pub(crate) fn text_elements(&self) -> Vec<TextElement> {
        self.textarea.text_elements()
    }

    #[cfg(test)]
    pub(crate) fn local_image_paths(&self) -> Vec<PathBuf> {
        self.attached_images
            .iter()
            .map(|img| img.path.clone())
            .collect()
    }

    pub(crate) fn local_images(&self) -> Vec<LocalImageAttachment> {
        self.attached_images
            .iter()
            .map(|img| LocalImageAttachment {
                placeholder: img.placeholder.clone(),
                path: img.path.clone(),
            })
            .collect()
    }

    fn prune_attached_images_for_submission(&mut self, text: &str, text_elements: &[TextElement]) {
        if self.attached_images.is_empty() {
            return;
        }
        let image_placeholders: HashSet<&str> = text_elements
            .iter()
            .filter_map(|elem| elem.placeholder(text))
            .collect();
        self.attached_images
            .retain(|img| image_placeholders.contains(img.placeholder.as_str()));
    }

    /// Insert an attachment placeholder and track it for the next submission.
    pub fn attach_image(&mut self, path: PathBuf) {
        let image_number = self.attached_images.len() + 1;
        let placeholder = local_image_label_text(image_number);
        // Insert as an element to match large paste placeholder behavior:
        // styled distinctly and treated atomically for cursor/mutations.
        self.textarea.insert_element(&placeholder);
        self.attached_images
            .push(AttachedImage { placeholder, path });
    }

    #[cfg(test)]
    pub fn take_recent_submission_images(&mut self) -> Vec<PathBuf> {
        let images = std::mem::take(&mut self.attached_images);
        images.into_iter().map(|img| img.path).collect()
    }

    pub fn take_recent_submission_images_with_placeholders(&mut self) -> Vec<LocalImageAttachment> {
        let images = std::mem::take(&mut self.attached_images);
        images
            .into_iter()
            .map(|img| LocalImageAttachment {
                placeholder: img.placeholder,
                path: img.path,
            })
            .collect()
    }

    /// Flushes any due paste-burst state.
    ///
    /// Call this from a UI tick to turn paste-burst transient state into explicit textarea edits:
    ///
    /// - If a burst times out, flush it via `handle_paste(String)`.
    /// - If only the first ASCII char was held (flicker suppression) and no burst followed, emit it
    ///   as normal typed input.
    ///
    /// This also allows a single "held" ASCII char to render even when it turns out not to be part
    /// of a paste burst.
    pub(crate) fn flush_paste_burst_if_due(&mut self) -> bool {
        self.handle_paste_burst_flush(Instant::now())
    }

    /// Returns whether the composer is currently in any paste-burst related transient state.
    ///
    /// This includes actively buffering, having a non-empty burst buffer, or holding the first
    /// ASCII char for flicker suppression.
    pub(crate) fn is_in_paste_burst(&self) -> bool {
        self.paste_burst.is_active()
    }

    /// Returns a delay that reliably exceeds the paste-burst timing threshold.
    ///
    /// Use this in tests to avoid boundary flakiness around the `PasteBurst` timeout.
    pub(crate) fn recommended_paste_flush_delay() -> Duration {
        PasteBurst::recommended_flush_delay()
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

    /// Show the transient "press again to quit" hint for `key`.
    ///
    /// The owner (`BottomPane`/`ChatWidget`) is responsible for scheduling a
    /// redraw after [`super::QUIT_SHORTCUT_TIMEOUT`] so the hint can disappear
    /// even when the UI is otherwise idle.
    pub fn show_quit_shortcut_hint(&mut self, key: KeyBinding, has_focus: bool) {
        self.quit_shortcut_expires_at = Instant::now()
            .checked_add(super::QUIT_SHORTCUT_TIMEOUT)
            .or_else(|| Some(Instant::now()));
        self.quit_shortcut_key = key;
        self.footer_mode = FooterMode::QuitShortcutReminder;
        self.set_has_focus(has_focus);
    }

    /// Clear the "press again to quit" hint immediately.
    pub fn clear_quit_shortcut_hint(&mut self, has_focus: bool) {
        self.quit_shortcut_expires_at = None;
        self.footer_mode = reset_mode_after_activity(self.footer_mode);
        self.set_has_focus(has_focus);
    }

    /// Whether the quit shortcut hint should currently be shown.
    ///
    /// This is time-based rather than event-based: it may become false without
    /// any additional user input, so the UI schedules a redraw when the hint
    /// expires.
    pub(crate) fn quit_shortcut_hint_visible(&self) -> bool {
        self.quit_shortcut_expires_at
            .is_some_and(|expires_at| Instant::now() < expires_at)
    }

    fn next_large_paste_placeholder(&mut self, char_count: usize) -> String {
        let base = format!("[Pasted Content {char_count} chars]");
        let next_suffix = self.large_paste_counters.entry(char_count).or_insert(0);
        *next_suffix += 1;
        if *next_suffix == 1 {
            base
        } else {
            format!("{base} #{next_suffix}")
        }
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.textarea.insert_str(text);
        self.sync_popups();
    }

    /// Handle a key event coming from the main UI.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        if !self.input_enabled {
            return (InputResult::None, false);
        }

        let result = match &mut self.active_popup {
            ActivePopup::Command(_) => self.handle_key_event_with_slash_popup(key_event),
            ActivePopup::File(_) => self.handle_key_event_with_file_popup(key_event),
            ActivePopup::Skill(_) => self.handle_key_event_with_skill_popup(key_event),
            ActivePopup::None => self.handle_key_event_without_popup(key_event),
        };

        // Update (or hide/show) popup after processing the key.
        self.sync_popups();

        result
    }

    /// Return true if either the slash-command popup or the file-search popup is active.
    pub(crate) fn popup_active(&self) -> bool {
        !matches!(self.active_popup, ActivePopup::None)
    }

    /// Handle key event when the slash-command popup is visible.
    fn handle_key_event_with_slash_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        if self.handle_shortcut_overlay_key(&key_event) {
            return (InputResult::None, true);
        }
        if key_event.code == KeyCode::Esc {
            let next_mode = esc_hint_mode(self.footer_mode, self.is_task_running);
            if next_mode != self.footer_mode {
                self.footer_mode = next_mode;
                return (InputResult::None, true);
            }
        } else {
            self.footer_mode = reset_mode_after_activity(self.footer_mode);
        }
        let ActivePopup::Command(popup) = &mut self.active_popup else {
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
                self.active_popup = ActivePopup::None;
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                // Ensure popup filtering/selection reflects the latest composer text
                // before applying completion.
                let first_line = self.textarea.text().lines().next().unwrap_or("");
                popup.on_composer_text_change(first_line.to_string());
                if let Some(sel) = popup.selected_item() {
                    let mut cursor_target: Option<usize> = None;
                    match sel {
                        CommandItem::Builtin(cmd) => {
                            if cmd == SlashCommand::Skills {
                                self.textarea.set_text_clearing_elements("");
                                return (InputResult::Command(cmd), true);
                            }

                            let starts_with_cmd = first_line
                                .trim_start()
                                .starts_with(&format!("/{}", cmd.command()));
                            if !starts_with_cmd {
                                self.textarea
                                    .set_text_clearing_elements(&format!("/{} ", cmd.command()));
                            }
                            if !self.textarea.text().is_empty() {
                                cursor_target = Some(self.textarea.text().len());
                            }
                        }
                        CommandItem::UserPrompt(idx) => {
                            if let Some(prompt) = popup.prompt(idx) {
                                match prompt_selection_action(
                                    prompt,
                                    first_line,
                                    PromptSelectionMode::Completion,
                                    &self.textarea.text_elements(),
                                ) {
                                    PromptSelectionAction::Insert { text, cursor } => {
                                        let target = cursor.unwrap_or(text.len());
                                        // Inserted prompt text is plain input; discard any elements.
                                        self.textarea.set_text_clearing_elements(&text);
                                        cursor_target = Some(target);
                                    }
                                    PromptSelectionAction::Submit { .. } => {}
                                }
                            }
                        }
                    }
                    if let Some(pos) = cursor_target {
                        self.textarea.set_cursor(pos);
                    }
                }
                (InputResult::None, true)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // If the current line starts with a custom prompt name and includes
                // positional args for a numeric-style template, expand and submit
                // immediately regardless of the popup selection.
                let mut text = self.textarea.text().to_string();
                let mut text_elements = self.textarea.text_elements();
                if !self.pending_pastes.is_empty() {
                    let (expanded, expanded_elements) =
                        Self::expand_pending_pastes(&text, text_elements, &self.pending_pastes);
                    text = expanded;
                    text_elements = expanded_elements;
                }
                let first_line = text.lines().next().unwrap_or("");
                if let Some((name, _rest, _rest_offset)) = parse_slash_name(first_line)
                    && let Some(prompt_name) = name.strip_prefix(&format!("{PROMPTS_CMD_PREFIX}:"))
                    && let Some(prompt) = self.custom_prompts.iter().find(|p| p.name == prompt_name)
                    && let Some(expanded) =
                        expand_if_numeric_with_positional_args(prompt, first_line, &text_elements)
                {
                    self.prune_attached_images_for_submission(
                        &expanded.text,
                        &expanded.text_elements,
                    );
                    self.pending_pastes.clear();
                    self.textarea.set_text_clearing_elements("");
                    return (
                        InputResult::Submitted {
                            text: expanded.text,
                            text_elements: expanded.text_elements,
                        },
                        true,
                    );
                }

                if let Some(sel) = popup.selected_item() {
                    match sel {
                        CommandItem::Builtin(cmd) => {
                            self.textarea.set_text_clearing_elements("");
                            return (InputResult::Command(cmd), true);
                        }
                        CommandItem::UserPrompt(idx) => {
                            if let Some(prompt) = popup.prompt(idx) {
                                match prompt_selection_action(
                                    prompt,
                                    first_line,
                                    PromptSelectionMode::Submit,
                                    &self.textarea.text_elements(),
                                ) {
                                    PromptSelectionAction::Submit {
                                        text,
                                        text_elements,
                                    } => {
                                        self.prune_attached_images_for_submission(
                                            &text,
                                            &text_elements,
                                        );
                                        self.textarea.set_text_clearing_elements("");
                                        return (
                                            InputResult::Submitted {
                                                text,
                                                text_elements,
                                            },
                                            true,
                                        );
                                    }
                                    PromptSelectionAction::Insert { text, cursor } => {
                                        let target = cursor.unwrap_or(text.len());
                                        // Inserted prompt text is plain input; discard any elements.
                                        self.textarea.set_text_clearing_elements(&text);
                                        self.textarea.set_cursor(target);
                                        return (InputResult::None, true);
                                    }
                                }
                            }
                            return (InputResult::None, true);
                        }
                    }
                }
                // Fallback to default newline handling if no command selected.
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

    /// Handle non-ASCII character input (often IME) while still supporting paste-burst detection.
    ///
    /// This handler exists because non-ASCII input often comes from IMEs, where characters can
    /// legitimately arrive in short bursts that should **not** be treated as paste.
    ///
    /// The key differences from the ASCII path:
    ///
    /// - We never hold the first character (`PasteBurst::on_plain_char_no_hold`), because holding a
    ///   non-ASCII char can feel like dropped input.
    /// - If a burst is detected, we may need to retroactively remove already-inserted text before
    ///   the cursor and move it into the paste buffer (see `PasteBurst::decide_begin_buffer`).
    ///
    /// Because this path mixes "insert immediately" with "maybe retro-grab later", it must clamp
    /// the cursor to a UTF-8 char boundary before slicing `textarea.text()`.
    #[inline]
    fn handle_non_ascii_char(&mut self, input: KeyEvent, now: Instant) -> (InputResult, bool) {
        if self.disable_paste_burst {
            // When burst detection is disabled, treat IME/non-ASCII input as normal typing.
            // In particular, do not retro-capture or buffer already-inserted prefix text.
            self.textarea.input(input);
            let text_after = self.textarea.text();
            self.pending_pastes
                .retain(|(placeholder, _)| text_after.contains(placeholder));
            return (InputResult::None, true);
        }
        if let KeyEvent {
            code: KeyCode::Char(ch),
            ..
        } = input
        {
            if self.paste_burst.try_append_char_if_active(ch, now) {
                return (InputResult::None, true);
            }
            // Non-ASCII input often comes from IMEs and can arrive in quick bursts.
            // We do not want to hold the first char (flicker suppression) on this path, but we
            // still want to detect paste-like bursts. Before applying any non-ASCII input, flush
            // any existing burst buffer (including a pending first char from the ASCII path) so
            // we don't carry that transient state forward.
            if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
                self.handle_paste(pasted);
            }
            if let Some(decision) = self.paste_burst.on_plain_char_no_hold(now) {
                match decision {
                    CharDecision::BufferAppend => {
                        self.paste_burst.append_char_to_buffer(ch, now);
                        return (InputResult::None, true);
                    }
                    CharDecision::BeginBuffer { retro_chars } => {
                        // For non-ASCII we inserted prior chars immediately, so if this turns out
                        // to be paste-like we need to retroactively grab & remove the already-
                        // inserted prefix from the textarea before buffering the burst.
                        let cur = self.textarea.cursor();
                        let txt = self.textarea.text();
                        let safe_cur = Self::clamp_to_char_boundary(txt, cur);
                        let before = &txt[..safe_cur];
                        if let Some(grab) =
                            self.paste_burst
                                .decide_begin_buffer(now, before, retro_chars as usize)
                        {
                            if !grab.grabbed.is_empty() {
                                self.textarea.replace_range(grab.start_byte..safe_cur, "");
                            }
                            // seed the paste burst buffer with everything (grabbed + new)
                            self.paste_burst.append_char_to_buffer(ch, now);
                            return (InputResult::None, true);
                        }
                        // If decide_begin_buffer opted not to start buffering,
                        // fall through to normal insertion below.
                    }
                    _ => unreachable!("on_plain_char_no_hold returned unexpected variant"),
                }
            }
        }
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
        if self.handle_shortcut_overlay_key(&key_event) {
            return (InputResult::None, true);
        }
        if key_event.code == KeyCode::Esc {
            let next_mode = esc_hint_mode(self.footer_mode, self.is_task_running);
            if next_mode != self.footer_mode {
                self.footer_mode = next_mode;
                return (InputResult::None, true);
            }
        } else {
            self.footer_mode = reset_mode_after_activity(self.footer_mode);
        }
        let ActivePopup::File(popup) = &mut self.active_popup else {
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
                // Hide popup without modifying text, remember token to avoid immediate reopen.
                if let Some(tok) = Self::current_at_token(&self.textarea) {
                    self.dismissed_file_popup_token = Some(tok);
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

                let sel_path = sel.to_string_lossy().to_string();
                // If selected path looks like an image (png/jpeg), attach as image instead of inserting text.
                let is_image = Self::is_image_path(&sel_path);
                if is_image {
                    // Determine dimensions; if that fails fall back to normal path insertion.
                    let path_buf = PathBuf::from(&sel_path);
                    match image::image_dimensions(&path_buf) {
                        Ok((width, height)) => {
                            tracing::debug!("selected image dimensions={}x{}", width, height);
                            // Remove the current @token (mirror logic from insert_selected_path without inserting text)
                            // using the flat text and byte-offset cursor API.
                            let cursor_offset = self.textarea.cursor();
                            let text = self.textarea.text();
                            // Clamp to a valid char boundary to avoid panics when slicing.
                            let safe_cursor = Self::clamp_to_char_boundary(text, cursor_offset);
                            let before_cursor = &text[..safe_cursor];
                            let after_cursor = &text[safe_cursor..];

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
                            let end_idx = safe_cursor + end_rel_idx;

                            self.textarea.replace_range(start_idx..end_idx, "");
                            self.textarea.set_cursor(start_idx);

                            self.attach_image(path_buf);
                            // Add a trailing space to keep typing fluid.
                            self.textarea.insert_str(" ");
                        }
                        Err(err) => {
                            tracing::trace!("image dimensions lookup failed: {err}");
                            // Fallback to plain path insertion if metadata read fails.
                            self.insert_selected_path(&sel_path);
                        }
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

    fn handle_key_event_with_skill_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        if self.handle_shortcut_overlay_key(&key_event) {
            return (InputResult::None, true);
        }
        self.footer_mode = reset_mode_after_activity(self.footer_mode);

        let ActivePopup::Skill(popup) = &mut self.active_popup else {
            unreachable!();
        };

        let mut selected_mention: Option<(String, Option<String>)> = None;
        let mut close_popup = false;

        let result = match key_event {
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
                if let Some(tok) = self.current_mention_token() {
                    self.dismissed_mention_popup_token = Some(tok);
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
                if let Some(mention) = popup.selected_mention() {
                    selected_mention = Some((mention.insert_text.clone(), mention.path.clone()));
                }
                close_popup = true;
                (InputResult::None, true)
            }
            input => self.handle_input_basic(input),
        };

        if close_popup {
            if let Some((insert_text, path)) = selected_mention {
                if let Some(path) = path.as_deref() {
                    self.record_mention_path(&insert_text, path);
                }
                self.insert_selected_mention(&insert_text);
            }
            self.active_popup = ActivePopup::None;
        }

        result
    }

    fn is_image_path(path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
    }

    fn trim_text_elements(
        original: &str,
        trimmed: &str,
        elements: Vec<TextElement>,
    ) -> Vec<TextElement> {
        if trimmed.is_empty() || elements.is_empty() {
            return Vec::new();
        }
        let trimmed_start = original.len().saturating_sub(original.trim_start().len());
        let trimmed_end = trimmed_start.saturating_add(trimmed.len());

        elements
            .into_iter()
            .filter_map(|elem| {
                let start = elem.byte_range.start;
                let end = elem.byte_range.end;
                if end <= trimmed_start || start >= trimmed_end {
                    return None;
                }
                let new_start = start.saturating_sub(trimmed_start);
                let new_end = end.saturating_sub(trimmed_start).min(trimmed.len());
                if new_start >= new_end {
                    return None;
                }
                let placeholder = trimmed.get(new_start..new_end).map(str::to_string);
                Some(TextElement::new(
                    ByteRange {
                        start: new_start,
                        end: new_end,
                    },
                    placeholder,
                ))
            })
            .collect()
    }

    /// Expand large-paste placeholders using element ranges and rebuild other element spans.
    pub(crate) fn expand_pending_pastes(
        text: &str,
        mut elements: Vec<TextElement>,
        pending_pastes: &[(String, String)],
    ) -> (String, Vec<TextElement>) {
        if pending_pastes.is_empty() || elements.is_empty() {
            return (text.to_string(), elements);
        }

        // Stage 1: index pending paste payloads by placeholder for deterministic replacements.
        let mut pending_by_placeholder: HashMap<&str, VecDeque<&str>> = HashMap::new();
        for (placeholder, actual) in pending_pastes {
            pending_by_placeholder
                .entry(placeholder.as_str())
                .or_default()
                .push_back(actual.as_str());
        }

        // Stage 2: walk elements in order and rebuild text/spans in a single pass.
        elements.sort_by_key(|elem| elem.byte_range.start);

        let mut rebuilt = String::with_capacity(text.len());
        let mut rebuilt_elements = Vec::with_capacity(elements.len());
        let mut cursor = 0usize;

        for elem in elements {
            let start = elem.byte_range.start.min(text.len());
            let end = elem.byte_range.end.min(text.len());
            if start > end {
                continue;
            }
            if start > cursor {
                rebuilt.push_str(&text[cursor..start]);
            }
            let elem_text = &text[start..end];
            let placeholder = elem.placeholder(text).map(str::to_string);
            let replacement = placeholder
                .as_deref()
                .and_then(|ph| pending_by_placeholder.get_mut(ph))
                .and_then(VecDeque::pop_front);
            if let Some(actual) = replacement {
                // Stage 3: inline actual paste payloads and drop their placeholder elements.
                rebuilt.push_str(actual);
            } else {
                // Stage 4: keep non-paste elements, updating their byte ranges for the new text.
                let new_start = rebuilt.len();
                rebuilt.push_str(elem_text);
                let new_end = rebuilt.len();
                let placeholder = placeholder.or_else(|| Some(elem_text.to_string()));
                rebuilt_elements.push(TextElement::new(
                    ByteRange {
                        start: new_start,
                        end: new_end,
                    },
                    placeholder,
                ));
            }
            cursor = end;
        }

        // Stage 5: append any trailing text that followed the last element.
        if cursor < text.len() {
            rebuilt.push_str(&text[cursor..]);
        }

        (rebuilt, rebuilt_elements)
    }

    pub fn skills(&self) -> Option<&Vec<SkillMetadata>> {
        self.skills.as_ref()
    }

    fn mentions_enabled(&self) -> bool {
        let skills_ready = self
            .skills
            .as_ref()
            .is_some_and(|skills| !skills.is_empty());
        let connectors_ready = self.connectors_enabled
            && self
                .connectors_snapshot
                .as_ref()
                .is_some_and(|snapshot| !snapshot.connectors.is_empty());
        skills_ready || connectors_ready
    }

    /// Extract a token prefixed with `prefix` under the cursor, if any.
    ///
    /// The returned string **does not** include the prefix.
    ///
    /// Behavior:
    /// - The cursor may be anywhere *inside* the token (including on the
    ///   leading prefix). It does **not** need to be at the end of the line.
    /// - A token is delimited by ASCII whitespace (space, tab, newline).
    /// - If the token under the cursor starts with `prefix`, that token is
    ///   returned without the leading prefix. When `allow_empty` is true, a
    ///   lone prefix character yields `Some(String::new())` to surface hints.
    fn current_prefixed_token(
        textarea: &TextArea,
        prefix: char,
        allow_empty: bool,
    ) -> Option<String> {
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
                .map(char::is_whitespace)
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
            .map(char::len_utf8)
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

        let prefix_str = prefix.to_string();
        let left_match = token_left.filter(|t| t.starts_with(prefix));
        let right_match = token_right.filter(|t| t.starts_with(prefix));

        let left_prefixed = left_match.map(|t| t[prefix.len_utf8()..].to_string());
        let right_prefixed = right_match.map(|t| t[prefix.len_utf8()..].to_string());

        if at_whitespace {
            if right_prefixed.is_some() {
                return right_prefixed;
            }
            if token_left.is_some_and(|t| t == prefix_str) {
                return allow_empty.then(String::new);
            }
            return left_prefixed;
        }
        if after_cursor.starts_with(prefix) {
            return right_prefixed.or(left_prefixed);
        }
        left_prefixed.or(right_prefixed)
    }

    /// Extract the `@token` that the cursor is currently positioned on, if any.
    ///
    /// The returned string **does not** include the leading `@`.
    fn current_at_token(textarea: &TextArea) -> Option<String> {
        Self::current_prefixed_token(textarea, '@', false)
    }

    fn current_mention_token(&self) -> Option<String> {
        if !self.mentions_enabled() {
            return None;
        }
        Self::current_prefixed_token(&self.textarea, '$', true)
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

        // Path replacement is plain text; rebuild without carrying elements.
        self.textarea.set_text_clearing_elements(&new_text);
        let new_cursor = start_idx.saturating_add(inserted.len()).saturating_add(1);
        self.textarea.set_cursor(new_cursor);
    }

    fn insert_selected_mention(&mut self, insert_text: &str) {
        let cursor_offset = self.textarea.cursor();
        let text = self.textarea.text();
        let safe_cursor = Self::clamp_to_char_boundary(text, cursor_offset);

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

        let inserted = insert_text.to_string();

        let mut new_text =
            String::with_capacity(text.len() - (end_idx - start_idx) + inserted.len() + 1);
        new_text.push_str(&text[..start_idx]);
        new_text.push_str(&inserted);
        new_text.push(' ');
        new_text.push_str(&text[end_idx..]);

        // Mention insertion rebuilds plain text, so drop existing elements.
        self.textarea.set_text_clearing_elements(&new_text);
        let new_cursor = start_idx.saturating_add(inserted.len()).saturating_add(1);
        self.textarea.set_cursor(new_cursor);
    }

    fn record_mention_path(&mut self, insert_text: &str, path: &str) {
        let Some(name) = Self::mention_name_from_insert_text(insert_text) else {
            return;
        };
        self.mention_paths.insert(name, path.to_string());
    }

    fn mention_name_from_insert_text(insert_text: &str) -> Option<String> {
        let name = insert_text.strip_prefix('$')?;
        if name.is_empty() {
            return None;
        }
        if name
            .as_bytes()
            .iter()
            .all(|byte| is_mention_name_char(*byte))
        {
            Some(name.to_string())
        } else {
            None
        }
    }

    /// Prepare text for submission/queuing. Returns None if submission should be suppressed.
    /// On success, clears pending paste payloads because placeholders have been expanded.
    fn prepare_submission_text(&mut self) -> Option<(String, Vec<TextElement>)> {
        let mut text = self.textarea.text().to_string();
        let original_input = text.clone();
        let original_text_elements = self.textarea.text_elements();
        let original_local_image_paths = self
            .attached_images
            .iter()
            .map(|img| img.path.clone())
            .collect::<Vec<_>>();
        let original_pending_pastes = self.pending_pastes.clone();
        let mut text_elements = original_text_elements.clone();
        let input_starts_with_space = original_input.starts_with(' ');
        self.textarea.set_text_clearing_elements("");

        if !self.pending_pastes.is_empty() {
            // Expand placeholders so element byte ranges stay aligned.
            let (expanded, expanded_elements) =
                Self::expand_pending_pastes(&text, text_elements, &self.pending_pastes);
            text = expanded;
            text_elements = expanded_elements;
        }

        let expanded_input = text.clone();

        // If there is neither text nor attachments, suppress submission entirely.
        text = text.trim().to_string();
        text_elements = Self::trim_text_elements(&expanded_input, &text, text_elements);

        if self.slash_commands_enabled()
            && let Some((name, _rest, _rest_offset)) = parse_slash_name(&text)
        {
            let treat_as_plain_text = input_starts_with_space || name.contains('/');
            if !treat_as_plain_text {
                let is_builtin = slash_commands::find_builtin_command(
                    name,
                    self.collaboration_modes_enabled,
                    self.connectors_enabled,
                    self.personality_command_enabled,
                    self.windows_degraded_sandbox_active,
                )
                .is_some();
                let prompt_prefix = format!("{PROMPTS_CMD_PREFIX}:");
                let is_known_prompt = name
                    .strip_prefix(&prompt_prefix)
                    .map(|prompt_name| {
                        self.custom_prompts
                            .iter()
                            .any(|prompt| prompt.name == prompt_name)
                    })
                    .unwrap_or(false);
                if !is_builtin && !is_known_prompt {
                    let message = format!(
                        r#"Unrecognized command '/{name}'. Type "/" for a list of supported commands."#
                    );
                    self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_info_event(message, None),
                    )));
                    self.set_text_content(
                        original_input.clone(),
                        original_text_elements,
                        original_local_image_paths,
                    );
                    self.pending_pastes.clone_from(&original_pending_pastes);
                    self.textarea.set_cursor(original_input.len());
                    return None;
                }
            }
        }

        if self.slash_commands_enabled() {
            let expanded_prompt =
                match expand_custom_prompt(&text, &text_elements, &self.custom_prompts) {
                    Ok(expanded) => expanded,
                    Err(err) => {
                        self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                            history_cell::new_error_event(err.user_message()),
                        )));
                        self.set_text_content(
                            original_input.clone(),
                            original_text_elements,
                            original_local_image_paths,
                        );
                        self.pending_pastes.clone_from(&original_pending_pastes);
                        self.textarea.set_cursor(original_input.len());
                        return None;
                    }
                };
            if let Some(expanded) = expanded_prompt {
                text = expanded.text;
                text_elements = expanded.text_elements;
            }
        }
        // Custom prompt expansion can remove or rewrite image placeholders, so prune any
        // attachments that no longer have a corresponding placeholder in the expanded text.
        self.prune_attached_images_for_submission(&text, &text_elements);
        if text.is_empty() && self.attached_images.is_empty() {
            return None;
        }
        if !text.is_empty() || !self.attached_images.is_empty() {
            let local_image_paths = self
                .attached_images
                .iter()
                .map(|img| img.path.clone())
                .collect();
            self.history.record_local_submission(HistoryEntry {
                text: text.clone(),
                text_elements: text_elements.clone(),
                local_image_paths,
            });
        }
        self.pending_pastes.clear();
        Some((text, text_elements))
    }

    /// Common logic for handling message submission/queuing.
    /// Returns the appropriate InputResult based on `should_queue`.
    fn handle_submission(&mut self, should_queue: bool) -> (InputResult, bool) {
        self.handle_submission_with_time(should_queue, Instant::now())
    }

    fn handle_submission_with_time(
        &mut self,
        should_queue: bool,
        now: Instant,
    ) -> (InputResult, bool) {
        // If the first line is a bare built-in slash command (no args),
        // dispatch it even when the slash popup isn't visible. This preserves
        // the workflow: type a prefix ("/di"), press Tab to complete to
        // "/diff ", then press Enter/Ctrl+Shift+Q to run it. Tab moves the cursor beyond
        // the '/name' token and our caret-based heuristic hides the popup,
        // but Enter/Ctrl+Shift+Q should still dispatch the command rather than submit
        // literal text.
        if let Some(result) = self.try_dispatch_bare_slash_command() {
            return (result, true);
        }

        // If we're in a paste-like burst capture, treat Enter/Ctrl+Shift+Q as part of the burst
        // and accumulate it rather than submitting or inserting immediately.
        // Do not treat as paste inside a slash-command context.
        let in_slash_context = self.slash_commands_enabled()
            && (matches!(self.active_popup, ActivePopup::Command(_))
                || self
                    .textarea
                    .text()
                    .lines()
                    .next()
                    .unwrap_or("")
                    .starts_with('/'));
        if !self.disable_paste_burst
            && self.paste_burst.is_active()
            && !in_slash_context
            && self.paste_burst.append_newline_if_active(now)
        {
            return (InputResult::None, true);
        }

        // During a paste-like burst, treat Enter/Ctrl+Shift+Q as a newline instead of submit.
        if !in_slash_context
            && !self.disable_paste_burst
            && self
                .paste_burst
                .newline_should_insert_instead_of_submit(now)
        {
            self.textarea.insert_str("\n");
            self.paste_burst.extend_window(now);
            return (InputResult::None, true);
        }

        let original_input = self.textarea.text().to_string();
        let original_text_elements = self.textarea.text_elements();
        let original_local_image_paths = self
            .attached_images
            .iter()
            .map(|img| img.path.clone())
            .collect::<Vec<_>>();
        let original_pending_pastes = self.pending_pastes.clone();
        if let Some(result) = self.try_dispatch_slash_command_with_args() {
            return (result, true);
        }

        if let Some((text, text_elements)) = self.prepare_submission_text() {
            if should_queue {
                (
                    InputResult::Queued {
                        text,
                        text_elements,
                    },
                    true,
                )
            } else {
                // Do not clear attached_images here; ChatWidget drains them via take_recent_submission_images().
                (
                    InputResult::Submitted {
                        text,
                        text_elements,
                    },
                    true,
                )
            }
        } else {
            // Restore text if submission was suppressed.
            self.set_text_content(
                original_input,
                original_text_elements,
                original_local_image_paths,
            );
            self.pending_pastes = original_pending_pastes;
            (InputResult::None, true)
        }
    }

    /// Check if the first line is a bare slash command (no args) and dispatch it.
    /// Returns Some(InputResult) if a command was dispatched, None otherwise.
    fn try_dispatch_bare_slash_command(&mut self) -> Option<InputResult> {
        if !self.slash_commands_enabled() {
            return None;
        }
        let first_line = self.textarea.text().lines().next().unwrap_or("");
        if let Some((name, rest, _rest_offset)) = parse_slash_name(first_line)
            && rest.is_empty()
            && let Some(cmd) = slash_commands::find_builtin_command(
                name,
                self.collaboration_modes_enabled,
                self.connectors_enabled,
                self.personality_command_enabled,
                self.windows_degraded_sandbox_active,
            )
        {
            self.textarea.set_text_clearing_elements("");
            Some(InputResult::Command(cmd))
        } else {
            None
        }
    }

    /// Check if the input is a slash command with args (e.g., /review args) and dispatch it.
    /// Returns Some(InputResult) if a command was dispatched, None otherwise.
    fn try_dispatch_slash_command_with_args(&mut self) -> Option<InputResult> {
        if !self.slash_commands_enabled() {
            return None;
        }
        let original_input = self.textarea.text().to_string();
        let input_starts_with_space = original_input.starts_with(' ');

        if !input_starts_with_space {
            let text = self.textarea.text().to_string();
            if let Some((name, rest, _rest_offset)) = parse_slash_name(&text)
                && !rest.is_empty()
                && !name.contains('/')
                && let Some(cmd) = slash_commands::find_builtin_command(
                    name,
                    self.collaboration_modes_enabled,
                    self.connectors_enabled,
                    self.personality_command_enabled,
                    self.windows_degraded_sandbox_active,
                )
                && matches!(cmd, SlashCommand::Review | SlashCommand::Rename)
            {
                self.textarea.set_text_clearing_elements("");
                return Some(InputResult::CommandWithArgs(cmd, rest.to_string()));
            }
        }
        None
    }

    /// Handle key event when no popup is visible.
    fn handle_key_event_without_popup(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
        if self.handle_shortcut_overlay_key(&key_event) {
            return (InputResult::None, true);
        }
        if key_event.code == KeyCode::Esc {
            if self.is_empty() {
                let next_mode = esc_hint_mode(self.footer_mode, self.is_task_running);
                if next_mode != self.footer_mode {
                    self.footer_mode = next_mode;
                    return (InputResult::None, true);
                }
            }
        } else {
            self.footer_mode = reset_mode_after_activity(self.footer_mode);
        }
        match key_event {
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } if self.is_empty() => (InputResult::None, false),
            // -------------------------------------------------------------
            // History navigation (Up / Down) – only when the composer is not
            // empty or when the cursor is at the correct position, to avoid
            // interfering with normal cursor movement.
            // -------------------------------------------------------------
            KeyEvent {
                code: KeyCode::Up | KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('p') | KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self
                    .history
                    .should_handle_navigation(self.textarea.text(), self.textarea.cursor())
                {
                    let replace_entry = match key_event.code {
                        KeyCode::Up => self.history.navigate_up(&self.app_event_tx),
                        KeyCode::Down => self.history.navigate_down(&self.app_event_tx),
                        KeyCode::Char('p') => self.history.navigate_up(&self.app_event_tx),
                        KeyCode::Char('n') => self.history.navigate_down(&self.app_event_tx),
                        _ => unreachable!(),
                    };
                    if let Some(entry) = replace_entry {
                        self.set_text_content(
                            entry.text,
                            entry.text_elements,
                            entry.local_image_paths,
                        );
                        return (InputResult::None, true);
                    }
                }
                self.handle_input_basic(key_event)
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            } if self.is_task_running => self.handle_submission(true),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let should_queue = !self.steer_enabled;
                self.handle_submission(should_queue)
            }
            input => self.handle_input_basic(input),
        }
    }

    /// Applies any due `PasteBurst` flush at time `now`.
    ///
    /// Converts [`PasteBurst::flush_if_due`] results into concrete textarea mutations.
    ///
    /// Callers:
    ///
    /// - UI ticks via [`ChatComposer::flush_paste_burst_if_due`], so held first-chars can render.
    /// - Input handling via [`ChatComposer::handle_input_basic`], so a due burst does not lag.
    fn handle_paste_burst_flush(&mut self, now: Instant) -> bool {
        match self.paste_burst.flush_if_due(now) {
            FlushResult::Paste(pasted) => {
                self.handle_paste(pasted);
                true
            }
            FlushResult::Typed(ch) => {
                // Mirror insert_str() behavior so popups stay in sync when a
                // pending fast char flushes as normal typed input.
                self.textarea.insert_str(ch.to_string().as_str());
                self.sync_popups();
                true
            }
            FlushResult::None => false,
        }
    }

    /// Handles keys that mutate the textarea, including paste-burst detection.
    ///
    /// Acts as the lowest-level keypath for keys that mutate the textarea. It is also where plain
    /// character streams are converted into explicit paste operations on terminals that do not
    /// reliably provide bracketed paste.
    ///
    /// Ordering is important:
    ///
    /// - Always flush any *due* paste burst first so buffered text does not lag behind unrelated
    ///   edits.
    /// - Then handle the incoming key, intercepting only "plain" (no Ctrl/Alt) char input.
    /// - For non-plain keys, flush via `flush_before_modified_input()` before applying the key;
    ///   otherwise `clear_window_after_non_char()` can leave buffered text waiting without a
    ///   timestamp to time out against.
    fn handle_input_basic(&mut self, input: KeyEvent) -> (InputResult, bool) {
        self.handle_input_basic_with_time(input, Instant::now())
    }

    fn handle_input_basic_with_time(
        &mut self,
        input: KeyEvent,
        now: Instant,
    ) -> (InputResult, bool) {
        // If we have a buffered non-bracketed paste burst and enough time has
        // elapsed since the last char, flush it before handling a new input.
        self.handle_paste_burst_flush(now);

        if !matches!(input.code, KeyCode::Esc) {
            self.footer_mode = reset_mode_after_activity(self.footer_mode);
        }

        // If we're capturing a burst and receive Enter, accumulate it instead of inserting.
        if matches!(input.code, KeyCode::Enter)
            && !self.disable_paste_burst
            && self.paste_burst.is_active()
            && self.paste_burst.append_newline_if_active(now)
        {
            return (InputResult::None, true);
        }

        // Intercept plain Char inputs to optionally accumulate into a burst buffer.
        //
        // This is intentionally limited to "plain" (no Ctrl/Alt) chars so shortcuts keep their
        // normal semantics, and so we can aggressively flush/clear any burst state when non-char
        // keys are pressed.
        if let KeyEvent {
            code: KeyCode::Char(ch),
            modifiers,
            ..
        } = input
        {
            let has_ctrl_or_alt = has_ctrl_or_alt(modifiers);
            if !has_ctrl_or_alt && !self.disable_paste_burst {
                // Non-ASCII characters (e.g., from IMEs) can arrive in quick bursts, so avoid
                // holding the first char while still allowing burst detection for paste input.
                if !ch.is_ascii() {
                    return self.handle_non_ascii_char(input, now);
                }

                match self.paste_burst.on_plain_char(ch, now) {
                    CharDecision::BufferAppend => {
                        self.paste_burst.append_char_to_buffer(ch, now);
                        return (InputResult::None, true);
                    }
                    CharDecision::BeginBuffer { retro_chars } => {
                        let cur = self.textarea.cursor();
                        let txt = self.textarea.text();
                        let safe_cur = Self::clamp_to_char_boundary(txt, cur);
                        let before = &txt[..safe_cur];
                        if let Some(grab) =
                            self.paste_burst
                                .decide_begin_buffer(now, before, retro_chars as usize)
                        {
                            if !grab.grabbed.is_empty() {
                                self.textarea.replace_range(grab.start_byte..safe_cur, "");
                            }
                            self.paste_burst.append_char_to_buffer(ch, now);
                            return (InputResult::None, true);
                        }
                        // If decide_begin_buffer opted not to start buffering,
                        // fall through to normal insertion below.
                    }
                    CharDecision::BeginBufferFromPending => {
                        // First char was held; now append the current one.
                        self.paste_burst.append_char_to_buffer(ch, now);
                        return (InputResult::None, true);
                    }
                    CharDecision::RetainFirstChar => {
                        // Keep the first fast char pending momentarily.
                        return (InputResult::None, true);
                    }
                }
            }
            if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
                self.handle_paste(pasted);
            }
        }

        // Flush any buffered burst before applying a non-char input (arrow keys, etc).
        //
        // `clear_window_after_non_char()` clears `last_plain_char_time`. If we cleared that while
        // `PasteBurst.buffer` is non-empty, `flush_if_due()` would no longer have a timestamp to
        // time out against, and the buffered paste could remain stuck until another plain char
        // arrives.
        if !matches!(input.code, KeyCode::Char(_) | KeyCode::Enter)
            && let Some(pasted) = self.paste_burst.flush_before_modified_input()
        {
            self.handle_paste(pasted);
        }
        // For non-char inputs (or after flushing), handle normally.
        // Track element removals so we can drop any corresponding placeholders without scanning
        // the full text. (Placeholders are atomic elements; when deleted, the element disappears.)
        let elements_before = if self.pending_pastes.is_empty() && self.attached_images.is_empty() {
            None
        } else {
            Some(self.textarea.element_payloads())
        };

        self.textarea.input(input);

        if let Some(elements_before) = elements_before {
            self.reconcile_deleted_elements(elements_before);
        }

        // Update paste-burst heuristic for plain Char (no Ctrl/Alt) events.
        let crossterm::event::KeyEvent {
            code, modifiers, ..
        } = input;
        match code {
            KeyCode::Char(_) => {
                let has_ctrl_or_alt = has_ctrl_or_alt(modifiers);
                if has_ctrl_or_alt {
                    self.paste_burst.clear_window_after_non_char();
                }
            }
            KeyCode::Enter => {
                // Keep burst window alive (supports blank lines in paste).
            }
            _ => {
                // Other keys: clear burst window (buffer should have been flushed above if needed).
                self.paste_burst.clear_window_after_non_char();
            }
        }

        (InputResult::None, true)
    }

    fn reconcile_deleted_elements(&mut self, elements_before: Vec<String>) {
        let elements_after: HashSet<String> =
            self.textarea.element_payloads().into_iter().collect();

        let mut removed_any_image = false;
        for removed in elements_before
            .into_iter()
            .filter(|payload| !elements_after.contains(payload))
        {
            self.pending_pastes.retain(|(ph, _)| ph != &removed);

            if let Some(idx) = self
                .attached_images
                .iter()
                .position(|img| img.placeholder == removed)
            {
                self.attached_images.remove(idx);
                removed_any_image = true;
            }
        }

        if removed_any_image {
            self.relabel_attached_images_and_update_placeholders();
        }
    }

    fn relabel_attached_images_and_update_placeholders(&mut self) {
        for idx in 0..self.attached_images.len() {
            let expected = local_image_label_text(idx + 1);
            let current = self.attached_images[idx].placeholder.clone();
            if current == expected {
                continue;
            }

            self.attached_images[idx].placeholder = expected.clone();
            let _renamed = self.textarea.replace_element_payload(&current, &expected);
        }
    }

    fn handle_shortcut_overlay_key(&mut self, key_event: &KeyEvent) -> bool {
        if key_event.kind != KeyEventKind::Press {
            return false;
        }

        let toggles = matches!(key_event.code, KeyCode::Char('?'))
            && !has_ctrl_or_alt(key_event.modifiers)
            && self.is_empty()
            && !self.is_in_paste_burst();

        if !toggles {
            return false;
        }

        let next = toggle_shortcut_mode(
            self.footer_mode,
            self.quit_shortcut_hint_visible(),
            self.is_empty(),
        );
        let changed = next != self.footer_mode;
        self.footer_mode = next;
        changed
    }

    fn footer_props(&self) -> FooterProps {
        let mode = self.footer_mode();
        let is_wsl = {
            #[cfg(target_os = "linux")]
            {
                mode == FooterMode::ShortcutOverlay && crate::clipboard_paste::is_probably_wsl()
            }
            #[cfg(not(target_os = "linux"))]
            {
                false
            }
        };

        FooterProps {
            mode,
            esc_backtrack_hint: self.esc_backtrack_hint,
            use_shift_enter_hint: self.use_shift_enter_hint,
            is_task_running: self.is_task_running,
            quit_shortcut_key: self.quit_shortcut_key,
            steer_enabled: self.steer_enabled,
            collaboration_modes_enabled: self.collaboration_modes_enabled,
            is_wsl,
            context_window_percent: self.context_window_percent,
            context_window_used_tokens: self.context_window_used_tokens,
        }
    }

    /// Resolve the effective footer mode via a small priority waterfall.
    ///
    /// The base mode is derived solely from whether the composer is empty:
    /// `ComposerEmpty` iff empty, otherwise `ComposerHasDraft`. Transient
    /// modes (Esc hint, overlay, quit reminder) can override that base when
    /// their conditions are active.
    fn footer_mode(&self) -> FooterMode {
        let base_mode = if self.is_empty() {
            FooterMode::ComposerEmpty
        } else {
            FooterMode::ComposerHasDraft
        };

        match self.footer_mode {
            FooterMode::EscHint => FooterMode::EscHint,
            FooterMode::ShortcutOverlay => FooterMode::ShortcutOverlay,
            FooterMode::QuitShortcutReminder if self.quit_shortcut_hint_visible() => {
                FooterMode::QuitShortcutReminder
            }
            FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
                if self.quit_shortcut_hint_visible() =>
            {
                FooterMode::QuitShortcutReminder
            }
            FooterMode::QuitShortcutReminder => base_mode,
            FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft => base_mode,
        }
    }

    fn custom_footer_height(&self) -> Option<u16> {
        if self.footer_flash_visible() {
            return Some(1);
        }
        self.footer_hint_override
            .as_ref()
            .map(|items| if items.is_empty() { 0 } else { 1 })
    }

    fn sync_popups(&mut self) {
        if !self.popups_enabled() {
            self.active_popup = ActivePopup::None;
            return;
        }
        let file_token = Self::current_at_token(&self.textarea);
        let browsing_history = self
            .history
            .should_handle_navigation(self.textarea.text(), self.textarea.cursor());
        // When browsing input history (shell-style Up/Down recall), skip all popup
        // synchronization so nothing steals focus from continued history navigation.
        if browsing_history {
            if self.current_file_query.is_some() {
                self.app_event_tx
                    .send(AppEvent::StartFileSearch(String::new()));
                self.current_file_query = None;
            }
            self.active_popup = ActivePopup::None;
            return;
        }
        let mention_token = self.current_mention_token();

        let allow_command_popup =
            self.slash_commands_enabled() && file_token.is_none() && mention_token.is_none();
        self.sync_command_popup(allow_command_popup);

        if matches!(self.active_popup, ActivePopup::Command(_)) {
            if self.current_file_query.is_some() {
                self.app_event_tx
                    .send(AppEvent::StartFileSearch(String::new()));
                self.current_file_query = None;
            }
            self.dismissed_file_popup_token = None;
            self.dismissed_mention_popup_token = None;
            return;
        }

        if let Some(token) = mention_token {
            if self.current_file_query.is_some() {
                self.app_event_tx
                    .send(AppEvent::StartFileSearch(String::new()));
                self.current_file_query = None;
            }
            self.sync_mention_popup(token);
            return;
        }
        self.dismissed_mention_popup_token = None;

        if let Some(token) = file_token {
            self.sync_file_search_popup(token);
            return;
        }

        if self.current_file_query.is_some() {
            self.app_event_tx
                .send(AppEvent::StartFileSearch(String::new()));
            self.current_file_query = None;
        }
        self.dismissed_file_popup_token = None;
        if matches!(
            self.active_popup,
            ActivePopup::File(_) | ActivePopup::Skill(_)
        ) {
            self.active_popup = ActivePopup::None;
        }
    }

    /// If the cursor is currently within a slash command on the first line,
    /// extract the command name and the rest of the line after it.
    /// Returns None if the cursor is outside a slash command.
    fn slash_command_under_cursor(first_line: &str, cursor: usize) -> Option<(&str, &str)> {
        if !first_line.starts_with('/') {
            return None;
        }

        let name_start = 1usize;
        let name_end = first_line[name_start..]
            .find(char::is_whitespace)
            .map(|idx| name_start + idx)
            .unwrap_or_else(|| first_line.len());

        if cursor > name_end {
            return None;
        }

        let name = &first_line[name_start..name_end];
        let rest_start = first_line[name_end..]
            .find(|c: char| !c.is_whitespace())
            .map(|idx| name_end + idx)
            .unwrap_or(name_end);
        let rest = &first_line[rest_start..];

        Some((name, rest))
    }

    /// Heuristic for whether the typed slash command looks like a valid
    /// prefix for any known command (built-in or custom prompt).
    /// Empty names only count when there is no extra content after the '/'.
    fn looks_like_slash_prefix(&self, name: &str, rest_after_name: &str) -> bool {
        if !self.slash_commands_enabled() {
            return false;
        }
        if name.is_empty() {
            return rest_after_name.is_empty();
        }

        if slash_commands::has_builtin_prefix(
            name,
            self.collaboration_modes_enabled,
            self.connectors_enabled,
            self.personality_command_enabled,
            self.windows_degraded_sandbox_active,
        ) {
            return true;
        }

        self.custom_prompts.iter().any(|prompt| {
            fuzzy_match(&format!("{PROMPTS_CMD_PREFIX}:{}", prompt.name), name).is_some()
        })
    }

    /// Synchronize `self.command_popup` with the current text in the
    /// textarea. This must be called after every modification that can change
    /// the text so the popup is shown/updated/hidden as appropriate.
    fn sync_command_popup(&mut self, allow: bool) {
        if !allow {
            if matches!(self.active_popup, ActivePopup::Command(_)) {
                self.active_popup = ActivePopup::None;
            }
            return;
        }
        // Determine whether the caret is inside the initial '/name' token on the first line.
        let text = self.textarea.text();
        let first_line_end = text.find('\n').unwrap_or(text.len());
        let first_line = &text[..first_line_end];
        let cursor = self.textarea.cursor();
        let caret_on_first_line = cursor <= first_line_end;

        let is_editing_slash_command_name = caret_on_first_line
            && Self::slash_command_under_cursor(first_line, cursor)
                .is_some_and(|(name, rest)| self.looks_like_slash_prefix(name, rest));

        // If the cursor is currently positioned within an `@token`, prefer the
        // file-search popup over the slash popup so users can insert a file path
        // as an argument to the command (e.g., "/review @docs/...").
        if Self::current_at_token(&self.textarea).is_some() {
            if matches!(self.active_popup, ActivePopup::Command(_)) {
                self.active_popup = ActivePopup::None;
            }
            return;
        }
        match &mut self.active_popup {
            ActivePopup::Command(popup) => {
                if is_editing_slash_command_name {
                    popup.on_composer_text_change(first_line.to_string());
                } else {
                    self.active_popup = ActivePopup::None;
                }
            }
            _ => {
                if is_editing_slash_command_name {
                    let collaboration_modes_enabled = self.collaboration_modes_enabled;
                    let connectors_enabled = self.connectors_enabled;
                    let personality_command_enabled = self.personality_command_enabled;
                    let mut command_popup = CommandPopup::new(
                        self.custom_prompts.clone(),
                        CommandPopupFlags {
                            collaboration_modes_enabled,
                            connectors_enabled,
                            personality_command_enabled,
                            windows_degraded_sandbox_active: self.windows_degraded_sandbox_active,
                        },
                    );
                    command_popup.on_composer_text_change(first_line.to_string());
                    self.active_popup = ActivePopup::Command(command_popup);
                }
            }
        }
    }
    pub(crate) fn set_custom_prompts(&mut self, prompts: Vec<CustomPrompt>) {
        self.custom_prompts = prompts.clone();
        if let ActivePopup::Command(popup) = &mut self.active_popup {
            popup.set_prompts(prompts);
        }
    }

    /// Synchronize `self.file_search_popup` with the current text in the textarea.
    /// Note this is only called when self.active_popup is NOT Command.
    fn sync_file_search_popup(&mut self, query: String) {
        // If user dismissed popup for this exact query, don't reopen until text changes.
        if self.dismissed_file_popup_token.as_ref() == Some(&query) {
            return;
        }

        if query.is_empty() {
            self.app_event_tx
                .send(AppEvent::StartFileSearch(String::new()));
        } else {
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

        if query.is_empty() {
            self.current_file_query = None;
        } else {
            self.current_file_query = Some(query);
        }
        self.dismissed_file_popup_token = None;
    }

    fn sync_mention_popup(&mut self, query: String) {
        if self.dismissed_mention_popup_token.as_ref() == Some(&query) {
            return;
        }

        let mentions = self.mention_items();
        if mentions.is_empty() {
            self.active_popup = ActivePopup::None;
            return;
        }

        match &mut self.active_popup {
            ActivePopup::Skill(popup) => {
                popup.set_query(&query);
                popup.set_mentions(mentions);
            }
            _ => {
                let mut popup = SkillPopup::new(mentions);
                popup.set_query(&query);
                self.active_popup = ActivePopup::Skill(popup);
            }
        }
    }

    fn mention_items(&self) -> Vec<MentionItem> {
        let mut mentions = Vec::new();

        if let Some(skills) = self.skills.as_ref() {
            for skill in skills {
                let display_name = skill_display_name(skill).to_string();
                let description = skill_description(skill);
                let skill_name = skill.name.clone();
                let search_terms = if display_name == skill.name {
                    vec![skill_name.clone()]
                } else {
                    vec![skill_name.clone(), display_name.clone()]
                };
                mentions.push(MentionItem {
                    display_name,
                    description,
                    insert_text: format!("${skill_name}"),
                    search_terms,
                    path: Some(skill.path.to_string_lossy().into_owned()),
                });
            }
        }

        if self.connectors_enabled
            && let Some(snapshot) = self.connectors_snapshot.as_ref()
        {
            for connector in &snapshot.connectors {
                if !connector.is_accessible {
                    continue;
                }
                let display_name = connectors::connector_display_label(connector);
                let description = Some(Self::connector_brief_description(connector));
                let slug = codex_core::connectors::connector_mention_slug(connector);
                let search_terms = vec![display_name.clone(), connector.id.clone(), slug.clone()];
                let connector_id = connector.id.as_str();
                mentions.push(MentionItem {
                    display_name: display_name.clone(),
                    description,
                    insert_text: format!("${slug}"),
                    search_terms,
                    path: Some(format!("app://{connector_id}")),
                });
            }
        }

        mentions
    }

    fn connector_brief_description(connector: &AppInfo) -> String {
        let status_label = if connector.is_accessible {
            "Connected"
        } else {
            "Can be installed"
        };
        match Self::connector_description(connector) {
            Some(description) => format!("{status_label} - {description}"),
            None => status_label.to_string(),
        }
    }

    fn connector_description(connector: &AppInfo) -> Option<String> {
        connector
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .map(str::to_string)
    }

    fn set_has_focus(&mut self, has_focus: bool) {
        self.has_focus = has_focus;
    }

    #[allow(dead_code)]
    pub(crate) fn set_input_enabled(&mut self, enabled: bool, placeholder: Option<String>) {
        self.input_enabled = enabled;
        self.input_disabled_placeholder = if enabled { None } else { placeholder };

        // Avoid leaving interactive popups open while input is blocked.
        if !enabled && !matches!(self.active_popup, ActivePopup::None) {
            self.active_popup = ActivePopup::None;
        }
    }

    pub fn set_task_running(&mut self, running: bool) {
        self.is_task_running = running;
    }

    pub(crate) fn set_context_window(&mut self, percent: Option<i64>, used_tokens: Option<i64>) {
        if self.context_window_percent == percent && self.context_window_used_tokens == used_tokens
        {
            return;
        }
        self.context_window_percent = percent;
        self.context_window_used_tokens = used_tokens;
    }

    pub(crate) fn set_esc_backtrack_hint(&mut self, show: bool) {
        self.esc_backtrack_hint = show;
        if show {
            self.footer_mode = esc_hint_mode(self.footer_mode, self.is_task_running);
        } else {
            self.footer_mode = reset_mode_after_activity(self.footer_mode);
        }
    }
}

fn skill_display_name(skill: &SkillMetadata) -> &str {
    skill
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .unwrap_or(&skill.name)
}

fn skill_description(skill: &SkillMetadata) -> Option<String> {
    let description = skill
        .interface
        .as_ref()
        .and_then(|interface| interface.short_description.as_deref())
        .or(skill.short_description.as_deref())
        .unwrap_or(&skill.description);
    let trimmed = description.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn is_mention_name_char(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-')
}

impl Renderable for ChatComposer {
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if !self.input_enabled {
            return None;
        }

        let [_, textarea_rect, _] = self.layout_areas(area);
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    fn desired_height(&self, width: u16) -> u16 {
        let footer_props = self.footer_props();
        let footer_hint_height = self
            .custom_footer_height()
            .unwrap_or_else(|| footer_height(footer_props));
        let footer_spacing = Self::footer_spacing(footer_hint_height);
        let footer_total_height = footer_hint_height + footer_spacing;
        const COLS_WITH_MARGIN: u16 = LIVE_PREFIX_COLS + 1;
        self.textarea
            .desired_height(width.saturating_sub(COLS_WITH_MARGIN))
            + 2
            + match &self.active_popup {
                ActivePopup::None => footer_total_height,
                ActivePopup::Command(c) => c.calculate_required_height(width),
                ActivePopup::File(c) => c.calculate_required_height(),
                ActivePopup::Skill(c) => c.calculate_required_height(width),
            }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_with_mask(area, buf, None);
    }
}

impl ChatComposer {
    pub(crate) fn render_with_mask(&self, area: Rect, buf: &mut Buffer, mask_char: Option<char>) {
        let [composer_rect, textarea_rect, popup_rect] = self.layout_areas(area);
        match &self.active_popup {
            ActivePopup::Command(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::File(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::Skill(popup) => {
                popup.render_ref(popup_rect, buf);
            }
            ActivePopup::None => {
                let footer_props = self.footer_props();
                let show_cycle_hint =
                    !footer_props.is_task_running && self.collaboration_mode_indicator.is_some();
                let show_shortcuts_hint = match footer_props.mode {
                    FooterMode::ComposerEmpty => !self.is_in_paste_burst(),
                    FooterMode::QuitShortcutReminder
                    | FooterMode::ShortcutOverlay
                    | FooterMode::EscHint
                    | FooterMode::ComposerHasDraft => false,
                };
                let show_queue_hint = match footer_props.mode {
                    FooterMode::ComposerHasDraft => {
                        footer_props.is_task_running && footer_props.steer_enabled
                    }
                    FooterMode::QuitShortcutReminder
                    | FooterMode::ComposerEmpty
                    | FooterMode::ShortcutOverlay
                    | FooterMode::EscHint => false,
                };
                let context_line = context_window_line(
                    footer_props.context_window_percent,
                    footer_props.context_window_used_tokens,
                );
                let context_width = context_line.width() as u16;
                let custom_height = self.custom_footer_height();
                let footer_hint_height =
                    custom_height.unwrap_or_else(|| footer_height(footer_props));
                let footer_spacing = Self::footer_spacing(footer_hint_height);
                let hint_rect = if footer_spacing > 0 && footer_hint_height > 0 {
                    let [_, hint_rect] = Layout::vertical([
                        Constraint::Length(footer_spacing),
                        Constraint::Length(footer_hint_height),
                    ])
                    .areas(popup_rect);
                    hint_rect
                } else {
                    popup_rect
                };
                let left_width = if self.footer_flash_visible() {
                    self.footer_flash
                        .as_ref()
                        .map(|flash| flash.line.width() as u16)
                        .unwrap_or(0)
                } else if let Some(items) = self.footer_hint_override.as_ref() {
                    footer_hint_items_width(items)
                } else {
                    footer_line_width(
                        footer_props,
                        self.collaboration_mode_indicator,
                        show_cycle_hint,
                        show_shortcuts_hint,
                        show_queue_hint,
                    )
                };
                let can_show_left_and_context =
                    can_show_left_with_context(hint_rect, left_width, context_width);
                let has_override =
                    self.footer_flash_visible() || self.footer_hint_override.is_some();
                let single_line_layout = if has_override {
                    None
                } else {
                    match footer_props.mode {
                        FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft => {
                            // Both of these modes render the single-line footer style (with
                            // either the shortcuts hint or the optional queue hint). We still
                            // want the single-line collapse rules so the mode label can win over
                            // the context indicator on narrow widths.
                            Some(single_line_footer_layout(
                                hint_rect,
                                context_width,
                                self.collaboration_mode_indicator,
                                show_cycle_hint,
                                show_shortcuts_hint,
                                show_queue_hint,
                            ))
                        }
                        FooterMode::EscHint
                        | FooterMode::QuitShortcutReminder
                        | FooterMode::ShortcutOverlay => None,
                    }
                };
                let show_context = if matches!(
                    footer_props.mode,
                    FooterMode::EscHint
                        | FooterMode::QuitShortcutReminder
                        | FooterMode::ShortcutOverlay
                ) {
                    false
                } else {
                    single_line_layout
                        .as_ref()
                        .map(|(_, show_context)| *show_context)
                        .unwrap_or(can_show_left_and_context)
                };

                if let Some((summary_left, _)) = single_line_layout {
                    match summary_left {
                        SummaryLeft::Default => {
                            render_footer_from_props(
                                hint_rect,
                                buf,
                                footer_props,
                                self.collaboration_mode_indicator,
                                show_cycle_hint,
                                show_shortcuts_hint,
                                show_queue_hint,
                            );
                        }
                        SummaryLeft::Custom(line) => {
                            render_footer_line(hint_rect, buf, line);
                        }
                        SummaryLeft::None => {}
                    }
                } else if self.footer_flash_visible() {
                    if let Some(flash) = self.footer_flash.as_ref() {
                        flash.line.render(inset_footer_hint_area(hint_rect), buf);
                    }
                } else if let Some(items) = self.footer_hint_override.as_ref() {
                    render_footer_hint_items(hint_rect, buf, items);
                } else {
                    render_footer_from_props(
                        hint_rect,
                        buf,
                        footer_props,
                        self.collaboration_mode_indicator,
                        show_cycle_hint,
                        show_shortcuts_hint,
                        show_queue_hint,
                    );
                }

                if show_context {
                    render_context_right(hint_rect, buf, &context_line);
                }
            }
        }
        let style = user_message_style();
        Block::default().style(style).render_ref(composer_rect, buf);
        if !textarea_rect.is_empty() {
            let prompt = if self.input_enabled {
                "›".bold()
            } else {
                "›".dim()
            };
            buf.set_span(
                textarea_rect.x - LIVE_PREFIX_COLS,
                textarea_rect.y,
                &prompt,
                textarea_rect.width,
            );
        }

        let mut state = self.textarea_state.borrow_mut();
        if let Some(mask_char) = mask_char {
            self.textarea
                .render_ref_masked(textarea_rect, buf, &mut state, mask_char);
        } else {
            StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
        }
        if self.textarea.text().is_empty() {
            let text = if self.input_enabled {
                self.placeholder_text.as_str().to_string()
            } else {
                self.input_disabled_placeholder
                    .as_deref()
                    .unwrap_or("Input disabled.")
                    .to_string()
            };
            if !textarea_rect.is_empty() {
                let placeholder = Span::from(text).dim();
                Line::from(vec![placeholder])
                    .render_ref(textarea_rect.inner(Margin::new(0, 0)), buf);
            }
        }
    }
}

fn prompt_selection_action(
    prompt: &CustomPrompt,
    first_line: &str,
    mode: PromptSelectionMode,
    text_elements: &[TextElement],
) -> PromptSelectionAction {
    let named_args = prompt_argument_names(&prompt.content);
    let has_numeric = prompt_has_numeric_placeholders(&prompt.content);

    match mode {
        PromptSelectionMode::Completion => {
            if !named_args.is_empty() {
                let (text, cursor) =
                    prompt_command_with_arg_placeholders(&prompt.name, &named_args);
                return PromptSelectionAction::Insert {
                    text,
                    cursor: Some(cursor),
                };
            }
            if has_numeric {
                let text = format!("/{PROMPTS_CMD_PREFIX}:{} ", prompt.name);
                return PromptSelectionAction::Insert { text, cursor: None };
            }
            let text = format!("/{PROMPTS_CMD_PREFIX}:{}", prompt.name);
            PromptSelectionAction::Insert { text, cursor: None }
        }
        PromptSelectionMode::Submit => {
            if !named_args.is_empty() {
                let (text, cursor) =
                    prompt_command_with_arg_placeholders(&prompt.name, &named_args);
                return PromptSelectionAction::Insert {
                    text,
                    cursor: Some(cursor),
                };
            }
            if has_numeric {
                if let Some(expanded) =
                    expand_if_numeric_with_positional_args(prompt, first_line, text_elements)
                {
                    return PromptSelectionAction::Submit {
                        text: expanded.text,
                        text_elements: expanded.text_elements,
                    };
                }
                let text = format!("/{PROMPTS_CMD_PREFIX}:{} ", prompt.name);
                return PromptSelectionAction::Insert { text, cursor: None };
            }
            PromptSelectionAction::Submit {
                text: prompt.content.clone(),
                // By now we know this custom prompt has no args, so no text elements to preserve.
                text_elements: Vec::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;
    use image::Rgba;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::tempdir;

    use crate::app_event::AppEvent;

    use crate::bottom_pane::AppEventSender;
    use crate::bottom_pane::ChatComposer;
    use crate::bottom_pane::InputResult;
    use crate::bottom_pane::chat_composer::AttachedImage;
    use crate::bottom_pane::chat_composer::LARGE_PASTE_CHAR_THRESHOLD;
    use crate::bottom_pane::prompt_args::PromptArg;
    use crate::bottom_pane::prompt_args::extract_positional_args_for_prompt_line;
    use crate::bottom_pane::textarea::TextArea;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn footer_hint_row_is_separated_from_composer() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);

        let row_to_string = |y: u16| {
            let mut row = String::new();
            for x in 0..area.width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            row
        };

        let mut hint_row: Option<(u16, String)> = None;
        for y in 0..area.height {
            let row = row_to_string(y);
            if row.contains("? for shortcuts") {
                hint_row = Some((y, row));
                break;
            }
        }

        let (hint_row_idx, hint_row_contents) =
            hint_row.expect("expected footer hint row to be rendered");
        assert_eq!(
            hint_row_idx,
            area.height - 1,
            "hint row should occupy the bottom line: {hint_row_contents:?}",
        );

        assert!(
            hint_row_idx > 0,
            "expected a spacing row above the footer hints",
        );

        let spacing_row = row_to_string(hint_row_idx - 1);
        assert_eq!(
            spacing_row.trim(),
            "",
            "expected blank spacing row above hints but saw: {spacing_row:?}",
        );
    }

    #[test]
    fn footer_flash_overrides_footer_hint_override() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_footer_hint_override(Some(vec![("K".to_string(), "label".to_string())]));
        composer.show_footer_flash(Line::from("FLASH"), Duration::from_secs(10));

        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);

        let mut bottom_row = String::new();
        for x in 0..area.width {
            bottom_row.push(
                buf[(x, area.height - 1)]
                    .symbol()
                    .chars()
                    .next()
                    .unwrap_or(' '),
            );
        }
        assert!(
            bottom_row.contains("FLASH"),
            "expected flash content to render in footer row, saw: {bottom_row:?}",
        );
        assert!(
            !bottom_row.contains("K label"),
            "expected flash to override hint override, saw: {bottom_row:?}",
        );
    }

    #[test]
    fn footer_flash_expires_and_falls_back_to_hint_override() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_footer_hint_override(Some(vec![("K".to_string(), "label".to_string())]));
        composer.show_footer_flash(Line::from("FLASH"), Duration::from_secs(10));
        composer.footer_flash.as_mut().unwrap().expires_at =
            Instant::now() - Duration::from_secs(1);

        let area = Rect::new(0, 0, 60, 6);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);

        let mut bottom_row = String::new();
        for x in 0..area.width {
            bottom_row.push(
                buf[(x, area.height - 1)]
                    .symbol()
                    .chars()
                    .next()
                    .unwrap_or(' '),
            );
        }
        assert!(
            bottom_row.contains("K label"),
            "expected hint override to render after flash expired, saw: {bottom_row:?}",
        );
        assert!(
            !bottom_row.contains("FLASH"),
            "expected expired flash to be hidden, saw: {bottom_row:?}",
        );
    }

    fn snapshot_composer_state_with_width<F>(
        name: &str,
        width: u16,
        enhanced_keys_supported: bool,
        setup: F,
    ) where
        F: FnOnce(&mut ChatComposer),
    {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            enhanced_keys_supported,
            "Ask Codex to do anything".to_string(),
            false,
        );
        setup(&mut composer);
        let footer_props = composer.footer_props();
        let footer_lines = footer_height(footer_props);
        let footer_spacing = ChatComposer::footer_spacing(footer_lines);
        let height = footer_lines + footer_spacing + 8;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| composer.render(f.area(), f.buffer_mut()))
            .unwrap();
        insta::assert_snapshot!(name, terminal.backend());
    }

    fn snapshot_composer_state<F>(name: &str, enhanced_keys_supported: bool, setup: F)
    where
        F: FnOnce(&mut ChatComposer),
    {
        snapshot_composer_state_with_width(name, 100, enhanced_keys_supported, setup);
    }

    #[test]
    fn footer_mode_snapshots() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        snapshot_composer_state("footer_mode_shortcut_overlay", true, |composer| {
            composer.set_esc_backtrack_hint(true);
            let _ =
                composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        });

        snapshot_composer_state("footer_mode_ctrl_c_quit", true, |composer| {
            composer.show_quit_shortcut_hint(key_hint::ctrl(KeyCode::Char('c')), true);
        });

        snapshot_composer_state("footer_mode_ctrl_c_interrupt", true, |composer| {
            composer.set_task_running(true);
            composer.show_quit_shortcut_hint(key_hint::ctrl(KeyCode::Char('c')), true);
        });

        snapshot_composer_state("footer_mode_ctrl_c_then_esc_hint", true, |composer| {
            composer.show_quit_shortcut_hint(key_hint::ctrl(KeyCode::Char('c')), true);
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        });

        snapshot_composer_state("footer_mode_esc_hint_from_overlay", true, |composer| {
            let _ =
                composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        });

        snapshot_composer_state("footer_mode_esc_hint_backtrack", true, |composer| {
            composer.set_esc_backtrack_hint(true);
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        });

        snapshot_composer_state(
            "footer_mode_overlay_then_external_esc_hint",
            true,
            |composer| {
                let _ = composer
                    .handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
                composer.set_esc_backtrack_hint(true);
            },
        );

        snapshot_composer_state("footer_mode_hidden_while_typing", true, |composer| {
            type_chars_humanlike(composer, &['h']);
        });
    }

    #[test]
    fn footer_collapse_snapshots() {
        fn setup_collab_footer(
            composer: &mut ChatComposer,
            context_percent: i64,
            indicator: Option<CollaborationModeIndicator>,
        ) {
            composer.set_collaboration_modes_enabled(true);
            composer.set_collaboration_mode_indicator(indicator);
            composer.set_context_window(Some(context_percent), None);
        }

        // Empty textarea, agent idle: shortcuts hint can show, and cycle hint is hidden.
        snapshot_composer_state_with_width("footer_collapse_empty_full", 120, true, |composer| {
            setup_collab_footer(composer, 100, None);
        });
        snapshot_composer_state_with_width(
            "footer_collapse_empty_mode_cycle_with_context",
            60,
            true,
            |composer| {
                setup_collab_footer(composer, 100, None);
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_empty_mode_cycle_without_context",
            44,
            true,
            |composer| {
                setup_collab_footer(composer, 100, None);
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_empty_mode_only",
            26,
            true,
            |composer| {
                setup_collab_footer(composer, 100, None);
            },
        );

        // Empty textarea, plan mode idle: shortcuts hint and cycle hint are available.
        snapshot_composer_state_with_width(
            "footer_collapse_plan_empty_full",
            120,
            true,
            |composer| {
                setup_collab_footer(composer, 100, Some(CollaborationModeIndicator::Plan));
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_empty_mode_cycle_with_context",
            60,
            true,
            |composer| {
                setup_collab_footer(composer, 100, Some(CollaborationModeIndicator::Plan));
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_empty_mode_cycle_without_context",
            44,
            true,
            |composer| {
                setup_collab_footer(composer, 100, Some(CollaborationModeIndicator::Plan));
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_empty_mode_only",
            26,
            true,
            |composer| {
                setup_collab_footer(composer, 100, Some(CollaborationModeIndicator::Plan));
            },
        );

        // Textarea has content, agent running, steer enabled: queue hint is shown.
        snapshot_composer_state_with_width("footer_collapse_queue_full", 120, true, |composer| {
            setup_collab_footer(composer, 98, None);
            composer.set_steer_enabled(true);
            composer.set_task_running(true);
            composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
        });
        snapshot_composer_state_with_width(
            "footer_collapse_queue_short_with_context",
            50,
            true,
            |composer| {
                setup_collab_footer(composer, 98, None);
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_queue_message_without_context",
            40,
            true,
            |composer| {
                setup_collab_footer(composer, 98, None);
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_queue_short_without_context",
            30,
            true,
            |composer| {
                setup_collab_footer(composer, 98, None);
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_queue_mode_only",
            20,
            true,
            |composer| {
                setup_collab_footer(composer, 98, None);
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );

        // Textarea has content, plan mode active, agent running, steer enabled: queue hint + mode.
        snapshot_composer_state_with_width(
            "footer_collapse_plan_queue_full",
            120,
            true,
            |composer| {
                setup_collab_footer(composer, 98, Some(CollaborationModeIndicator::Plan));
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_queue_short_with_context",
            50,
            true,
            |composer| {
                setup_collab_footer(composer, 98, Some(CollaborationModeIndicator::Plan));
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_queue_message_without_context",
            40,
            true,
            |composer| {
                setup_collab_footer(composer, 98, Some(CollaborationModeIndicator::Plan));
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_queue_short_without_context",
            30,
            true,
            |composer| {
                setup_collab_footer(composer, 98, Some(CollaborationModeIndicator::Plan));
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
        snapshot_composer_state_with_width(
            "footer_collapse_plan_queue_mode_only",
            20,
            true,
            |composer| {
                setup_collab_footer(composer, 98, Some(CollaborationModeIndicator::Plan));
                composer.set_steer_enabled(true);
                composer.set_task_running(true);
                composer.set_text_content("Test".to_string(), Vec::new(), Vec::new());
            },
        );
    }

    #[test]
    fn esc_hint_stays_hidden_with_draft_content() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            true,
            "Ask Codex to do anything".to_string(),
            false,
        );

        type_chars_humanlike(&mut composer, &['d']);

        assert!(!composer.is_empty());
        assert_eq!(composer.current_text(), "d");
        assert_eq!(composer.footer_mode, FooterMode::ComposerEmpty);
        assert!(matches!(composer.active_popup, ActivePopup::None));

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(composer.footer_mode, FooterMode::ComposerEmpty);
        assert!(!composer.esc_backtrack_hint);
    }

    #[test]
    fn base_footer_mode_tracks_empty_state_after_quit_hint_expires() {
        use crossterm::event::KeyCode;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        type_chars_humanlike(&mut composer, &['d']);
        composer.show_quit_shortcut_hint(key_hint::ctrl(KeyCode::Char('c')), true);
        composer.quit_shortcut_expires_at =
            Some(Instant::now() - std::time::Duration::from_secs(1));

        assert_eq!(composer.footer_mode(), FooterMode::ComposerHasDraft);

        composer.set_text_content(String::new(), Vec::new(), Vec::new());
        assert_eq!(composer.footer_mode(), FooterMode::ComposerEmpty);
    }

    #[test]
    fn clear_for_ctrl_c_records_cleared_draft() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        composer.set_text_content("draft text".to_string(), Vec::new(), Vec::new());
        assert_eq!(composer.clear_for_ctrl_c(), Some("draft text".to_string()));
        assert!(composer.is_empty());

        assert_eq!(
            composer.history.navigate_up(&composer.app_event_tx),
            Some(HistoryEntry::from_text("draft text".to_string()))
        );
    }

    /// Behavior: `?` toggles the shortcut overlay only when the composer is otherwise empty. After
    /// any typing has occurred, `?` should be inserted as a literal character.
    #[test]
    fn question_mark_only_toggles_on_first_char() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        let (result, needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert_eq!(result, InputResult::None);
        assert!(needs_redraw, "toggling overlay should request redraw");
        assert_eq!(composer.footer_mode, FooterMode::ShortcutOverlay);

        // Toggle back to prompt mode so subsequent typing captures characters.
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert_eq!(composer.footer_mode, FooterMode::ComposerEmpty);

        type_chars_humanlike(&mut composer, &['h']);
        assert_eq!(composer.textarea.text(), "h");
        assert_eq!(composer.footer_mode(), FooterMode::ComposerHasDraft);

        let (result, needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert_eq!(result, InputResult::None);
        assert!(needs_redraw, "typing should still mark the view dirty");
        let _ = flush_after_paste_burst(&mut composer);
        assert_eq!(composer.textarea.text(), "h?");
        assert_eq!(composer.footer_mode, FooterMode::ComposerEmpty);
        assert_eq!(composer.footer_mode(), FooterMode::ComposerHasDraft);
    }

    /// Behavior: while a paste-like burst is being captured, `?` must not toggle the shortcut
    /// overlay; it should be treated as part of the pasted content.
    #[test]
    fn question_mark_does_not_toggle_during_paste_burst() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        // Force an active paste burst so this test doesn't depend on tight timing.
        composer
            .paste_burst
            .begin_with_retro_grabbed(String::new(), Instant::now());

        for ch in ['h', 'i', '?', 't', 'h', 'e', 'r', 'e'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert!(composer.is_in_paste_burst());
        assert_eq!(composer.textarea.text(), "");

        let _ = flush_after_paste_burst(&mut composer);

        assert_eq!(composer.textarea.text(), "hi?there");
        assert_ne!(composer.footer_mode, FooterMode::ShortcutOverlay);
    }

    #[test]
    fn shortcut_overlay_persists_while_task_running() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert_eq!(composer.footer_mode, FooterMode::ShortcutOverlay);

        composer.set_task_running(true);

        assert_eq!(composer.footer_mode, FooterMode::ShortcutOverlay);
        assert_eq!(composer.footer_mode(), FooterMode::ShortcutOverlay);
    }

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

    /// Behavior: if the ASCII path has a pending first char (flicker suppression) and a non-ASCII
    /// char arrives next, the pending ASCII char should still be preserved and the overall input
    /// should submit normally (i.e. we should not misclassify this as a paste burst).
    #[test]
    fn ascii_prefix_survives_non_ascii_followup() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert!(composer.is_in_paste_burst());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('あ'), KeyModifiers::NONE));

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted { text, .. } => assert_eq!(text, "1あ"),
            _ => panic!("expected Submitted"),
        }
    }

    /// Behavior: a single non-ASCII char should be inserted immediately (IME-friendly) and should
    /// not create any paste-burst state.
    #[test]
    fn non_ascii_char_inserts_immediately_without_burst_state() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('あ'), KeyModifiers::NONE));

        assert_eq!(composer.textarea.text(), "あ");
        assert!(!composer.is_in_paste_burst());
    }

    /// Behavior: while we're capturing a paste-like burst, Enter should be treated as a newline
    /// within the burst (not as "submit"), and the whole payload should flush as one paste.
    #[test]
    fn non_ascii_burst_buffers_enter_and_flushes_multiline() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer
            .paste_burst
            .begin_with_retro_grabbed(String::new(), Instant::now());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('你'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('好'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));

        assert!(composer.textarea.text().is_empty());
        let _ = flush_after_paste_burst(&mut composer);
        assert_eq!(composer.textarea.text(), "你好\nhi");
    }

    /// Behavior: a paste-like burst may include a full-width/ideographic space (U+3000). It should
    /// still be captured as a single paste payload and preserve the exact Unicode content.
    #[test]
    fn non_ascii_burst_preserves_ideographic_space_and_ascii() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer
            .paste_burst
            .begin_with_retro_grabbed(String::new(), Instant::now());

        for ch in ['你', '　', '好'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        for ch in ['h', 'i'] {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert!(composer.textarea.text().is_empty());
        let _ = flush_after_paste_burst(&mut composer);
        assert_eq!(composer.textarea.text(), "你　好\nhi");
    }

    /// Behavior: a large multi-line payload containing both non-ASCII and ASCII (e.g. "UTF-8",
    /// "Unicode") should be captured as a single paste-like burst, and Enter key events should
    /// become `\n` within the buffered content.
    #[test]
    fn non_ascii_burst_buffers_large_multiline_mixed_ascii_and_unicode() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        const LARGE_MIXED_PAYLOAD: &str = "天地玄黄 宇宙洪荒\n\
日月盈昃 辰宿列张\n\
寒来暑往 秋收冬藏\n\
\n\
你好世界 编码测试\n\
汉字处理 UTF-8\n\
终端显示 正确无误\n\
\n\
风吹竹林 月照大江\n\
白云千载 青山依旧\n\
程序员 与 Unicode 同行";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Force an active burst so the test doesn't depend on timing heuristics.
        composer
            .paste_burst
            .begin_with_retro_grabbed(String::new(), Instant::now());

        for ch in LARGE_MIXED_PAYLOAD.chars() {
            let code = if ch == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(ch)
            };
            let _ = composer.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
        }

        assert!(composer.textarea.text().is_empty());
        let _ = flush_after_paste_burst(&mut composer);
        assert_eq!(composer.textarea.text(), LARGE_MIXED_PAYLOAD);
    }

    /// Behavior: while a paste-like burst is active, Enter should not submit; it should insert a
    /// newline into the buffered payload and flush as a single paste later.
    #[test]
    fn ascii_burst_treats_enter_as_newline() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        let mut now = Instant::now();
        let step = Duration::from_millis(1);

        let _ = composer.handle_input_basic_with_time(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            now,
        );
        now += step;
        let _ = composer.handle_input_basic_with_time(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            now,
        );
        now += step;

        let (result, _) = composer.handle_submission_with_time(!composer.steer_enabled, now);
        assert!(
            matches!(result, InputResult::None),
            "Enter during a burst should insert newline, not submit"
        );

        for ch in ['t', 'h', 'e', 'r', 'e'] {
            now += step;
            let _ = composer.handle_input_basic_with_time(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                now,
            );
        }

        assert!(composer.textarea.text().is_empty());
        let flush_time = now + PasteBurst::recommended_active_flush_delay() + step;
        let flushed = composer.handle_paste_burst_flush(flush_time);
        assert!(flushed, "expected paste burst to flush");
        assert_eq!(composer.textarea.text(), "hi\nthere");
    }

    /// Behavior: even if Enter suppression would normally be active for a burst, Enter should
    /// still dispatch a built-in slash command when the first line begins with `/`.
    #[test]
    fn slash_context_enter_ignores_paste_burst_enter_suppression() {
        use crate::slash_command::SlashCommand;
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer.textarea.set_text_clearing_elements("/diff");
        composer.textarea.set_cursor("/diff".len());
        composer
            .paste_burst
            .begin_with_retro_grabbed(String::new(), Instant::now());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, InputResult::Command(SlashCommand::Diff)));
    }

    /// Behavior: if a burst is buffering text and the user presses a non-char key, flush the
    /// buffered burst *before* applying that key so the buffer cannot get stuck.
    #[test]
    fn non_char_key_flushes_active_burst_before_input() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Force an active burst so we can deterministically buffer characters without relying on
        // timing.
        composer
            .paste_burst
            .begin_with_retro_grabbed(String::new(), Instant::now());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert!(composer.textarea.text().is_empty());
        assert!(composer.is_in_paste_burst());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(composer.textarea.text(), "hi");
        assert_eq!(composer.textarea.cursor(), 1);
        assert!(!composer.is_in_paste_burst());
    }

    /// Behavior: enabling `disable_paste_burst` flushes any held first character (flicker
    /// suppression) and then inserts subsequent chars immediately without creating burst state.
    #[test]
    fn disable_paste_burst_flushes_pending_first_char_and_inserts_immediately() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // First ASCII char is normally held briefly. Flip the config mid-stream and ensure the
        // held char is not dropped.
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(composer.is_in_paste_burst());
        assert!(composer.textarea.text().is_empty());

        composer.set_disable_paste_burst(true);
        assert_eq!(composer.textarea.text(), "a");
        assert!(!composer.is_in_paste_burst());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(composer.textarea.text(), "ab");
        assert!(!composer.is_in_paste_burst());
    }

    /// Behavior: a small explicit paste inserts text directly (no placeholder), and the submitted
    /// text matches what is visible in the textarea.
    #[test]
    fn handle_paste_small_inserts_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        let needs_redraw = composer.handle_paste("hello".to_string());
        assert!(needs_redraw);
        assert_eq!(composer.textarea.text(), "hello");
        assert!(composer.pending_pastes.is_empty());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("expected Submitted"),
        }
    }

    #[test]
    fn empty_enter_returns_none() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);
        composer.set_steer_enabled(true);

        // Ensure composer is empty and press Enter.
        assert!(composer.textarea.text().is_empty());
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match result {
            InputResult::None => {}
            other => panic!("expected None for empty enter, got: {other:?}"),
        }
    }

    /// Behavior: a large explicit paste inserts a placeholder into the textarea, stores the full
    /// content in `pending_pastes`, and expands the placeholder to the full content on submit.
    #[test]
    fn handle_paste_large_uses_placeholder_and_replaces_on_submit() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

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
            InputResult::Submitted { text, .. } => assert_eq!(text, large),
            _ => panic!("expected Submitted"),
        }
        assert!(composer.pending_pastes.is_empty());
    }

    /// Behavior: editing that removes a paste placeholder should also clear the associated
    /// `pending_pastes` entry so it cannot be submitted accidentally.
    #[test]
    fn edit_clears_pending_paste() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let large = "y".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1);
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

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
                false,
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
                .draw(|f| composer.render(f.area(), f.buffer_mut()))
                .unwrap_or_else(|e| panic!("Failed to draw {name} composer: {e}"));

            insta::assert_snapshot!(name, terminal.backend());
        }
    }

    #[test]
    fn image_placeholder_snapshots() {
        snapshot_composer_state("image_placeholder_single", false, |composer| {
            composer.attach_image(PathBuf::from("/tmp/image1.png"));
        });

        snapshot_composer_state("image_placeholder_multiple", false, |composer| {
            composer.attach_image(PathBuf::from("/tmp/image1.png"));
            composer.attach_image(PathBuf::from("/tmp/image2.png"));
        });
    }

    #[test]
    fn slash_popup_model_first_for_mo_ui() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);

        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        // Type "/mo" humanlike so paste-burst doesn’t interfere.
        type_chars_humanlike(&mut composer, &['/', 'm', 'o']);

        let mut terminal = match Terminal::new(TestBackend::new(60, 5)) {
            Ok(t) => t,
            Err(e) => panic!("Failed to create terminal: {e}"),
        };
        terminal
            .draw(|f| composer.render(f.area(), f.buffer_mut()))
            .unwrap_or_else(|e| panic!("Failed to draw composer: {e}"));

        // Visual snapshot should show the slash popup with /model as the first entry.
        insta::assert_snapshot!("slash_popup_mo", terminal.backend());
    }

    #[test]
    fn slash_popup_model_first_for_mo_logic() {
        use super::super::command_popup::CommandItem;
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        type_chars_humanlike(&mut composer, &['/', 'm', 'o']);

        match &composer.active_popup {
            ActivePopup::Command(popup) => match popup.selected_item() {
                Some(CommandItem::Builtin(cmd)) => {
                    assert_eq!(cmd.command(), "model")
                }
                Some(CommandItem::UserPrompt(_)) => {
                    panic!("unexpected prompt selected for '/mo'")
                }
                None => panic!("no selected command for '/mo'"),
            },
            _ => panic!("slash popup not active after typing '/mo'"),
        }
    }

    #[test]
    fn slash_popup_resume_for_res_ui() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);

        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        // Type "/res" humanlike so paste-burst doesn’t interfere.
        type_chars_humanlike(&mut composer, &['/', 'r', 'e', 's']);

        let mut terminal = Terminal::new(TestBackend::new(60, 6)).expect("terminal");
        terminal
            .draw(|f| composer.render(f.area(), f.buffer_mut()))
            .expect("draw composer");

        // Snapshot should show /resume as the first entry for /res.
        insta::assert_snapshot!("slash_popup_res", terminal.backend());
    }

    #[test]
    fn slash_popup_resume_for_res_logic() {
        use super::super::command_popup::CommandItem;
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        type_chars_humanlike(&mut composer, &['/', 'r', 'e', 's']);

        match &composer.active_popup {
            ActivePopup::Command(popup) => match popup.selected_item() {
                Some(CommandItem::Builtin(cmd)) => {
                    assert_eq!(cmd.command(), "resume")
                }
                Some(CommandItem::UserPrompt(_)) => {
                    panic!("unexpected prompt selected for '/res'")
                }
                None => panic!("no selected command for '/res'"),
            },
            _ => panic!("slash popup not active after typing '/res'"),
        }
    }

    fn flush_after_paste_burst(composer: &mut ChatComposer) -> bool {
        std::thread::sleep(PasteBurst::recommended_active_flush_delay());
        composer.flush_paste_burst_if_due()
    }

    // Test helper: simulate human typing with a brief delay and flush the paste-burst buffer
    fn type_chars_humanlike(composer: &mut ChatComposer, chars: &[char]) {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        for &ch in chars {
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            std::thread::sleep(ChatComposer::recommended_paste_flush_delay());
            let _ = composer.flush_paste_burst_if_due();
        }
    }

    #[test]
    fn slash_init_dispatches_command_and_does_not_submit_literal_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Type the slash command.
        type_chars_humanlike(&mut composer, &['/', 'i', 'n', 'i', 't']);

        // Press Enter to dispatch the selected command.
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // When a slash command is dispatched, the composer should return a
        // Command result (not submit literal text) and clear its textarea.
        match result {
            InputResult::Command(cmd) => {
                assert_eq!(cmd.command(), "init");
            }
            InputResult::CommandWithArgs(_, _) => {
                panic!("expected command dispatch without args for '/init'")
            }
            InputResult::Submitted { text, .. } => {
                panic!("expected command dispatch, but composer submitted literal text: {text}")
            }
            InputResult::Queued { .. } => {
                panic!("expected command dispatch, but composer queued literal text")
            }
            InputResult::None => panic!("expected Command result for '/init'"),
        }
        assert!(composer.textarea.is_empty(), "composer should be cleared");
    }

    #[test]
    fn extract_args_supports_quoted_paths_single_arg() {
        let args = extract_positional_args_for_prompt_line(
            "/prompts:review \"docs/My File.md\"",
            "review",
            &[],
        );
        assert_eq!(
            args,
            vec![PromptArg {
                text: "docs/My File.md".to_string(),
                text_elements: Vec::new(),
            }]
        );
    }

    #[test]
    fn extract_args_supports_mixed_quoted_and_unquoted() {
        let args = extract_positional_args_for_prompt_line(
            "/prompts:cmd \"with spaces\" simple",
            "cmd",
            &[],
        );
        assert_eq!(
            args,
            vec![
                PromptArg {
                    text: "with spaces".to_string(),
                    text_elements: Vec::new(),
                },
                PromptArg {
                    text: "simple".to_string(),
                    text_elements: Vec::new(),
                }
            ]
        );
    }

    #[test]
    fn slash_tab_completion_moves_cursor_to_end() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        type_chars_humanlike(&mut composer, &['/', 'c']);

        let (_result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(composer.textarea.text(), "/compact ");
        assert_eq!(composer.textarea.cursor(), composer.textarea.text().len());
    }

    #[test]
    fn slash_tab_then_enter_dispatches_builtin_command() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Type a prefix and complete with Tab, which inserts a trailing space
        // and moves the cursor beyond the '/name' token (hides the popup).
        type_chars_humanlike(&mut composer, &['/', 'd', 'i']);
        let (_res, _redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(composer.textarea.text(), "/diff ");

        // Press Enter: should dispatch the command, not submit literal text.
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Command(cmd) => assert_eq!(cmd.command(), "diff"),
            InputResult::CommandWithArgs(_, _) => {
                panic!("expected command dispatch without args for '/diff'")
            }
            InputResult::Submitted { text, .. } => {
                panic!("expected command dispatch after Tab completion, got literal submit: {text}")
            }
            InputResult::Queued { .. } => {
                panic!("expected command dispatch after Tab completion, got literal queue")
            }
            InputResult::None => panic!("expected Command result for '/diff'"),
        }
        assert!(composer.textarea.is_empty());
    }

    #[test]
    fn slash_mention_dispatches_command_and_inserts_at() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        type_chars_humanlike(&mut composer, &['/', 'm', 'e', 'n', 't', 'i', 'o', 'n']);

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        match result {
            InputResult::Command(cmd) => {
                assert_eq!(cmd.command(), "mention");
            }
            InputResult::CommandWithArgs(_, _) => {
                panic!("expected command dispatch without args for '/mention'")
            }
            InputResult::Submitted { text, .. } => {
                panic!("expected command dispatch, but composer submitted literal text: {text}")
            }
            InputResult::Queued { .. } => {
                panic!("expected command dispatch, but composer queued literal text")
            }
            InputResult::None => panic!("expected Command result for '/mention'"),
        }
        assert!(composer.textarea.is_empty(), "composer should be cleared");
        composer.insert_str("@");
        assert_eq!(composer.textarea.text(), "@");
    }

    /// Behavior: multiple paste operations can coexist; placeholders should be expanded to their
    /// original content on submission.
    #[test]
    fn test_multiple_pastes_submission() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

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
        if let InputResult::Submitted { text, .. } = result {
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
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

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

    /// Behavior: if multiple large pastes share the same placeholder label (same char count),
    /// deleting one placeholder removes only its corresponding `pending_pastes` entry.
    #[test]
    fn deleting_duplicate_length_pastes_removes_only_target() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let paste = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 4);
        let placeholder_base = format!("[Pasted Content {} chars]", paste.chars().count());
        let placeholder_second = format!("{placeholder_base} #2");

        composer.handle_paste(paste.clone());
        composer.handle_paste(paste.clone());
        assert_eq!(
            composer.textarea.text(),
            format!("{placeholder_base}{placeholder_second}")
        );
        assert_eq!(composer.pending_pastes.len(), 2);

        composer.textarea.set_cursor(composer.textarea.text().len());
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert_eq!(composer.textarea.text(), placeholder_base);
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, placeholder_base);
        assert_eq!(composer.pending_pastes[0].1, paste);
    }

    /// Behavior: large-paste placeholder numbering does not get reused after deletion, so a new
    /// paste of the same length gets a new unique placeholder label.
    #[test]
    fn large_paste_numbering_does_not_reuse_after_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let paste = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 4);
        let base = format!("[Pasted Content {} chars]", paste.chars().count());
        let second = format!("{base} #2");
        let third = format!("{base} #3");

        composer.handle_paste(paste.clone());
        composer.handle_paste(paste.clone());
        assert_eq!(composer.textarea.text(), format!("{base}{second}"));

        composer.textarea.set_cursor(base.len());
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(composer.textarea.text(), second);
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, second);

        composer.textarea.set_cursor(composer.textarea.text().len());
        composer.handle_paste(paste);

        assert_eq!(composer.textarea.text(), format!("{second}{third}"));
        assert_eq!(composer.pending_pastes.len(), 2);
        assert_eq!(composer.pending_pastes[0].0, second);
        assert_eq!(composer.pending_pastes[1].0, third);
    }

    #[test]
    fn test_partial_placeholder_deletion() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

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
                    .set_cursor(placeholder.len() - pos_from_end);
                composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                let result = (
                    composer.textarea.text().contains(&placeholder),
                    composer.pending_pastes.len(),
                );
                composer.textarea.set_text_clearing_elements("");
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
    fn attach_image_and_submit_includes_local_image_paths() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        let path = PathBuf::from("/tmp/image1.png");
        composer.attach_image(path.clone());
        composer.handle_paste(" hi".into());
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                assert_eq!(text, "[Image #1] hi");
                assert_eq!(text_elements.len(), 1);
                assert_eq!(text_elements[0].placeholder(&text), Some("[Image #1]"));
                assert_eq!(
                    text_elements[0].byte_range,
                    ByteRange {
                        start: 0,
                        end: "[Image #1]".len()
                    }
                );
            }
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(vec![path], imgs);
    }

    #[test]
    fn history_navigation_restores_image_attachments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        let path = PathBuf::from("/tmp/image1.png");
        composer.attach_image(path.clone());

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, InputResult::Submitted { .. }));

        composer.set_text_content(String::new(), Vec::new(), Vec::new());

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        let text = composer.current_text();
        assert_eq!(text, "[Image #1]");
        let text_elements = composer.text_elements();
        assert_eq!(text_elements.len(), 1);
        assert_eq!(text_elements[0].placeholder(&text), Some("[Image #1]"));
        assert_eq!(composer.local_image_paths(), vec![path]);
    }

    #[test]
    fn set_text_content_reattaches_images_without_placeholder_metadata() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let placeholder = local_image_label_text(1);
        let text = format!("{placeholder} restored");
        let text_elements = vec![TextElement::new((0..placeholder.len()).into(), None)];
        let path = PathBuf::from("/tmp/image1.png");

        composer.set_text_content(text, text_elements, vec![path.clone()]);

        assert_eq!(composer.local_image_paths(), vec![path]);
    }

    #[test]
    fn large_paste_preserves_image_text_elements_on_submit() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        let large_content = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5);
        composer.handle_paste(large_content.clone());
        composer.handle_paste(" ".into());
        let path = PathBuf::from("/tmp/image_with_paste.png");
        composer.attach_image(path.clone());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                let expected = format!("{large_content} [Image #1]");
                assert_eq!(text, expected);
                assert_eq!(text_elements.len(), 1);
                assert_eq!(text_elements[0].placeholder(&text), Some("[Image #1]"));
                assert_eq!(
                    text_elements[0].byte_range,
                    ByteRange {
                        start: large_content.len() + 1,
                        end: large_content.len() + 1 + "[Image #1]".len(),
                    }
                );
            }
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(vec![path], imgs);
    }

    #[test]
    fn large_paste_with_leading_whitespace_trims_and_shifts_elements() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        let large_content = format!("  {}", "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5));
        composer.handle_paste(large_content.clone());
        composer.handle_paste(" ".into());
        let path = PathBuf::from("/tmp/image_with_trim.png");
        composer.attach_image(path.clone());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                let trimmed = large_content.trim().to_string();
                assert_eq!(text, format!("{trimmed} [Image #1]"));
                assert_eq!(text_elements.len(), 1);
                assert_eq!(text_elements[0].placeholder(&text), Some("[Image #1]"));
                assert_eq!(
                    text_elements[0].byte_range,
                    ByteRange {
                        start: trimmed.len() + 1,
                        end: trimmed.len() + 1 + "[Image #1]".len(),
                    }
                );
            }
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(vec![path], imgs);
    }

    #[test]
    fn pasted_crlf_normalizes_newlines_for_elements() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        let pasted = "line1\r\nline2\r\n".to_string();
        composer.handle_paste(pasted);
        composer.handle_paste(" ".into());
        let path = PathBuf::from("/tmp/image_crlf.png");
        composer.attach_image(path.clone());

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                assert_eq!(text, "line1\nline2\n [Image #1]");
                assert!(!text.contains('\r'));
                assert_eq!(text_elements.len(), 1);
                assert_eq!(text_elements[0].placeholder(&text), Some("[Image #1]"));
                assert_eq!(
                    text_elements[0].byte_range,
                    ByteRange {
                        start: "line1\nline2\n ".len(),
                        end: "line1\nline2\n [Image #1]".len(),
                    }
                );
            }
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(vec![path], imgs);
    }

    #[test]
    fn suppressed_submission_restores_pending_paste_payload() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.textarea.set_text_clearing_elements("/unknown ");
        composer.textarea.set_cursor("/unknown ".len());
        let large_content = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5);
        composer.handle_paste(large_content.clone());
        let placeholder = composer
            .pending_pastes
            .first()
            .expect("expected pending paste")
            .0
            .clone();

        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, InputResult::None));
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.textarea.text(), format!("/unknown {placeholder}"));

        composer.textarea.set_cursor(0);
        composer.textarea.insert_str(" ");
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                assert_eq!(text, format!("/unknown {large_content}"));
                assert!(text_elements.is_empty());
            }
            _ => panic!("expected Submitted"),
        }
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn attach_image_without_text_submits_empty_text_and_images() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);
        let path = PathBuf::from("/tmp/image2.png");
        composer.attach_image(path.clone());
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                assert_eq!(text, "[Image #1]");
                assert_eq!(text_elements.len(), 1);
                assert_eq!(text_elements[0].placeholder(&text), Some("[Image #1]"));
                assert_eq!(
                    text_elements[0].byte_range,
                    ByteRange {
                        start: 0,
                        end: "[Image #1]".len()
                    }
                );
            }
            _ => panic!("expected Submitted"),
        }
        let imgs = composer.take_recent_submission_images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0], path);
        assert!(composer.attached_images.is_empty());
    }

    #[test]
    fn duplicate_image_placeholders_get_suffix() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        let path = PathBuf::from("/tmp/image_dup.png");
        composer.attach_image(path.clone());
        composer.handle_paste(" ".into());
        composer.attach_image(path);

        let text = composer.textarea.text().to_string();
        assert!(text.contains("[Image #1]"));
        assert!(text.contains("[Image #2]"));
        assert_eq!(composer.attached_images[0].placeholder, "[Image #1]");
        assert_eq!(composer.attached_images[1].placeholder, "[Image #2]");
    }

    #[test]
    fn image_placeholder_backspace_behaves_like_text_placeholder() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        let path = PathBuf::from("/tmp/image3.png");
        composer.attach_image(path.clone());
        let placeholder = composer.attached_images[0].placeholder.clone();

        // Case 1: backspace at end
        composer.textarea.move_cursor_to_end_of_line(false);
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(!composer.textarea.text().contains(&placeholder));
        assert!(composer.attached_images.is_empty());

        // Re-add and ensure backspace at element start does not delete the placeholder.
        composer.attach_image(path);
        let placeholder2 = composer.attached_images[0].placeholder.clone();
        // Move cursor to roughly middle of placeholder
        if let Some(start_pos) = composer.textarea.text().find(&placeholder2) {
            let mid_pos = start_pos + (placeholder2.len() / 2);
            composer.textarea.set_cursor(mid_pos);
            composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            assert!(composer.textarea.text().contains(&placeholder2));
            assert_eq!(composer.attached_images.len(), 1);
        } else {
            panic!("Placeholder not found in textarea");
        }
    }

    #[test]
    fn backspace_with_multibyte_text_before_placeholder_does_not_panic() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Insert an image placeholder at the start
        let path = PathBuf::from("/tmp/image_multibyte.png");
        composer.attach_image(path);
        // Add multibyte text after the placeholder
        composer.textarea.insert_str("日本語");

        // Cursor is at end; pressing backspace should delete the last character
        // without panicking and leave the placeholder intact.
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert_eq!(composer.attached_images.len(), 1);
        assert!(composer.textarea.text().starts_with("[Image #1]"));
    }

    #[test]
    fn deleting_one_of_duplicate_image_placeholders_removes_one_entry() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let path1 = PathBuf::from("/tmp/image_dup1.png");
        let path2 = PathBuf::from("/tmp/image_dup2.png");

        composer.attach_image(path1);
        // separate placeholders with a space for clarity
        composer.handle_paste(" ".into());
        composer.attach_image(path2.clone());

        let placeholder1 = composer.attached_images[0].placeholder.clone();
        let placeholder2 = composer.attached_images[1].placeholder.clone();
        let text = composer.textarea.text().to_string();
        let start1 = text.find(&placeholder1).expect("first placeholder present");
        let end1 = start1 + placeholder1.len();
        composer.textarea.set_cursor(end1);

        // Backspace should delete the first placeholder and its mapping.
        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        let new_text = composer.textarea.text().to_string();
        assert_eq!(
            1,
            new_text.matches(&placeholder1).count(),
            "one placeholder remains after deletion"
        );
        assert_eq!(
            0,
            new_text.matches(&placeholder2).count(),
            "second placeholder was relabeled"
        );
        assert_eq!(
            1,
            new_text.matches("[Image #1]").count(),
            "remaining placeholder relabeled to #1"
        );
        assert_eq!(
            vec![AttachedImage {
                path: path2,
                placeholder: "[Image #1]".to_string()
            }],
            composer.attached_images,
            "one image mapping remains"
        );
    }

    #[test]
    fn deleting_reordered_image_one_renumbers_text_in_place() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let path1 = PathBuf::from("/tmp/image_first.png");
        let path2 = PathBuf::from("/tmp/image_second.png");
        let placeholder1 = local_image_label_text(1);
        let placeholder2 = local_image_label_text(2);

        // Placeholders can be reordered in the text buffer; deleting image #1 should renumber
        // image #2 wherever it appears, not just after the cursor.
        let text = format!("Test {placeholder2} test {placeholder1}");
        let start2 = text.find(&placeholder2).expect("placeholder2 present");
        let start1 = text.find(&placeholder1).expect("placeholder1 present");
        let text_elements = vec![
            TextElement::new(
                ByteRange {
                    start: start2,
                    end: start2 + placeholder2.len(),
                },
                Some(placeholder2),
            ),
            TextElement::new(
                ByteRange {
                    start: start1,
                    end: start1 + placeholder1.len(),
                },
                Some(placeholder1.clone()),
            ),
        ];
        composer.set_text_content(text, text_elements, vec![path1, path2.clone()]);

        let end1 = start1 + placeholder1.len();
        composer.textarea.set_cursor(end1);

        composer.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert_eq!(
            composer.textarea.text(),
            format!("Test {placeholder1} test ")
        );
        assert_eq!(
            vec![AttachedImage {
                path: path2,
                placeholder: placeholder1
            }],
            composer.attached_images,
            "attachment renumbered after deletion"
        );
    }

    #[test]
    fn deleting_first_text_element_renumbers_following_text_element() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let path1 = PathBuf::from("/tmp/image_first.png");
        let path2 = PathBuf::from("/tmp/image_second.png");

        // Insert two adjacent atomic elements.
        composer.attach_image(path1);
        composer.attach_image(path2.clone());
        assert_eq!(composer.textarea.text(), "[Image #1][Image #2]");
        assert_eq!(composer.attached_images.len(), 2);

        // Delete the first element using normal textarea editing (forward Delete at cursor start).
        composer.textarea.set_cursor(0);
        composer.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));

        // Remaining image should be renumbered and the textarea element updated.
        assert_eq!(composer.attached_images.len(), 1);
        assert_eq!(composer.attached_images[0].path, path2);
        assert_eq!(composer.attached_images[0].placeholder, "[Image #1]");
        assert_eq!(composer.textarea.text(), "[Image #1]");
    }

    #[test]
    fn pasting_filepath_attaches_image() {
        let tmp = tempdir().expect("create TempDir");
        let tmp_path: PathBuf = tmp.path().join("codex_tui_test_paste_image.png");
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_fn(3, 2, |_x, _y| Rgba([1, 2, 3, 255]));
        img.save(&tmp_path).expect("failed to write temp png");

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let needs_redraw = composer.handle_paste(tmp_path.to_string_lossy().to_string());
        assert!(needs_redraw);
        assert!(composer.textarea.text().starts_with("[Image #1] "));

        let imgs = composer.take_recent_submission_images();
        assert_eq!(imgs, vec![tmp_path]);
    }

    #[test]
    fn selecting_custom_prompt_without_args_submits_content() {
        let prompt_text = "Hello from saved prompt";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        // Inject prompts as if received via event.
        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: prompt_text.to_string(),
            description: None,
            argument_hint: None,
        }]);

        type_chars_humanlike(
            &mut composer,
            &[
                '/', 'p', 'r', 'o', 'm', 'p', 't', 's', ':', 'm', 'y', '-', 'p', 'r', 'o', 'm',
                'p', 't',
            ],
        );

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == prompt_text
        ));
        assert!(composer.textarea.is_empty());
    }

    #[test]
    fn custom_prompt_submission_expands_arguments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes on $BRANCH".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt USER=Alice BRANCH=main");

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. }
                if text == "Review Alice changes on main"
        ));
        assert!(composer.textarea.is_empty());
    }

    #[test]
    fn custom_prompt_submission_accepts_quoted_values() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Pair $USER with $BRANCH".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt USER=\"Alice Smith\" BRANCH=dev-main");

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. }
                if text == "Pair Alice Smith with dev-main"
        ));
        assert!(composer.textarea.is_empty());
    }

    #[test]
    fn custom_prompt_submission_preserves_image_placeholder_unquoted() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $IMG".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt IMG=");
        composer.textarea.set_cursor(composer.textarea.text().len());
        let path = PathBuf::from("/tmp/image_prompt.png");
        composer.attach_image(path);

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                let placeholder = local_image_label_text(1);
                assert_eq!(text, format!("Review {placeholder}"));
                assert_eq!(
                    text_elements,
                    vec![TextElement::new(
                        ByteRange {
                            start: "Review ".len(),
                            end: "Review ".len() + placeholder.len(),
                        },
                        Some(placeholder),
                    )]
                );
            }
            _ => panic!("expected Submitted"),
        }
    }

    #[test]
    fn custom_prompt_submission_preserves_image_placeholder_quoted() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $IMG".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt IMG=\"");
        composer.textarea.set_cursor(composer.textarea.text().len());
        let path = PathBuf::from("/tmp/image_prompt_quoted.png");
        composer.attach_image(path);
        composer.handle_paste("\"".to_string());

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                let placeholder = local_image_label_text(1);
                assert_eq!(text, format!("Review {placeholder}"));
                assert_eq!(
                    text_elements,
                    vec![TextElement::new(
                        ByteRange {
                            start: "Review ".len(),
                            end: "Review ".len() + placeholder.len(),
                        },
                        Some(placeholder),
                    )]
                );
            }
            _ => panic!("expected Submitted"),
        }
    }

    #[test]
    fn custom_prompt_submission_drops_unused_image_arg() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review changes".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt IMG=");
        composer.textarea.set_cursor(composer.textarea.text().len());
        let path = PathBuf::from("/tmp/unused_image.png");
        composer.attach_image(path);

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                assert_eq!(text, "Review changes");
                assert!(text_elements.is_empty());
            }
            _ => panic!("expected Submitted"),
        }
        assert!(composer.take_recent_submission_images().is_empty());
    }

    /// Behavior: selecting a custom prompt that includes a large paste placeholder should expand
    /// to the full pasted content before submission.
    #[test]
    fn custom_prompt_with_large_paste_expands_correctly() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        // Create a custom prompt with positional args (no named args like $USER)
        composer.set_custom_prompts(vec![CustomPrompt {
            name: "code-review".to_string(),
            path: "/tmp/code-review.md".to_string().into(),
            content: "Please review the following code:\n\n$1".to_string(),
            description: None,
            argument_hint: None,
        }]);

        // Type the slash command
        let command_text = "/prompts:code-review ";
        composer.textarea.set_text_clearing_elements(command_text);
        composer.textarea.set_cursor(command_text.len());

        // Paste large content (>3000 chars) to trigger placeholder
        let large_content = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 3000);
        composer.handle_paste(large_content.clone());

        // Verify placeholder was created
        let placeholder = format!("[Pasted Content {} chars]", large_content.chars().count());
        assert_eq!(
            composer.textarea.text(),
            format!("/prompts:code-review {}", placeholder)
        );
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, placeholder);
        assert_eq!(composer.pending_pastes[0].1, large_content);

        // Submit by pressing Enter
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Verify the custom prompt was expanded with the large content as positional arg
        match result {
            InputResult::Submitted { text, .. } => {
                // The prompt should be expanded, with the large content replacing $1
                assert_eq!(
                    text,
                    format!("Please review the following code:\n\n{}", large_content),
                    "Expected prompt expansion with large content as $1"
                );
            }
            _ => panic!("expected Submitted, got: {result:?}"),
        }
        assert!(composer.textarea.is_empty());
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn custom_prompt_with_large_paste_and_image_preserves_elements() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $IMG\n\n$CODE".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt IMG=");
        composer.textarea.set_cursor(composer.textarea.text().len());
        let path = PathBuf::from("/tmp/image_prompt_combo.png");
        composer.attach_image(path);
        composer.handle_paste(" CODE=".to_string());
        let large_content = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5);
        composer.handle_paste(large_content.clone());

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            } => {
                let placeholder = local_image_label_text(1);
                assert_eq!(text, format!("Review {placeholder}\n\n{large_content}"));
                assert_eq!(
                    text_elements,
                    vec![TextElement::new(
                        ByteRange {
                            start: "Review ".len(),
                            end: "Review ".len() + placeholder.len(),
                        },
                        Some(placeholder),
                    )]
                );
            }
            _ => panic!("expected Submitted"),
        }
    }

    #[test]
    fn slash_path_input_submits_without_command_error() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer
            .textarea
            .set_text_clearing_elements("/Users/example/project/src/main.rs");

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        if let InputResult::Submitted { text, .. } = result {
            assert_eq!(text, "/Users/example/project/src/main.rs");
        } else {
            panic!("expected Submitted");
        }
        assert!(composer.textarea.is_empty());
        match rx.try_recv() {
            Ok(event) => panic!("unexpected event: {event:?}"),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(err) => panic!("unexpected channel state: {err:?}"),
        }
    }

    #[test]
    fn slash_with_leading_space_submits_as_text() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer
            .textarea
            .set_text_clearing_elements(" /this-looks-like-a-command");

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        if let InputResult::Submitted { text, .. } = result {
            assert_eq!(text, "/this-looks-like-a-command");
        } else {
            panic!("expected Submitted");
        }
        assert!(composer.textarea.is_empty());
        match rx.try_recv() {
            Ok(event) => panic!("unexpected event: {event:?}"),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(err) => panic!("unexpected channel state: {err:?}"),
        }
    }

    #[test]
    fn custom_prompt_invalid_args_reports_error() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt USER=Alice stray");

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(InputResult::None, result);
        assert_eq!(
            "/prompts:my-prompt USER=Alice stray",
            composer.textarea.text()
        );

        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                let message = cell
                    .display_lines(80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(message.contains("expected key=value"));
                found_error = true;
                break;
            }
        }
        assert!(found_error, "expected error history cell to be sent");
    }

    #[test]
    fn custom_prompt_missing_required_args_reports_error() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes on $BRANCH".to_string(),
            description: None,
            argument_hint: None,
        }]);

        // Provide only one of the required args
        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt USER=Alice");

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(InputResult::None, result);
        assert_eq!("/prompts:my-prompt USER=Alice", composer.textarea.text());

        let mut found_error = false;
        while let Ok(event) = rx.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                let message = cell
                    .display_lines(80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(message.to_lowercase().contains("missing required args"));
                assert!(message.contains("BRANCH"));
                found_error = true;
                break;
            }
        }
        assert!(
            found_error,
            "expected missing args error history cell to be sent"
        );
    }

    #[test]
    fn selecting_custom_prompt_with_args_expands_placeholders() {
        // Support $1..$9 and $ARGUMENTS in prompt content.
        let prompt_text = "Header: $1\nArgs: $ARGUMENTS\nNinth: $9\n";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: prompt_text.to_string(),
            description: None,
            argument_hint: None,
        }]);

        // Type the slash command with two args and hit Enter to submit.
        type_chars_humanlike(
            &mut composer,
            &[
                '/', 'p', 'r', 'o', 'm', 'p', 't', 's', ':', 'm', 'y', '-', 'p', 'r', 'o', 'm',
                'p', 't', ' ', 'f', 'o', 'o', ' ', 'b', 'a', 'r',
            ],
        );
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let expected = "Header: foo\nArgs: foo bar\nNinth: \n".to_string();
        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == expected
        ));
    }

    #[test]
    fn popup_prompt_submission_prunes_unused_image_attachments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Hello".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer.attach_image(PathBuf::from("/tmp/unused.png"));
        composer.textarea.set_cursor(0);
        composer.handle_paste(format!("/{PROMPTS_CMD_PREFIX}:my-prompt "));

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == "Hello"
        ));
        assert!(
            composer
                .take_recent_submission_images_with_placeholders()
                .is_empty()
        );
    }

    #[test]
    fn numeric_prompt_auto_submit_prunes_unused_image_attachments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Hello $1".to_string(),
            description: None,
            argument_hint: None,
        }]);

        type_chars_humanlike(
            &mut composer,
            &[
                '/', 'p', 'r', 'o', 'm', 'p', 't', 's', ':', 'm', 'y', '-', 'p', 'r', 'o', 'm',
                'p', 't', ' ', 'f', 'o', 'o', ' ',
            ],
        );
        composer.attach_image(PathBuf::from("/tmp/unused.png"));

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == "Hello foo"
        ));
        assert!(
            composer
                .take_recent_submission_images_with_placeholders()
                .is_empty()
        );
    }

    #[test]
    fn numeric_prompt_auto_submit_expands_pending_pastes() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Echo: $1".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt ");
        composer.textarea.set_cursor(composer.textarea.text().len());
        let large_content = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 5);
        composer.handle_paste(large_content.clone());

        assert_eq!(composer.pending_pastes.len(), 1);

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let expected = format!("Echo: {large_content}");
        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == expected
        ));
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn queued_prompt_submission_prunes_unused_image_attachments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(false);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Hello $1".to_string(),
            description: None,
            argument_hint: None,
        }]);

        composer
            .textarea
            .set_text_clearing_elements("/prompts:my-prompt foo ");
        composer.textarea.set_cursor(composer.textarea.text().len());
        composer.attach_image(PathBuf::from("/tmp/unused.png"));

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Queued { text, .. } if text == "Hello foo"
        ));
        assert!(
            composer
                .take_recent_submission_images_with_placeholders()
                .is_empty()
        );
    }

    #[test]
    fn selecting_custom_prompt_with_positional_args_submits_numeric_expansion() {
        let prompt_text = "Header: $1\nArgs: $ARGUMENTS\n";

        let prompt = CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: prompt_text.to_string(),
            description: None,
            argument_hint: None,
        };

        let action = prompt_selection_action(
            &prompt,
            "/prompts:my-prompt foo bar",
            PromptSelectionMode::Submit,
            &[],
        );
        match action {
            PromptSelectionAction::Submit {
                text,
                text_elements,
            } => {
                assert_eq!(text, "Header: foo\nArgs: foo bar\n");
                assert!(text_elements.is_empty());
            }
            _ => panic!("expected Submit action"),
        }
    }

    #[test]
    fn numeric_prompt_positional_args_does_not_error() {
        // Ensure that a prompt with only numeric placeholders does not trigger
        // key=value parsing errors when given positional arguments.
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "elegant".to_string(),
            path: "/tmp/elegant.md".to_string().into(),
            content: "Echo: $ARGUMENTS".to_string(),
            description: None,
            argument_hint: None,
        }]);

        // Type positional args; should submit with numeric expansion, no errors.
        composer
            .textarea
            .set_text_clearing_elements("/prompts:elegant hi");
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == "Echo: hi"
        ));
        assert!(composer.textarea.is_empty());
    }

    #[test]
    fn selecting_custom_prompt_with_no_args_inserts_template() {
        let prompt_text = "X:$1 Y:$2 All:[$ARGUMENTS]";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "p".to_string(),
            path: "/tmp/p.md".to_string().into(),
            content: prompt_text.to_string(),
            description: None,
            argument_hint: None,
        }]);

        type_chars_humanlike(
            &mut composer,
            &['/', 'p', 'r', 'o', 'm', 'p', 't', 's', ':', 'p'],
        );
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // With no args typed, selecting the prompt inserts the command template
        // and does not submit immediately.
        assert_eq!(InputResult::None, result);
        assert_eq!("/prompts:p ", composer.textarea.text());
    }

    #[test]
    fn selecting_custom_prompt_preserves_literal_dollar_dollar() {
        // '$$' should remain untouched.
        let prompt_text = "Cost: $$ and first: $1";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "price".to_string(),
            path: "/tmp/price.md".to_string().into(),
            content: prompt_text.to_string(),
            description: None,
            argument_hint: None,
        }]);

        type_chars_humanlike(
            &mut composer,
            &[
                '/', 'p', 'r', 'o', 'm', 'p', 't', 's', ':', 'p', 'r', 'i', 'c', 'e', ' ', 'x',
            ],
        );
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(matches!(
            result,
            InputResult::Submitted { text, .. }
                if text == "Cost: $$ and first: x"
        ));
    }

    #[test]
    fn selecting_custom_prompt_reuses_cached_arguments_join() {
        let prompt_text = "First: $ARGUMENTS\nSecond: $ARGUMENTS";

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );
        composer.set_steer_enabled(true);

        composer.set_custom_prompts(vec![CustomPrompt {
            name: "repeat".to_string(),
            path: "/tmp/repeat.md".to_string().into(),
            content: prompt_text.to_string(),
            description: None,
            argument_hint: None,
        }]);

        type_chars_humanlike(
            &mut composer,
            &[
                '/', 'p', 'r', 'o', 'm', 'p', 't', 's', ':', 'r', 'e', 'p', 'e', 'a', 't', ' ',
                'o', 'n', 'e', ' ', 't', 'w', 'o',
            ],
        );
        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let expected = "First: one two\nSecond: one two".to_string();
        assert!(matches!(
            result,
            InputResult::Submitted { text, .. } if text == expected
        ));
    }

    /// Behavior: the first fast ASCII character is held briefly to avoid flicker; if no burst
    /// follows, it should eventually flush as normal typed input (not as a paste).
    #[test]
    fn pending_first_ascii_char_flushes_as_typed() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(composer.is_in_paste_burst());
        assert!(composer.textarea.text().is_empty());

        std::thread::sleep(ChatComposer::recommended_paste_flush_delay());
        let flushed = composer.flush_paste_burst_if_due();
        assert!(flushed, "expected pending first char to flush");
        assert_eq!(composer.textarea.text(), "h");
        assert!(!composer.is_in_paste_burst());
    }

    /// Behavior: fast "paste-like" ASCII input should buffer and then flush as a single paste. If
    /// the payload is small, it should insert directly (no placeholder).
    #[test]
    fn burst_paste_fast_small_buffers_and_flushes_on_stop() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let count = 32;
        let mut now = Instant::now();
        let step = Duration::from_millis(1);
        for _ in 0..count {
            let _ = composer.handle_input_basic_with_time(
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                now,
            );
            assert!(
                composer.is_in_paste_burst(),
                "expected active paste burst during fast typing"
            );
            assert!(
                composer.textarea.text().is_empty(),
                "text should not appear during burst"
            );
            now += step;
        }

        assert!(
            composer.textarea.text().is_empty(),
            "text should remain empty until flush"
        );
        let flush_time = now + PasteBurst::recommended_active_flush_delay() + step;
        let flushed = composer.handle_paste_burst_flush(flush_time);
        assert!(flushed, "expected buffered text to flush after stop");
        assert_eq!(composer.textarea.text(), "a".repeat(count));
        assert!(
            composer.pending_pastes.is_empty(),
            "no placeholder for small burst"
        );
    }

    /// Behavior: fast "paste-like" ASCII input should buffer and then flush as a single paste. If
    /// the payload is large, it should insert a placeholder and defer the full text until submit.
    #[test]
    fn burst_paste_fast_large_inserts_placeholder_on_flush() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let count = LARGE_PASTE_CHAR_THRESHOLD + 1; // > threshold to trigger placeholder
        let mut now = Instant::now();
        let step = Duration::from_millis(1);
        for _ in 0..count {
            let _ = composer.handle_input_basic_with_time(
                KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
                now,
            );
            now += step;
        }

        // Nothing should appear until we stop and flush
        assert!(composer.textarea.text().is_empty());
        let flush_time = now + PasteBurst::recommended_active_flush_delay() + step;
        let flushed = composer.handle_paste_burst_flush(flush_time);
        assert!(flushed, "expected flush after stopping fast input");

        let expected_placeholder = format!("[Pasted Content {count} chars]");
        assert_eq!(composer.textarea.text(), expected_placeholder);
        assert_eq!(composer.pending_pastes.len(), 1);
        assert_eq!(composer.pending_pastes[0].0, expected_placeholder);
        assert_eq!(composer.pending_pastes[0].1.len(), count);
        assert!(composer.pending_pastes[0].1.chars().all(|c| c == 'x'));
    }

    /// Behavior: human-like typing (with delays between chars) should not be classified as a paste
    /// burst. Characters should appear immediately and should not trigger a paste placeholder.
    #[test]
    fn humanlike_typing_1000_chars_appears_live_no_placeholder() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let count = LARGE_PASTE_CHAR_THRESHOLD; // 1000 in current config
        let chars: Vec<char> = vec!['z'; count];
        type_chars_humanlike(&mut composer, &chars);

        assert_eq!(composer.textarea.text(), "z".repeat(count));
        assert!(composer.pending_pastes.is_empty());
    }

    #[test]
    fn slash_popup_not_activated_for_slash_space_text_history_like_input() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;
        use tokio::sync::mpsc::unbounded_channel;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Simulate history-like content: "/ test"
        composer.set_text_content("/ test".to_string(), Vec::new(), Vec::new());

        // After set_text_content -> sync_popups is called; popup should NOT be Command.
        assert!(
            matches!(composer.active_popup, ActivePopup::None),
            "expected no slash popup for '/ test'"
        );

        // Up should be handled by history navigation path, not slash popup handler.
        let (result, _redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(result, InputResult::None);
    }

    #[test]
    fn slash_popup_activated_for_bare_slash_and_valid_prefixes() {
        // use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        use tokio::sync::mpsc::unbounded_channel;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        // Case 1: bare "/"
        composer.set_text_content("/".to_string(), Vec::new(), Vec::new());
        assert!(
            matches!(composer.active_popup, ActivePopup::Command(_)),
            "bare '/' should activate slash popup"
        );

        // Case 2: valid prefix "/re" (matches /review, /resume, etc.)
        composer.set_text_content("/re".to_string(), Vec::new(), Vec::new());
        assert!(
            matches!(composer.active_popup, ActivePopup::Command(_)),
            "'/re' should activate slash popup via prefix match"
        );

        // Case 3: fuzzy match "/ac" (subsequence of /compact and /feedback)
        composer.set_text_content("/ac".to_string(), Vec::new(), Vec::new());
        assert!(
            matches!(composer.active_popup, ActivePopup::Command(_)),
            "'/ac' should activate slash popup via fuzzy match"
        );

        // Case 4: invalid prefix "/zzz" – still allowed to open popup if it
        // matches no built-in command; our current logic will not open popup.
        // Verify that explicitly.
        composer.set_text_content("/zzz".to_string(), Vec::new(), Vec::new());
        assert!(
            matches!(composer.active_popup, ActivePopup::None),
            "'/zzz' should not activate slash popup because it is not a prefix of any built-in command"
        );
    }

    #[test]
    fn apply_external_edit_rebuilds_text_and_attachments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let placeholder = local_image_label_text(1);
        composer.textarea.insert_element(&placeholder);
        composer.attached_images.push(AttachedImage {
            placeholder: placeholder.clone(),
            path: PathBuf::from("img.png"),
        });
        composer
            .pending_pastes
            .push(("[Pasted]".to_string(), "data".to_string()));

        composer.apply_external_edit(format!("Edited {placeholder} text"));

        assert_eq!(
            composer.current_text(),
            format!("Edited {placeholder} text")
        );
        assert!(composer.pending_pastes.is_empty());
        assert_eq!(composer.attached_images.len(), 1);
        assert_eq!(composer.attached_images[0].placeholder, placeholder);
        assert_eq!(composer.textarea.cursor(), composer.current_text().len());
    }

    #[test]
    fn apply_external_edit_drops_missing_attachments() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let placeholder = local_image_label_text(1);
        composer.textarea.insert_element(&placeholder);
        composer.attached_images.push(AttachedImage {
            placeholder: placeholder.clone(),
            path: PathBuf::from("img.png"),
        });

        composer.apply_external_edit("No images here".to_string());

        assert_eq!(composer.current_text(), "No images here".to_string());
        assert!(composer.attached_images.is_empty());
    }

    #[test]
    fn current_text_with_pending_expands_placeholders() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let placeholder = "[Pasted Content 5 chars]".to_string();
        composer.textarea.insert_element(&placeholder);
        composer
            .pending_pastes
            .push((placeholder.clone(), "hello".to_string()));

        assert_eq!(
            composer.current_text_with_pending(),
            "hello".to_string(),
            "placeholder should expand to actual text"
        );
    }

    #[test]
    fn apply_external_edit_limits_duplicates_to_occurrences() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        let placeholder = local_image_label_text(1);
        composer.textarea.insert_element(&placeholder);
        composer.attached_images.push(AttachedImage {
            placeholder: placeholder.clone(),
            path: PathBuf::from("img.png"),
        });

        composer.apply_external_edit(format!("{placeholder} extra {placeholder}"));

        assert_eq!(
            composer.current_text(),
            format!("{placeholder} extra {placeholder}")
        );
        assert_eq!(composer.attached_images.len(), 1);
    }

    #[test]
    fn input_disabled_ignores_keypresses_and_hides_cursor() {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyEvent;
        use crossterm::event::KeyModifiers;

        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let sender = AppEventSender::new(tx);
        let mut composer = ChatComposer::new(
            true,
            sender,
            false,
            "Ask Codex to do anything".to_string(),
            false,
        );

        composer.set_text_content("hello".to_string(), Vec::new(), Vec::new());
        composer.set_input_enabled(false, Some("Input disabled for test.".to_string()));

        let (result, needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));

        assert_eq!(result, InputResult::None);
        assert!(!needs_redraw);
        assert_eq!(composer.current_text(), "hello");

        let area = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 5,
        };
        assert_eq!(composer.cursor_pos(area), None);
    }
}
