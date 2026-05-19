//! Proposed-plan and plan-update history cells.

use super::*;

/// Transient active-cell representation of the mutable tail of a proposed-plan stream.
///
/// The controller prepares the full styled plan lines because plan tails need the same header,
/// padding, and background treatment as committed `ProposedPlanStreamCell`s while remaining
/// preview-only during streaming.
#[derive(Debug)]
pub(crate) struct StreamingPlanTailCell {
    lines: Vec<Line<'static>>,
    is_stream_continuation: bool,
}

impl StreamingPlanTailCell {
    pub(crate) fn new(lines: Vec<Line<'static>>, is_stream_continuation: bool) -> Self {
        Self {
            lines,
            is_stream_continuation,
        }
    }
}

impl HistoryCell for StreamingPlanTailCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.lines.clone())
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
pub(crate) fn new_proposed_plan_stream(
    lines: Vec<Line<'static>>,
    is_stream_continuation: bool,
) -> ProposedPlanStreamCell {
    ProposedPlanStreamCell {
        lines,
        is_stream_continuation,
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
    lines: Vec<Line<'static>>,
    is_stream_continuation: bool,
}

impl HistoryCell for ProposedPlanCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(vec!["• ".dim(), "Proposed Plan".bold()].into());
        lines.push(Line::from(" "));

        let mut plan_lines: Vec<Line<'static>> = vec![Line::from(" ")];
        let plan_style = proposed_plan_style();
        let wrap_width = width.saturating_sub(4).max(1) as usize;
        let mut body: Vec<Line<'static>> = Vec::new();
        append_markdown_agent_with_cwd(
            &self.plan_markdown,
            Some(wrap_width),
            Some(self.cwd.as_path()),
            &mut body,
        );
        if body.is_empty() {
            body.push(Line::from("(empty)".dim().italic()));
        }
        plan_lines.extend(prefix_lines(body, "  ".into(), "  ".into()));
        plan_lines.push(Line::from(" "));

        lines.extend(plan_lines.into_iter().map(|line| line.style(plan_style)));
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        raw_lines_from_source(&self.plan_markdown)
    }
}

impl HistoryCell for ProposedPlanStreamCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.lines.clone())
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
}
