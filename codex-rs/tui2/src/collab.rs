use crate::history_cell::PlainHistoryCell;
use crate::render::line_utils::prefix_lines;
use crate::text_formatting::truncate_text;
use codex_core::protocol::AgentStatus;
use codex_core::protocol::CollabAgentInteractionEndEvent;
use codex_core::protocol::CollabAgentSpawnEndEvent;
use codex_core::protocol::CollabCloseEndEvent;
use codex_core::protocol::CollabWaitingBeginEvent;
use codex_core::protocol::CollabWaitingEndEvent;
use ratatui::style::Stylize;
use ratatui::text::Line;

const COLLAB_PROMPT_PREVIEW_GRAPHEMES: usize = 160;

pub(crate) fn spawn_end(ev: CollabAgentSpawnEndEvent) -> PlainHistoryCell {
    let CollabAgentSpawnEndEvent {
        call_id,
        sender_thread_id,
        new_thread_id,
        prompt,
        status,
    } = ev;
    let new_agent = new_thread_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());
    let mut details = vec![
        detail_line("call", call_id),
        detail_line("sender", sender_thread_id),
        detail_line("new_agent", new_agent),
        status_line(&status),
    ];
    if let Some(line) = prompt_line(&prompt) {
        details.push(line);
    }
    collab_event("Collab spawn", details)
}

pub(crate) fn interaction_end(ev: CollabAgentInteractionEndEvent) -> PlainHistoryCell {
    let CollabAgentInteractionEndEvent {
        call_id,
        sender_thread_id,
        receiver_thread_id,
        prompt,
        status,
    } = ev;
    let mut details = vec![
        detail_line("call", call_id),
        detail_line("sender", sender_thread_id),
        detail_line("receiver", receiver_thread_id),
        status_line(&status),
    ];
    if let Some(line) = prompt_line(&prompt) {
        details.push(line);
    }
    collab_event("Collab send input", details)
}

pub(crate) fn waiting_begin(ev: CollabWaitingBeginEvent) -> PlainHistoryCell {
    let CollabWaitingBeginEvent {
        call_id,
        sender_thread_id,
        receiver_thread_ids,
    } = ev;
    let details = vec![
        detail_line("call", call_id),
        detail_line("sender", sender_thread_id),
        detail_line("receiver", format!("{receiver_thread_ids:?}")),
    ];
    collab_event("Collab wait begin", details)
}

pub(crate) fn waiting_end(ev: CollabWaitingEndEvent) -> PlainHistoryCell {
    let CollabWaitingEndEvent {
        call_id,
        sender_thread_id,
        statuses,
    } = ev;
    let details = vec![
        detail_line("call", call_id),
        detail_line("sender", sender_thread_id),
        detail_line("statuses", format!("{statuses:#?}")),
    ];
    collab_event("Collab wait end", details)
}

pub(crate) fn close_end(ev: CollabCloseEndEvent) -> PlainHistoryCell {
    let CollabCloseEndEvent {
        call_id,
        sender_thread_id,
        receiver_thread_id,
        status,
    } = ev;
    let details = vec![
        detail_line("call", call_id),
        detail_line("sender", sender_thread_id),
        detail_line("receiver", receiver_thread_id),
        status_line(&status),
    ];
    collab_event("Collab close", details)
}

fn collab_event(title: impl Into<String>, details: Vec<Line<'static>>) -> PlainHistoryCell {
    let title = title.into();
    let mut lines: Vec<Line<'static>> = vec![vec!["• ".dim(), title.bold()].into()];
    if !details.is_empty() {
        lines.extend(prefix_lines(details, "  └ ".dim(), "    ".into()));
    }
    PlainHistoryCell::new(lines)
}

fn detail_line(label: &str, value: impl std::fmt::Display) -> Line<'static> {
    Line::from(format!("{label}: {value}").dim())
}

fn status_line(status: &AgentStatus) -> Line<'static> {
    Line::from(format!("status: {}", status_text(status)).dim())
}

fn status_text(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::PendingInit => "pending_init",
        AgentStatus::Running => "running",
        AgentStatus::Completed(_) => "completed",
        AgentStatus::Errored(_) => "errored",
        AgentStatus::Shutdown => "shutdown",
        AgentStatus::NotFound => "not_found",
    }
}

fn prompt_line(prompt: &str) -> Option<Line<'static>> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(detail_line(
            "prompt",
            truncate_text(trimmed, COLLAB_PROMPT_PREVIEW_GRAPHEMES),
        ))
    }
}
