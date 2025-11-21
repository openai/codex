use super::HistoryCell;
use crate::render::line_utils::prefix_lines;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

/// Checkbox-style rendering of plan updates pushed from the plan tool.
///
/// Displays an optional italic note followed by each step with a checkbox indicating pending,
/// in-progress, or completed status, wrapping lines with consistent indentation.
///
/// # Output
///
/// ```plain
/// • Updated Plan
///   └ ✔ done step
///     □ pending step
/// ```
#[derive(Debug)]
pub(crate) struct PlanUpdateCell {
    explanation: Option<String>,
    plan: Vec<PlanItemArg>,
}

impl PlanUpdateCell {
    pub(crate) fn new(update: UpdatePlanArgs) -> Self {
        let UpdatePlanArgs { explanation, plan } = update;
        Self { explanation, plan }
    }
}

pub(crate) fn new_plan_update(update: UpdatePlanArgs) -> PlanUpdateCell {
    PlanUpdateCell::new(update)
}

impl HistoryCell for PlanUpdateCell {
    /// Render the plan title plus wrapped note and checkbox steps.
    ///
    /// Emits a bold "Updated Plan" header, then indents an optional dim/italic note. Each plan item
    /// wraps under a checkbox (`✔` crossed out/dim for completed, cyan `□` for in-progress, dim `□`
    /// for pending) with hanging indent so wrapped lines align under the step text.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let render_note = |text: &str| -> Vec<Line<'static>> {
            let wrap_width = width.saturating_sub(4).max(1) as usize;
            textwrap::wrap(text, wrap_width)
                .into_iter()
                .map(|s| s.to_string().dim().italic().into())
                .collect()
        };

        let render_step = |status: &StepStatus, text: &str| -> Vec<Line<'static>> {
            let (box_str, step_style) = match status {
                StepStatus::Completed => ("✔ ", Style::default().crossed_out().dim()),
                StepStatus::InProgress => ("□ ", Style::default().cyan().bold()),
                StepStatus::Pending => ("□ ", Style::default().dim()),
            };
            let wrap_width = (width as usize)
                .saturating_sub(4)
                .saturating_sub(box_str.width())
                .max(1);
            let parts = textwrap::wrap(text, wrap_width);
            let step_text = parts
                .into_iter()
                .map(|s| s.to_string().set_style(step_style).into())
                .collect();
            prefix_lines(step_text, box_str.into(), "  ".into())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::plan_tool::StepStatus;
    use insta::assert_snapshot;

    #[test]
    fn renders_checkbox_steps_and_note() {
        let update = UpdatePlanArgs {
            explanation: Some("Wraps the note provided by the plan tool".into()),
            plan: vec![
                PlanItemArg {
                    step: "Investigate errors".into(),
                    status: StepStatus::Completed,
                },
                PlanItemArg {
                    step: "Add retries to client".into(),
                    status: StepStatus::InProgress,
                },
            ],
        };

        let cell = PlanUpdateCell::new(update);
        let rendered = cell.display_string(32);

        assert_snapshot!(rendered);
    }

    #[test]
    fn shows_placeholder_when_no_steps() {
        let cell = PlanUpdateCell::new(UpdatePlanArgs {
            explanation: None,
            plan: Vec::new(),
        });

        let rendered = cell.display_string(40);
        assert_snapshot!(rendered);
    }
}
