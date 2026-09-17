//! Patch summaries and image-tool transcript helpers.

use super::*;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use codex_utils_path_uri::LegacyAppPathString;

#[cfg(test)]
#[path = "patches_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) struct PatchHistoryCell {
    changes: HashMap<PathBuf, FileChange>,
    cwd: PathBuf,
}

impl HistoryCell for PatchHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        create_diff_summary(&self.changes, &self.cwd, width as usize)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(create_diff_summary(
            &self.changes,
            &self.cwd,
            RAW_DIFF_SUMMARY_WIDTH,
        ))
    }
}
/// Create a new `PendingPatch` cell that lists the file‑level summary of
/// a proposed patch. The summary lines should already be formatted (e.g.
/// "A path/to/file.rs").
pub(crate) fn new_patch_event(
    changes: HashMap<PathBuf, FileChange>,
    cwd: &Path,
) -> PatchHistoryCell {
    PatchHistoryCell {
        changes,
        cwd: cwd.to_path_buf(),
    }
}

pub(crate) fn new_patch_apply_failure(stderr: String) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Failure title
    lines.push(Line::from("✘ Failed to apply patch".magenta().bold()));

    if !stderr.trim().is_empty() {
        let output = output_lines(
            Some(&CommandOutput::new(/*exit_code*/ 1, stderr)),
            OutputLinesParams {
                line_limit: TOOL_CALL_MAX_LINES,
                only_err: true,
                include_angle_pipe: true,
                include_prefix: true,
            },
        );
        lines.extend(output.lines);
    }

    PlainHistoryCell { lines }
}

#[derive(Debug)]
pub(crate) struct ViewImageHistoryCell {
    filename: String,
    path_label: String,
}

impl HistoryCell for ViewImageHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let line = vec![
            "• ".dim(),
            "Viewed image ".bold(),
            self.filename.replace(['\n', '\r', '\t'], " ").dim(),
        ]
        .into();
        vec![truncate_line_with_ellipsis_if_overflow(
            line,
            width as usize,
        )]
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        PrefixedWrappedHistoryCell::new(
            Line::from(vec!["Viewed image ".bold(), self.path_label.clone().dim()]),
            vec!["• ".dim()],
            "  ",
        )
        .display_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from(format!("Viewed image {}", self.path_label))]
    }
}

pub(crate) fn new_view_image_tool_call(path: LegacyAppPathString) -> ViewImageHistoryCell {
    let filename = path
        .to_inferred_path_uri()
        .and_then(|path| path.basename())
        .unwrap_or_else(|| path.render_for_ui());
    ViewImageHistoryCell {
        filename,
        path_label: path.into_string(),
    }
}

pub(crate) fn new_image_generation_call(
    call_id: String,
    status: &str,
    revised_prompt: Option<String>,
    saved_path: Option<AbsolutePathBuf>,
) -> PlainHistoryCell {
    let detail = revised_prompt.unwrap_or(call_id);
    let heading = if status == "failed" {
        vec!["✗ ".red().bold(), "Image generation failed".bold()].into()
    } else {
        vec!["• ".dim(), "Generated Image:".bold()].into()
    };
    let mut lines: Vec<Line<'static>> = vec![heading, vec!["  └ ".dim(), detail.dim()].into()];
    if let Some(saved_path) = saved_path {
        let saved_path = Url::from_file_path(saved_path.as_path())
            .map(|url| url.to_string())
            .unwrap_or_else(|_| saved_path.display().to_string());
        lines.push(vec!["  └ ".dim(), "Saved to: ".dim(), saved_path.into()].into());
    }

    PlainHistoryCell { lines }
}
