//! RepoMap view for the retrieval TUI.
//!
//! Renders the repository map with:
//! - Token usage and file count status
//! - Generated dependency graph content
//! - Scroll support for large content

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::tui::app::RepoMapState;

/// RepoMap view widget.
///
/// Displays the generated repository map with PageRank rankings.
pub struct RepoMapView<'a> {
    /// RepoMap state.
    repomap: &'a RepoMapState,
}

impl<'a> RepoMapView<'a> {
    /// Create a new repomap view.
    pub fn new(repomap: &'a RepoMapState) -> Self {
        Self { repomap }
    }
}

impl Widget for RepoMapView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let scroll_info = if self.repomap.scroll_offset > 0 {
            format!(" (offset:{}) ", self.repomap.scroll_offset)
        } else {
            String::new()
        };

        let generating_indicator = if self.repomap.generating {
            " [Generating...] "
        } else {
            ""
        };

        let status = format!(
            "Tokens: {}/{} | Files: {} | {}ms{}{}",
            self.repomap.tokens,
            self.repomap.max_tokens,
            self.repomap.files,
            self.repomap.duration_ms,
            scroll_info,
            generating_indicator
        );

        let content = self
            .repomap
            .content
            .as_deref()
            .unwrap_or("Press 'g' to generate RepoMap\n\nKeyboard shortcuts:\n  g/r      Generate/refresh\n  +/-      Adjust token budget\n  Up/Down  Scroll\n  PgUp/PgDn  Scroll page\n  Home     Jump to top");

        let repomap = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" RepoMap: {} ", status)),
            )
            .scroll((self.repomap.scroll_offset as u16, 0));
        repomap.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repomap_view_creation() {
        let repomap = RepoMapState::default();
        let _view = RepoMapView::new(&repomap);
    }

    #[test]
    fn test_repomap_view_with_content() {
        let repomap = RepoMapState {
            max_tokens: 1024,
            content: Some("test content".to_string()),
            tokens: 100,
            files: 5,
            duration_ms: 50,
            generating: false,
            scroll_offset: 0,
        };
        let _view = RepoMapView::new(&repomap);
    }
}
