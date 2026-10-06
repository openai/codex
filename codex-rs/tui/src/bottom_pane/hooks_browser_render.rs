//! Responsive hook browsing presentation, including URL-preserving wrapped details.
//! Trust and toggle policies stay in the view; truncation ellipses stay outside links.

use super::*;
use crate::bottom_pane::picker_style;
use crate::bottom_pane::selection_popup_common::RenderedRows;
use crate::render::Insets;
use crate::render::RectExt;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthStr;

impl HooksBrowserView {
    // Keep the existing yellow warning treatment for hooks awaiting review.
    #[allow(clippy::disallowed_methods)]
    fn event_parts(&self, width: u16) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
        let events = self.event_rows();
        let mut header = self.event_page_lines();
        let mut rows = header.split_off(header.len().saturating_sub(events.len()));
        if width < 72 {
            header.pop();
            header.push("Event               Active/Installed".dim().into());
            rows = events
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let name = event_label(row.event_name);
                    let review = if row.needs_review > 0 { " !" } else { "" };
                    let line = Line::from(format!(
                        "{name:<18}  {}/{}{review}",
                        row.active, row.installed
                    ));
                    if self.state.selected_idx == Some(index) {
                        line.style(selection_style())
                    } else if row.needs_review > 0 {
                        line.yellow()
                    } else {
                        line
                    }
                })
                .collect();
        }
        for (index, line) in rows.iter_mut().enumerate() {
            let prefix = if self.state.selected_idx == Some(index) {
                "› "
            } else {
                "  "
            };
            line.spans.insert(/*index*/ 0, prefix.into());
        }
        (header, rows)
    }
}

fn render_line_rows(
    area: Rect,
    buf: &mut Buffer,
    rows: Vec<Line<'static>>,
    mut state: ScrollState,
) {
    let body = area.inset(Insets::vh(u16::from(area.height >= 3), /*h*/ 0));
    let visible = usize::from(body.height);
    state.ensure_visible(rows.len(), visible.max(/*other*/ 1));
    for (offset, line) in rows.iter().skip(state.scroll_top).take(visible).enumerate() {
        let row_area = Rect::new(
            body.x,
            body.y + offset as u16,
            body.width,
            /*height*/ 1,
        );
        Paragraph::new(line.clone())
            .style(line.style)
            .render(row_area, buf);
    }
    picker_style::render_scroll_indicators(
        area,
        buf,
        RenderedRows {
            has_above: state.scroll_top > 0,
            has_below: state.scroll_top + visible < rows.len(),
            ..Default::default()
        },
    );
}

impl Renderable for HooksBrowserView {
    fn desired_height(&self, width: u16) -> u16 {
        let inner_width = width.saturating_sub(/*rhs*/ 4).max(/*other*/ 1);
        let (header, rows, detail) = match self.page {
            HooksBrowserPage::Events => {
                let (header, rows) = self.event_parts(inner_width);
                (header, rows.len(), 0)
            }
            HooksBrowserPage::Handlers(event) => {
                let rows = self.handlers_for_event(event).count();
                let detail = if rows == 0 {
                    0
                } else {
                    self.detail_lines(event, usize::from(inner_width)).len() + 1
                };
                (
                    Self::handler_header_lines(event, self.review_needed_count(event)),
                    rows,
                    detail,
                )
            }
        };
        let header_height = Paragraph::new(header)
            .wrap(Wrap { trim: false })
            .line_count(inner_width);
        u16::try_from(header_height + rows.clamp(/*min*/ 1, MAX_POPUP_ROWS) + detail + 5)
            .unwrap_or(u16::MAX)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let [panel, footer] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let content = render_menu_surface(panel, buf);
        if content.is_empty() {
            self.render_footer(footer, buf);
            return;
        }
        let (header, rows) = match self.page {
            HooksBrowserPage::Events => self.event_parts(content.width),
            HooksBrowserPage::Handlers(event) => (
                Self::handler_header_lines(event, self.review_needed_count(event)),
                self.handler_row_lines(event, usize::from(area.width)),
            ),
        };
        let header = Paragraph::new(header).wrap(Wrap { trim: false });
        let header_height =
            (header.line_count(content.width) as u16).min(content.height.saturating_sub(/*rhs*/ 3));
        let [header_area, body] =
            Layout::vertical([Constraint::Length(header_height), Constraint::Fill(1)])
                .areas(content);
        header.render(header_area, buf);
        let body = Rect::new(area.x, body.y, area.width, body.height);
        match self.page {
            HooksBrowserPage::Events => render_line_rows(body, buf, rows, self.state),
            HooksBrowserPage::Handlers(event) => {
                if rows.is_empty() {
                    Paragraph::new("  No hooks installed for this event.")
                        .dim()
                        .render(body, buf);
                } else {
                    let desired = rows.len().min(MAX_POPUP_ROWS) as u16 + 2;
                    let list_height = desired
                        .min(body.height.saturating_sub(/*rhs*/ 3).max(/*other*/ 3))
                        .min(body.height);
                    let [list, details] =
                        Layout::vertical([Constraint::Length(list_height), Constraint::Fill(1)])
                            .areas(body);
                    render_line_rows(list, buf, rows, self.state);
                    crate::terminal_hyperlinks::HyperlinkParagraph::new(
                        &self.detail_lines(event, usize::from(content.width)),
                        ratatui::style::Style::default(),
                    )
                    .render(details.inset(Insets::vh(/*v*/ 0, /*h*/ 2)), buf);
                }
            }
        }
        self.render_footer(footer, buf);
    }
}

