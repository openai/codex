use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::render::renderable::Renderable;

use super::DESIRED_SPACERS_WHEN_NOTES_HIDDEN;
use super::RequestUserInputOverlay;
use super::TIP_SEPARATOR;

impl Renderable for RequestUserInputOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        let outer = Rect::new(0, 0, width, u16::MAX);
        let inner = menu_surface_inset(outer);
        let inner_width = inner.width.max(1);
        let has_options = self.has_options();
        let question_height = self.wrapped_question_lines(inner_width).len();
        let options_height = if has_options {
            self.options_preferred_height(inner_width) as usize
        } else {
            0
        };
        let notes_visible = !has_options || self.notes_ui_visible();
        let notes_height = if notes_visible {
            self.notes_input_height(inner_width) as usize
        } else {
            0
        };
        let spacer_rows = if has_options && !notes_visible {
            DESIRED_SPACERS_WHEN_NOTES_HIDDEN as usize
        } else {
            0
        };
        let footer_height = self.footer_required_height(inner_width) as usize;

        // Tight minimum height: progress + header + question + (optional) titles/options
        // + notes composer + footer + menu padding.
        let mut height = question_height
            .saturating_add(options_height)
            .saturating_add(spacer_rows)
            .saturating_add(notes_height)
            .saturating_add(footer_height)
            .saturating_add(2); // progress + header
        if has_options && notes_visible {
            height = height.saturating_add(1); // notes title
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
        let notes_visible = self.notes_ui_visible();
        let unanswered = self.unanswered_count();

        // Progress header keeps the user oriented across multiple questions.
        let progress_line = if self.question_count() > 0 {
            let idx = self.current_index() + 1;
            let total = self.question_count();
            let base = format!("Question {idx}/{total}");
            if unanswered > 0 {
                Line::from(format!("{base} ({unanswered} unanswered)").dim())
            } else {
                Line::from(base.dim())
            }
        } else {
            Line::from("No questions".dim())
        };
        Paragraph::new(progress_line).render(sections.progress_area, buf);

        // Question title and wrapped prompt text.
        let question_header = self.current_question().map(|q| q.header.clone());
        let answered = self.current_question_answered();
        let header_line = if let Some(header) = question_header {
            if answered {
                Line::from(header.bold())
            } else {
                Line::from(header.cyan().bold())
            }
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

        // Build rows with selection markers for the shared selection renderer.
        let option_rows = self.option_rows();

        if self.has_options() {
            let mut options_ui_state = self
                .current_answer()
                .map(|answer| answer.options_ui_state)
                .unwrap_or_default();
            if sections.options_area.height > 0 {
                // Ensure the selected option is visible in the scroll window.
                options_ui_state
                    .ensure_visible(option_rows.len(), sections.options_area.height as usize);
                render_rows(
                    sections.options_area,
                    buf,
                    &option_rows,
                    &options_ui_state,
                    option_rows.len().max(1),
                    "No options",
                );
            }
        }

        if notes_visible && sections.notes_title_area.height > 0 {
            let notes_label = if self.has_options()
                && self
                    .current_answer()
                    .is_some_and(|answer| answer.committed_option_idx.is_some())
            {
                if let Some(label) = self.current_option_label() {
                    format!("Notes for {label}")
                } else {
                    "Notes".to_string()
                }
            } else {
                "Notes".to_string()
            };
            let notes_active = self.focus_is_notes();
            let notes_title = if notes_active {
                notes_label.as_str().cyan().bold()
            } else {
                notes_label.as_str().dim()
            };
            Paragraph::new(Line::from(notes_title)).render(sections.notes_title_area, buf);
        }

        if notes_visible && sections.notes_area.height > 0 {
            self.render_notes_input(sections.notes_area, buf);
        }

        let footer_y = sections
            .notes_area
            .y
            .saturating_add(sections.notes_area.height);
        let footer_area = Rect {
            x: content_area.x,
            y: footer_y,
            width: content_area.width,
            height: sections.footer_lines,
        };
        if footer_area.height == 0 {
            return;
        }
        let tip_lines = self.footer_tip_lines(footer_area.width);
        for (row_idx, tips) in tip_lines
            .into_iter()
            .take(footer_area.height as usize)
            .enumerate()
        {
            let mut spans = Vec::new();
            for (tip_idx, tip) in tips.into_iter().enumerate() {
                if tip_idx > 0 {
                    spans.push(TIP_SEPARATOR.into());
                }
                if tip.highlight {
                    spans.push(tip.text.cyan().bold().not_dim());
                } else {
                    spans.push(tip.text.into());
                }
            }
            let line = Line::from(spans).dim();
            let line = truncate_line_word_boundary_with_ellipsis(line, footer_area.width as usize);
            let row_area = Rect {
                x: footer_area.x,
                y: footer_area.y.saturating_add(row_idx as u16),
                width: footer_area.width,
                height: 1,
            };
            Paragraph::new(line).render(row_area, buf);
        }
    }

    /// Return the cursor position when editing notes, if visible.
    pub(super) fn cursor_pos_impl(&self, area: Rect) -> Option<(u16, u16)> {
        let has_options = self.has_options();
        let notes_visible = self.notes_ui_visible();

        if !self.focus_is_notes() {
            return None;
        }
        if has_options && !notes_visible {
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
}

fn line_width(line: &Line<'_>) -> usize {
    line.iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum()
}

/// Truncate a styled line to `max_width`, preferring a word boundary, and append an ellipsis.
///
/// This walks spans character-by-character, tracking the last width-safe position and the last
/// whitespace boundary within the available width (excluding the ellipsis width). If the line
/// overflows, it truncates at the last word boundary when possible (falling back to the last
/// fitting character), trims trailing whitespace, then appends an ellipsis styled to match the
/// last visible span (or the line style if nothing was kept).
fn truncate_line_word_boundary_with_ellipsis(
    line: Line<'static>,
    max_width: usize,
) -> Line<'static> {
    if max_width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    if line_width(&line) <= max_width {
        return line;
    }

    let ellipsis = "…";
    let ellipsis_width = UnicodeWidthStr::width(ellipsis);
    if ellipsis_width >= max_width {
        return Line::from(ellipsis);
    }
    let limit = max_width.saturating_sub(ellipsis_width);

    #[derive(Clone, Copy)]
    struct BreakPoint {
        span_idx: usize,
        byte_end: usize,
    }

    // Track display width as we scan, along with the best "cut here" positions.
    let mut used = 0usize;
    let mut last_fit: Option<BreakPoint> = None;
    let mut last_word_break: Option<BreakPoint> = None;
    let mut overflowed = false;

    'outer: for (span_idx, span) in line.spans.iter().enumerate() {
        let text = span.content.as_ref();
        for (byte_idx, ch) in text.char_indices() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used.saturating_add(ch_width) > limit {
                overflowed = true;
                break 'outer;
            }
            used = used.saturating_add(ch_width);
            let bp = BreakPoint {
                span_idx,
                byte_end: byte_idx + ch.len_utf8(),
            };
            last_fit = Some(bp);
            if ch.is_whitespace() {
                last_word_break = Some(bp);
            }
        }
    }

    // If we never overflowed, the original line already fits.
    if !overflowed {
        return line;
    }

    // Prefer breaking on whitespace; otherwise fall back to the last fitting character.
    let chosen_break = last_word_break.or(last_fit);
    let Some(chosen_break) = chosen_break else {
        return Line::from(ellipsis);
    };

    let line_style = line.style;
    let mut spans_out: Vec<Span<'static>> = Vec::new();
    for (idx, span) in line.spans.into_iter().enumerate() {
        if idx < chosen_break.span_idx {
            spans_out.push(span);
            continue;
        }
        if idx == chosen_break.span_idx {
            let text = span.content.into_owned();
            let truncated = text[..chosen_break.byte_end].to_string();
            if !truncated.is_empty() {
                spans_out.push(Span::styled(truncated, span.style));
            }
        }
        break;
    }

    while let Some(last) = spans_out.last_mut() {
        let trimmed = last
            .content
            .trim_end_matches(char::is_whitespace)
            .to_string();
        if trimmed.is_empty() {
            spans_out.pop();
        } else {
            last.content = trimmed.into();
            break;
        }
    }

    let ellipsis_style = spans_out
        .last()
        .map(|span| span.style)
        .unwrap_or(line_style);
    spans_out.push(Span::styled(ellipsis, ellipsis_style));

    Line::from(spans_out).style(line_style)
}
