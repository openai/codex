//! Search pipeline visualization widget.
//!
//! Shows the progress of search through various stages with timing and counts.

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

use crate::tui::app::SearchPipelineState;
use crate::tui::app::SearchStage;

/// Search pipeline visualization widget.
pub struct SearchPipeline<'a> {
    state: &'a SearchPipelineState,
}

impl<'a> SearchPipeline<'a> {
    /// Create a new search pipeline widget.
    pub fn new(state: &'a SearchPipelineState) -> Self {
        Self { state }
    }

    fn stage_icon(current: SearchStage, target: SearchStage) -> &'static str {
        if current == SearchStage::Error {
            if target == current { "✗" } else { "○" }
        } else if current == SearchStage::Idle {
            "○"
        } else if current > target {
            "✓"
        } else if current == target {
            "●"
        } else {
            "○"
        }
    }

    fn stage_style(current: SearchStage, target: SearchStage) -> Style {
        if current == SearchStage::Error && target == current {
            Style::default().red()
        } else if current == SearchStage::Idle {
            Style::default().dim()
        } else if current > target {
            Style::default().green()
        } else if current == target {
            Style::default().cyan().bold()
        } else {
            Style::default().dim()
        }
    }

    fn format_duration(ms: Option<i64>) -> String {
        match ms {
            Some(d) => format!("{}ms", d),
            None => String::new(),
        }
    }

    fn format_count(count: Option<i32>, label: &str) -> String {
        match count {
            Some(c) => format!("{} {}", c, label),
            None => String::new(),
        }
    }
}

