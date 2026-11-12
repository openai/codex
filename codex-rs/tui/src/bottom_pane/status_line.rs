//! Measure and paint status lines for the main conversation and side chats.

use super::footer::inset_footer_hint_area;
use super::footer_content::FooterContent;
use super::footer_content::StatusLineFocus;
use super::footer_content::render_status_row;
use crate::render::line_utils::line_to_static;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::project_hyperlink_columns;
use crate::width::display_width;
use crate::wrapping::adaptive_wrap_line_to_width;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use ratatui::text::Span;
use std::cell::Cell;
use std::cell::Ref;
use std::cell::RefCell;
use unicode_segmentation::UnicodeSegmentation;

/// The owning composer resets this when its configured status changes.
#[derive(Default)]
pub(crate) struct StatusLine {
    measured_extension: Cell<Option<(Size, u16)>>,
    wrapped_native: RefCell<Option<WrappedNativeStatus>>,
}

struct WrappedNativeStatus {
    bounds: Size,
    content: HyperlinkLine,
    rows: Vec<HyperlinkLine>,
}

impl StatusLine {
    pub(crate) fn height(
        &self,
        content: Option<&FooterContent>,
        width: u16,
        available: u16,
    ) -> u16 {
        let bounds = inset_footer_hint_area(Rect::new(0, 0, width, available)).as_size();
        if bounds.width == 0 || bounds.height == 0 {
            return 0;
        }
        match content {
            Some(FooterContent::Extension { content }) => {
                if let Some((previous, height)) = self.measured_extension.get()
                    && previous == bounds
                {
                    return height;
                }
                let height = content.measure(bounds).height;
                self.measured_extension.set(Some((bounds, height)));
                height
            }
            Some(native) => self.wrapped_rows(native, bounds).map_or_else(
                || u16::from(native.width() > 0),
                |rows| rows.len().max(1).min(usize::from(bounds.height)) as u16,
            ),
            None => 0,
        }
    }

    pub(crate) fn render(
        &self,
        content: Option<&FooterContent>,
        area: Rect,
        buf: &mut Buffer,
        focus: StatusLineFocus,
    ) {
        let area = inset_footer_hint_area(area);
        let Some(content) = content.filter(|_| !area.is_empty()) else {
            return;
        };
        if let Some(rows) = self.wrapped_rows(content, area.as_size()) {
            let height = usize::from(area.height);
            for (index, line) in rows.iter().take(height).enumerate() {
                let row = Rect::new(area.x, area.y + index as u16, area.width, 1);
                render_status_row(
                    line,
                    row,
                    buf,
                    focus,
                    index + 1 == height && rows.len() > height,
                );
            }
        } else {
            content.render_with_focus(area, buf, focus);
        }
    }

    fn wrapped_rows(
        &self,
        content: &FooterContent,
        bounds: Size,
    ) -> Option<Ref<'_, [HyperlinkLine]>> {
        let FooterContent::HyperlinkLine(line) = content else {
            return None;
        };
        if bounds.height <= 1 {
            return None;
        }
        // Reuse the same wrapped prefix across small height changes.
        let bounds = Size {
            height: bounds.height.max(4),
            ..bounds
        };
        let line = line.as_ref();
        let cache = &self.wrapped_native;
        if cache
            .borrow()
            .as_ref()
            .is_none_or(|entry| entry.bounds != bounds || &entry.content != line)
        {
            if line.width() <= usize::from(bounds.width) {
                return None;
            }
            cache.replace(Some(WrappedNativeStatus {
                bounds,
                content: line.clone(),
                rows: wrap_rows(line, bounds),
            }));
        }
        Ref::filter_map(cache.borrow(), |entry| {
            entry.as_ref().map(|entry| entry.rows.as_slice())
        })
        .ok()
    }
}

fn wrap_rows(status: &HyperlinkLine, bounds: Size) -> Vec<HyperlinkLine> {
    let line = &status.line;
    let width = usize::from(bounds.width);
    // One row of lookahead settles the last visible break. Whitespace discarded by
    // wrapping cannot count toward that budget or it could hide a later word.
    let limit = usize::from(bounds.height) + 1;
    let mut remaining = width * limit;
    let mut prefix = Line::default().style(line.style);
    for span in &line.spans {
        let mut end = 0;
        for grapheme in span.content.graphemes(true) {
            end += grapheme.len();
            if !grapheme.chars().all(char::is_whitespace) {
                remaining = remaining.saturating_sub(display_width(grapheme).min(width));
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
    adaptive_wrap_line_to_width(&prefix, width.into())
        .into_iter()
        .take(limit)
        .map(|row| {
            let source = column_at(row.range.start)..column_at(row.range.end);
            HyperlinkLine {
                line: line_to_static(&row.line),
                hyperlinks: project_hyperlink_columns(
                    &status.hyperlinks,
                    source,
                    /*prefix*/ 0,
                ),
                source: None,
            }
        })
        .collect()
}
