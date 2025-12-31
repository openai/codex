//! Event log widget for displaying real-time retrieval events.
//!
//! Shows a scrollable list of events with timestamps and formatting.

use std::collections::VecDeque;

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
use ratatui::widgets::Wrap;

use crate::events::RetrievalEvent;

/// Maximum number of events to keep in the log.
const MAX_EVENTS: usize = 100;

/// Event log widget state.
#[derive(Debug, Clone)]
pub struct EventLogState {
    /// Events in the log (newest last).
    events: VecDeque<TimestampedEvent>,
    /// Scroll offset (0 = show latest).
    scroll_offset: usize,
    /// Whether this widget is focused.
    pub focused: bool,
    /// Whether to auto-scroll to new events.
    pub auto_scroll: bool,
}

/// An event with its timestamp.
#[derive(Debug, Clone)]
struct TimestampedEvent {
    /// Unix timestamp in seconds.
    timestamp: i64,
    /// The event.
    event: RetrievalEvent,
}

impl Default for EventLogState {
    fn default() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_EVENTS),
            scroll_offset: 0,
            focused: false,
            auto_scroll: true,
        }
    }
}

impl EventLogState {
    /// Create a new event log state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new event to the log.
    pub fn push(&mut self, event: RetrievalEvent) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        self.events.push_back(TimestampedEvent { timestamp, event });

        // Trim if over capacity
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }

        // Auto-scroll to bottom if enabled
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
        self.scroll_offset = 0;
    }

    /// Scroll up by n lines.
    pub fn scroll_up(&mut self, n: usize) {
        let max_scroll = self.events.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + n).min(max_scroll);
        self.auto_scroll = false;
    }

    /// Scroll down by n lines.
    pub fn scroll_down(&mut self, n: usize) {
        if n >= self.scroll_offset {
            self.scroll_offset = 0;
            self.auto_scroll = true;
        } else {
            self.scroll_offset -= n;
        }
    }

    /// Scroll to top.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.events.len().saturating_sub(1);
        self.auto_scroll = false;
    }

    /// Scroll to bottom (newest events).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    /// Toggle auto-scroll mode.
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    /// Get the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Event log widget.
pub struct EventLog<'a> {
    state: &'a EventLogState,
}

impl<'a> EventLog<'a> {
    /// Create a new event log widget.
    pub fn new(state: &'a EventLogState) -> Self {
        Self { state }
    }

    fn format_timestamp(ts: i64) -> String {
        // Format as HH:MM:SS
        let secs = ts % 86400; // seconds in day
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }

    fn event_to_line(ts: i64, event: &RetrievalEvent) -> Line<'static> {
        let time_str = Self::format_timestamp(ts);
        let (level, message) = Self::format_event(event);

        let level_style = match level {
            "INFO" => Style::default().cyan(),
            "DEBUG" => Style::default().dim(),
            "WARN" => Style::default().yellow(),
            "ERROR" => Style::default().red(),
            _ => Style::default(),
        };

