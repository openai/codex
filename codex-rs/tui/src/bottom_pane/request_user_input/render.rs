use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::key_hint;
use crate::render::renderable::Renderable;

use super::RequestUserInputOverlay;

impl Renderable for RequestUserInputOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        let outer = Rect::new(0, 0, width, u16::MAX);
        let inner = menu_surface_inset(outer);
        let inner_width = inner.width.max(1);
        let question_height = self.wrapped_question_lines(inner_width).len();
        let options_height = self.options_required_height(inner_width) as usize;
        let notes_height = self.notes_input_height(inner_width) as usize;
        let footer_height = if self.unanswered_count() > 0 { 2 } else { 1 };

        // Tight minimum height: progress + header + question + (optional) titles/options
        // + notes composer + footer + menu padding.
        let mut height = question_height
            .saturating_add(options_height)
            .saturating_add(notes_height)
            .saturating_add(footer_height)
            .saturating_add(2); // progress + header
        if self.has_options() {
            height = height
                .saturating_add(1) // answer title
                .saturating_add(1); // notes title
        }
        height = height.saturating_add(menu_surface_padding_height() as usize);
        height.max(8) as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ui(area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_impl(area)
    }
}

impl RequestUserInputOverlay {
    /// Render the full request-user-input overlay.
    pub(super) fn render_ui(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        // Paint the same menu surface used by other bottom-pane overlays and
        // then render the overlay content inside its inset area.
        let content_area = render_menu_surface(area, buf);
        if content_area.width == 0 || content_area.height == 0 {
            return;
        }
        let sections = self.layout_sections(content_area);

        // Progress header keeps the user oriented across multiple questions.
        let progress_line = if self.question_count() > 0 {
            let idx = self.current_index() + 1;
            let total = self.question_count();
            Line::from(format!("Question {idx}/{total}").dim())
        } else {
            Line::from("No questions".dim())
        };
        Paragraph::new(progress_line).render(sections.progress_area, buf);

        // Question title and wrapped prompt text.
        let question_header = self.current_question().map(|q| q.header.clone());
        let header_line = if let Some(header) = question_header {
            Line::from(header.bold())
        } else {
            Line::from("No questions".dim())
        };
        Paragraph::new(header_line).render(sections.header_area, buf);

        let question_y = sections.question_area.y;
        for (offset, line) in sections.question_lines.iter().enumerate() {
            if question_y.saturating_add(offset as u16)
                >= sections.question_area.y + sections.question_area.height
            {
                break;
            }
            Paragraph::new(Line::from(line.clone())).render(
                Rect {
                    x: sections.question_area.x,
                    y: question_y.saturating_add(offset as u16),
                    width: sections.question_area.width,
                    height: 1,
                },
                buf,
            );
        }

        if sections.answer_title_area.height > 0 {
            let answer_label = "Answer";
            let answer_title = if self.focus_is_options() || self.focus_is_notes_without_options() {
                answer_label.cyan().bold()
            } else {
                answer_label.dim()
            };
            Paragraph::new(Line::from(answer_title)).render(sections.answer_title_area, buf);
        }

        // Build rows with selection markers for the shared selection renderer.
        let option_rows = self.option_rows();

        if self.has_options() {
            let mut option_state = self
                .current_answer()
                .map(|answer| answer.option_state)
                .unwrap_or_default();
            if sections.options_area.height > 0 {
                // Ensure the selected option is visible in the scroll window.
                option_state
                    .ensure_visible(option_rows.len(), sections.options_area.height as usize);
                render_rows(
                    sections.options_area,
                    buf,
                    &option_rows,
                    &option_state,
                    option_rows.len().max(1),
                    "No options",
                );
            }
        }

        if sections.notes_title_area.height > 0 {
            let notes_label = if self.has_options()
                && self
                    .current_answer()
                    .is_some_and(|answer| answer.selected.is_some())
            {
                if let Some(label) = self.current_option_label() {
                    format!("Notes for {label} (optional)")
                } else {
                    "Notes (optional)".to_string()
                }
            } else {
                "Notes (optional)".to_string()
            };
            let notes_title = if self.focus_is_notes() {
                notes_label.as_str().cyan().bold()
            } else {
                notes_label.as_str().dim()
            };
            Paragraph::new(Line::from(notes_title)).render(sections.notes_title_area, buf);
        }

        if sections.notes_area.height > 0 {
            self.render_notes_input(sections.notes_area, buf);
        }

        let footer_y = sections
            .notes_area
            .y
            .saturating_add(sections.notes_area.height);
        if sections.footer_lines == 2 {
            // Status line for unanswered count when any question is empty.
            let warning = format!(
                "Unanswered: {} | Will submit as skipped",
                self.unanswered_count()
            );
            Paragraph::new(Line::from(warning.dim())).render(
                Rect {
                    x: content_area.x,
                    y: footer_y,
                    width: content_area.width,
                    height: 1,
                },
                buf,
            );
        }
        let hint_y = footer_y.saturating_add(sections.footer_lines.saturating_sub(1));
        // Footer hints (selection index + navigation keys).
        let mut hint_spans = Vec::new();
        if self.has_options() {
            let options_len = self.options_len();
            let option_index = self.selected_option_index().map_or(0, |idx| idx + 1);
            hint_spans.extend(vec![
                format!("Option {option_index} of {options_len}").into(),
                " | ".into(),
            ]);
        }
        hint_spans.extend(vec![
            key_hint::plain(KeyCode::Up).into(),
            "/".into(),
            key_hint::plain(KeyCode::Down).into(),
            " scroll | ".into(),
            key_hint::plain(KeyCode::Enter).into(),
            " next question | ".into(),
        ]);
        if self.question_count() > 1 {
            hint_spans.extend(vec![
                key_hint::plain(KeyCode::PageUp).into(),
                " prev | ".into(),
                key_hint::plain(KeyCode::PageDown).into(),
                " next | ".into(),
            ]);
        }
        hint_spans.extend(vec![
            key_hint::plain(KeyCode::Esc).into(),
            " interrupt".into(),
        ]);
        Paragraph::new(Line::from(hint_spans).dim()).render(
            Rect {
                x: content_area.x,
                y: hint_y,
                width: content_area.width,
                height: 1,
            },
            buf,
        );
    }

    /// Return the cursor position when editing notes, if visible.
    pub(super) fn cursor_pos_impl(&self, area: Rect) -> Option<(u16, u16)> {
        if !self.focus_is_notes() {
            return None;
        }
        let content_area = menu_surface_inset(area);
        if content_area.width == 0 || content_area.height == 0 {
            return None;
        }
        let sections = self.layout_sections(content_area);
        let input_area = sections.notes_area;
        if input_area.width == 0 || input_area.height == 0 {
            return None;
        }
        self.composer.cursor_pos(input_area)
    }

    /// Render the notes composer.
    fn render_notes_input(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.composer.render(area, buf);
    }

    fn focus_is_options(&self) -> bool {
        matches!(self.focus, super::Focus::Options)
    }

    fn focus_is_notes(&self) -> bool {
        matches!(self.focus, super::Focus::Notes)
    }

    fn focus_is_notes_without_options(&self) -> bool {
        !self.has_options() && self.focus_is_notes()
    }
}
