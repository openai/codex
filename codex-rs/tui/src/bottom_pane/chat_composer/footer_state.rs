//! Footer and status-row presentation state for the chat composer.
//! Owners schedule flash expiry redraws; replacing a draft clears its flash.
//! Borrowed transcript feedback uses one resolved presentation for height, paint, and cursor.

use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::bottom_pane::footer::CollaborationModeIndicator;
use crate::bottom_pane::footer::FooterMode;
use crate::bottom_pane::footer::GoalStatusIndicator;
use crate::bottom_pane::footer::inset_footer_hint_area;
use crate::key_hint::KeyBinding;
use crate::key_hint::ShortcutHint;
use std::time::Duration;

/// Per-frame transcript feedback rendered by the composer footer owner.
pub(crate) struct TranscriptFooter {
    pub(crate) text: ratatui::text::Text<'static>,
    /// Caret in the first footer row, measured from its inset content area.
    pub(crate) cursor_column: Option<u16>,
    pub(crate) is_interactive: bool,
}

/// Borrowed presentation shared by composer measurement, painting, and cursor placement.
#[derive(Clone, Copy, Default)]
pub(crate) struct ComposerRenderOptions<'a> {
    pub(crate) textarea_right_reserve: u16,
    pub(crate) footer: Option<&'a TranscriptFooter>,
}

impl super::ChatComposer {
    pub(crate) fn cursor_pos_with_options(
        &self,
        area: Rect,
        options: ComposerRenderOptions<'_>,
    ) -> Option<(u16, u16)> {
        let options = self.resolve_render_options(options);
        if let Some(footer) = options.footer.filter(|footer| footer.is_interactive) {
            let [_, _, _, popup_rect] = self.layout_areas_with_options(area, options);
            let area = inset_footer_hint_area(self.footer_hint_area(popup_rect, options));
            return footer
                .cursor_column
                .filter(|column| *column < area.width && area.height > 0)
                .map(|column| (area.x + column, area.y));
        }
        if !self.draft.input_enabled || self.attachments.selected_remote_image_index.is_some() {
            return None;
        }

        if let Some(pos) = self
            .vim_search_cursor_pos(area)
            .or_else(|| self.history_search_cursor_pos(area))
        {
            return Some(pos);
        }

        let [_, _, textarea_rect, _] = self.layout_areas_with_options(area, options);
        let state = *self.draft.textarea_state.borrow();
        self.draft
            .textarea
            .cursor_pos_with_state(textarea_rect, state)
    }

    pub(super) fn resolve_render_options<'a>(
        &self,
        mut options: ComposerRenderOptions<'a>,
    ) -> ComposerRenderOptions<'a> {
        options.footer = options.footer.filter(|footer| {
            matches!(self.popups.active, super::ActivePopup::None)
                && self.history_search.is_none()
                && self.draft.textarea.vim_query().is_none()
                && !self.quit_shortcut_hint_visible()
                && (footer.is_interactive
                    || (!self.footer.flash_visible()
                        && self.footer.hint_override.is_none()
                        && matches!(
                            self.footer_mode(),
                            FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
                        )))
        });
        options
    }

    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    pub(crate) fn shortcut_overlay_visible(&self) -> bool {
        self.footer_mode() == FooterMode::ShortcutOverlay
            && matches!(self.popups.active, super::ActivePopup::None)
            && self.custom_footer_height().is_none()
    }

    pub(super) fn footer_hint_height(&self, options: ComposerRenderOptions<'_>) -> u16 {
        options.footer.map_or_else(
            || {
                self.custom_footer_height()
                    .unwrap_or_else(|| super::super::footer::footer_height(&self.footer_props()))
            },
            |footer| footer.text.height().try_into().unwrap_or(u16::MAX),
        )
    }

    pub(super) fn footer_hint_area(
        &self,
        popup_rect: ratatui::layout::Rect,
        options: ComposerRenderOptions<'_>,
    ) -> ratatui::layout::Rect {
        let footer_hint_height = self.footer_hint_height(options);
        let footer_spacing = Self::footer_spacing(footer_hint_height);
        if footer_spacing > 0 && footer_hint_height > 0 {
            let [_, hint_rect] = ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(footer_spacing),
                ratatui::layout::Constraint::Length(footer_hint_height),
            ])
            .areas(popup_rect);
            hint_rect
        } else {
            popup_rect
        }
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
    pub(super) insert_newline_key: Option<ShortcutHint>,
    pub(super) queue_key: Option<ShortcutHint>,
    pub(super) toggle_shortcuts_key: Option<ShortcutHint>,
    pub(super) history_search_key: Option<ShortcutHint>,
    pub(super) reasoning_down_key: Option<ShortcutHint>,
    pub(super) reasoning_up_key: Option<ShortcutHint>,
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
