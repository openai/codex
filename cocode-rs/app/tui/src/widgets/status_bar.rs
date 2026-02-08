//! Status bar widget.
//!
//! Displays:
//! - Current model name
//! - Thinking level
//! - Plan mode indicator
//! - Thinking duration
//! - Token usage

use std::time::Duration;

use cocode_protocol::ReasoningEffort;
use cocode_protocol::ThinkingLevel;
use cocode_protocol::TokenUsage;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::i18n::t;
use crate::state::PlanPhase;

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
    /// Whether currently streaming thinking content.
    is_thinking: bool,
    /// Whether thinking display is enabled.
    show_thinking_enabled: bool,
    /// Current or last thinking duration.
    thinking_duration: Option<Duration>,
    /// Thinking tokens used in current turn.
    thinking_tokens_used: i32,
    /// Thinking budget remaining (if set).
    thinking_budget_remaining: Option<i32>,
    /// Current phase in plan mode.
    plan_phase: Option<PlanPhase>,
    /// Number of connected MCP servers.
    mcp_server_count: i32,
    /// Number of queued commands (also serves as steering).
    queued_count: i32,
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
            is_thinking: false,
            show_thinking_enabled: false,
            thinking_duration: None,
            thinking_tokens_used: 0,
            thinking_budget_remaining: None,
            plan_phase: None,
            mcp_server_count: 0,
            queued_count: 0,
        }
    }

    /// Set whether the assistant is currently thinking.
    pub fn is_thinking(mut self, thinking: bool) -> Self {
        self.is_thinking = thinking;
        self
    }

    /// Set whether thinking display is enabled.
    pub fn show_thinking_enabled(mut self, enabled: bool) -> Self {
        self.show_thinking_enabled = enabled;
        self
    }

    /// Set the thinking duration (current or last completed).
    pub fn thinking_duration(mut self, duration: Option<Duration>) -> Self {
        self.thinking_duration = duration;
        self
    }

    /// Set thinking budget info.
    pub fn thinking_budget(mut self, used: i32, remaining: Option<i32>) -> Self {
        self.thinking_tokens_used = used;
        self.thinking_budget_remaining = remaining;
        self
    }

    /// Set the plan phase.
    pub fn plan_phase(mut self, phase: Option<PlanPhase>) -> Self {
        self.plan_phase = phase;
        self
    }

    /// Set the MCP server count.
    pub fn mcp_server_count(mut self, count: i32) -> Self {
        self.mcp_server_count = count;
        self
    }

    /// Set the queue count.
    ///
    /// Queued commands are consumed once and injected as steering into the
    /// current turn (consume-then-remove pattern).
    pub fn queue_counts(mut self, queued: i32, _steering: i32) -> Self {
        self.queued_count = queued;
        // steering_count is deprecated - queued commands now serve as steering
        self
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
            ReasoningEffort::None => (t!("status.think_off").to_string(), "dim"),
            ReasoningEffort::Minimal => (t!("status.think_min").to_string(), "dim"),
            ReasoningEffort::Low => (t!("status.think_low").to_string(), "green"),
            ReasoningEffort::Medium => (t!("status.think_med").to_string(), "yellow"),
            ReasoningEffort::High => (t!("status.think_high").to_string(), "magenta"),
            ReasoningEffort::XHigh => (t!("status.think_max").to_string(), "red"),
        };

        let text = format!(" {} ", t!("status.think_label", level = label));
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
            if let Some(phase) = self.plan_phase {
                // Show phase indicator when in plan mode
                let phase_text = format!(" {} {} ", phase.emoji(), phase.display_name());
                Some(Span::raw(phase_text).on_blue().bold())
            } else {
                let plan_text = format!(" {} ", t!("status.plan"));
                Some(Span::raw(plan_text).on_blue().bold())
            }
        } else {
            None
        }
    }

    /// Format the MCP server indicator.
    fn format_mcp_status(&self) -> Option<Span<'static>> {
        if self.mcp_server_count > 0 {
            Some(
                Span::raw(format!(
                    " {} ",
                    t!("status.mcp", count = self.mcp_server_count)
                ))
                .green(),
            )
        } else {
            None
        }
    }

    /// Format the queue status indicator.
    fn format_queue_status(&self) -> Option<Span<'static>> {
        if self.queued_count == 0 {
            return None;
        }

        let text = format!(" {} ", t!("status.queued", count = self.queued_count));
        Some(Span::raw(text).yellow())
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
        Span::raw(format!(" {} ", t!("status.tokens", count = formatted))).dim()
    }

    /// Format keyboard hints.
    fn format_hints(&self) -> Span<'static> {
        Span::raw(format!(" {} ", t!("status.hints"))).dim()
    }

    /// Format the thinking status indicator.
    fn format_thinking_status(&self) -> Option<Span<'static>> {
        if self.is_thinking {
            // Show spinner and current duration while thinking
            let duration_text = if let Some(duration) = self.thinking_duration {
                let secs = duration.as_secs();
                if secs > 0 {
                    format!(" {} ", t!("status.thinking_with_duration", duration = secs))
                } else {
                    format!(" {} ", t!("status.thinking"))
                }
            } else {
                format!(" {} ", t!("status.thinking"))
            };
            Some(Span::raw(format!("🤔{duration_text}")).magenta().italic())
        } else if let Some(duration) = self.thinking_duration {
            // Show completed duration after thinking
            let secs = duration.as_secs();
            if secs > 0 {
                Some(Span::raw(format!(" {} ", t!("status.thought_for", duration = secs))).dim())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Format the thinking display toggle indicator.
    fn format_thinking_toggle(&self) -> Option<Span<'static>> {
        if self.show_thinking_enabled {
            Some(Span::raw(" 💭 ").dim())
        } else {
            None
        }
    }

    /// Format the thinking budget display.
    fn format_thinking_budget(&self) -> Option<Span<'static>> {
        // Only show if we have a budget set or if we've used thinking tokens
        if self.thinking_budget_remaining.is_none() && self.thinking_tokens_used == 0 {
            return None;
        }

        let text = if let Some(remaining) = self.thinking_budget_remaining {
            let used = self.thinking_tokens_used;
            let total = remaining + used;
            // Format with k suffix for thousands
            if total >= 1000 {
                format!(
                    " 🧠 {:.1}k/{:.1}k ",
                    used as f64 / 1000.0,
                    total as f64 / 1000.0
                )
            } else {
                format!(" 🧠 {used}/{total} ")
            }
        } else if self.thinking_tokens_used > 0 {
            // No budget, just show used
            if self.thinking_tokens_used >= 1000 {
                format!(" 🧠 {:.1}k ", self.thinking_tokens_used as f64 / 1000.0)
            } else {
                format!(" 🧠 {} ", self.thinking_tokens_used)
            }
        } else {
            return None;
        };

        Some(Span::raw(text).cyan())
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

        // Thinking status (if currently thinking)
        if let Some(thinking_span) = self.format_thinking_status() {
            spans.push(thinking_span);
            spans.push(Span::raw("│").dim());
        }

        // Thinking toggle indicator (if enabled)
        if let Some(toggle_span) = self.format_thinking_toggle() {
            spans.push(toggle_span);
            spans.push(Span::raw("│").dim());
        }

        // Thinking budget (if set or tokens used)
        if let Some(budget_span) = self.format_thinking_budget() {
            spans.push(budget_span);
            spans.push(Span::raw("│").dim());
        }

        // MCP servers (if any connected)
        if let Some(mcp_span) = self.format_mcp_status() {
            spans.push(mcp_span);
            spans.push(Span::raw("│").dim());
        }

        // Queue status (if items pending)
        if let Some(queue_span) = self.format_queue_status() {
            spans.push(queue_span);
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

    #[test]
    fn test_thinking_duration_display() {
        let usage = TokenUsage::default();
        let thinking = ThinkingLevel::default();

        // While thinking
        let bar = StatusBar::new("model", &thinking, false, &usage)
            .is_thinking(true)
            .thinking_duration(Some(Duration::from_secs(5)));
        let span = bar.format_thinking_status().unwrap();
        assert!(span.content.contains("thinking"));
        assert!(span.content.contains("5s"));

        // After thinking
        let bar = StatusBar::new("model", &thinking, false, &usage)
            .is_thinking(false)
            .thinking_duration(Some(Duration::from_secs(10)));
        let span = bar.format_thinking_status().unwrap();
        assert!(span.content.contains("thought for 10s"));
    }

    #[test]
    fn test_thinking_budget_display() {
        let usage = TokenUsage::default();
        let thinking = ThinkingLevel::default();

        // No budget, no tokens used - should not display
        let bar = StatusBar::new("model", &thinking, false, &usage);
        assert!(bar.format_thinking_budget().is_none());

        // With budget and usage (total < 1000, uses plain format)
        let bar = StatusBar::new("model", &thinking, false, &usage).thinking_budget(300, Some(400));
        let span = bar.format_thinking_budget().unwrap();
        assert!(span.content.contains("300"));
        assert!(span.content.contains("700")); // 300 + 400 = 700

        // With large numbers (k format, total >= 1000)
        let bar =
            StatusBar::new("model", &thinking, false, &usage).thinking_budget(5000, Some(5000));
        let span = bar.format_thinking_budget().unwrap();
        assert!(span.content.contains("5.0k"));
        assert!(span.content.contains("10.0k"));

        // Tokens used without budget
        let bar = StatusBar::new("model", &thinking, false, &usage).thinking_budget(250, None);
        let span = bar.format_thinking_budget().unwrap();
        assert!(span.content.contains("250"));
    }

    #[test]
    fn test_queue_status_display() {
        let usage = TokenUsage::default();
        let thinking = ThinkingLevel::default();

        // No queued items - should not display
        let bar = StatusBar::new("model", &thinking, false, &usage).queue_counts(0, 0);
        assert!(bar.format_queue_status().is_none());

        // Queued commands (also serve as steering)
        let bar = StatusBar::new("model", &thinking, false, &usage).queue_counts(2, 0);
        let span = bar.format_queue_status().unwrap();
        assert!(span.content.contains("2"));
        assert!(span.content.contains("queued"));

        // More queued commands
        let bar = StatusBar::new("model", &thinking, false, &usage).queue_counts(3, 0);
        let span = bar.format_queue_status().unwrap();
        assert!(span.content.contains("3"));
        assert!(span.content.contains("queued"));
    }
}
