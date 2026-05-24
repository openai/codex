use std::path::Path;

use codex_app_server_protocol::ReviewStoryAnchor;
use codex_app_server_protocol::ReviewStoryStep;
use codex_app_server_protocol::ReviewStoryStepReadiness;
use codex_app_server_protocol::ReviewTarget;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;

use super::ReviewStoryOverlay;
use crate::diff_render::render_story_anchor_diff;
use crate::key_hint::KeyBinding;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextKind {
    Normal,
    Error,
}

impl ReviewStoryOverlay {
    pub(super) fn selected_step(&self) -> Option<&ReviewStoryStep> {
        self.snapshot.steps.get(self.selected_step)
    }

    pub(super) fn selected_anchors(&self) -> Vec<ReviewStoryAnchor> {
        let Some(step) = self.selected_step() else {
            return Vec::new();
        };
        step.anchor_ids
            .iter()
            .filter_map(|id| {
                self.snapshot
                    .anchors
                    .iter()
                    .find(|anchor| &anchor.anchor_id == id)
            })
            .cloned()
            .collect()
    }

    pub(super) fn details_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(step) = self.selected_step() else {
            return vec!["No story steps.".dim().into()];
        };
        let mut lines = Vec::new();
        if let Some(error) = &step.error {
            push_text(&mut lines, "ERROR", error, width, TextKind::Error);
        }
        push_text(&mut lines, "GOAL", &step.goal, width, TextKind::Normal);
        push_text(&mut lines, "WHAT", &step.summary, width, TextKind::Normal);
        push_text(
            &mut lines,
            "WHY HERE",
            &step.dependency_rationale,
            width,
            TextKind::Normal,
        );
        if !step.review_focus.is_empty() {
            lines.push("FOCUS".bold().into());
            for focus in &step.review_focus {
                for wrapped in textwrap::wrap(focus, width.saturating_sub(2).max(1) as usize) {
                    lines.push(vec!["- ".dim(), wrapped.into_owned().into()].into());
                }
            }
        }
        lines
    }

    pub(super) fn overview_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec![self.snapshot.title.clone().bold().into(), Line::from("")];
        push_text(
            &mut lines,
            "OVERVIEW",
            &self.snapshot.overview,
            width,
            TextKind::Normal,
        );
        lines.push(Line::from(""));
        lines.push(format!("Source: {}", source_label(&self.snapshot.target)).into());
        lines.push(
            format!(
                "Files: {}  Steps: {}",
                self.snapshot.anchors.len(),
                self.snapshot.steps.len()
            )
            .into(),
        );
        lines.push(
            format!("Fingerprint: {}", self.snapshot.source_fingerprint)
                .dim()
                .into(),
        );
        lines
    }

    pub(super) fn contents_lines(&self) -> Vec<Line<'static>> {
        self.snapshot
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let marker = if index == self.selected_step {
                    ">".cyan().bold()
                } else {
                    " ".into()
                };
                vec![
                    marker,
                    format!(" {}. ", step.index).dim(),
                    step.title.clone().into(),
                ]
                .into()
            })
            .collect()
    }
}

pub(super) fn adjust_scroll(scroll: &mut u16, amount: i32) {
    if amount.is_negative() {
        *scroll = scroll.saturating_sub(amount.unsigned_abs() as u16);
    } else {
        *scroll = scroll.saturating_add(amount as u16);
    }
}

pub(super) fn clamped_scroll(scroll: u16, line_count: usize, height: u16) -> u16 {
    scroll.min(line_count.saturating_sub(height as usize) as u16)
}

pub(super) fn pane_block(title: &str, focused: bool) -> Block<'_> {
    let style = if focused {
        Style::default().cyan()
    } else {
        Style::default().dim()
    };
    Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
        .border_style(style)
}

pub(super) fn source_label(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::BaseBranch { branch } => format!("base: {branch}...HEAD"),
        ReviewTarget::UncommittedChanges => "uncommitted changes".to_string(),
        ReviewTarget::Commit { sha, .. } => {
            format!("commit {}", sha.chars().take(8).collect::<String>())
        }
        ReviewTarget::Custom { .. } => "custom source".to_string(),
    }
}

pub(super) fn step_state_label(step: &ReviewStoryStep) -> ratatui::text::Span<'static> {
    if step.error.is_some() || step.readiness == ReviewStoryStepReadiness::Failed {
        " !".red()
    } else {
        match step.readiness {
            ReviewStoryStepReadiness::Outline => " o".dim(),
            ReviewStoryStepReadiness::Enriching => " ~".cyan(),
            ReviewStoryStepReadiness::Ready => "".into(),
            ReviewStoryStepReadiness::Failed => " !".red(),
        }
    }
}

fn push_text(lines: &mut Vec<Line<'static>>, label: &str, text: &str, width: u16, kind: TextKind) {
    if text.trim().is_empty() {
        return;
    }
    let label = label.to_string();
    lines.push(if kind == TextKind::Error {
        label.red().bold().into()
    } else {
        label.bold().into()
    });
    lines.extend(
        textwrap::wrap(text, width.max(1) as usize)
            .into_iter()
            .map(|line| Line::from(line.into_owned())),
    );
    lines.push(Line::from(""));
}

pub(super) fn diff_lines(
    anchors: &[ReviewStoryAnchor],
    width: u16,
) -> (Vec<Line<'static>>, Vec<usize>) {
    if anchors.is_empty() {
        return (vec!["(no diff)".dim().into()], Vec::new());
    }
    let mut lines = Vec::new();
    let mut offsets = Vec::new();
    for anchor in anchors {
        offsets.push(lines.len());
        lines.push(
            vec![
                anchor.file_path.clone().bold(),
                "  ".into(),
                format!("{:?}", anchor.change_kind).to_lowercase().dim(),
            ]
            .into(),
        );
        if !anchor.summary.is_empty() {
            lines.push(anchor.summary.clone().dim().into());
        }
        let rendered =
            render_story_anchor_diff(&anchor.diff, Path::new(&anchor.file_path), width as usize);
        if rendered.is_empty() {
            lines.extend(
                anchor
                    .diff
                    .lines()
                    .map(|line| line.to_string().dim().into()),
            );
        } else {
            lines.extend(rendered);
        }
        lines.push(Line::from(""));
    }
    (lines, offsets)
}

pub(super) fn help_lines() -> Vec<Line<'static>> {
    vec![
        "n / p        next / previous step".into(),
        "tab          change focused pane".into(),
        "up / down    select or scroll focused pane".into(),
        "[ / ]        previous / next file in Diff".into(),
        "o            story overview".into(),
        "t            contents (narrow layout)".into(),
        "q / esc      close".into(),
    ]
}

pub(super) fn primary_label(bindings: &[KeyBinding], fallback: &str) -> String {
    bindings
        .first()
        .map(KeyBinding::display_label)
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}
