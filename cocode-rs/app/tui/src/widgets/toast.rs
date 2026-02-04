//! Toast notification widget.
//!
//! Provides transient notifications for system events.
//! Toasts automatically expire after a configurable duration.

use std::time::Duration;
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

/// Severity level for toast notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    /// Informational message.
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

impl ToastSeverity {
    /// Get the icon for this severity level.
    pub fn icon(&self) -> &'static str {
        match self {
            ToastSeverity::Info => "i",
            ToastSeverity::Success => "+",
            ToastSeverity::Warning => "!",
            ToastSeverity::Error => "x",
        }
    }
}

/// A toast notification.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Unique identifier.
    pub id: String,
    /// Message to display.
    pub message: String,
    /// Severity level.
    pub severity: ToastSeverity,
    /// When this toast was created.
    pub created_at: Instant,
    /// How long to display (default 3s).
    pub duration: Duration,
}

impl Toast {
    /// Create a new toast notification.
    pub fn new(id: impl Into<String>, message: impl Into<String>, severity: ToastSeverity) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
            severity,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    /// Create an info toast.
    pub fn info(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, message, ToastSeverity::Info)
    }

    /// Create a success toast.
    pub fn success(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, message, ToastSeverity::Success)
    }

    /// Create a warning toast.
    pub fn warning(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, message, ToastSeverity::Warning)
    }

    /// Create an error toast.
    pub fn error(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, message, ToastSeverity::Error)
    }

    /// Set a custom duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Check if this toast has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }

    /// Get the remaining time as a percentage (0.0 to 1.0).
    pub fn remaining_percent(&self) -> f64 {
        let elapsed = self.created_at.elapsed().as_secs_f64();
        let total = self.duration.as_secs_f64();
        (1.0 - elapsed / total).max(0.0)
    }
}

/// Widget to render a stack of toast notifications.
pub struct ToastWidget<'a> {
    toasts: &'a [Toast],
    max_display: i32,
}

impl<'a> ToastWidget<'a> {
    /// Create a new toast widget.
    pub fn new(toasts: &'a [Toast]) -> Self {
        Self {
            toasts,
            max_display: 3,
        }
    }

    /// Set the maximum number of toasts to display.
    pub fn max_display(mut self, max: i32) -> Self {
        self.max_display = max;
        self
    }

    /// Calculate the area for rendering toasts in the top-right corner.
    pub fn calculate_area(&self, frame_area: Rect) -> Rect {
        if self.toasts.is_empty() {
            return Rect::default();
        }

        let width = 40.min(frame_area.width.saturating_sub(4));
        let count = (self.toasts.len() as u16).min(self.max_display as u16);
        let height = count * 3; // Each toast is 3 lines (border + content + border)

        let x = frame_area.width.saturating_sub(width + 2);
        let y = 1; // Small margin from top

        Rect::new(x, y, width, height)
    }
}

impl Widget for ToastWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 3 {
            return;
        }

        let toast_height = 3_u16;
        let mut y_offset = 0_u16;

        for toast in self.toasts.iter().take(self.max_display as usize) {
            if y_offset + toast_height > area.height {
                break;
            }

            let toast_area = Rect::new(area.x, area.y + y_offset, area.width, toast_height);

            // Clear the background
            Clear.render(toast_area, buf);

            // Build the toast content
            let (icon, border_style) = match toast.severity {
                ToastSeverity::Info => (
                    toast.severity.icon(),
                    ratatui::style::Style::default().cyan(),
                ),
                ToastSeverity::Success => (
                    toast.severity.icon(),
                    ratatui::style::Style::default().green(),
                ),
                ToastSeverity::Warning => (
                    toast.severity.icon(),
                    ratatui::style::Style::default().yellow(),
                ),
                ToastSeverity::Error => (
                    toast.severity.icon(),
                    ratatui::style::Style::default().red(),
                ),
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style);

            let inner = block.inner(toast_area);
            block.render(toast_area, buf);

            // Render the message with icon
            let line = match toast.severity {
                ToastSeverity::Info => Line::from(vec![
                    Span::raw(format!("[{icon}] ")).cyan(),
                    Span::raw(&toast.message),
                ]),
                ToastSeverity::Success => Line::from(vec![
                    Span::raw(format!("[{icon}] ")).green(),
                    Span::raw(&toast.message),
                ]),
                ToastSeverity::Warning => Line::from(vec![
                    Span::raw(format!("[{icon}] ")).yellow(),
                    Span::raw(&toast.message),
                ]),
                ToastSeverity::Error => Line::from(vec![
                    Span::raw(format!("[{icon}] ")).red(),
                    Span::raw(&toast.message),
                ]),
            };

            Paragraph::new(line).render(inner, buf);

            y_offset += toast_height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_creation() {
        let toast = Toast::info("t1", "Test message");
        assert_eq!(toast.id, "t1");
        assert_eq!(toast.message, "Test message");
        assert_eq!(toast.severity, ToastSeverity::Info);
        assert!(!toast.is_expired());
    }

    #[test]
    fn test_toast_severity_icons() {
        assert_eq!(ToastSeverity::Info.icon(), "i");
        assert_eq!(ToastSeverity::Success.icon(), "+");
        assert_eq!(ToastSeverity::Warning.icon(), "!");
        assert_eq!(ToastSeverity::Error.icon(), "x");
    }

    #[test]
    fn test_toast_with_duration() {
        let toast = Toast::info("t1", "Test").with_duration(Duration::from_secs(5));
        assert_eq!(toast.duration, Duration::from_secs(5));
    }

    #[test]
    fn test_toast_expired() {
        let mut toast = Toast::info("t1", "Test");
        toast.duration = Duration::from_millis(1);
        std::thread::sleep(Duration::from_millis(5));
        assert!(toast.is_expired());
    }

    #[test]
    fn test_toast_widget_render() {
        let toasts = vec![
            Toast::info("t1", "Info message"),
            Toast::warning("t2", "Warning message"),
        ];
        let widget = ToastWidget::new(&toasts);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Should render without panic
    }

    #[test]
    fn test_toast_widget_calculate_area() {
        let toasts = vec![Toast::info("t1", "Test")];
        let widget = ToastWidget::new(&toasts);

        let frame_area = Rect::new(0, 0, 100, 50);
        let area = widget.calculate_area(frame_area);

        assert!(area.x > 0);
        assert_eq!(area.y, 1);
        assert!(area.width > 0);
        assert!(area.height > 0);
    }
}
