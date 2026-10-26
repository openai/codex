//! Render each tagged title and the following suggestions from the tool's visible Markdown.

use crate::style::accent_color_on;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use codex_protocol::items::AsyncUserInputQuestion;
use ratatui::style::Style;
use ratatui::text::Span;
use std::ops::Range;

/// Core joins each title and its Markdown bullets, then separates questions by a blank line.
/// Stop decorating if the metadata and visible Markdown diverge.
pub(super) fn title_ranges(
    source: &str,
    questions: &[AsyncUserInputQuestion],
) -> Vec<Range<usize>> {
    let mut titles = Vec::new();
    let mut rest = source;
    for question in questions {
        rest = rest.trim_start_matches('\n');
        let title = question.title.replace("\r\n", "\n");
        let title = title.trim_matches('\n');
        let Some(after_title) = rest.strip_prefix(title) else {
            break;
        };
        let start = source.len() - rest.len();
        titles.push(start..start + title.len());
        rest = after_title;
        for option in question.options.iter().flatten() {
            let option = option.replace("\r\n", "\n");
            let Some(after_option) = rest
                .trim_start_matches('\n')
                .strip_prefix("- ")
                .and_then(|s| s.strip_prefix(&option))
            else {
                return titles;
            };
            rest = after_option;
        }
    }
    titles
}

pub(super) fn render(
    source: &str,
    titles: &[Range<usize>],
    markdown: impl Fn(&str) -> Vec<HyperlinkLine>,
) -> Vec<HyperlinkLine> {
    let accent = Style::new().fg(accent_color_on(/*background*/ None));
    let mut lines = Vec::new();
    for (index, title) in titles.iter().enumerate() {
        if !lines.is_empty() {
            lines.push(HyperlinkLine::default());
        }
        let heading = markdown(&source[title.clone()])
            .into_iter()
            .map(|row| {
                let style = accent.patch(row.line.style);
                row.style(style).resolve_span_styles()
            })
            .collect();
        lines.extend(prefix_hyperlink_lines(
            heading,
            Span::styled("? ", accent),
            "  ".into(),
        ));

        let end = titles
            .get(index + 1)
            .map_or(source.len(), |next| next.start);
        let body = source[title.end..end].trim_matches('\n');
        if !body.is_empty() {
            lines.push(HyperlinkLine::default());
            lines.extend(prefix_hyperlink_lines(
                markdown(body),
                "  ".into(),
                "  ".into(),
            ));
        }
    }
    lines
}
