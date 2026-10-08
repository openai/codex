//! Footer and status-row presentation state for the chat composer.
//! Owners schedule flash expiry redraws; replacing a draft clears its flash.
//! One resolved layout supplies the status, input, popup, and hint rectangles to each consumer.
//! Slash-command and unified mention suggestions preserve passive footer content while keeping input.
//! Interactive transcript footers keep focus over nonempty composer hints. Their owner projects
//! actual pending chord hints; unrelated hints resume when the transcript interaction closes.
//! While the transcript owns input, the composer is dimmed and yields its cursor to the footer.
//! Hidden suggestions retain their query and selection until their input owner returns.
//! A pending quit contributes a derived release hint without changing stored footer state.
//! Warning notices use the passive hint row, including while typing, and yield to input controls.
//! Only the two base composer modes opt into fresh-thread decoration; queries and help do not.
//! Shortcut help occupies the space above the composer and keeps its close hint on the final row.
//! Passive transcript hints retain the shortcuts entry when it fits beside the complete hint.

use std::time::Instant;

use super::super::footer::footer_height;
use super::super::footer::reset_mode_after_activity;
use super::super::footer::shows_passive_footer_line;
use super::ActivePopup;
use crate::bottom_pane::footer::FooterKeyHints;
use crate::bottom_pane::footer::FooterProps;
use crate::key_hint;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::bottom_pane::footer::CollaborationModeIndicator;
use crate::bottom_pane::footer::FooterMode;
use crate::bottom_pane::footer::GoalStatusIndicator;
use crate::key_hint::KeyBinding;
use crate::key_hint::ShortcutHint;
use crate::style::secondary_text_style;
use std::time::Duration;

/// Resolved rectangles shared by painting and cursor placement, without retained layout state.
pub(super) struct ComposerLayout {
    pub(super) status: Rect,
    pub(super) composer: Rect,
    pub(super) remote_images: Rect,
    pub(super) textarea: Rect,
    pub(super) popup: Rect,
    pub(super) footer: Rect,
}

/// Per-frame transcript content; the composer remains the only footer layout owner.
pub(crate) struct TranscriptFooter {
    pub(crate) text: Text<'static>,
    pub(crate) cursor_column: Option<u16>,
    pub(crate) is_interactive: bool,
}

/// Whether composer suggestions reserve space, cover transcript rows, or stay hidden.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CommandPopupPlacement {
    #[default]
    AboveComposer,
    Overlay,
    /// Retain suggestions while another surface owns input above the draft.
    Hidden,
}

/// A borrowed presentation shared by measurement, painting, and cursor placement.
#[derive(Clone, Copy, Default)]
pub(crate) struct ComposerRenderOptions<'a> {
    /// Fullscreen limits the composer independently of activity and modal views.
    pub(crate) max_height: Option<u16>,
    pub(crate) rendered_selection:
        Option<&'a std::cell::RefCell<crate::rendered_selection::RenderedSelection>>,
    /// Reserve a shared hint row independently of whether it currently contains a notice.
    pub(crate) composer_gap: Option<&'a crate::bottom_pane::ComposerGap>,
    pub(crate) working_tip: Option<&'a crate::turn_tip::TurnTip>,
    pub(crate) warning_count: usize,
    pub(crate) textarea_right_reserve: u16,
    /// Keep configured status below the composer while hints occupy the final row.
    pub(crate) separate_status_line: bool,
    pub(crate) command_popup_placement: CommandPopupPlacement,
    pub(crate) footer: Option<&'a TranscriptFooter>,
}

