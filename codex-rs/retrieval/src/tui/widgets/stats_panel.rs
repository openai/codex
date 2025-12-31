//! Statistics panel widget for displaying index/search statistics.
//!
//! Shows key metrics in a compact format.

use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

/// Stats panel widget state.
#[derive(Debug, Clone, Default)]
pub struct StatsPanelState {
    /// Total files indexed.
    pub file_count: i32,
    /// Total chunks created.
    pub chunk_count: i32,
    /// Total symbols extracted.
    pub symbol_count: i32,
    /// Index size in bytes.
    pub index_size_bytes: i64,
    /// Last index time.
    pub last_indexed: Option<String>,
    /// Languages detected.
    pub languages: Vec<String>,
    /// Whether index is ready.
    pub is_ready: bool,
    /// Whether file watching is active.
    pub is_watching: bool,
}

impl StatsPanelState {
    /// Create a new stats panel state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Format the index size for display.
    pub fn format_size(&self) -> String {
        let bytes = self.index_size_bytes;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Set stats from index completion event.
    pub fn set_stats(&mut self, file_count: i32, chunk_count: i32, symbol_count: i32) {
        self.file_count = file_count;
        self.chunk_count = chunk_count;
        self.symbol_count = symbol_count;
        self.is_ready = true;
    }
}

/// Stats panel widget.
pub struct StatsPanel<'a> {
    state: &'a StatsPanelState,
}

impl<'a> StatsPanel<'a> {
    /// Create a new stats panel widget.
    pub fn new(state: &'a StatsPanelState) -> Self {
        Self { state }
    }

    fn status_indicator(&self) -> Line<'static> {
        let (icon, status, style) = if self.state.is_watching {
            ("◉", "Watching", Style::default().green())
        } else if self.state.is_ready {
            ("●", "Ready", Style::default().cyan())
        } else {
            ("○", "Not Indexed", Style::default().dim())
        };

        Line::from(vec![
            Span::styled("Status: ", Style::default().dim()),
            Span::styled(format!("{} {}", icon, status), style),
        ])
    }

    fn stat_line(label: &str, value: String) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{}: ", label), Style::default().dim()),
            Span::raw(value),
        ])
    }
}

impl Widget for StatsPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default().borders(Borders::ALL).title(" Statistics ");

        let inner = block.inner(area);
        block.render(area, buf);

        // Split into two columns
        let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        // Left column: counts
        let left_lines = vec![
            self.status_indicator(),
            Self::stat_line("Files", format!("{}", self.state.file_count)),
            Self::stat_line("Chunks", format!("{}", self.state.chunk_count)),
            Self::stat_line("Symbols", format!("{}", self.state.symbol_count)),
        ];
        Paragraph::new(left_lines).render(columns[0], buf);

        // Right column: metadata
        let size_str = self.state.format_size();
        let last_indexed = self.state.last_indexed.as_deref().unwrap_or("Never");
        let languages = if self.state.languages.is_empty() {
            "-".to_string()
        } else {
            self.state.languages.join(", ")
        };

        let right_lines = vec![
            Self::stat_line("Size", size_str),
            Self::stat_line("Last indexed", last_indexed.to_string()),
            Self::stat_line("Languages", languages),
        ];
        Paragraph::new(right_lines).render(columns[1], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        let mut state = StatsPanelState::new();

        state.index_size_bytes = 512;
        assert_eq!(state.format_size(), "512 B");

        state.index_size_bytes = 2048;
        assert_eq!(state.format_size(), "2.0 KB");

        state.index_size_bytes = 5 * 1024 * 1024;
        assert_eq!(state.format_size(), "5.0 MB");

        state.index_size_bytes = 2 * 1024 * 1024 * 1024;
        assert_eq!(state.format_size(), "2.0 GB");
    }

    #[test]
    fn test_set_stats() {
        let mut state = StatsPanelState::new();
        state.set_stats(100, 500, 200);

        assert_eq!(state.file_count, 100);
        assert_eq!(state.chunk_count, 500);
        assert_eq!(state.symbol_count, 200);
        assert!(state.is_ready);
    }
}
