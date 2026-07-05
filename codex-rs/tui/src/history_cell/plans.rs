//! Proposed-plan and plan-update history cells.

use super::*;
use crate::conversation_selection::CellSelectionProjection;
use crate::conversation_selection::CellSelectionProjectionPart;
use std::ops::Range;

/// Transient active-cell representation of the mutable tail of a proposed-plan stream.
///
/// The controller prepares the full styled plan lines because plan tails need the same header,
/// padding, and background treatment as committed `ProposedPlanStreamCell`s while remaining
/// preview-only during streaming.
#[derive(Debug)]
pub(crate) struct StreamingPlanTailCell {
    lines: Vec<HyperlinkLine>,
    is_stream_continuation: bool,
    selection_fragment: Option<StreamSelectionFragment>,
    body_line_range: Range<usize>,
}

impl StreamingPlanTailCell {
    pub(crate) fn new_source_backed(
        lines: Vec<HyperlinkLine>,
        is_stream_continuation: bool,
        selection_fragment: Option<StreamSelectionFragment>,
        body_line_range: Range<usize>,
    ) -> Self {
        Self {
            lines,
            is_stream_continuation,
            selection_fragment,
            body_line_range,
        }
    }
}

impl HistoryCell for StreamingPlanTailCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        visible_lines(self.lines.clone())
    }

    fn display_hyperlink_lines(&self, _width: u16) -> Vec<HyperlinkLine> {
        self.lines.clone()
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.selection_fragment
            .as_ref()
            .map(|fragment| raw_lines_from_source(fragment.text()))
            .unwrap_or_else(|| plain_lines(visible_lines(self.lines.clone())))
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        self.selection_fragment
            .as_ref()
            .map(|fragment| {
                stream_plan_selection_contribution(
                    &self.lines,
                    self.body_line_range.clone(),
                    fragment,
                    self.is_stream_continuation,
                    width,
                    mode,
                )
            })
            .unwrap_or(SelectionContribution::Transparent)
    }

    fn is_stream_continuation(&self) -> bool {
        self.is_stream_continuation
    }
}
/// Render a user‑friendly plan update styled like a checkbox todo list.
pub(crate) fn new_plan_update(update: UpdatePlanArgs) -> PlanUpdateCell {
    let UpdatePlanArgs { explanation, plan } = update;
    PlanUpdateCell { explanation, plan }
}

/// Create a proposed-plan cell that snapshots the session cwd for later markdown rendering.
///
/// The plan body is stored as raw markdown so terminal resize reflow can render it again at the
/// current width. Callers should use `new_proposed_plan_stream` only for transient live streaming
/// cells, then consolidate to this source-backed cell when the plan is complete.
pub(crate) fn new_proposed_plan(plan_markdown: String, cwd: &Path) -> ProposedPlanCell {
    ProposedPlanCell {
        plan_markdown,
        cwd: cwd.to_path_buf(),
    }
}

/// Create a transient proposed-plan stream cell from already rendered lines.
///
/// Stream cells are display fragments, not source-backed history. They should be replaced by
/// `ProposedPlanCell` during consolidation before relying on resize reflow for finalized history.
#[cfg(test)]
pub(crate) fn new_proposed_plan_stream(
    lines: Vec<impl Into<HyperlinkLine>>,
    is_stream_continuation: bool,
) -> ProposedPlanStreamCell {
    let lines = lines.into_iter().map(Into::into).collect::<Vec<_>>();
    let body_line_range = 0..lines.len();
    ProposedPlanStreamCell {
        lines,
        is_stream_continuation,
        selection_fragment: None,
        body_line_range,
    }
}

pub(crate) fn new_source_backed_proposed_plan_stream(
    lines: Vec<HyperlinkLine>,
    is_stream_continuation: bool,
    selection_fragment: Option<StreamSelectionFragment>,
    body_line_range: Range<usize>,
) -> ProposedPlanStreamCell {
    ProposedPlanStreamCell {
        lines,
        is_stream_continuation,
        selection_fragment,
        body_line_range,
    }
}