impl Widget for SearchPipeline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Search Pipeline ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 {
            return;
        }

        let current = self.state.stage;
        let mut lines = Vec::new();

        // Stage 1: Preprocess
        let preprocess_info = Self::format_duration(self.state.preprocess_duration_ms);
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "  {} ",
                    Self::stage_icon(current, SearchStage::Preprocessing)
                ),
                Self::stage_style(current, SearchStage::Preprocessing),
            ),
            Span::styled(
                "Preprocess",
                Self::stage_style(current, SearchStage::Preprocessing),
            ),
            Span::raw("  "),
            Span::styled(preprocess_info, Style::default().dim()),
        ]));

        // Stage 2: Query Rewrite
        let rewrite_info = if let Some(ref rewritten) = self.state.rewritten_query {
            if self.state.original_query.as_ref() != Some(rewritten) {
                format!(
                    "\"{}\" → \"{}\" {}",
                    self.state.original_query.as_deref().unwrap_or(""),
                    truncate_str(rewritten, 20),
                    Self::format_duration(self.state.rewrite_duration_ms)
                )
            } else {
                Self::format_duration(self.state.rewrite_duration_ms)
            }
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "  {} ",
                    Self::stage_icon(current, SearchStage::QueryRewriting)
                ),
                Self::stage_style(current, SearchStage::QueryRewriting),
            ),
            Span::styled(
                "Query Rewrite",
                Self::stage_style(current, SearchStage::QueryRewriting),
            ),
            Span::raw("  "),
            Span::styled(rewrite_info, Style::default().dim()),
        ]));

        // Stage 3: BM25
        let bm25_info = format!(
            "{}  {}",
            Self::format_count(self.state.bm25_count, "results"),
            Self::format_duration(self.state.bm25_duration_ms)
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", Self::stage_icon(current, SearchStage::Bm25Search)),
                Self::stage_style(current, SearchStage::Bm25Search),
            ),
            Span::styled("BM25", Self::stage_style(current, SearchStage::Bm25Search)),
            Span::raw("  "),
            Span::styled(bm25_info.trim().to_string(), Style::default().dim()),
        ]));

        // Stage 4: Vector
        let vector_info = format!(
            "{}  {}",
            Self::format_count(self.state.vector_count, "results"),
            Self::format_duration(self.state.vector_duration_ms)
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "  {} ",
                    Self::stage_icon(current, SearchStage::VectorSearch)
                ),
                Self::stage_style(current, SearchStage::VectorSearch),
            ),
            Span::styled(
                "Vector",
                Self::stage_style(current, SearchStage::VectorSearch),
            ),
            Span::raw("  "),
            Span::styled(vector_info.trim().to_string(), Style::default().dim()),
        ]));

        // Stage 5: Snippet
        let snippet_info = format!(
            "{}  {}",
            Self::format_count(self.state.snippet_count, "results"),
            Self::format_duration(self.state.snippet_duration_ms)
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "  {} ",
                    Self::stage_icon(current, SearchStage::SnippetSearch)
                ),
                Self::stage_style(current, SearchStage::SnippetSearch),
            ),
            Span::styled(
                "Snippet",
                Self::stage_style(current, SearchStage::SnippetSearch),
            ),
            Span::raw("  "),
            Span::styled(snippet_info.trim().to_string(), Style::default().dim()),
        ]));

        // Stage 6: Fusion
        let fusion_info = format!(
            "{}  {}",
            Self::format_count(self.state.fusion_count, "merged"),
            Self::format_duration(self.state.fusion_duration_ms)
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", Self::stage_icon(current, SearchStage::Fusion)),
                Self::stage_style(current, SearchStage::Fusion),
            ),
            Span::styled("Fusion", Self::stage_style(current, SearchStage::Fusion)),
            Span::raw("  "),
            Span::styled(fusion_info.trim().to_string(), Style::default().dim()),
        ]));

        // Stage 7: Reranking
        let rerank_info = Self::format_duration(self.state.rerank_duration_ms);
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", Self::stage_icon(current, SearchStage::Reranking)),
                Self::stage_style(current, SearchStage::Reranking),
            ),
            Span::styled(
                "Reranking",
                Self::stage_style(current, SearchStage::Reranking),
            ),
            Span::raw("  "),
            Span::styled(rerank_info, Style::default().dim()),
        ]));

        // Total duration / Error
        if let Some(ref error) = self.state.error {
            lines.push(Line::from(vec![
                Span::styled("  ✗ ", Style::default().red()),
                Span::styled(error.clone(), Style::default().red()),
            ]));
        } else if let Some(total) = self.state.total_duration_ms {
            lines.push(Line::from(vec![
                Span::styled("  Total: ", Style::default().dim()),
                Span::styled(format!("{}ms", total), Style::default().bold()),
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

/// Truncate a string to max length, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

// Implement comparison for SearchStage to determine completion
impl PartialOrd for SearchStage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchStage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order().cmp(&other.order())
    }
}

impl SearchStage {
    fn order(&self) -> i32 {
        match self {
            SearchStage::Idle => 0,
            SearchStage::Preprocessing => 1,
            SearchStage::QueryRewriting => 2,
            SearchStage::Bm25Search => 3,
            SearchStage::VectorSearch => 4,
            SearchStage::SnippetSearch => 5,
            SearchStage::Fusion => 6,
            SearchStage::Reranking => 7,
            SearchStage::Complete => 8,
            SearchStage::Error => 9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_ordering() {
        assert!(SearchStage::Preprocessing < SearchStage::Bm25Search);
        assert!(SearchStage::Complete > SearchStage::Reranking);
        assert!(SearchStage::Idle < SearchStage::Preprocessing);
    }

    #[test]
    fn test_stage_icon() {
        // Completed stage
        assert_eq!(
            SearchPipeline::stage_icon(SearchStage::Bm25Search, SearchStage::Preprocessing),
            "✓"
        );
        // Current stage
        assert_eq!(
            SearchPipeline::stage_icon(SearchStage::Bm25Search, SearchStage::Bm25Search),
            "●"
        );
        // Future stage
        assert_eq!(
            SearchPipeline::stage_icon(SearchStage::Bm25Search, SearchStage::Fusion),
            "○"
        );
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        // max_len=10, so we take first 7 chars ("a very ") and add "..."
        assert_eq!(truncate_str("a very long string", 10), "a very ...");
    }
}
