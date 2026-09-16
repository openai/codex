//! Bounded, theme-aware Mermaid previews for completed Markdown fences.
//!
//! Source remains owned by the transcript. Unsupported syntax, resource limits, and terminal
//! overflow retain the original code block; diagrams never emit terminal control sequences.

use crate::render::highlight::foreground_style_for_scopes_with_theme;
use codex_mermaid::Role;
use ratatui::style::Style;
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

pub(super) fn render(
    source: &str,
    width: Option<usize>,
    theme: &Theme,
) -> Option<Vec<Line<'static>>> {
    let diagram = codex_mermaid::render_spans(source, width.unwrap_or(120)).ok()?;
    let node = foreground_style_for_scopes_with_theme(
        theme,
        &["entity.name.type", "support.type", "variable"],
    )
    .unwrap_or_else(|| Style::default().cyan());
    let edge = foreground_style_for_scopes_with_theme(theme, &["comment"])
        .unwrap_or_else(|| Style::default().dim());
    Some(
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
            .collect(),
    )
}

#[cfg(test)]
#[path = "mermaid_tests.rs"]
mod tests;
