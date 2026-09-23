//! Bounded, theme-aware Mermaid previews for completed Markdown fences.
//!
//! Source remains owned by the transcript. Unsupported syntax, resource limits, and terminal
//! overflow retain the original code block with a reason; diagrams never emit terminal control
//! sequences. Callers only render completed fences when Mermaid rendering is enabled.

use crate::render::highlight::foreground_style_for_scopes_with_theme;
use crate::render::highlight::highlight_code_to_lines;
use codex_mermaid::RenderError;
use codex_mermaid::Role;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::ops::Range;
use syntect::highlighting::Theme;

/// CommonMark emits an End event even at EOF. A real closer is outside the last Text event.
pub(super) fn has_closing_fence(input: &str, range: Range<usize>, content_end: usize) -> bool {
    let Some(block) = input.get(range.clone()) else {
        return false;
    };
    let Some(marker @ (b'`' | b'~')) = block.as_bytes().first().copied() else {
        return false;
    };
    let opening_len = block.bytes().take_while(|byte| *byte == marker).count();
    let Some(suffix) = input.get(content_end..range.end) else {
        return false;
    };
    suffix
        .trim_end_matches([' ', '\t', '\r', '\n'])
        .bytes()
        .rev()
        .take_while(|byte| *byte == marker)
        .count()
        >= opening_len
}

pub(super) fn render(source: &str, width: Option<usize>, theme: &Theme) -> Vec<Line<'static>> {
    let width = width.unwrap_or(120);
    let diagram = match codex_mermaid::render_spans(source, width) {
        Ok(diagram) => diagram,
        Err(error) => {
            let notice = match error {
                RenderError::Unsupported => {
                    "This Mermaid diagram uses features the terminal renderer doesn't support."
                }
                RenderError::TooWide => {
                    "This Mermaid diagram doesn't fit the current terminal width."
                }
                RenderError::Limit => {
                    "This Mermaid diagram exceeds the terminal renderer's size limits."
                }
            };
            let mut lines = textwrap::wrap(notice, width.max(/*other*/ 1))
                .into_iter()
                .map(|line| Line::from(line.into_owned().dim()))
                .collect::<Vec<_>>();
            lines.extend(highlight_code_to_lines(source, "mermaid"));
            return lines;
        }
    };
    let node = foreground_style_for_scopes_with_theme(
        theme,
        &["entity.name.type", "support.type", "variable"],
    )
    .unwrap_or_else(|| Style::default().cyan());
    let edge = foreground_style_for_scopes_with_theme(theme, &["comment"])
        .unwrap_or_else(|| Style::default().dim());
    diagram
        .into_iter()
        .map(|line| {
            Line::from(
                line.into_iter()
                    .map(|span| {
                        let style = match span.role {
                            Role::Node => node,
                            Role::Edge => edge,
                            Role::Text => Style::default(),
                        };
                        Span::styled(span.text, style)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "mermaid_tests.rs"]
mod tests;
