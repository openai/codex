//! Shared-picker prompt layout; rendering and cursor positioning use the same input rectangle.

use super::*;
use crate::style::accent_color;
use crate::style::user_message_style;
use ratatui::widgets::Block;
use ratatui::widgets::Wrap;

struct PromptAreas {
    panel: Rect,
    header: Rect,
    input: Rect,
    footer: Rect,
}

impl CustomPromptView {
    fn picker_header(&self) -> Paragraph<'_> {
        let mut lines = vec![Line::from(self.title.as_str().bold())];
        if let Some(context) = &self.context_label {
            lines.push(Line::from(context.as_str().fg(accent_color())));
        }
        Paragraph::new(lines).wrap(Wrap { trim: false })
    }

    pub(super) fn picker_desired_height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(/*rhs*/ 4);
        self.picker_header()
            .desired_height(content_width)
            .saturating_add(
                self.textarea
                    .desired_height(content_width)
                    .clamp(/*min*/ 1, /*max*/ 8),
            )
            .saturating_add(/*rhs*/ 4)
    }

    fn picker_areas(&self, area: Rect) -> PromptAreas {
        let panel = Rect {
            height: area.height.saturating_sub(/*rhs*/ 1),
            ..area
        };
        let width = area.width.saturating_sub(/*rhs*/ 4);
        let top_gap = u16::from(panel.height >= 4);
        let header_height = self
            .picker_header()
            .desired_height(width)
            .min(panel.height.saturating_sub(top_gap + 1));
        let remaining = panel.height.saturating_sub(top_gap + header_height);
        let gap = u16::from(remaining >= 3);
        let header = Rect {
            x: area.x.saturating_add(/*rhs*/ 2),
            y: area.y.saturating_add(top_gap),
            width,
            height: header_height,
        };
        PromptAreas {
            panel,
            header,
            input: Rect {
                y: header.bottom().saturating_add(gap),
                height: remaining.saturating_sub(gap * 2),
                ..header
            },
            footer: Rect {
                y: panel.bottom(),
                height: u16::from(area.height > 0),
                ..header
            },
        }
    }

    pub(super) fn render_picker(&self, area: Rect, buf: &mut Buffer) {
        let areas = self.picker_areas(area);
        Clear.render(area, buf);
        Block::default()
            .style(user_message_style())
            .render(areas.panel, buf);
        self.picker_header().render(areas.header, buf);
        if !areas.input.is_empty() {
            let mut state = self.textarea_state.borrow_mut();
            StatefulWidgetRef::render_ref(&(&self.textarea), areas.input, buf, &mut state);
            if self.textarea.text().is_empty() {
                Paragraph::new(self.placeholder.as_str().dim()).render(areas.input, buf);
            }
            Line::from("›".fg(accent_color())).render(
                Rect {
                    x: area.x,
                    width: 1,
                    height: 1,
                    ..areas.input
                },
                buf,
            );
        }

        let cancel_label = if self.prefer_esc_to_handle_key_event() {
            " normal mode"
        } else {
            " back"
        };
        let hint = Line::from(vec![
            key_hint::plain(KeyCode::Enter).into(),
            " submit · ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            cancel_label.dim(),
        ]);
        let vim_mode = self.textarea.vim_mode_indicator_span().map(Line::from);
        Paragraph::new(hint.clone()).render(areas.footer, buf);
        if let Some(mode) = vim_mode
            && max_left_width_for_right(areas.footer, mode.width() as u16)
                .is_some_and(|width| usize::from(width) >= hint.width())
        {
            render_context_right(areas.footer, buf, &mode);
        }
    }

    pub(super) fn picker_cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let input = self.picker_areas(area).input;
        if input.is_empty() {
            return None;
        }
        self.textarea
            .cursor_pos_with_state(input, *self.textarea_state.borrow())
    }
}