impl super::ChatComposer {
    pub(super) fn footer_props(&self) -> FooterProps {
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
            esc_backtrack_hint: self.footer.esc_backtrack_hint,
            is_task_running: self.is_task_running,
            queue_submissions: self.queue_submissions,
            quit_shortcut_key: self.footer.quit_shortcut_key,
            collaboration_modes_enabled: self.collaboration_modes_enabled,
            is_wsl,
            status_line_value: self.footer.status_line_value.clone(),
            status_line_enabled: self.footer.status_line_enabled,
            key_hints: FooterKeyHints {
                agents: self
                    .agents_navigation_available()
                    .then_some(key_hint::plain(KeyCode::Left).into()),
                toggle_shortcuts: self.footer.toggle_shortcuts_key,
                queue: self.footer.queue_key,
                insert_newline: self.footer.insert_newline_key,
                external_editor: self.footer.external_editor_key,
                edit_previous: Some(key_hint::plain(KeyCode::Esc).into()),
                show_transcript: self.footer.show_transcript_key,
                find_transcript: self.footer.find_transcript_key,
                focus_activity: self.footer.focus_activity_key,
                history_search: self.footer.history_search_key,
                reasoning_down: self.footer.reasoning_down_key,
                reasoning_up: self.footer.reasoning_up_key,
                toggle_voice: self
                    .footer
                    .toggle_voice_key
                    .filter(|_| self.voice_command_enabled && !self.side_conversation_active),
            },
            active_agent_label: self.footer.active_agent_label.clone(),
        }
    }

    /// Resolve the effective footer mode via a small priority waterfall.
    ///
    /// The base mode is derived solely from whether the composer is empty:
    /// `ComposerEmpty` iff empty, otherwise `ComposerHasDraft`. Transient
    /// modes (Esc hint, overlay, quit reminder) can override that base when
    /// their conditions are active.
    pub(super) fn footer_mode(&self) -> FooterMode {
        if self.history_search.is_some() || self.draft.textarea.vim_query().is_some() {
            return FooterMode::HistorySearch;
        }

        let base_mode = if self.is_empty() {
            FooterMode::ComposerEmpty
        } else {
            FooterMode::ComposerHasDraft
        };

        match self.footer.mode {
            FooterMode::HistorySearch => FooterMode::HistorySearch,
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

    /// Override the footer hint items displayed beneath the composer. Passing
    /// `None` restores the default shortcut footer.
    pub(crate) fn set_footer_hint_override(&mut self, items: Option<Vec<(String, String)>>) {
        self.footer.hint_override = items;
    }

    /// Show the transient "press again to quit" hint for `key`.
    ///
    /// The owner (`BottomPane`/`ChatWidget`) is responsible for scheduling a
    /// redraw after [`super::super::QUIT_SHORTCUT_TIMEOUT`] so the hint can disappear
    /// even when the UI is otherwise idle.
    pub fn show_quit_shortcut_hint(&mut self, key: KeyBinding, has_focus: bool) {
        self.footer.quit_shortcut_expires_at = Instant::now()
            .checked_add(super::super::QUIT_SHORTCUT_TIMEOUT)
            .or_else(|| Some(Instant::now()));
        self.footer.quit_shortcut_key = key;
        self.footer.mode = FooterMode::QuitShortcutReminder;
        self.set_has_focus(has_focus);
    }

    /// Clear the "press again to quit" hint immediately.
    ///
    /// Key routing calls this before dispatching ordinary input, so unrelated footer modes
    /// such as shortcut help must remain available to their own toggle handlers.
    pub fn clear_quit_shortcut_hint(&mut self, has_focus: bool) {
        self.footer.quit_shortcut_expires_at = None;
        if self.footer.mode == FooterMode::QuitShortcutReminder {
            self.footer.mode = reset_mode_after_activity(self.footer.mode);
        }
        self.set_has_focus(has_focus);
    }

    /// Whether the quit shortcut hint should currently be shown.
    ///
    /// This is time-based rather than event-based: it may become false without
    /// any additional user input, so the UI schedules a redraw when the hint
    /// expires.
    pub(crate) fn quit_shortcut_hint_visible(&self) -> bool {
        self.footer
            .quit_shortcut_expires_at
            .is_some_and(|expires_at| Instant::now() < expires_at)
    }

    pub(super) fn custom_footer_height(&self) -> Option<u16> {
        if self.draft.textarea.vim_query().is_some() || self.footer.flash_visible() {
            return Some(1);
        }
        self.footer
            .hint_override
            .as_ref()
            .map(|items| if items.is_empty() { 0 } else { 1 })
    }

    pub(crate) fn empty_state_composer(
        &self,
    ) -> Option<crate::empty_state_animation::ComposerState> {
        use crate::empty_state_animation::ComposerState;

        let composer = match self.footer_mode() {
            FooterMode::ComposerEmpty if self.is_in_paste_burst() => Some(ComposerState::Draft),
            FooterMode::ComposerEmpty => Some(ComposerState::Empty),
            FooterMode::ComposerHasDraft => Some(ComposerState::Draft),
            FooterMode::HistorySearch
            | FooterMode::QuitShortcutReminder
            | FooterMode::ShortcutOverlay
            | FooterMode::EscHint => None,
        };
        match &self.popups.active {
            ActivePopup::None => composer,
            ActivePopup::Command(_)
            | ActivePopup::File(_)
            | ActivePopup::Skill(_)
            | ActivePopup::MentionV2(_) => None,
        }
    }

    pub(crate) fn resolve_render_options<'a>(
        &self,
        mut options: ComposerRenderOptions<'a>,
    ) -> ComposerRenderOptions<'a> {
        options.footer = options.footer.filter(|footer| {
            if (self.popups.active.is_above_composer()
                && options.command_popup_placement != CommandPopupPlacement::Hidden
                && footer.is_interactive)
                || self
                    .footer
                    .hint_override
                    .as_ref()
                    .is_some_and(|items| items.is_empty() || !footer.is_interactive)
                || self.history_search.is_some()
                || self.draft.textarea.vim_query().is_some()
                || self.quit_shortcut_hint_visible()
            {
                return false;
            }
            footer.is_interactive || shows_passive_footer_line(&self.footer_props())
        });
        options
    }

    pub(crate) fn shortcut_overlay_visible(&self) -> bool {
        self.footer_mode() == FooterMode::ShortcutOverlay
            && matches!(self.popups.active, ActivePopup::None)
            && self.custom_footer_height().is_none()
    }

    pub(super) fn shortcuts_above_composer(&self, options: ComposerRenderOptions<'_>) -> bool {
        self.shortcut_overlay_visible() && options.footer.is_none()
    }

    pub(super) fn footer_hint_height(&self, width: u16, options: ComposerRenderOptions<'_>) -> u16 {
        if self.show_warning_notice(options) || self.shortcuts_above_composer(options) {
            return 1;
        }
        options
            .footer
            .map_or_else(
                || {
                    self.custom_footer_height()
                        .unwrap_or_else(|| footer_height(&self.hint_footer_props(options), width))
                },
                |footer| footer.text.height().try_into().unwrap_or(u16::MAX),
            )
            .max(u16::from(options.separate_status_line))
    }

    pub(super) fn render_transcript_footer(
        &self,
        mut hint_area: Rect,
        buf: &mut Buffer,
        footer: &TranscriptFooter,
        warning_area: Option<Rect>,
    ) {
        let mut text = footer.text.clone();
        if footer.cursor_column.is_none()
            && self.footer.flash_visible()
            && let Some(flash) = &self.footer.flash
            && let Some(line) = text.lines.last_mut()
        {
            *line = flash.line.clone();
        }
        if let Some(warning_area) = warning_area {
            hint_area.width = warning_area
                .x
                .saturating_sub(hint_area.x)
                .saturating_sub(/*rhs*/ 2);
        }
        if !footer.is_interactive
            && !self.footer.flash_visible()
            && self.footer_mode() == FooterMode::ComposerEmpty
            && !self.is_in_paste_burst()
            && let Some(key) = self.footer.toggle_shortcuts_key
            && let Some(line) = text.lines.last_mut()
        {
            let mut shortcuts = Line::default();
            if line.width() > 0 {
                shortcuts.push_span(" · ".set_style(secondary_text_style()));
            }
            shortcuts.extend(key.spans());
            shortcuts.push_span(" shortcuts".set_style(secondary_text_style()));
            if line.width() + shortcuts.width() <= usize::from(hint_area.width) {
                line.extend(shortcuts.spans);
            }
        }
        Paragraph::new(text).render(hint_area, buf);
    }

    pub(crate) fn footer_flash_delay(&self) -> Option<Duration> {
        self.footer
            .flash
            .as_ref()
            .and_then(|flash| flash.expires_at.checked_duration_since(Instant::now()))
    }

    pub(crate) fn show_footer_flash(&mut self, line: Line<'static>, duration: Duration) {
        self.footer.show_flash(line, duration);
    }
}