/// Finalized proposed-plan history that can render itself again for a new width.
///
/// This is the source-backed counterpart to `ProposedPlanStreamCell`. It owns raw markdown and the
/// session cwd needed for stable local-link rendering during later transcript reflow.
#[derive(Debug)]
pub(crate) struct ProposedPlanCell {
    plan_markdown: String,
    /// Session cwd used to keep local file-link display aligned with live streamed plan rendering.
    cwd: PathBuf,
}

/// Transient proposed-plan history emitted while a plan is still streaming.
///
/// The lines are already rendered for the stream's current width. A finalized transcript should not
/// keep these cells after consolidation, because they cannot re-render their source on a later
/// terminal resize.
#[derive(Debug)]
pub(crate) struct ProposedPlanStreamCell {
    lines: Vec<HyperlinkLine>,
    is_stream_continuation: bool,
    selection_fragment: Option<StreamSelectionFragment>,
    body_line_range: Range<usize>,
}

fn stream_plan_selection_contribution(
    lines: &[HyperlinkLine],
    body_line_range: Range<usize>,
    fragment: &StreamSelectionFragment,
    is_stream_continuation: bool,
    width: u16,
    mode: HistoryRenderMode,
) -> SelectionContribution {
    if width == 0 {
        return SelectionContribution::Transparent;
    }
    if mode == HistoryRenderMode::Raw {
        return fragment
            .projection_for_display(
                width,
                raw_lines_from_source(fragment.text()),
                width,
                /*outer_prefix_columns*/ 0,
            )
            .map(|projection| {
                if is_stream_continuation {
                    projection.with_separator_before("")
                } else {
                    projection
                }
            })
            .map(SelectionContribution::Selectable)
            .unwrap_or(SelectionContribution::Transparent);
    }

    let body_start = body_line_range.start.min(lines.len());
    let body_end = body_line_range.end.min(lines.len()).max(body_start);
    let body_width = width.saturating_sub(/*rhs*/ 4).max(/*other*/ 1);
    let body_projection = fragment.projection_for_display(
        body_width,
        visible_lines(lines[body_start..body_end].to_vec()),
        width,
        /*outer_prefix_columns*/ 2,
    );
    let display_row_count = |lines: &[HyperlinkLine]| {
        visible_lines(lines.to_vec())
            .into_iter()
            .map(|line| {
                Paragraph::new(line)
                    .wrap(Wrap { trim: false })
                    .line_count(width)
                    .max(/*other*/ 1)
            })
            .sum::<usize>()
    };
    let leading_rows = display_row_count(&lines[..body_start]);
    let trailing_rows = display_row_count(&lines[body_end..]);
    let mut parts = Vec::new();
    if !is_stream_continuation && let Some(header_line) = lines.first() {
        let header_rows = display_row_count(std::slice::from_ref(header_line));
        let header = CellSelectionProjection::from_display_lines(
            "Proposed Plan".to_string(),
            visible_lines(vec![header_line.clone()]),
            width,
            /*first_row_prefix_columns*/ 2,
        );
        parts.push(
            header
                .map(CellSelectionProjectionPart::Selectable)
                .unwrap_or(CellSelectionProjectionPart::Transparent {
                    row_count: header_rows,
                }),
        );
        let remaining_leading_rows = leading_rows.saturating_sub(header_rows);
        if remaining_leading_rows > 0 {
            parts.push(CellSelectionProjectionPart::Transparent {
                row_count: remaining_leading_rows,
            });
        }
    } else if leading_rows > 0 {
        parts.push(CellSelectionProjectionPart::Transparent {
            row_count: leading_rows,
        });
    }
    parts.push(
        body_projection
            .map(CellSelectionProjectionPart::Selectable)
            .unwrap_or(CellSelectionProjectionPart::Transparent {
                row_count: display_row_count(&lines[body_start..body_end]),
            }),
    );
    if trailing_rows > 0 {
        parts.push(CellSelectionProjectionPart::Transparent {
            row_count: trailing_rows,
        });
    }
    CellSelectionProjection::compose(
        parts, /*blank_rows_between*/ 0, /*text_separator*/ "\n\n",
    )
    .map(|projection| {
        if is_stream_continuation {
            projection.with_separator_before("")
        } else {
            projection
        }
    })
    .map(SelectionContribution::Selectable)
    .unwrap_or(SelectionContribution::Transparent)
}

