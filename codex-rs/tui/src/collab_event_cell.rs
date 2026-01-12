use codex_core::protocol::AgentStatus;
use codex_core::protocol::CollabInteractionEvent;
use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::history_cell::HistoryCell;
use crate::text_formatting::truncate_text;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

const COLLAB_PROMPT_MAX_GRAPHEMES: usize = 120;

#[derive(Debug)]
pub(crate) struct CollabInteractionCell {
    summary: Line<'static>,
    detail: Option<Line<'static>>,
}

impl CollabInteractionCell {
    fn new(summary: Line<'static>, detail: Option<Line<'static>>) -> Self {
        Self { summary, detail }
    }
}

impl HistoryCell for CollabInteractionCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let wrap_width = width.max(1) as usize;
        let mut lines = word_wrap_lines(
            std::iter::once(self.summary.clone()),
            RtOptions::new(wrap_width)
                .initial_indent("• ".dim().into())
                .subsequent_indent("  ".into()),
        );

        if let Some(detail) = &self.detail {
            let detail_lines = word_wrap_lines(
                std::iter::once(detail.clone()),
                RtOptions::new(wrap_width)
                    .initial_indent("  └ ".dim().into())
                    .subsequent_indent("    ".into()),
            );
            lines.extend(detail_lines);
        }

        lines
    }
}

fn collab_status_label(status: &AgentStatus) -> String {
    match status {
        AgentStatus::PendingInit => "pending init".to_string(),
        AgentStatus::Running => "running".to_string(),
        AgentStatus::Completed(message) => format!("completed: {message:?}"),
        AgentStatus::Errored(_) => "errored".to_string(),
        AgentStatus::Shutdown => "shutdown".to_string(),
        AgentStatus::NotFound => "not found".to_string(),
    }
}

fn collab_detail_line(label: &str, message: &str) -> Option<Line<'static>> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }

    let collapsed = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let truncated = truncate_text(&collapsed, COLLAB_PROMPT_MAX_GRAPHEMES);
    let label = format!("{label}: ");
    Some(Line::from(vec![label.dim(), truncated.into()]))
}

pub(crate) fn new_collab_interaction(event: CollabInteractionEvent) -> CollabInteractionCell {
    let (summary, detail) = match event {
        CollabInteractionEvent::AgentSpawned { new_id, prompt, .. } => {
            let summary = Line::from(vec![
                "Spawned agent".bold(),
                " ".into(),
                new_id.to_string().dim(),
            ]);
            let detail = collab_detail_line("Prompt", &prompt);
            (summary, detail)
        }
        CollabInteractionEvent::AgentInteraction {
            receiver_id,
            prompt,
            ..
        } => {
            let summary = Line::from(vec![
                "Sent to agent".bold(),
                " ".into(),
                receiver_id.to_string().dim(),
            ]);
            let detail = collab_detail_line("Message", &prompt);
            (summary, detail)
        }
        CollabInteractionEvent::WaitingBegin { receiver_id, .. } => {
            let summary = Line::from(vec![
                "Waiting on agent".bold(),
                " ".into(),
                receiver_id.to_string().dim(),
            ]);
            (summary, None)
        }
        CollabInteractionEvent::WaitingEnd {
            receiver_id,
            status,
            ..
        } => {
            let summary = Line::from(vec![
                "Wait ended for agent".bold(),
                " ".into(),
                receiver_id.to_string().dim(),
                " · ".dim(),
                collab_status_label(&status).dim(),
            ]);
            (summary, None)
        }
        CollabInteractionEvent::Close {
            receiver_id,
            status,
            ..
        } => {
            let summary = Line::from(vec![
                "Closed agent".bold(),
                " ".into(),
                receiver_id.to_string().dim(),
                " · ".dim(),
                collab_status_label(&status).dim(),
            ]);
            (summary, None)
        }
    };

    CollabInteractionCell::new(summary, detail)
}
