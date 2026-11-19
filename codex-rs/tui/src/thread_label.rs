//! Display a thread's backend marker without changing its name, search text, or identity.

use crate::line_truncation::span_width;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::text_formatting::truncate_text;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ThreadLabelStyle {
    #[default]
    Plain,
    Cloud,
}

impl ThreadLabelStyle {
    pub(crate) fn from_is_cloud(is_cloud: bool) -> Self {
        if is_cloud { Self::Cloud } else { Self::Plain }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Plain => "",
            // U+FE0F requests emoji presentation.
            Self::Cloud => "☁\u{fe0f} ",
        }
    }

    pub(crate) fn prefix_width(self) -> usize {
        span_width(&Span::raw(self.prefix()))
    }

    pub(crate) fn render(self, line: Line<'static>, width: usize) -> Line<'static> {
        let prefix_width = self.prefix_width();
        if prefix_width == 0 || width <= prefix_width {
            return truncate_line_with_ellipsis_if_overflow(line, width);
        }
        let mut line = truncate_line_with_ellipsis_if_overflow(line, width - prefix_width);
        line.spans.insert(0, self.prefix().dim());
        line
    }

    /// Keep the picker's existing plain-text truncation for undecorated labels.
    pub(crate) fn render_text(self, title: &str, width: usize) -> String {
        match self {
            Self::Plain => truncate_text(title, width),
            Self::Cloud => self.render(Line::from(title.to_owned()), width).to_string(),
        }
    }

    pub(crate) fn decorate(self, mut title: String) -> String {
        title.insert_str(0, self.prefix());
        title
    }
}
