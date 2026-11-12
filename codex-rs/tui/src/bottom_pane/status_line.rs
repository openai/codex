//! Measure and paint native and extension status lines within the caller's content rectangle.

use crate::line_truncation::truncate_line_with_ellipsis;
use crate::render::highlight::with_syntax_theme;
use crate::render::line_utils::line_to_static;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::project_hyperlink_columns;
use crate::ui_tree::PreparedUiTree;
use crate::ui_tree::UiTree;
use crate::width::display_width;
use crate::wrapping::adaptive_wrap_line_to_width;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::borrow::Cow;
use std::cell::Ref;
use std::cell::RefCell;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusLineFocus {
    Focused,
    Unfocused,
}

/// A status line and the cached layout for its content.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatusLine(StatusContent);

#[derive(Clone, Debug, PartialEq)]
enum StatusContent {
    Native(CachedLayout<Box<HyperlinkLine>, Option<Vec<HyperlinkLine>>>),
    Extension(CachedLayout<Arc<PreparedUiTree>, u16>),
}

#[derive(Debug)]
struct CachedLayout<C, L> {
    content: C,
    layout: RefCell<Option<(Size, L)>>,
}

impl<C, L> CachedLayout<C, L> {
    fn new(content: C) -> Self {
        Self {
            content,
            layout: RefCell::default(),
        }
    }

    fn get(&self, bounds: Size, compute: impl FnOnce(&C) -> L) -> Ref<'_, L> {
        if self
            .layout
            .borrow()
            .as_ref()
            .is_none_or(|(previous, _)| *previous != bounds)
        {
            self.layout.replace(Some((bounds, compute(&self.content))));
        }
        Ref::map(self.layout.borrow(), |entry| {
            let Some((_, layout)) = entry else {
                unreachable!("layout cache was populated");
            };
            layout
        })
    }
}

// Cached geometry belongs to the current surface. Clones and comparisons use only the content.
impl<C: Clone, L> Clone for CachedLayout<C, L> {
    fn clone(&self) -> Self {
        Self::new(self.content.clone())
    }
}

impl<C: PartialEq, L> PartialEq for CachedLayout<C, L> {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
    }
}

impl From<UiTree> for StatusLine {
    fn from(tree: UiTree) -> Self {
        Self(StatusContent::Extension(CachedLayout::new(Arc::new(
            tree.prepare(),
        ))))
    }
}

impl From<HyperlinkLine> for StatusLine {
    fn from(line: HyperlinkLine) -> Self {
        Self(StatusContent::Native(CachedLayout::new(Box::new(line))))
    }
}

impl From<Line<'static>> for StatusLine {
    fn from(line: Line<'static>) -> Self {
        Self::from(HyperlinkLine::from(line))
    }
}

impl From<&'static str> for StatusLine {
    fn from(text: &'static str) -> Self {
        Self::from(Line::from(text))
    }
}

impl StatusLine {
    pub(crate) fn as_native(&self) -> Option<&HyperlinkLine> {
        match &self.0 {
            StatusContent::Native(native) => Some(&native.content),
            StatusContent::Extension(_) => None,
        }
    }

    /// Measure within the space available for content after the caller applies padding.
    pub(crate) fn height(&self, bounds: Size) -> u16 {
        if bounds.width == 0 || bounds.height == 0 {
            return 0;
        }
        match &self.0 {
            StatusContent::Native(native) => wrapped_rows(native, bounds).map_or_else(
                || u16::from(native.content.width() > 0),
                |rows| rows.len().max(1).min(usize::from(bounds.height)) as u16,
            ),
            StatusContent::Extension(extension) => {
                *extension.get(bounds, |tree| tree.measure(bounds).height)
            }
        }
    }

    /// Paint within the final content rectangle after the caller applies padding.
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer, focus: StatusLineFocus) {
        if area.is_empty() {
            return;
        }
        match &self.0 {
            StatusContent::Native(native) => {
                let wrapped = wrapped_rows(native, area.as_size());
                let rows = wrapped
                    .as_deref()
                    .unwrap_or_else(|| std::slice::from_ref(native.content.as_ref()));
                let more_rows = rows.len() > usize::from(area.height);
                for (row, line) in area.rows().zip(rows) {
                    render_status_row(
                        line,
                        row,
                        buf,
                        focus,
                        more_rows && row.bottom() == area.bottom(),
                    );
                }
            }
            StatusContent::Extension(extension) => {
                let plain_style =
                    (focus == StatusLineFocus::Unfocused).then(|| Style::reset().dim());
                with_syntax_theme(|theme| extension.content.render(area, theme, buf, plain_style));
            }
        }
    }
}

fn wrapped_rows(
    native: &CachedLayout<Box<HyperlinkLine>, Option<Vec<HyperlinkLine>>>,
    bounds: Size,
) -> Option<Ref<'_, [HyperlinkLine]>> {
    if bounds.height <= 1 {
        return None;
    }
    // Reuse the same wrapped prefix across small height changes.
    let bounds = Size {
        height: bounds.height.max(4),
        ..bounds
    };
    let cached = native.get(bounds, |line| {
        (line.width() > usize::from(bounds.width)).then(|| wrap_rows(line, bounds))
    });
    Ref::filter_map(cached, Option::as_deref).ok()
}

fn render_status_row(
    line: &HyperlinkLine,
    area: Rect,
    buf: &mut Buffer,
    focus: StatusLineFocus,
    more_rows: bool,
) {
    let mut line = Cow::Borrowed(line);
    let width = usize::from(area.width);
    if more_rows || line.width() > width {
        let line = line.to_mut();
        line.line = truncate_line_with_ellipsis(std::mem::take(&mut line.line), width);
        line.clip_hyperlinks_to_width(line.width().saturating_sub(1));
    }
    if focus == StatusLineFocus::Unfocused {
        let line = line.to_mut();
        line.hyperlinks.clear();
        line.line.style = Style::reset().dim();
        for span in &mut line.line.spans {
            span.style = Style::default();
        }
    }
    line.render_fitted(area, buf);
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

#[cfg(test)]
#[path = "status_line_tests.rs"]
mod tests;
