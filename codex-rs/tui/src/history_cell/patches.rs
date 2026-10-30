//! Patch summaries and image-tool transcript helpers.

use super::*;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines;
use codex_ansi_escape::ansi_escape;
use codex_utils_path_uri::LegacyAppPathString;

#[cfg(test)]
#[path = "patches_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) struct PatchHistoryCell {
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
    activity_id: String,
    filename: String,
    path_label: String,
    link: Option<TerminalHyperlink>,
}

impl ViewImageHistoryCell {
    pub(crate) fn link_local_image(&mut self) {
        let start = "Viewed image ".len();
        let end = start + crate::width::display_width(&self.path_label);
        self.link = self
            .image_path()
            .and_then(|path| TerminalHyperlink::viewed_image(start..end, path.as_path()));
    }
}

impl HistoryCell for ViewImageHistoryCell {
    fn activity_ids(&self) -> Vec<String> {
        vec![self.activity_id.clone()]
    }

    fn image_path(&self) -> Option<AbsolutePathBuf> {
        LegacyAppPathString::from_string(self.path_label.clone()).to_inferred_abs_path()
    }

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

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = plain_hyperlink_lines(self.display_lines(width));
        if let ([line], Some(link)) = (lines.as_mut_slice(), &self.link) {
            let start = crate::width::display_width("• Viewed image ");
            if start < line.width() {
                line.hyperlinks.push(link.with_columns(start..line.width()));
            }
        }
        lines
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.transcript_hyperlink_lines(width))
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        if width == 0 {
            return Vec::new();
        }
        let mut line = HyperlinkLine::new(Line::from(vec![
            "Viewed image ".bold(),
            self.path_label.clone().dim(),
        ]));
        line.hyperlinks.extend(self.link.iter().cloned());
        adaptive_wrap_hyperlink_lines(
            &[line],
            RtOptions::new(usize::from(width))
                .initial_indent("• ".dim().into())
                .subsequent_indent("  ".into()),
        )
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from(format!("Viewed image {}", self.path_label))]
    }
}

pub(crate) fn new_view_image_tool_call(
    id: String,
    path: LegacyAppPathString,
) -> ViewImageHistoryCell {
    let filename = path
        .to_inferred_path_uri()
        .and_then(|path| path.basename())
        .unwrap_or_else(|| path.render_for_ui());
    ViewImageHistoryCell {
        activity_id: format!("image-view:{id}"),
        filename,
        path_label: path.into_string(),
        link: None,
    }
}

pub(crate) fn new_image_generation_call(
    call_id: String,
    status: &str,
    revised_prompt: Option<String>,
    saved_path: Option<AbsolutePathBuf>,
) -> ImageGenerationHistoryCell {
    let detail = revised_prompt.unwrap_or(call_id);
    let heading = if status == "failed" {
        vec!["✗ ".red().bold(), "Image generation failed".bold()].into()
    } else {
        vec!["• ".dim(), "Generated Image:".bold()].into()
    };
    let mut lines: Vec<Line<'static>> = vec![heading, vec!["  └ ".dim(), detail.dim()].into()];
    if let Some(saved_path) = &saved_path {
        let saved_path = Url::from_file_path(saved_path.as_path())
            .map(|url| url.to_string())
            .unwrap_or_else(|_| saved_path.display().to_string());
        lines.push(vec!["  └ ".dim(), "Saved to: ".dim(), saved_path.into()].into());
    }

    ImageGenerationHistoryCell {
        lines,
        saved_path: saved_path.filter(|_| status != "failed"),
    }
}

#[derive(Debug)]
pub(crate) struct ImageGenerationHistoryCell {
    lines: Vec<Line<'static>>,
    saved_path: Option<AbsolutePathBuf>,
}

impl HistoryCell for ImageGenerationHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.lines.clone())
    }

    fn image_path(&self) -> Option<AbsolutePathBuf> {
        self.saved_path.clone()
    }
}
