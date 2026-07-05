//! Patch summaries and image-tool transcript helpers.

use super::*;
use codex_utils_path_uri::LegacyAppPathString;

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

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        match mode {
            HistoryRenderMode::Raw => {
                selection_contribution_from_display_lines(self.raw_lines(), width)
            }
            HistoryRenderMode::Rich => {
                let semantic = self
                    .raw_lines()
                    .iter()
                    .map(|line| {
                        let rendered = selection_text_from_lines(std::slice::from_ref(line));
                        rendered
                            .strip_prefix("• ")
                            .or_else(|| rendered.strip_prefix("  └ "))
                            .or_else(|| rendered.strip_prefix("    "))
                            .unwrap_or(&rendered)
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                selection_contribution_from_semantic_text(
                    semantic,
                    self.display_lines(width),
                    width,
                    /*first_row_prefix_columns*/ 0,
                )
            }
        }
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
            Some(&CommandOutput {
                exit_code: 1,
                formatted_output: String::new(),
                aggregated_output: stderr,
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

    let mut selection_lines = Vec::with_capacity(lines.len());
    let mut prefix_columns = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let (selection_line, prefix_width) = if index == 0 {
            ("Failed to apply patch".to_string(), 2)
        } else if let Some(content) = rendered
            .strip_prefix("  └ ")
            .or_else(|| rendered.strip_prefix("    "))
        {
            (content.to_string(), 4)
        } else {
            (rendered, 0)
        };
        selection_lines.push(selection_line);
        prefix_columns.push(prefix_width);
    }

    PlainHistoryCell::new(lines).with_selection_text(selection_lines.join("\n"), prefix_columns)
}

pub(crate) fn new_view_image_tool_call(path: LegacyAppPathString, cwd: &Path) -> PlainHistoryCell {
    let display_path = path
        .to_inferred_path_uri()
        .and_then(|path| path.to_abs_path().ok())
        .map(|path| display_path_for(path.as_path(), cwd))
        .unwrap_or_else(|| path.into_string());

    let selection_text = format!("Viewed Image\n{display_path}");
    let lines: Vec<Line<'static>> = vec![
        vec!["• ".dim(), "Viewed Image".bold()].into(),
        vec!["  └ ".dim(), display_path.dim()].into(),
    ];

    PlainHistoryCell::new(lines)
        .with_selection_text(selection_text, /*first_row_prefix_columns*/ vec![2, 4])
}

pub(crate) fn new_image_generation_call(
    call_id: String,
    status: &str,
    revised_prompt: Option<String>,
    saved_path: Option<AbsolutePathBuf>,
) -> PlainHistoryCell {
    let detail = revised_prompt.unwrap_or(call_id);
    let heading_text = if status == "failed" {
        "Image generation failed"
    } else {
        "Generated Image:"
    };
    let heading = if status == "failed" {
        vec!["✗ ".red().bold(), heading_text.bold()].into()
    } else {
        vec!["• ".dim(), heading_text.bold()].into()
    };
    let mut selection_lines = vec![heading_text.to_string(), detail.clone()];
    let mut lines: Vec<Line<'static>> = vec![heading, vec!["  └ ".dim(), detail.dim()].into()];
    if let Some(saved_path) = saved_path {
        let saved_path = Url::from_file_path(saved_path.as_path())
            .map(|url| url.to_string())
            .unwrap_or_else(|_| saved_path.display().to_string());
        selection_lines.push(format!("Saved to: {saved_path}"));
        lines.push(vec!["  └ ".dim(), "Saved to: ".dim(), saved_path.into()].into());
    }

    let mut prefix_columns = vec![4; lines.len()];
    if let Some(first) = prefix_columns.first_mut() {
        *first = 2;
    }
    PlainHistoryCell::new(lines).with_selection_text(selection_lines.join("\n"), prefix_columns)
}
