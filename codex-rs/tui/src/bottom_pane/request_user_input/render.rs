use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::borrow::Cow;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::bottom_pane::selection_popup_common::wrap_styled_line;
use crate::render::renderable::Renderable;

use super::DESIRED_SPACERS_BETWEEN_SECTIONS;
use super::RequestUserInputOverlay;
use super::TIP_SEPARATOR;

const MIN_OVERLAY_HEIGHT: usize = 8;
const PROGRESS_ROW_HEIGHT: usize = 1;
const SPACER_ROWS_WITH_NOTES: usize = 1;
const SPACER_ROWS_NO_OPTIONS: usize = 0;

struct UnansweredConfirmationData {
    title_line: Line<'static>,
    subtitle_line: Line<'static>,
    hint_line: Line<'static>,
    rows: Vec<crate::bottom_pane::selection_popup_common::GenericDisplayRow>,
    state: ScrollState,
}

struct UnansweredConfirmationLayout {
    header_lines: Vec<Line<'static>>,
    hint_lines: Vec<Line<'static>>,
    rows: Vec<crate::bottom_pane::selection_popup_common::GenericDisplayRow>,
    state: ScrollState,
}

fn line_to_owned(line: Line<'_>) -> Line<'static> {
    Line {
        style: line.style,
        alignment: line.alignment,
        spans: line
            .spans
            .into_iter()
            .map(|span| Span {
                style: span.style,
                content: Cow::Owned(span.content.into_owned()),
            })
            .collect(),
    }
}

impl Renderable for RequestUserInputOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        if self.confirm_unanswered_active() {
            return self.unanswered_confirmation_height(width);
        }
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
        // When notes are visible, the composer already separates options from the footer.
        // Without notes, we keep extra spacing so the footer hints don't crowd the options.
        let spacer_rows = if has_options {
            if notes_visible {
                SPACER_ROWS_WITH_NOTES
            } else {
                DESIRED_SPACERS_BETWEEN_SECTIONS as usize
            }
        } else {
            SPACER_ROWS_NO_OPTIONS
        };
        let footer_height = self.footer_required_height(inner_width) as usize;

        // Tight minimum height: progress + question + (optional) titles/options
        // + notes composer + footer + menu padding.
        let mut height = question_height
            .saturating_add(options_height)
            .saturating_add(spacer_rows)
            .saturating_add(notes_height)
            .saturating_add(footer_height)
            .saturating_add(PROGRESS_ROW_HEIGHT); // progress
        height = height.saturating_add(menu_surface_padding_height() as usize);
        height.max(MIN_OVERLAY_HEIGHT) as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ui(area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_impl(area)
    }
}

impl RequestUserInputOverlay {
    fn unanswered_confirmation_data(&self) -> UnansweredConfirmationData {
        let unanswered = self.unanswered_question_count();
        let subtitle = format!(
            "{unanswered} unanswered question{}",
            if unanswered == 1 { "" } else { "s" }
        );
        UnansweredConfirmationData {
            title_line: Line::from(super::UNANSWERED_CONFIRM_TITLE.bold()),
            subtitle_line: Line::from(subtitle.dim()),
            hint_line: standard_popup_hint_line(),
            rows: self.unanswered_confirmation_rows(),
            state: self.confirm_unanswered.unwrap_or_default(),
        }
    }

    fn unanswered_confirmation_layout(&self, width: u16) -> UnansweredConfirmationLayout {
        let data = self.unanswered_confirmation_data();
        let content_width = width.max(1);
        let mut header_lines = wrap_styled_line(&data.title_line, content_width);
        let mut subtitle_lines = wrap_styled_line(&data.subtitle_line, content_width);
        header_lines.append(&mut subtitle_lines);
        let header_lines = header_lines.into_iter().map(line_to_owned).collect();
        let hint_lines = wrap_styled_line(&data.hint_line, content_width)
            .into_iter()
            .map(line_to_owned)
            .collect();
        UnansweredConfirmationLayout {
            header_lines,
            hint_lines,
            rows: data.rows,
            state: data.state,
        }
    }