pub(super) struct FooterState {
    pub(super) quit_shortcut_expires_at: Option<Instant>,
    pub(super) quit_shortcut_key: KeyBinding,
    pub(super) esc_backtrack_hint: bool,
    pub(super) use_shift_enter_hint: bool,
    pub(super) mode: FooterMode,
    pub(super) hint_override: Option<Vec<(String, String)>>,
    pub(super) flash: Option<FooterFlash>,
    pub(super) context_window_percent: Option<i64>,
    pub(super) context_window_used_tokens: Option<i64>,
    pub(super) context_window_pending: bool,
    pub(super) collaboration_mode_indicator: Option<CollaborationModeIndicator>,
    pub(super) goal_status_indicator: Option<GoalStatusIndicator>,
    pub(super) ide_context_active: bool,
    pub(super) status_line_value: Option<Line<'static>>,
    pub(super) status_line_hyperlink_url: Option<String>,
    pub(super) status_line_enabled: bool,
    pub(super) side_conversation_context_label: Option<String>,
    pub(super) active_agent_label: Option<String>,
    pub(super) external_editor_key: Option<ShortcutHint>,
    pub(super) show_transcript_key: Option<ShortcutHint>,
    pub(super) show_warnings_key: Option<ShortcutHint>,
    pub(super) warning_notice_area: std::cell::Cell<Option<Rect>>,
    pub(super) selection_revision: std::cell::Cell<(usize, bool)>,
    pub(super) find_transcript_key: Option<ShortcutHint>,
    pub(super) focus_activity_key: Option<ShortcutHint>,
    pub(super) insert_newline_key: Option<ShortcutHint>,
    pub(super) queue_key: Option<ShortcutHint>,
    pub(super) toggle_shortcuts_key: Option<ShortcutHint>,
    pub(super) history_search_key: Option<ShortcutHint>,
    pub(super) reasoning_down_key: Option<ShortcutHint>,
    pub(super) reasoning_up_key: Option<ShortcutHint>,
    pub(super) toggle_voice_key: Option<ShortcutHint>,
}

#[derive(Clone, Debug)]
pub(super) struct FooterFlash {
    pub(super) line: Line<'static>,
    pub(super) expires_at: Instant,
}

impl FooterState {
    pub(super) fn flash_visible(&self) -> bool {
        self.flash
            .as_ref()
            .is_some_and(|flash| Instant::now() < flash.expires_at)
    }

    pub(super) fn show_flash(&mut self, line: Line<'static>, duration: Duration) {
        let expires_at = Instant::now()
            .checked_add(duration)
            .unwrap_or_else(Instant::now);
        self.flash = Some(FooterFlash { line, expires_at });
    }

    #[cfg(test)]
    pub(super) fn status_line_text(&self) -> Option<String> {
        self.status_line_value.as_ref().map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
    }
}

#[cfg(test)]
#[path = "footer_state_tests.rs"]
mod tests;