        Line::from(vec![
            Span::styled(time_str, Style::default().dim()),
            Span::raw(" "),
            Span::styled(format!("[{}]", level), level_style),
            Span::raw(" "),
            Span::raw(message),
        ])
    }

    fn format_event(event: &RetrievalEvent) -> (&'static str, String) {
        match event {
            // Search events
            RetrievalEvent::SearchStarted { query, mode, .. } => {
                ("INFO", format!("Search started: \"{}\" ({})", query, mode))
            }
            RetrievalEvent::QueryPreprocessed {
                tokens,
                language,
                duration_ms,
                ..
            } => (
                "DEBUG",
                format!(
                    "Preprocessed: {} tokens, lang={} ({}ms)",
                    tokens.len(),
                    language,
                    duration_ms
                ),
            ),
            RetrievalEvent::QueryRewritten {
                original,
                rewritten,
                ..
            } => {
                if original != rewritten {
                    (
                        "DEBUG",
                        format!("Query rewritten: \"{}\" → \"{}\"", original, rewritten),
                    )
                } else {
                    ("DEBUG", format!("Query: \"{}\"", original))
                }
            }
            RetrievalEvent::Bm25SearchStarted { query_terms, .. } => (
                "DEBUG",
                format!("BM25: searching {} terms", query_terms.len()),
            ),
            RetrievalEvent::Bm25SearchCompleted {
                results,
                duration_ms,
                ..
            } => (
                "DEBUG",
                format!("BM25: {} results ({}ms)", results.len(), duration_ms),
            ),
            RetrievalEvent::VectorSearchStarted { .. } => {
                ("DEBUG", "Vector: generating embedding".to_string())
            }
            RetrievalEvent::VectorSearchCompleted {
                results,
                duration_ms,
                ..
            } => (
                "DEBUG",
                format!("Vector: {} results ({}ms)", results.len(), duration_ms),
            ),
            RetrievalEvent::SnippetSearchStarted { .. } => {
                ("DEBUG", "Snippet: searching symbols".to_string())
            }
            RetrievalEvent::SnippetSearchCompleted {
                results,
                duration_ms,
                ..
            } => (
                "DEBUG",
                format!("Snippet: {} results ({}ms)", results.len(), duration_ms),
            ),
            RetrievalEvent::FusionStarted {
                bm25_count,
                vector_count,
                snippet_count,
                ..
            } => (
                "DEBUG",
                format!(
                    "Fusion: merging {} BM25 + {} Vector + {} Snippet",
                    bm25_count, vector_count, snippet_count
                ),
            ),
            RetrievalEvent::FusionCompleted {
                merged_count,
                duration_ms,
                ..
            } => (
                "DEBUG",
                format!("Fusion: {} merged ({}ms)", merged_count, duration_ms),
            ),
            RetrievalEvent::RerankingStarted {
                backend,
                input_count,
                ..
            } => (
                "DEBUG",
                format!("Reranking: {} items with {}", input_count, backend),
            ),
            RetrievalEvent::RerankingCompleted { duration_ms, .. } => {
                ("DEBUG", format!("Reranking: completed ({}ms)", duration_ms))
            }
            RetrievalEvent::SearchCompleted {
                results,
                total_duration_ms,
                ..
            } => (
                "INFO",
                format!(
                    "Search completed: {} results ({}ms)",
                    results.len(),
                    total_duration_ms
                ),
            ),
            RetrievalEvent::SearchError { error, .. } => {
                ("ERROR", format!("Search error: {}", error))
            }

            // Index events
            RetrievalEvent::IndexBuildStarted {
                workspace,
                estimated_files,
                ..
            } => (
                "INFO",
                format!(
                    "Indexing started: {} (~{} files)",
                    workspace, estimated_files
                ),
            ),
            RetrievalEvent::IndexPhaseChanged {
                phase, progress, ..
            } => (
                "INFO",
                format!("Index phase: {} ({:.0}%)", phase, progress * 100.0),
            ),
            RetrievalEvent::IndexFileProcessed { path, chunks, .. } => {
                ("DEBUG", format!("Indexed: {} ({} chunks)", path, chunks))
            }
            RetrievalEvent::IndexBuildCompleted {
                stats, duration_ms, ..
            } => (
                "INFO",
                format!(
                    "Indexing completed: {} files, {} chunks ({}ms)",
                    stats.file_count, stats.chunk_count, duration_ms
                ),
            ),
            RetrievalEvent::IndexBuildFailed { error, .. } => {
                ("ERROR", format!("Indexing failed: {}", error))
            }

            // Watch events
            RetrievalEvent::WatchStarted { workspace, .. } => {
                ("INFO", format!("Watch started: {}", workspace))
            }
            RetrievalEvent::FileChanged { path, kind, .. } => {
                ("DEBUG", format!("File {}: {}", kind, path))
            }
            RetrievalEvent::IncrementalIndexTriggered { changed_files, .. } => (
                "INFO",
                format!("Incremental index: {} files changed", changed_files),
            ),
            RetrievalEvent::WatchStopped { workspace } => {
                ("INFO", format!("Watch stopped: {}", workspace))
            }

            // RepoMap events
            RetrievalEvent::RepoMapStarted { max_tokens, .. } => (
                "INFO",
                format!("RepoMap: generating (max {} tokens)", max_tokens),
            ),
            RetrievalEvent::PageRankComputed { iterations, .. } => {
                ("DEBUG", format!("PageRank: {} iterations", iterations))
            }
            RetrievalEvent::RepoMapGenerated {
                tokens,
                files,
                duration_ms,
                ..
            } => (
                "INFO",
                format!(
                    "RepoMap: {} tokens, {} files ({}ms)",
                    tokens, files, duration_ms
                ),
            ),
            RetrievalEvent::RepoMapCacheHit { .. } => ("DEBUG", "RepoMap: cache hit".to_string()),

            // Session events
            RetrievalEvent::SessionStarted { session_id, .. } => {
                ("INFO", format!("Session started: {}", session_id))
            }
            RetrievalEvent::SessionEnded { duration_ms, .. } => {
                ("INFO", format!("Session ended ({}ms)", duration_ms))
            }

            // Diagnostic
            RetrievalEvent::DiagnosticLog { level, message, .. } => {
                let level_str = match level {
                    crate::events::LogLevel::Error => "ERROR",
                    crate::events::LogLevel::Warn => "WARN",
                    crate::events::LogLevel::Info => "INFO",
                    crate::events::LogLevel::Debug => "DEBUG",
                    crate::events::LogLevel::Trace => "TRACE",
                };
                (level_str, message.clone())
            }
        }
    }
}