pub(super) fn detail_line(label: &str, value: &str) -> crate::terminal_hyperlinks::HyperlinkLine {
    crate::terminal_hyperlinks::annotate_web_urls_in_line(Line::from(vec![
        format!("{label:<10}").into(),
        value.to_string().dim(),
    ]))
}

pub(super) fn detail_wrapped_lines(
    label: &str,
    value: &str,
    width: usize,
    max_lines: Option<usize>,
) -> Vec<crate::terminal_hyperlinks::HyperlinkLine> {
    let label_width = label.width().saturating_add(1).max(10);
    let available = width.saturating_sub(label_width).max(1);
    let mut first = true;
    let mut lines = value
        .split('\n')
        .flat_map(|text| {
            let source =
                crate::terminal_hyperlinks::annotate_web_urls_in_line(text.to_owned().dim().into());
            let wrapped = crate::wrapping::wrap_ranges_trim(text, available)
                .into_iter()
                .map(|range| {
                    let prefix = if first {
                        first = false;
                        format!("{label:<label_width$}")
                    } else {
                        " ".repeat(label_width)
                    };
                    crate::wrapping::WrappedLine {
                        line: vec![prefix.into(), text[range.clone()].to_owned().dim()].into(),
                        range,
                        prefix_bytes: label_width,
                    }
                })
                .collect();
            crate::terminal_hyperlinks::remap_source_wrapped_line(&source, wrapped)
        })
        .collect::<Vec<_>>();
    let Some(max_lines) = max_lines else {
        return lines;
    };
    if lines.len() <= max_lines {
        return lines;
    }

    lines.truncate(max_lines);
    if let Some(last_line) = lines.last_mut() {
        let prefix_width = last_line.line.spans[..last_line.line.spans.len().saturating_sub(1)]
            .iter()
            .map(ratatui::prelude::Span::width)
            .sum::<usize>();
        let max_width = width.saturating_sub(prefix_width);
        let Some(last_span) = last_line.line.spans.last_mut() else {
            return lines;
        };
        let truncated = truncate_line_with_ellipsis_if_overflow(
            Line::from(format!("{}…", last_span.content)),
            max_width,
        );
        let content = truncated
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<String>();
        let visible_end = prefix_width + crate::width::display_width(content.trim_end_matches('…'));
        last_span.content = content.into();
        last_line.hyperlinks.retain_mut(|link| {
            link.columns.end = link.columns.end.min(visible_end);
            !link.columns.is_empty()
        });
    }
    lines
}

#[cfg(test)]
#[path = "hooks_browser_render_tests.rs"]
mod tests;
