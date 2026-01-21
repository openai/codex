use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;

use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::key_hint;
use crate::render::renderable::Renderable;

use super::RequestUserInputOverlay;

impl Renderable for RequestUserInputOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        let sections = self.layout_sections(Rect::new(0, 0, width, u16::MAX));
        let mut height = sections
            .question_lines
            .len()
            .saturating_add(5)
            .saturating_add(self.notes_input_height(width) as usize)
            .saturating_add(sections.footer_lines as usize);
        if self.has_options() {
            height = height.saturating_add(2);
        }
        height = height.max(8);
        height as u16
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
        let sections = self.layout_sections(area);

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
        let option_rows = self
            .current_question()
            .and_then(|question| question.options.as_ref())
            .map(|options| {
                options
                    .iter()
                    .enumerate()
                    .map(|(idx, opt)| {
                        let selected = self
                            .current_answer()
                            .and_then(|answer| answer.selected)
                            .is_some_and(|sel| sel == idx);
                        let prefix = if selected { "(x)" } else { "( )" };
                        GenericDisplayRow {
                            name: format!("{prefix} {}", opt.label),
                            description: Some(opt.description.clone()),
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

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
                    x: area.x,
                    y: footer_y,
                    width: area.width,
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
                x: area.x,
                y: hint_y,
                width: area.width,
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
        let sections = self.layout_sections(area);
        let entry = self.current_notes_entry()?;
        let input_area = sections.notes_area;
        if input_area.width <= 2 || input_area.height == 0 {
            return None;
        }
        if input_area.height < 3 {
            // Inline notes layout uses a prefix and a single-line text area.
            let prefix = notes_prefix();
            let prefix_width = prefix.len() as u16;
            if input_area.width <= prefix_width {
                return None;
            }
            let textarea_rect = Rect {
                x: input_area.x.saturating_add(prefix_width),
                y: input_area.y,
                width: input_area.width.saturating_sub(prefix_width),
                height: 1,
            };
            let state = *entry.state.borrow();
            return entry.text.cursor_pos_with_state(textarea_rect, state);
        }
        let text_area_height = input_area.height.saturating_sub(2);
        let textarea_rect = Rect {
            x: input_area.x.saturating_add(1),
            y: input_area.y.saturating_add(1),
            width: input_area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *entry.state.borrow();
        entry.text.cursor_pos_with_state(textarea_rect, state)
    }

    /// Render the notes input box or inline notes field.
    fn render_notes_input(&self, area: Rect, buf: &mut Buffer) {
        let Some(entry) = self.current_notes_entry() else {
            return;
        };
        if area.width < 2 || area.height == 0 {
            return;
        }
        if area.height < 3 {
            // Inline notes field for tight layouts.
            let prefix = notes_prefix();
            let prefix_width = prefix.len() as u16;
            if area.width <= prefix_width {
                Paragraph::new(Line::from(prefix.dim())).render(area, buf);
                return;
            }
            Paragraph::new(Line::from(prefix.dim())).render(
                Rect {
                    x: area.x,
                    y: area.y,
                    width: prefix_width,
                    height: 1,
                },
                buf,
            );
            let textarea_rect = Rect {
                x: area.x.saturating_add(prefix_width),
                y: area.y,
                width: area.width.saturating_sub(prefix_width),
                height: 1,
            };
            let mut state = entry.state.borrow_mut();
            Clear.render(textarea_rect, buf);
            StatefulWidgetRef::render_ref(&(&entry.text), textarea_rect, buf, &mut state);
            if entry.text.text().is_empty() {
                Paragraph::new(Line::from(self.notes_placeholder().dim()))
                    .render(textarea_rect, buf);
            }
            return;
        }
        // Draw a light ASCII frame around the notes area.
        let top_border = format!("+{}+", "-".repeat(area.width.saturating_sub(2) as usize));
        let bottom_border = top_border.clone();
        Paragraph::new(Line::from(top_border)).render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );
        Paragraph::new(Line::from(bottom_border)).render(
            Rect {
                x: area.x,
                y: area.y.saturating_add(area.height.saturating_sub(1)),
                width: area.width,
                height: 1,
            },
            buf,
        );
        for row in 1..area.height.saturating_sub(1) {
            Line::from(vec![
                "|".into(),
                " ".repeat(area.width.saturating_sub(2) as usize).into(),
                "|".into(),
            ])
            .render(
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(row),
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
        let text_area_height = area.height.saturating_sub(2);
        let textarea_rect = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let mut state = entry.state.borrow_mut();
        Clear.render(textarea_rect, buf);
        StatefulWidgetRef::render_ref(&(&entry.text), textarea_rect, buf, &mut state);
        if entry.text.text().is_empty() {
            Paragraph::new(Line::from(self.notes_placeholder().dim())).render(textarea_rect, buf);
        }
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

fn notes_prefix() -> &'static str {
    "Notes: "
}
