//! Lay out question text, choices, inline editing, and key hints within the available rows.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::render::renderable::Renderable;

use super::AsyncQuestions;
use super::TIP_SEPARATOR;
use crate::bottom_pane::request_user_input::render::render_rows_bottom_aligned;
use crate::bottom_pane::request_user_input::render::truncate_line_word_boundary_with_ellipsis;
use crate::keymap::KeymapContext;

impl Renderable for AsyncQuestions {
    fn cursor_style(&self, area: Rect) -> crossterm::cursor::SetCursorStyle {
        self.composer.cursor_style(area)
    }
    fn desired_height(&self, width: u16) -> u16 {
        let width = menu_surface_inset(Rect::new(/*x*/ 0, /*y*/ 0, width, u16::MAX)).width;
        let extra_height = self.options_required_height(width)
            + if !self.has_options() {
                self.composer.inline_input_height(width).clamp(1, 8)
            } else {
                0
            }
            + 2
            + u16::from(!self.has_options() && self.unanswered_count() > 1)
            + self.footer_lines(width, /*option_tip*/ None).len() as u16
            + u16::from(self.unanswered_count() > 1)
            + menu_surface_padding_height();
        u16::try_from(self.wrapped_question_lines(width).len() + usize::from(extra_height))
            .unwrap_or(u16::MAX)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.visible_options.set((0, 0));
        ratatui::widgets::Clear.render(area, buf);
        let content_area = render_menu_surface(area, buf);
        if content_area.is_empty() {
            return;
        }
        let sections = self.layout_sections(content_area);
        Paragraph::new(self.progress_prefix_text().dim()).render(sections.progress_area, buf);
        Paragraph::new(sections.question_lines.join("\n"))
            .style(crate::style::accent_style())
            .bold()
            .render(sections.question_area, buf);

        // The shared measurer reserves a scrollbar column; this renderer uses the full width.
        let option_rows = self.option_rows();

        if self.other_selected() {
            self.render_inline_options(sections.options_area, buf);
        } else if self.has_options() && sections.options_area.height > 0 {
            let mut options_state = self.state.pending[self.state.current_idx].options_state;
            let selected = options_state.selected_idx.unwrap_or(0);
            let heights: Vec<_> = option_rows
                .iter()
                .map(|row| {
                    super::measure_rows_height(
                        std::slice::from_ref(row),
                        &super::ScrollState::default(),
                        /*max_results*/ 1,
                        sections.options_area.width.saturating_add(1),
                    )
                })
                .collect();
            let available = sections.options_area.height;
            if heights.iter().sum::<u16>() <= available {
                options_state.scroll_top = 0;
            }
            let mut first = options_state.scroll_top.min(selected);
            while first < selected && heights[first..=selected].iter().sum::<u16>() > available {
                first += 1;
            }
            let mut used = 0;
            let visible = heights[first..]
                .iter()
                .take_while(|&&height| {
                    used += height;
                    used <= available
                })
                .count();
            options_state.scroll_top = first;
            self.visible_options.set((first, visible));
            render_rows_bottom_aligned(
                sections.options_area,
                buf,
                &option_rows,
                &options_state,
                option_rows.len().max(1),
                "No options",
            );
        }

        if !self.has_options() {
            self.composer.render_inline_input(sections.notes_area, buf);
        }

        let footer_area = Rect::new(
            content_area.x,
            sections.notes_area.bottom() + sections.spacer_after_input,
            content_area.width,
            sections.footer_lines,
        );
        let input_bottom = if self.other_selected() {
            sections.options_area.bottom()
        } else {
            sections.notes_area.bottom()
        };
        if self.focus_is_notes()
            && footer_area.y > input_bottom
            && let Some(mode) = self.composer.vim_mode_indicator_span()
        {
            Line::from(mode).right_aligned().render(
                Rect::new(
                    content_area.x,
                    footer_area.y - 1,
                    content_area.width,
                    /*height*/ 1,
                ),
                buf,
            );
        }
        let option_tip = (self.has_options()
            && sections.options_area.height > 0
            && self.options_required_height(content_area.width) > sections.options_area.height)
            .then(|| self.option_tip());
        let lines = self
            .footer_lines(footer_area.width, option_tip)
            .into_iter()
            .map(|line| truncate_line_word_boundary_with_ellipsis(line, footer_area.width as usize))
            .collect::<Vec<_>>();
        Paragraph::new(lines).render(footer_area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if !self.focus_is_notes() {
            return None;
        }
        let content_area = menu_surface_inset(area);
        if content_area.width == 0 || content_area.height == 0 {
            return None;
        }
        let sections = self.layout_sections(content_area);
        let input_area = if self.other_selected() {
            self.other_input_area(sections.options_area)
        } else {
            sections.notes_area
        };
        if input_area.width == 0 || input_area.height == 0 {
            return None;
        }
        self.composer.inline_cursor_pos(input_area)
    }
}

impl AsyncQuestions {
    pub(super) fn footer_lines(
        &self,
        width: u16,
        option_tip: Option<Span<'static>>,
    ) -> Vec<Line<'static>> {
        if !self.focus_is_notes()
            && let Some(flash) = self.composer.inline_flash()
        {
            return vec![flash];
        }
        let mut tips = Vec::new();
        let chat_hint = |action| self.keymap.primary_hint(KeymapContext::Chat, action);
        let (context, action) = if self.focus_is_notes() {
            (KeymapContext::Composer, "submit")
        } else {
            (KeymapContext::List, "accept")
        };
        // Existing cross-context keymaps keep chat priority in questions.
        if let Some(key) = self.keymap.primary_hint(context, action)
            && (self.focus_is_notes()
                || !matches!(key, crate::key_hint::ShortcutHint::Single(binding)
                    if self.keymap.chat.interrupt_turn.contains(&binding)
                        || self.keymap.chat.edit_queued_message.contains(&binding)))
        {
            tips.push(
                Span::styled(
                    format!("{} submit", key.display_label()),
                    crate::style::accent_style(),
                )
                .bold(),
            );
        }
        if let Some(key) = chat_hint("skip_question") {
            tips.push(format!("{} skip", key.display_label()).dim());
        }
        tips.extend(option_tip);
        if let Some(key) = chat_hint("prompt_stack_back") {
            let label = if self.state.current_idx > 0 {
                "prev question"
            } else {
                "main prompt"
            };
            tips.push(format!("{} {label}", key.display_label()).dim());
        }
        let next = if self.state.current_idx + 1 < self.state.pending.len() {
            Some("next question")
        } else if self.has_queued_messages {
            Some("queued messages")
        } else {
            None
        };
        if let Some(label) = next
            && let Some(key) = self.next_hint
        {
            tips.push(format!("{} {label}", key.display_label()).dim());
        }
        let mut lines = Vec::new();
        let mut line = Line::default();
        for tip in tips {
            if !line.spans.is_empty() {
                if line.width() + TIP_SEPARATOR.len() + tip.width() > usize::from(width) {
                    lines.push(std::mem::take(&mut line));
                } else {
                    line.spans.push(TIP_SEPARATOR.into());
                }
            }
            line.spans.push(tip);
        }
        lines.push(line);
        lines
    }

    pub(super) fn option_tip(&self) -> Span<'static> {
        format!(
            "option {}/{}",
            self.selected_option_index().unwrap_or(0) + 1,
            self.options_len()
        )
        .dim()
    }
}