    fn unanswered_confirmation_height(&self, width: u16) -> u16 {
        let outer = Rect::new(0, 0, width, u16::MAX);
        let inner = menu_surface_inset(outer);
        let inner_width = inner.width.max(1);
        let layout = self.unanswered_confirmation_layout(inner_width);
        let rows_height = measure_rows_height(
            &layout.rows,
            &layout.state,
            layout.rows.len().max(1),
            inner_width.max(1),
        );
        let height = layout.header_lines.len() as u16
            + 1
            + rows_height
            + 1
            + layout.hint_lines.len() as u16
            + menu_surface_padding_height();
        height.max(MIN_OVERLAY_HEIGHT as u16)
    }

    fn render_unanswered_confirmation(&self, area: Rect, buf: &mut Buffer) {
        let content_area = render_menu_surface(area, buf);
        if content_area.width == 0 || content_area.height == 0 {
            return;
        }
        let width = content_area.width.max(1);
        let layout = self.unanswered_confirmation_layout(width);

        let mut cursor_y = content_area.y;
        for line in layout.header_lines {
            if cursor_y >= content_area.y + content_area.height {
                return;
            }
            Paragraph::new(line).render(
                Rect {
                    x: content_area.x,
                    y: cursor_y,
                    width: content_area.width,
                    height: 1,
                },
                buf,
            );
            cursor_y = cursor_y.saturating_add(1);
        }

        if cursor_y < content_area.y + content_area.height {
            cursor_y = cursor_y.saturating_add(1);
        }

        let remaining = content_area
            .height
            .saturating_sub(cursor_y.saturating_sub(content_area.y));
        if remaining == 0 {
            return;
        }

        let hint_height = layout.hint_lines.len() as u16;
        let spacer_before_hint = u16::from(remaining > hint_height);
        let rows_height = remaining.saturating_sub(hint_height + spacer_before_hint);

        let rows_area = Rect {
            x: content_area.x,
            y: cursor_y,
            width: content_area.width,
            height: rows_height,
        };
        render_rows(
            rows_area,
            buf,
            &layout.rows,
            &layout.state,
            layout.rows.len().max(1),
            "No choices",
        );

        cursor_y = cursor_y.saturating_add(rows_height);
        if spacer_before_hint > 0 {
            cursor_y = cursor_y.saturating_add(1);
        }
        for (offset, line) in layout.hint_lines.into_iter().enumerate() {
            let y = cursor_y.saturating_add(offset as u16);
            if y >= content_area.y + content_area.height {
                break;
            }
            Paragraph::new(line).render(
                Rect {
                    x: content_area.x,
                    y,
                    width: content_area.width,
                    height: 1,
                },
                buf,
            );
        }
    }

    /// Render the full request-user-input overlay.
    pub(super) fn render_ui(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.confirm_unanswered_active() {
            self.render_unanswered_confirmation(area, buf);
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

        // Question prompt text.
        let question_y = sections.question_area.y;
        let answered =
            self.is_question_answered(self.current_index(), &self.composer.current_text());
        for (offset, line) in sections.question_lines.iter().enumerate() {
            if question_y.saturating_add(offset as u16)
                >= sections.question_area.y + sections.question_area.height
            {
                break;
            }
            let question_line = if answered {
                Line::from(line.clone())
            } else {
                Line::from(line.clone()).cyan()
            };
            Paragraph::new(question_line).render(
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
            let mut options_state = self
                .current_answer()
                .map(|answer| answer.options_state)
                .unwrap_or_default();
            if sections.options_area.height > 0 {
                // Ensure the selected option is visible in the scroll window.
                options_state
                    .ensure_visible(option_rows.len(), sections.options_area.height as usize);
                render_rows(
                    sections.options_area,
                    buf,
                    &option_rows,
                    &options_state,
                    option_rows.len().max(1),
                    "No options",
                );
            }
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
        let options_hidden = self.has_options()
            && sections.options_area.height > 0
            && self.options_required_height(content_area.width) > sections.options_area.height;
        let option_tip = if options_hidden {
            let selected = self.selected_option_index().unwrap_or(0).saturating_add(1);
            let total = self.options_len();
            Some(super::FooterTip::new(format!("option {selected}/{total}")))
        } else {
            None
        };
        let tip_lines = self.footer_tip_lines_with_prefix(footer_area.width, option_tip);
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
        if self.confirm_unanswered_active() {
            return None;
        }
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
        let is_secret = self
            .current_question()
            .is_some_and(|question| question.is_secret);
        if is_secret {
            self.composer.render_with_mask(area, buf, Some('*'));
        } else {
            self.composer.render(area, buf);
        }
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
