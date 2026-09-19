//! Patch summaries and image-tool transcript helpers.

use super::*;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use codex_ansi_escape::ansi_escape;
use codex_utils_path_uri::LegacyAppPathString;

#[cfg(test)]
#[path = "patches_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) struct PatchHistoryCell {
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    activity_id: String,
    changes: HashMap<PathBuf, FileChange>,
    cwd: PathBuf,
}

impl PatchHistoryCell {
    pub(crate) fn with_activity_id(mut self, id: String) -> Self {
        self.activity_id = format!("patch:{id}");
        self
    }
}

impl HistoryCell for PatchHistoryCell {
    fn activity_ids(&self) -> Vec<String> {
        vec![self.activity_id.clone()]
    }

    fn compact_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        crate::diff_render::create_diff_preview_with_links(
            &self.changes,
            &self.cwd,
            usize::from(width),
            super::activity_preview::DETAIL_PREVIEW_LINES,
        )
    }

    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        crate::diff_render::create_diff_summary_with_links(
            &self.changes,
            &self.cwd,
            usize::from(width),
        )
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
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
        activity_id: format!("patch:{}", uuid::Uuid::new_v4()),
        changes,
        cwd: cwd.to_path_buf(),
    }
}

pub(crate) fn new_patch_apply_failure(stderr: String) -> PatchFailureCell {
    PatchFailureCell {
        activity_id: format!("patch-failure:{}", uuid::Uuid::new_v4()),
        stderr,
    }
}

/// Failed patch attempts retain available diagnostics for local disclosure and full transcript.
#[derive(Debug)]
pub(crate) struct PatchFailureCell {
    #[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
    activity_id: String,
    stderr: String,
}

impl HistoryCell for PatchFailureCell {
    fn activity_ids(&self) -> Vec<String> {
        vec![self.activity_id.clone()]
    }

    fn compact_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = self.transcript_hyperlink_lines(width);
        lines.truncate(1 + super::activity_preview::DETAIL_PREVIEW_LINES);
        lines
            .into_iter()
            .map(|line| super::activity_preview::clipped_line(line.line, width))
            .collect()
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = vec![Line::from("✘ Failed to apply patch".magenta().bold()).into()];
        let error = if self.stderr.trim().is_empty() {
            "(error details unavailable)"
        } else {
            &self.stderr
        };
        let mut diagnostics = ansi_escape(&error.replace('\t', "    ")).lines;
        for line in &mut diagnostics {
            for span in &mut line.spans {
                span.style = span.style.add_modifier(Modifier::DIM);
            }
        }
        lines.extend(crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines(
            &plain_hyperlink_lines(diagnostics),
            RtOptions::new(usize::from(width).max(/*other*/ 1))
                .initial_indent("  └ ".dim().into())
                .subsequent_indent("    ".into()),
        ));
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from("Failed to apply patch")];
        lines.extend(plain_lines(ansi_escape(&self.stderr).lines));
        lines
    }

    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Failure title
        lines.push(Line::from("✘ Failed to apply patch".magenta().bold()));

        if !self.stderr.trim().is_empty() {
            let output = output_lines(
                Some(&CommandOutput::new(
                    /*exit_code*/ 1,
                    self.stderr.clone(),
                )),
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
