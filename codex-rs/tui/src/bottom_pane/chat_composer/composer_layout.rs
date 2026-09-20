//! Shared composer geometry keeps rendering, measurement, and the cursor aligned.

use super::*;
use crate::bottom_pane::voice_strip::VoiceStrip;
use crate::bottom_pane::voice_strip::VoiceStripState;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

impl ChatComposer {
    pub(crate) fn set_voice_strip(
        &mut self,
        state: Option<VoiceStripState>,
        frame_requester: FrameRequester,
    ) {
        match (self.voice_strip.as_mut(), state) {
            (Some(strip), Some(state)) => strip.update(state),
            (_, Some(state)) => self.voice_strip = Some(VoiceStrip::new(state, frame_requester)),
            (_, None) => self.voice_strip = None,
        }
    }

    pub(super) fn render_voice_strip(&self, composer_rect: Rect, buf: &mut Buffer) {
        if let Some(strip) = &self.voice_strip
            && composer_rect.height >= 6
        {
            let voice_rect = Rect {
                y: composer_rect.y.saturating_add(/*rhs*/ 1),
                height: 2,
                ..composer_rect
            };
            strip.render(voice_rect, buf);
        }
    }
}

impl ChatComposer {
    pub(super) fn layout_with_options(
        &self,
        area: Rect,
        options: ComposerRenderOptions<'_>,
    ) -> ComposerLayout {
        let footer_hint_height = self.footer_hint_height(area.width, options);
        // Preserve input and hints before allocating the persistent status row.
        let status_height = self.status_surface_height(options).min(
            area.height
                .saturating_sub(footer_hint_height.saturating_add(/*rhs*/ 3)),
        );
        let status = Rect {
            y: area
                .bottom()
                .saturating_sub(footer_hint_height)
                .saturating_sub(status_height)
                .max(area.y),
            height: status_height,
            ..area
        };
        let area = Rect {
            height: area.height - status_height,
            ..area
        };
        let popup_height = if self.popups.active.is_above_composer()
            && options.command_popup_placement != CommandPopupPlacement::AboveComposer
        {
            footer_hint_height
        } else {
            self.popups
                .active
                .required_height(area.width, footer_hint_height)
        };
        let voice_rows = if self.voice_strip.is_some() { 3 } else { 0 };
        let shortcuts_above = self.shortcuts_above_composer(options);
        let (composer_rect, popup_rect, mut footer_rect) = if shortcuts_above {
            let [shortcuts, composer, footer] = Layout::vertical([
                Constraint::Max(footer_height(&self.hint_footer_props(options), area.width)),
                Constraint::Min(3 + voice_rows),
                Constraint::Length(footer_hint_height),
            ])
            .areas(area);
            (composer, shortcuts, footer)
        } else if self.popups.active.is_above_composer() {
            // Preserve the draft first, then its footer, and use remaining space for suggestions.
            let [mut composer, mut popup, footer] = Layout::vertical([
                Constraint::Min(3 + voice_rows),
                Constraint::Max(popup_height.saturating_sub(footer_hint_height)),
                Constraint::Length(footer_hint_height),
            ])
            .areas(area);
            popup.y = area.y;
            composer.y = popup.bottom();
            (composer, popup, footer)
        } else {
            let [composer, popup] = Layout::vertical([
                Constraint::Min(3 + voice_rows),
                Constraint::Max(popup_height),
            ])
            .areas(area);
            (composer, popup, popup)
        };
        footer_rect.y = footer_rect.y.saturating_add(status.height);
        // Keep the draft visible when clipped.
        let voice_rows = voice_rows * u16::from(composer_rect.height >= 6);
        let mut textarea_rect = composer_rect.inset(Insets::tlbr(
            /*top*/ 1 + voice_rows,
            LIVE_PREFIX_COLS,
            /*bottom*/ 1,
            /*right*/ 1u16.saturating_add(options.textarea_right_reserve),
        ));
        let remote_images_height = self
            .attachments
            .remote_image_lines()
            .len()
            .try_into()
            .unwrap_or(u16::MAX)
            .min(textarea_rect.height.saturating_sub(1));
        let remote_images_separator = u16::from(remote_images_height > 0);
        let consumed = remote_images_height.saturating_add(remote_images_separator);
        let remote_images_rect = Rect {
            x: textarea_rect.x,
            y: textarea_rect.y,
            width: textarea_rect.width,
            height: remote_images_height,
        };
        textarea_rect.y = textarea_rect.y.saturating_add(consumed);
        textarea_rect.height = textarea_rect.height.saturating_sub(consumed);
        ComposerLayout {
            status,
            composer: composer_rect,
            remote_images: remote_images_rect,
            textarea: textarea_rect,
            popup: popup_rect,
            footer: footer_rect,
        }
    }

    pub(crate) fn cursor_pos_with_options(
        &self,
        area: Rect,
        options: ComposerRenderOptions<'_>,
    ) -> Option<(u16, u16)> {
        let layout = self.layout_with_options(area, options);
        if let Some(footer) = options.footer.filter(|footer| footer.is_interactive) {
            let area = inset_footer_hint_area(layout.footer);
            return footer
                .cursor_column
                .filter(|column| *column < area.width && area.height > 0)
                .map(|column| (area.x + column, area.y));
        }
        if !self.draft.input_enabled || self.attachments.selected_remote_image_index.is_some() {
            return None;
        }

        if let Some(pos) = self
            .vim_search_cursor_pos(layout.footer)
            .or_else(|| self.history_search_cursor_pos(layout.footer))
        {
            return Some(pos);
        }

        let state = *self.draft.textarea_state.borrow();
        self.draft
            .textarea
            .cursor_pos_with_state(layout.textarea, state)
    }

    pub(crate) fn desired_height_with_options(
        &self,
        width: u16,
        options: ComposerRenderOptions<'_>,
    ) -> u16 {
        let footer_hint_height = self.footer_hint_height(width, options);
        const COLS_WITH_MARGIN: u16 = LIVE_PREFIX_COLS + 1;
        let inner_width =
            width.saturating_sub(COLS_WITH_MARGIN.saturating_add(options.textarea_right_reserve));
        let remote_images_height: u16 = self
            .attachments
            .remote_image_lines()
            .len()
            .try_into()
            .unwrap_or(u16::MAX);
        let remote_images_separator = u16::from(remote_images_height > 0);
        self.draft.textarea.desired_height(inner_width)
            + remote_images_height
            + remote_images_separator
            + self.status_surface_height(options)
            + if self.shortcuts_above_composer(options) {
                footer_height(&self.hint_footer_props(options), width)
            } else {
                0
            }
            + 2
            + if self.voice_strip.is_some() { 3 } else { 0 }
            + if self.popups.active.is_above_composer()
                && options.command_popup_placement != CommandPopupPlacement::AboveComposer
            {
                footer_hint_height
            } else {
                self.popups
                    .active
                    .required_height(width, footer_hint_height)
            }
    }
}