impl Widget for EventLog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.state.focused {
            Style::default().cyan()
        } else {
            Style::default().dim()
        };

        let scroll_indicator = if self.state.scroll_offset > 0 {
            format!(" (↑{}) ", self.state.scroll_offset)
        } else {
            String::new()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(
                " Events ({}) {}",
                self.state.events.len(),
                scroll_indicator
            ));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.state.events.is_empty() {
            let msg = Line::from(Span::styled(
                "No events yet",
                Style::default().dim().italic(),
            ));
            Paragraph::new(msg).render(inner, buf);
            return;
        }

        // Calculate visible range
        let visible_height = inner.height as usize;
        let total_events = self.state.events.len();

        // Events are stored oldest first, we display newest at bottom
        // scroll_offset = 0 means show the latest events
        let end_idx = total_events.saturating_sub(self.state.scroll_offset);
        let start_idx = end_idx.saturating_sub(visible_height);

        let lines: Vec<Line> = self
            .state
            .events
            .iter()
            .skip(start_idx)
            .take(end_idx - start_idx)
            .map(|te| Self::event_to_line(te.timestamp, &te.event))
            .collect();

        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::LogLevel;

    #[test]
    fn test_event_log_push() {
        let mut state = EventLogState::new();
        assert!(state.is_empty());

        state.push(RetrievalEvent::WatchStarted {
            workspace: "test".to_string(),
            paths: vec![],
        });

        assert_eq!(state.len(), 1);
        assert!(!state.is_empty());
    }

    #[test]
    fn test_event_log_max_capacity() {
        let mut state = EventLogState::new();

        for i in 0..(MAX_EVENTS + 10) {
            state.push(RetrievalEvent::DiagnosticLog {
                level: LogLevel::Info,
                module: "test".to_string(),
                message: format!("event {}", i),
                fields: Default::default(),
            });
        }

        assert_eq!(state.len(), MAX_EVENTS);
    }

    #[test]
    fn test_event_log_scroll() {
        let mut state = EventLogState::new();

        for i in 0..20 {
            state.push(RetrievalEvent::DiagnosticLog {
                level: LogLevel::Info,
                module: "test".to_string(),
                message: format!("event {}", i),
                fields: Default::default(),
            });
        }

        assert_eq!(state.scroll_offset, 0);
        assert!(state.auto_scroll);

        state.scroll_up(5);
        assert_eq!(state.scroll_offset, 5);
        assert!(!state.auto_scroll);

        state.scroll_down(3);
        assert_eq!(state.scroll_offset, 2);

        state.scroll_to_bottom();
        assert_eq!(state.scroll_offset, 0);
        assert!(state.auto_scroll);
    }

    #[test]
    fn test_format_timestamp() {
        // 10:30:45
        let ts = 10 * 3600 + 30 * 60 + 45;
        assert_eq!(EventLog::format_timestamp(ts), "10:30:45");
    }
}
