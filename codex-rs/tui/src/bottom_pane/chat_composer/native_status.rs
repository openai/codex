//! Wrap the built-in status line using its painted grapheme widths and native hyperlink ranges.
//! Measurement and paint share the visible rows at the current width.

use super::ChatComposer;
use super::FooterContent;
use super::composer_layout::MAX_FOOTER_ROWS;
use crate::render::line_utils::line_to_static;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::project_hyperlink_columns;
use crate::width::display_width;
use crate::wrapping::adaptive_wrap_line_to_width;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use std::cell::Ref;
use unicode_segmentation::UnicodeSegmentation;

pub(super) struct WrappedNativeStatus {
    width: u16,
    suffix: Vec<Span<'static>>,
    rows: Vec<HyperlinkLine>,
}

impl WrappedNativeStatus {
    pub(super) fn height(&self) -> u16 {
        self.rows.len().max(1) as u16
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer) {
        let height = usize::from(area.height);
        for (index, line) in self.rows.iter().take(height).enumerate() {
            let row = Rect::new(area.x, area.y + index as u16, area.width, 1);
            if (index + 1 == height && self.rows.len() > height)
                || line.width() > usize::from(area.width)
            {
                line.clone().render_with_ellipsis(row, buf);
            } else {
                line.render_fitted(row, buf);
            }
        }
    }
}

impl ChatComposer {
    pub(super) fn wrapped_native_status(
        &self,
        content: &FooterContent,
        width: u16,
    ) -> Option<Ref<'_, WrappedNativeStatus>> {
        let (line, links) = content.native_parts()?;
        if width == 0 {
            return None;
        }
        // The setter invalidates configured spans. The styled agent suffix can change independently.
        let configured = self
            .footer
            .status_line_value
            .as_ref()
            .and_then(FooterContent::native_parts)
            .map_or(0, |(line, _)| line.spans.len());
        let suffix = &line.spans[configured..];
        let cache = &self.footer.wrapped_native_status;
        if cache
            .borrow()
            .as_ref()
            .is_none_or(|entry| entry.width != width || entry.suffix != suffix)
        {
            if content.width() <= width {
                return None;
            }
            cache.replace(Some(WrappedNativeStatus {
                width,
                suffix: suffix.to_vec(),
                rows: wrap_rows(line, links, width),
            }));
        }
        Ref::filter_map(cache.borrow(), Option::as_ref).ok()
    }
}

fn wrap_rows(line: &Line<'_>, links: &[TerminalHyperlink], width: u16) -> Vec<HyperlinkLine> {
    // One row of lookahead settles the last visible break. Whitespace discarded by
    // wrapping cannot count toward that budget or it could hide a later word.
    let limit = usize::from(MAX_FOOTER_ROWS) + 1;
    let mut remaining = usize::from(width) * limit;
    let mut prefix = Line::default().style(line.style);
    for span in &line.spans {
        let mut end = 0;
        for grapheme in span.content.graphemes(true) {
            end += grapheme.len();
            if !grapheme.chars().all(char::is_whitespace) {
                remaining =
                    remaining.saturating_sub(display_width(grapheme).min(usize::from(width)));
                if remaining == 0 {
                    break;
                }
            }
        }
        prefix.push_span(Span::styled(&span.content[..end], span.style));
        if remaining == 0 {
            break;
        }
    }
    // Native link columns are counted per grapheme within each painted span.
    let mut graphemes = prefix
        .spans
        .iter()
        .flat_map(|span| span.content.graphemes(true));
    let (mut byte, mut column) = (0, 0);
    let mut column_at = |target| {
        while byte < target
            && let Some(grapheme) = graphemes.next()
        {
            byte += grapheme.len();
            column += display_width(grapheme);
        }
        column
    };
    adaptive_wrap_line_to_width(&prefix, usize::from(width).into())
        .into_iter()
        .take(limit)
        .map(|row| {
            let source = column_at(row.range.start)..column_at(row.range.end);
            HyperlinkLine {
                line: line_to_static(&row.line),
                hyperlinks: project_hyperlink_columns(links, source, /*prefix*/ 0),
                source: None,
            }
        })
        .collect()
}
