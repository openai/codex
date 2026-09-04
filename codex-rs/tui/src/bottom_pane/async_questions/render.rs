//! Render freeform questions inline using the existing modal editing engine.
use super::AsyncQuestions;
use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

impl AsyncQuestions {
    fn question_lines(&self, width: u16) -> Vec<Line<'_>> {
        self.current_question()
            .map(|q| {
                textwrap::wrap(&q.title, usize::from(width.max(1)))
                    .into_iter()
                    .map(Line::from)
                    .collect()
            })
            .unwrap_or_default()
    }
    fn input_area(&self, area: Rect) -> Rect {
        let area = menu_surface_inset(area);
        let input_height = self
            .composer
            .inline_input_height(area.width)
            .min(area.height);
        Rect::new(
            area.x,
            area.bottom().saturating_sub(input_height),
            area.width,
            input_height,
        )
    }
}
impl Renderable for AsyncQuestions {
    fn cursor_style(&self, area: Rect) -> crossterm::cursor::SetCursorStyle {
        self.composer.cursor_style(area)
    }
    fn desired_height(&self, width: u16) -> u16 {
        let width = menu_surface_inset(Rect::new(/*x*/ 0, /*y*/ 0, width, u16::MAX)).width;
        self.question_lines(width).len() as u16
            + self.composer.inline_input_height(width)
            + 1
            + u16::from(self.unanswered_count() > 1)
            + u16::from(self.unanswered_count() > 1)
            + menu_surface_padding_height()
    }
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let input = self.input_area(area);
        let mut prompt = render_menu_surface(area, buf);
        prompt.height = input.y.saturating_sub(prompt.y);
        if self.unanswered_count() > 1 && prompt.height > 0 {
            Paragraph::new(self.progress_prefix_text().dim()).render(
                Rect::new(prompt.x, prompt.y, prompt.width, /*height*/ 1),
                buf,
            );
            prompt.y += 1;
            prompt.height -= 1;
        }
        if self.unanswered_count() > 1
            && prompt.height > self.question_lines(prompt.width).len() as u16
        {
            prompt.y += 1;
            prompt.height -= 1;
        }
        Paragraph::new(self.question_lines(prompt.width))
            .style(crate::style::accent_style())
            .bold()
            .render(prompt, buf);
        self.composer.render_inline_input(input, buf);
    }
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.composer.inline_cursor_pos(self.input_area(area))
    }
}