impl HistoryCell for ProposedPlanCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = vec![
            HyperlinkLine::new(vec!["• ".dim(), "Proposed Plan".bold()].into()),
            HyperlinkLine::new(Line::from(" ")),
        ];

        let mut plan_lines = vec![HyperlinkLine::new(Line::from(" "))];
        let plan_style = proposed_plan_style();
        let wrap_width = width.saturating_sub(4).max(1) as usize;
        let mut body = crate::markdown::render_markdown_agent_with_links_and_cwd(
            &self.plan_markdown,
            Some(wrap_width),
            Some(self.cwd.as_path()),
        );
        if body.is_empty() {
            body.push(HyperlinkLine::new(Line::from("(empty)".dim().italic())));
        }
        plan_lines.extend(prefix_hyperlink_lines(body, "  ".into(), "  ".into()));
        plan_lines.push(HyperlinkLine::new(Line::from(" ")));

        lines.extend(plan_lines.into_iter().map(|line| line.style(plan_style)));
        lines
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        raw_lines_from_source(&self.plan_markdown)
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        match mode {
            HistoryRenderMode::Raw => {
                selection_contribution_from_display_lines(self.raw_lines(), width)
            }
            HistoryRenderMode::Rich => {
                let normalized = crate::markdown::unwrap_markdown_fences(&self.plan_markdown);
                let markdown_width = width.saturating_sub(/*rhs*/ 4).max(/*other*/ 1) as usize;
                let mut body_lines = crate::markdown::render_markdown_agent_with_links_and_cwd(
                    &self.plan_markdown,
                    Some(markdown_width),
                    Some(self.cwd.as_path()),
                );
                let body_row_count = body_lines.len().max(/*other*/ 1);
                let body_projection = if body_lines.is_empty() {
                    let lines = vec![Line::from("  (empty)")];
                    CellSelectionProjection::from_display_lines(
                        "(empty)".to_string(),
                        lines,
                        width,
                        /*first_row_prefix_columns*/ 2,
                    )
                } else {
                    body_lines = prefix_hyperlink_lines(body_lines, "  ".into(), "  ".into());
                    crate::markdown_render::render_markdown_selection_projection(
                        &normalized,
                        markdown_width,
                        Some(self.cwd.as_path()),
                        visible_lines(body_lines),
                        width,
                        /*outer_prefix_columns*/ 2,
                    )
                };
                let header = CellSelectionProjection::from_display_lines(
                    "Proposed Plan".to_string(),
                    vec![vec!["• ".dim(), "Proposed Plan".bold()].into()],
                    width,
                    /*first_row_prefix_columns*/ 2,
                );
                let parts = vec![
                    header
                        .map(CellSelectionProjectionPart::Selectable)
                        .unwrap_or(CellSelectionProjectionPart::Transparent { row_count: 1 }),
                    CellSelectionProjectionPart::Transparent { row_count: 2 },
                    body_projection
                        .map(CellSelectionProjectionPart::Selectable)
                        .unwrap_or(CellSelectionProjectionPart::Transparent {
                            row_count: body_row_count,
                        }),
                    CellSelectionProjectionPart::Transparent { row_count: 1 },
                ];
                CellSelectionProjection::compose(
                    parts, /*blank_rows_between*/ 0, /*text_separator*/ "\n\n",
                )
                .map(SelectionContribution::Selectable)
                .unwrap_or(SelectionContribution::Transparent)
            }
        }
    }
}

impl HistoryCell for ProposedPlanStreamCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        visible_lines(self.lines.clone())
    }

    fn display_hyperlink_lines(&self, _width: u16) -> Vec<HyperlinkLine> {
        self.lines.clone()
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.selection_fragment
            .as_ref()
            .map(|fragment| raw_lines_from_source(fragment.text()))
            .unwrap_or_else(|| plain_lines(visible_lines(self.lines.clone())))
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        self.selection_fragment
            .as_ref()
            .map(|fragment| {
                stream_plan_selection_contribution(
                    &self.lines,
                    self.body_line_range.clone(),
                    fragment,
                    self.is_stream_continuation,
                    width,
                    mode,
                )
            })
            .unwrap_or(SelectionContribution::Transparent)
    }

    fn is_stream_continuation(&self) -> bool {
        self.is_stream_continuation
    }
}

