//! Renders the footer row that summarizes active unified exec processes.
//!
//! The footer shows a terse count of background terminals and instructs users
//! to open `/ps` for details. It intentionally renders nothing when the area is
//! too small or there are no active processes, so it can be composed into
//! layouts without separate visibility logic.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::live_wrap::take_prefix_by_width;
use crate::render::renderable::Renderable;

/// View-model for the unified exec footer line.
pub(crate) struct UnifiedExecFooter {
    /// Cached command labels for currently running processes.
    processes: Vec<String>,
}

impl UnifiedExecFooter {
    /// Create an empty footer with no running processes.
    pub(crate) fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    /// Replace the tracked process list, returning whether the content changed.
    pub(crate) fn set_processes(&mut self, processes: Vec<String>) -> bool {
        if self.processes == processes {
            return false;
        }
        self.processes = processes;
        true
    }

    /// Report whether there are any running processes to summarize.
    pub(crate) fn is_empty(&self) -> bool {
        self.processes.is_empty()
    }

    /// Build the single-line footer text, truncated to the available width.
    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.processes.is_empty() || width < 4 {
            return Vec::new();
        }

        let count = self.processes.len();
        let plural = if count == 1 { "" } else { "s" };
        let message = format!("  {count} background terminal{plural} running · /ps to view");
        let (truncated, _, _) = take_prefix_by_width(&message, width as usize);
        vec![Line::from(truncated.dim())]
    }
}

impl Renderable for UnifiedExecFooter {
    /// Render the footer content into the provided buffer.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        Paragraph::new(self.render_lines(area.width)).render(area, buf);
    }

    /// Return the number of rows required to render the footer.
    fn desired_height(&self, width: u16) -> u16 {
        self.render_lines(width).len() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    /// Verifies empty process lists report no visible height.
    #[test]
    fn desired_height_empty() {
        let footer = UnifiedExecFooter::new();
        assert_eq!(footer.desired_height(40), 0);
    }

    /// Snapshots the footer rendering for a single active process.
    #[test]
    fn render_more_sessions() {
        let mut footer = UnifiedExecFooter::new();
        footer.set_processes(vec!["rg \"foo\" src".to_string()]);
        let width = 50;
        let height = footer.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        footer.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_more_sessions", format!("{buf:?}"));
    }

    /// Snapshots pluralized output for many active processes.
    #[test]
    fn render_many_sessions() {
        let mut footer = UnifiedExecFooter::new();
        footer.set_processes((0..123).map(|idx| format!("cmd {idx}")).collect());
        let width = 50;
        let height = footer.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        footer.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_many_sessions", format!("{buf:?}"));
    }
}
