//! Progress bar widget for displaying multi-phase operations.
//!
//! Shows progress with phase information and timing.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::events::IndexPhaseInfo;

/// Progress bar widget state.
#[derive(Debug, Clone, Default)]
pub struct ProgressBarState {
    /// Current progress (0.0 to 1.0).
    pub progress: f32,
    /// Current phase.
    pub phase: Option<IndexPhaseInfo>,
    /// Phase description.
    pub description: String,
    /// Whether an operation is in progress.
    pub in_progress: bool,
    /// Number of items processed.
    pub items_processed: i32,
    /// Total number of items.
    pub total_items: i32,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: i64,
    /// Error message if failed.
    pub error: Option<String>,
}

impl ProgressBarState {
    /// Create a new progress bar state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new operation.
    pub fn start(&mut self, total_items: i32) {
        self.progress = 0.0;
        self.phase = Some(IndexPhaseInfo::Scanning);
        self.description = "Starting...".to_string();
        self.in_progress = true;
        self.items_processed = 0;
        self.total_items = total_items;
        self.elapsed_ms = 0;
        self.error = None;
    }

    /// Update progress.
    pub fn update(&mut self, phase: IndexPhaseInfo, progress: f32, description: &str) {
        self.phase = Some(phase);
        self.progress = progress.clamp(0.0, 1.0);
        self.description = description.to_string();
    }

    /// Update item count.
    pub fn set_items(&mut self, processed: i32, total: i32) {
        self.items_processed = processed;
        self.total_items = total;
        if total > 0 {
            self.progress = processed as f32 / total as f32;
        }
    }

    /// Complete the operation.
    pub fn complete(&mut self, elapsed_ms: i64) {
        self.progress = 1.0;
        self.in_progress = false;
        self.elapsed_ms = elapsed_ms;
        self.description = "Completed".to_string();
    }

    /// Fail the operation.
    pub fn fail(&mut self, error: String) {
        self.in_progress = false;
        self.error = Some(error);
    }

    /// Reset to idle state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Progress bar widget.
pub struct ProgressBar<'a> {
    state: &'a ProgressBarState,
    title: &'a str,
}

impl<'a> ProgressBar<'a> {
    /// Create a new progress bar widget.
    pub fn new(state: &'a ProgressBarState) -> Self {
        Self {
            state,
            title: "Progress",
        }
    }

    /// Set the title.
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    fn render_progress_bar(&self, width: usize) -> Line<'static> {
        let filled = (self.state.progress * width as f32) as usize;
        let empty = width.saturating_sub(filled);

        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        let pct = (self.state.progress * 100.0) as i32;
        let pct_str = format!(" {}%", pct);

        Line::from(vec![
            Span::styled(bar, Style::default().cyan()),
            Span::styled(pct_str, Style::default().bold()),
        ])
    }
}

impl Widget for ProgressBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 {
            return;
        }

        // Build content lines
        let mut lines = Vec::new();

        // Status line
        let status = if let Some(ref error) = self.state.error {
            Line::from(vec![
                Span::styled("✗ Error: ", Style::default().red().bold()),
                Span::styled(error.clone(), Style::default().red()),
            ])
        } else if self.state.in_progress {
            let phase_str = self
                .state
                .phase
                .map(|p| format!("{}", p))
                .unwrap_or_else(|| "Working".to_string());
            Line::from(vec![
                Span::styled("● ", Style::default().cyan()),
                Span::styled(phase_str, Style::default().bold()),
                Span::raw(" - "),
                Span::raw(self.state.description.clone()),
            ])
        } else if self.state.progress >= 1.0 {
            Line::from(vec![
                Span::styled("✓ ", Style::default().green()),
                Span::styled("Completed", Style::default().green().bold()),
                Span::raw(format!(" ({}ms)", self.state.elapsed_ms)),
            ])
        } else {
            Line::from(Span::styled("Idle", Style::default().dim()))
        };
        lines.push(status);

        // Progress bar (only if in progress or completed)
        if self.state.in_progress || self.state.progress > 0.0 {
            let bar_width = (inner.width as usize).saturating_sub(6).max(10);
            lines.push(self.render_progress_bar(bar_width));
        }

        // Item count
        if self.state.total_items > 0 {
            let items_line = Line::from(vec![
                Span::styled("Items: ", Style::default().dim()),
                Span::raw(format!(
                    "{} / {}",
                    self.state.items_processed, self.state.total_items
                )),
            ]);
            lines.push(items_line);
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_state_start() {
        let mut state = ProgressBarState::new();
        state.start(100);

        assert!(state.in_progress);
        assert_eq!(state.total_items, 100);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn test_progress_bar_state_update() {
        let mut state = ProgressBarState::new();
        state.start(100);
        state.update(IndexPhaseInfo::Chunking, 0.5, "Chunking files...");

        assert_eq!(state.phase, Some(IndexPhaseInfo::Chunking));
        assert_eq!(state.progress, 0.5);
        assert_eq!(state.description, "Chunking files...");
    }

    #[test]
    fn test_progress_bar_state_complete() {
        let mut state = ProgressBarState::new();
        state.start(100);
        state.complete(5000);

        assert!(!state.in_progress);
        assert_eq!(state.progress, 1.0);
        assert_eq!(state.elapsed_ms, 5000);
    }

    #[test]
    fn test_progress_bar_state_fail() {
        let mut state = ProgressBarState::new();
        state.start(100);
        state.fail("Disk full".to_string());

        assert!(!state.in_progress);
        assert_eq!(state.error, Some("Disk full".to_string()));
    }
}
