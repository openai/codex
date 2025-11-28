use super::HistoryCell;
use crate::diff_render::display_path_for;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::path::Path;
use std::path::PathBuf;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

/// History entry indicating an image preview was opened from a tool call.
///
/// Displays the label `Viewed Image` and the image path relative to the session root so users can
/// quickly confirm which artifact was opened. Paths wrap under a dim connector line so deep paths
/// stay aligned.
///
/// # Output
///
/// ```plain
/// • Viewed Image
///   └ /repo/images/
///     very/deep/path/to/
///     output.png
/// ```
#[derive(Debug)]
pub(crate) struct ViewImageToolCallCell {
    path: PathBuf,
    cwd: PathBuf,
}

impl ViewImageToolCallCell {
    /// Build a cell for an image opened at `path`, relativized against `cwd`.
    pub(crate) fn new(path: PathBuf, cwd: &Path) -> Self {
        Self {
            path,
            cwd: cwd.to_path_buf(),
        }
    }
}

impl HistoryCell for ViewImageToolCallCell {
    /// Render the label and relative path with a connector that wraps under the arrow.
    ///
    /// The leading bullet and label announce the action. The file path is dimmed and wrapped to the
    /// available width minus the `└ ` connector indentation, with subsequent lines padded by four
    /// spaces to keep the path visually grouped.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let display_path = display_path_for(&self.path, &self.cwd);
        let prefix = "  └ ";
        let prefix_width = UnicodeWidthStr::width(prefix);
        let wrap_width = usize::from(width).saturating_sub(prefix_width).max(1);
        let wrapped = wrap(&display_path, wrap_width);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(vec!["• ".dim(), "Viewed Image".bold()].into());
        for (idx, segment) in wrapped.into_iter().enumerate() {
            let prefix_str = if idx == 0 {
                "  └ ".dim()
            } else {
                "    ".dim()
            };
            lines.push(vec![prefix_str, segment.to_string().dim()].into());
        }

        lines
    }
}

/// Factory wrapper used by `history_cell::mod` to create view-image cells.
pub(crate) fn new_view_image_tool_call(path: PathBuf, cwd: &Path) -> ViewImageToolCallCell {
    ViewImageToolCallCell::new(path, cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn renders_relative_path() {
        let cell = ViewImageToolCallCell::new(
            PathBuf::from("/repo/images/output.png"),
            Path::new("/repo"),
        );

        let rendered = cell.display_string(80);
        assert!(rendered.contains("output.png"), "rendered: {rendered}");
    }

    #[test]
    fn snapshot_wide() {
        let cell = ViewImageToolCallCell::new(
            PathBuf::from("/repo/images/very/deep/path/to/output.png"),
            Path::new("/repo"),
        );
        assert_snapshot!(cell.display_string(80));
    }

    #[test]
    fn snapshot_narrow() {
        let cell = ViewImageToolCallCell::new(
            PathBuf::from("/repo/images/very/deep/path/to/output.png"),
            Path::new("/repo"),
        );
        assert_snapshot!(cell.display_string(24));
    }
}