#[derive(Debug)]
pub(crate) struct PlanUpdateCell {
    explanation: Option<String>,
    plan: Vec<PlanItemArg>,
}

impl HistoryCell for PlanUpdateCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let render_note = |text: &str| -> Vec<Line<'static>> {
            let wrap_width = width.saturating_sub(4).max(1) as usize;
            let note = Line::from(text.to_string().dim().italic());
            let wrapped = adaptive_wrap_line(&note, RtOptions::new(wrap_width));
            let mut out = Vec::new();
            push_owned_lines(&wrapped, &mut out);
            out
        };

        let render_step = |status: &StepStatus, text: &str| -> Vec<Line<'static>> {
            let (box_str, step_style) = match status {
                StepStatus::Completed => ("✔ ", Style::default().crossed_out().dim()),
                StepStatus::InProgress => ("□ ", Style::default().cyan().bold()),
                StepStatus::Pending => ("□ ", Style::default().dim()),
            };

            let opts = RtOptions::new(width.saturating_sub(4).max(1) as usize)
                .initial_indent(box_str.into())
                .subsequent_indent("  ".into());
            let step = Line::from(text.to_string().set_style(step_style));
            let wrapped = adaptive_wrap_line(&step, opts);
            let mut out = Vec::new();
            push_owned_lines(&wrapped, &mut out);
            out
        };

        let mut lines: Vec<Line<'static>> = vec![];
        lines.push(vec!["• ".dim(), "Updated Plan".bold()].into());

        let mut indented_lines = vec![];
        let note = self
            .explanation
            .as_ref()
            .map(|s| s.trim())
            .filter(|t| !t.is_empty());
        if let Some(expl) = note {
            indented_lines.extend(render_note(expl));
        };

        if self.plan.is_empty() {
            indented_lines.push(Line::from("(no steps provided)".dim().italic()));
        } else {
            for PlanItemArg { step, status } in self.plan.iter() {
                indented_lines.extend(render_step(status, step));
            }
        }
        lines.extend(prefix_lines(indented_lines, "  └ ".dim(), "    ".into()));

        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from("Updated Plan")];
        if let Some(explanation) = self
            .explanation
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            lines.extend(raw_lines_from_source(explanation));
        }
        if self.plan.is_empty() {
            lines.push(Line::from("(no steps provided)"));
        } else {
            for PlanItemArg { step, status } in &self.plan {
                lines.push(Line::from(format!("{status:?}: {step}")));
            }
        }
        lines
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        match mode {
            HistoryRenderMode::Raw => {
                selection_contribution_from_display_lines(self.raw_lines(), width)
            }
            HistoryRenderMode::Rich => {
                let mut lines = vec!["Updated Plan".to_string()];
                if let Some(explanation) = self
                    .explanation
                    .as_ref()
                    .map(|explanation| explanation.trim())
                    .filter(|explanation| !explanation.is_empty())
                {
                    lines.push(explanation.to_string());
                }
                if self.plan.is_empty() {
                    lines.push("(no steps provided)".to_string());
                } else {
                    lines.extend(self.plan.iter().map(|item| item.step.clone()));
                }
                let display_lines = self.display_lines(width);
                let prefix_columns = display_lines
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        if index == 0 {
                            return 2;
                        }
                        let rendered = line
                            .spans
                            .iter()
                            .map(|span| span.content.as_ref())
                            .collect::<String>();
                        let after_outer_prefix = rendered
                            .strip_prefix("  └ ")
                            .or_else(|| rendered.strip_prefix("    "));
                        match after_outer_prefix {
                            Some(rest)
                                if rest.starts_with("✔ ")
                                    || rest.starts_with("□ ")
                                    || rest.starts_with("  ") =>
                            {
                                6
                            }
                            Some(_) => 4,
                            None => 0,
                        }
                    })
                    .collect::<Vec<_>>();
                selection_contribution_from_semantic_rows(
                    lines.join("\n"),
                    display_lines,
                    width,
                    &prefix_columns,
                )
            }
        }
    }
}
