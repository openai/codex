//! TUI extensions for thinking mode.
//!
//! Provides UI helpers for rendering thinking indicators and highlighting
//! the "ultrathink" keyword in user input.

use codex_protocol::openai_models::ReasoningEffort;
use ratatui::prelude::*;
use ratatui::style::Stylize;

/// Render thinking indicator for status bar.
///
/// Returns a styled span showing the current reasoning effort level.
pub fn render_effort_indicator(effort: ReasoningEffort, ultrathink_active: bool) -> Span<'static> {
    if ultrathink_active {
        " ∴ Ultrathink ".magenta().bold().into()
    } else {
        match effort {
            ReasoningEffort::None => Span::raw(""),
            ReasoningEffort::Minimal => " ∴ Minimal ".dim().into(),
            ReasoningEffort::Low => " ∴ Low ".dim().into(),
            ReasoningEffort::Medium => " ∴ Medium ".cyan().into(),
            ReasoningEffort::High => " ∴ High ".cyan().bold().into(),
            ReasoningEffort::XHigh => " ∴ XHigh ".cyan().bold().into(),
        }
    }
}

/// Render ultrathink toggle indicator for status bar.
///
/// Shows whether ultrathink toggle is currently ON or OFF.
pub fn render_toggle_indicator(enabled: bool) -> Span<'static> {
    if enabled {
        " [Ultrathink ON] ".magenta().bold().into()
    } else {
        Span::raw("")
    }
}

/// Apply cyan highlight to ultrathink keyword positions in text.
///
/// Returns a Line with the "ultrathink" keyword highlighted in cyan.
pub fn highlight_ultrathink(text: &str, positions: &[(usize, usize)]) -> Line<'static> {
    if positions.is_empty() {
        return Line::from(text.to_string());
    }

    let mut spans = Vec::new();
    let mut last_end = 0;

    for &(start, end) in positions {
        // Add text before the keyword
        if start > last_end {
            spans.push(Span::raw(text[last_end..start].to_string()));
        }
        // Add highlighted keyword
        spans.push(text[start..end].to_string().cyan().bold().into());
        last_end = end;
    }

    // Add remaining text after last keyword
    if last_end < text.len() {
        spans.push(Span::raw(text[last_end..].to_string()));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_effort_indicator_ultrathink() {
        let span = render_effort_indicator(ReasoningEffort::XHigh, true);
        assert!(span.content.contains("Ultrathink"));
    }

    #[test]
    fn test_render_effort_indicator_normal() {
        let span = render_effort_indicator(ReasoningEffort::Medium, false);
        assert!(span.content.contains("Medium"));
    }

    #[test]
    fn test_render_effort_indicator_none() {
        let span = render_effort_indicator(ReasoningEffort::None, false);
        assert!(span.content.is_empty());
    }

    #[test]
    fn test_highlight_ultrathink_no_match() {
        let line = highlight_ultrathink("hello world", &[]);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "hello world");
    }

    #[test]
    fn test_highlight_ultrathink_single_match() {
        let line = highlight_ultrathink("test ultrathink here", &[(5, 15)]);
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "test ");
        assert_eq!(line.spans[1].content, "ultrathink");
        assert_eq!(line.spans[2].content, " here");
    }
}
