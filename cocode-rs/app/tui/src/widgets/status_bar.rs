//! Status bar widget.
//!
//! Displays:
//! - Current model name
//! - Thinking level
//! - Plan mode indicator
//! - Token usage

use cocode_protocol::ReasoningEffort;
use cocode_protocol::ThinkingLevel;
use cocode_protocol::TokenUsage;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

/// Status bar widget showing model, thinking level, plan mode, and tokens.
///
/// # Example
///
/// ```ignore
/// let status_bar = StatusBar::new()
///     .model("claude-sonnet-4-20250514")
///     .thinking_level(&ThinkingLevel::new(ReasoningEffort::High))
///     .plan_mode(true)
///     .token_usage(&usage);
/// ```
pub struct StatusBar<'a> {
    model: &'a str,
    thinking_level: &'a ThinkingLevel,
    plan_mode: bool,
    token_usage: &'a TokenUsage,
}

impl<'a> StatusBar<'a> {
    /// Create a new status bar.
    pub fn new(
        model: &'a str,
        thinking_level: &'a ThinkingLevel,
        plan_mode: bool,
        token_usage: &'a TokenUsage,
    ) -> Self {
        Self {
            model,
            thinking_level,
            plan_mode,
            token_usage,
        }
    }

    /// Format the model name for display.
    fn format_model(&self) -> Span<'static> {
        // Shorten long model names
        let name = if self.model.len() > 24 {
            let parts: Vec<&str> = self.model.split('-').collect();
            if parts.len() >= 2 {
                format!("{}-{}", parts[0], parts.last().unwrap_or(&""))
            } else {
                self.model[..24].to_string()
            }
        } else {
            self.model.to_string()
        };
        Span::raw(format!(" {name} ")).cyan()
    }

    /// Format the thinking level for display.
    fn format_thinking(&self) -> Span<'static> {
        let (label, style) = match self.thinking_level.effort {
            ReasoningEffort::None => ("off", "dim"),
            ReasoningEffort::Minimal => ("min", "dim"),
            ReasoningEffort::Low => ("low", "green"),
            ReasoningEffort::Medium => ("med", "yellow"),
            ReasoningEffort::High => ("high", "magenta"),
            ReasoningEffort::XHigh => ("max", "red"),
        };

        let text = format!(" Think:{label} ");
        match style {
            "dim" => Span::raw(text).dim(),
            "green" => Span::raw(text).green(),
            "yellow" => Span::raw(text).yellow(),
            "magenta" => Span::raw(text).magenta(),
            "red" => Span::raw(text).red(),
            _ => Span::raw(text),
        }
    }

    /// Format the plan mode indicator.
    fn format_plan_mode(&self) -> Option<Span<'static>> {
        if self.plan_mode {
            Some(Span::raw(" PLAN ").on_blue().bold())
        } else {
            None
        }
    }

    /// Format the token usage.
    fn format_tokens(&self) -> Span<'static> {
        let total = self.token_usage.total();
        let formatted = if total >= 1_000_000 {
            format!("{:.1}M", total as f64 / 1_000_000.0)
        } else if total >= 1_000 {
            format!("{:.1}k", total as f64 / 1_000.0)
        } else {
            format!("{total}")
        };
        Span::raw(format!(" {formatted} tokens ")).dim()
    }

    /// Format keyboard hints.
    fn format_hints(&self) -> Span<'static> {
        Span::raw(" Tab:plan Ctrl+T:think Ctrl+M:model ").dim()
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        // Build the status line
        let mut spans: Vec<Span> = vec![];

        // Model
        spans.push(self.format_model());
        spans.push(Span::raw("│").dim());

        // Thinking level
        spans.push(self.format_thinking());
        spans.push(Span::raw("│").dim());

        // Plan mode (if active)
        if let Some(plan_span) = self.format_plan_mode() {
            spans.push(plan_span);
            spans.push(Span::raw("│").dim());
        }

        // Tokens
        spans.push(self.format_tokens());

        // Calculate used width
        let used_width: usize = spans.iter().map(|s| s.width()).sum();

        // Add hints if there's room
        let hints = self.format_hints();
        let hints_width = hints.width();
        if used_width + hints_width + 2 <= area.width as usize {
            // Add spacer
            let spacer_width = area.width as usize - used_width - hints_width;
            spans.push(Span::raw(" ".repeat(spacer_width)));
            spans.push(hints);
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_model_short() {
        let usage = TokenUsage::default();
        let thinking = ThinkingLevel::default();
        let bar = StatusBar::new("gpt-4", &thinking, false, &usage);
        let span = bar.format_model();
        assert!(span.content.contains("gpt-4"));
    }

    #[test]
    fn test_format_thinking_levels() {
        let usage = TokenUsage::default();

        for effort in [
            ReasoningEffort::None,
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
        ] {
            let thinking = ThinkingLevel::new(effort);
            let bar = StatusBar::new("model", &thinking, false, &usage);
            let span = bar.format_thinking();
            assert!(span.content.contains("Think:"));
        }
    }

    #[test]
    fn test_format_plan_mode() {
        let usage = TokenUsage::default();
        let thinking = ThinkingLevel::default();

        let bar = StatusBar::new("model", &thinking, false, &usage);
        assert!(bar.format_plan_mode().is_none());

        let bar = StatusBar::new("model", &thinking, true, &usage);
        assert!(bar.format_plan_mode().is_some());
    }

    #[test]
    fn test_format_tokens() {
        let thinking = ThinkingLevel::default();

        let usage = TokenUsage::new(500, 200);
        let bar = StatusBar::new("model", &thinking, false, &usage);
        let span = bar.format_tokens();
        assert!(span.content.contains("700"));

        let usage = TokenUsage::new(1500, 500);
        let bar = StatusBar::new("model", &thinking, false, &usage);
        let span = bar.format_tokens();
        assert!(span.content.contains("2.0k"));
    }

    #[test]
    fn test_render() {
        let usage = TokenUsage::new(1000, 500);
        let thinking = ThinkingLevel::new(ReasoningEffort::High);
        let bar = StatusBar::new("claude-sonnet-4", &thinking, true, &usage);

        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);

        // Check that the buffer contains expected content
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("claude-sonnet-4"));
        assert!(content.contains("PLAN"));
    }
}
