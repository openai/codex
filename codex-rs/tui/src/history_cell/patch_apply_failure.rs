use super::HistoryCell;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::OutputLinesParams;
use crate::exec_cell::TOOL_CALL_MAX_LINES;
use crate::exec_cell::output_lines;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Patch application failure banner with truncated stderr.
///
/// Displays a magenta failure heading and the tail of stderr with tree prefixes so users can see
/// the error cause without overwhelming the history pane.
///
/// # Output
///
/// ```plain
/// ✘ Failed to apply patch
///   └ line one
///     line two
/// ```
#[derive(Debug)]
pub(crate) struct PatchApplyFailureCell {
    stderr: String,
}

impl PatchApplyFailureCell {
    pub(crate) fn new(stderr: String) -> Self {
        Self { stderr }
    }
}

impl HistoryCell for PatchApplyFailureCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from("✘ Failed to apply patch".magenta().bold()));

        if !self.stderr.trim().is_empty() {
            let output = output_lines(
                Some(&CommandOutput {
                    exit_code: 1,
                    formatted_output: String::new(),
                    aggregated_output: self.stderr.clone(),
                }),
                OutputLinesParams {
                    line_limit: TOOL_CALL_MAX_LINES,
                    only_err: true,
                    include_angle_pipe: true,
                    include_prefix: true,
                },
            );
            lines.extend(output.lines);
        }

        lines
    }
}

pub(crate) fn new_patch_apply_failure(stderr: String) -> PatchApplyFailureCell {
    PatchApplyFailureCell::new(stderr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn renders_heading_and_stderr_tail() {
        let stderr = "line one\nline two\nline three\nline four\nline five\nline six";
        let cell = PatchApplyFailureCell::new(stderr.into());

        let rendered = cell.display_string(80);
        assert!(rendered.starts_with("✘ Failed to apply patch"));
        assert!(rendered.contains("└ line one"));
        assert!(rendered.contains("line six"));
    }

    #[test]
    fn snapshot_wide() {
        let stderr = "line one\nline two\nline three\nline four\nline five\nline six";
        let cell = PatchApplyFailureCell::new(stderr.into());

        assert_snapshot!(cell.display_string(80));
    }

    #[test]
    fn snapshot_narrow() {
        let stderr = "line one\nline two\nline three\nline four\nline five\nline six";
        let cell = PatchApplyFailureCell::new(stderr.into());

        assert_snapshot!(cell.display_string(24));
    }
}
