use crate::history_cell::PlainHistoryCell;
use crate::render::line_utils::prefix_lines;
use crate::render::model::RenderColor;
use crate::render::model::RenderLine as Line;
use crate::render::model::RenderLine;
use crate::render::model::RenderStyle;
use crate::render::model::RenderStylize;
use crate::text_formatting::truncate_text;
use codex_core::protocol::AgentStatus;
use codex_core::protocol::CollabAgentInteractionEndEvent;
use codex_core::protocol::CollabAgentSpawnEndEvent;
use codex_core::protocol::CollabCloseEndEvent;
use codex_core::protocol::CollabWaitingBeginEvent;
use codex_core::protocol::CollabWaitingEndEvent;
use codex_protocol::ThreadId;
use std::collections::HashMap;

const COLLAB_PROMPT_PREVIEW_GRAPHEMES: usize = 160;
const COLLAB_AGENT_ERROR_PREVIEW_GRAPHEMES: usize = 160;
const COLLAB_AGENT_RESPONSE_PREVIEW_GRAPHEMES: usize = 240;

#[derive(Clone, Debug)]
struct StyledText {
    text: String,
    style: RenderStyle,
}

/// Formats a spawn-end event as a collaboration history cell.
///
/// # Arguments
/// - `ev` (CollabAgentSpawnEndEvent): Event payload describing the spawned agent.
///
/// # Returns
/// - `PlainHistoryCell`: Rendered history cell for the event.
pub(crate) fn spawn_end(ev: CollabAgentSpawnEndEvent) -> PlainHistoryCell {
    let CollabAgentSpawnEndEvent {
        call_id,
        sender_thread_id: _,
        new_thread_id,
        prompt,
        status,
    } = ev;
    let new_agent = new_thread_id
        .map(|id| styled_text(id.to_string(), RenderStyle::default()))
        .unwrap_or_else(|| styled_text("not created", style_dim()));
    let mut details = vec![
        detail_line("call", styled_text(call_id, RenderStyle::default())),
        detail_line("agent", new_agent),
        status_line(&status),
    ];
    if let Some(line) = prompt_line(&prompt) {
        details.push(line);
    }
    collab_event("Agent spawned", details)
}

/// Formats an interaction-end event as a collaboration history cell.
///
/// # Arguments
/// - `ev` (CollabAgentInteractionEndEvent): Event payload describing the interaction.
///
/// # Returns
/// - `PlainHistoryCell`: Rendered history cell for the event.
pub(crate) fn interaction_end(ev: CollabAgentInteractionEndEvent) -> PlainHistoryCell {
    let CollabAgentInteractionEndEvent {
        call_id,
        sender_thread_id: _,
        receiver_thread_id,
        prompt,
        status,
    } = ev;
    let mut details = vec![
        detail_line("call", styled_text(call_id, RenderStyle::default())),
        detail_line(
            "receiver",
            styled_text(receiver_thread_id.to_string(), RenderStyle::default()),
        ),
        status_line(&status),
    ];
    if let Some(line) = prompt_line(&prompt) {
        details.push(line);
    }
    collab_event("Input sent", details)
}

/// Formats a waiting-begin event as a collaboration history cell.
///
/// # Arguments
/// - `ev` (CollabWaitingBeginEvent): Event payload describing the waiting state.
///
/// # Returns
/// - `PlainHistoryCell`: Rendered history cell for the event.
pub(crate) fn waiting_begin(ev: CollabWaitingBeginEvent) -> PlainHistoryCell {
    let CollabWaitingBeginEvent {
        call_id,
        sender_thread_id: _,
        receiver_thread_ids,
    } = ev;
    let details = vec![
        detail_line("call", styled_text(call_id, RenderStyle::default())),
        detail_line("receivers", format_thread_ids(&receiver_thread_ids)),
    ];
    collab_event("Waiting for agents", details)
}

/// Formats a waiting-end event as a collaboration history cell.
///
/// # Arguments
/// - `ev` (CollabWaitingEndEvent): Event payload describing the completion state.
///
/// # Returns
/// - `PlainHistoryCell`: Rendered history cell for the event.
pub(crate) fn waiting_end(ev: CollabWaitingEndEvent) -> PlainHistoryCell {
    let CollabWaitingEndEvent {
        call_id,
        sender_thread_id: _,
        statuses,
    } = ev;
    let mut details = vec![detail_line(
        "call",
        styled_text(call_id, RenderStyle::default()),
    )];
    details.extend(wait_complete_lines(&statuses));
    collab_event("Wait complete", details)
}

/// Formats a close-end event as a collaboration history cell.
///
/// # Arguments
/// - `ev` (CollabCloseEndEvent): Event payload describing the close result.
///
/// # Returns
/// - `PlainHistoryCell`: Rendered history cell for the event.
pub(crate) fn close_end(ev: CollabCloseEndEvent) -> PlainHistoryCell {
    let CollabCloseEndEvent {
        call_id,
        sender_thread_id: _,
        receiver_thread_id,
        status,
    } = ev;
    let details = vec![
        detail_line("call", styled_text(call_id, RenderStyle::default())),
        detail_line(
            "receiver",
            styled_text(receiver_thread_id.to_string(), RenderStyle::default()),
        ),
        status_line(&status),
    ];
    collab_event("Agent closed", details)
}

/// Builds a collaboration history cell from title and detail lines.
///
/// # Arguments
/// - `title` (impl Into<String>): Title to display.
/// - `details` (Vec<RenderLine>): Detail lines beneath the title.
///
/// # Returns
/// - `PlainHistoryCell`: Rendered history cell with prefixed details.
fn collab_event(title: impl Into<String>, details: Vec<RenderLine>) -> PlainHistoryCell {
    let title = title.into();
    let title_line = RenderLine::builder()
        .cell("• ", style_dim())
        .cell(title, style_bold())
        .build();
    let mut lines: Vec<Line> = vec![title_line];
    if !details.is_empty() {
        lines.extend(prefix_lines(
            details,
            Line::from(vec!["  └ ".dim()]),
            Line::from(vec!["    ".into()]),
        ));
    }
    PlainHistoryCell::new(lines)
}

/// Formats a label/value pair into a render line.
///
/// # Arguments
/// - `label` (&str): Label prefix for the line.
/// - `value` (StyledText): Styled value to append.
///
/// # Returns
/// - `RenderLine`: Render line with label and value.
fn detail_line(label: &str, value: StyledText) -> RenderLine {
    RenderLine::builder()
        .cell(format!("{label}: "), style_dim())
        .cell(value.text, value.style)
        .build()
}

/// Formats a label with multiple value cells into a render line.
///
/// # Arguments
/// - `label` (&str): Label prefix for the line.
/// - `value` (Vec<StyledText>): Styled value cells to append.
///
/// # Returns
/// - `RenderLine`: Render line with label and value cells.
fn detail_line_cells(label: &str, value: Vec<StyledText>) -> RenderLine {
    let mut builder = RenderLine::builder().cell(format!("{label}: "), style_dim());
    for cell in value {
        builder = builder.cell(cell.text, cell.style);
    }
    builder.build()
}

/// Formats agent status into a detail line.
///
/// # Arguments
/// - `status` (&AgentStatus): Agent status to render.
///
/// # Returns
/// - `RenderLine`: Render line describing the status.
fn status_line(status: &AgentStatus) -> RenderLine {
    detail_line("status", status_text(status))
}

/// Formats agent status into styled text.
///
/// # Arguments
/// - `status` (&AgentStatus): Agent status to render.
///
/// # Returns
/// - `StyledText`: Styled status text.
fn status_text(status: &AgentStatus) -> StyledText {
    match status {
        AgentStatus::PendingInit => styled_text("pending init", style_dim()),
        AgentStatus::Running => styled_text("running", style_cyan_bold()),
        AgentStatus::Completed(_) => styled_text("completed", style_green()),
        AgentStatus::Errored(_) => styled_text("errored", style_red()),
        AgentStatus::Shutdown => styled_text("shutdown", style_dim()),
        AgentStatus::NotFound => styled_text("not found", style_red()),
    }
}

/// Formats the prompt preview line when present.
///
/// # Arguments
/// - `prompt` (&str): Prompt text to preview.
///
/// # Returns
/// - `Option<RenderLine>`: Render line if prompt is non-empty.
fn prompt_line(prompt: &str) -> Option<RenderLine> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(detail_line(
            "prompt",
            styled_text(
                truncate_text(trimmed, COLLAB_PROMPT_PREVIEW_GRAPHEMES),
                style_dim(),
            ),
        ))
    }
}

/// Formats a list of thread IDs into styled text.
///
/// # Arguments
/// - `ids` (&[ThreadId]): Thread identifiers to format.
///
/// # Returns
/// - `StyledText`: Styled summary text of thread IDs.
fn format_thread_ids(ids: &[ThreadId]) -> StyledText {
    if ids.is_empty() {
        return styled_text("none", style_dim());
    }
    let joined = ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    styled_text(joined, RenderStyle::default())
}

/// Formats the list of completion statuses for waiting agents.
///
/// # Arguments
/// - `statuses` (&HashMap<ThreadId, AgentStatus>): Agent status map.
///
/// # Returns
/// - `Vec<RenderLine>`: Render lines describing the wait result.
fn wait_complete_lines(statuses: &HashMap<ThreadId, AgentStatus>) -> Vec<RenderLine> {
    if statuses.is_empty() {
        return vec![detail_line("agents", styled_text("none", style_dim()))];
    }

    let mut pending_init = 0usize;
    let mut running = 0usize;
    let mut completed = 0usize;
    let mut errored = 0usize;
    let mut shutdown = 0usize;
    let mut not_found = 0usize;
    for status in statuses.values() {
        match status {
            AgentStatus::PendingInit => pending_init += 1,
            AgentStatus::Running => running += 1,
            AgentStatus::Completed(_) => completed += 1,
            AgentStatus::Errored(_) => errored += 1,
            AgentStatus::Shutdown => shutdown += 1,
            AgentStatus::NotFound => not_found += 1,
        }
    }

    let mut summary = vec![styled_text(
        format!("{} total", statuses.len()),
        style_dim(),
    )];
    push_status_count(&mut summary, pending_init, "pending init", style_dim());
    push_status_count(&mut summary, running, "running", style_cyan_bold());
    push_status_count(&mut summary, completed, "completed", style_green());
    push_status_count(&mut summary, errored, "errored", style_red());
    push_status_count(&mut summary, shutdown, "shutdown", style_dim());
    push_status_count(&mut summary, not_found, "not found", style_red());

    let mut entries: Vec<(String, &AgentStatus)> = statuses
        .iter()
        .map(|(thread_id, status)| (thread_id.to_string(), status))
        .collect();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut lines = Vec::with_capacity(entries.len() + 1);
    lines.push(detail_line_cells("agents", summary));
    lines.extend(entries.into_iter().map(|(thread_id, status)| {
        let mut builder = RenderLine::builder()
            .cell(thread_id, style_dim())
            .cell(" ", style_dim());
        let status_value = status_text(status);
        builder = builder.cell(status_value.text, status_value.style);
        match status {
            AgentStatus::Completed(Some(message)) => {
                let message_preview = truncate_text(
                    &message.split_whitespace().collect::<Vec<_>>().join(" "),
                    COLLAB_AGENT_RESPONSE_PREVIEW_GRAPHEMES,
                );
                builder = builder
                    .cell(": ", style_dim())
                    .cell(message_preview, RenderStyle::default());
            }
            AgentStatus::Errored(error) => {
                let error_preview = truncate_text(
                    &error.split_whitespace().collect::<Vec<_>>().join(" "),
                    COLLAB_AGENT_ERROR_PREVIEW_GRAPHEMES,
                );
                builder = builder
                    .cell(": ", style_dim())
                    .cell(error_preview, style_dim());
            }
            _ => {}
        }
        builder.build()
    }));
    lines
}

/// Appends a formatted status count into the summary list.
///
/// # Arguments
/// - `summary` (&mut Vec<StyledText>): Summary list to append to.
/// - `count` (usize): Count for the status type.
/// - `label` (&'static str): Label to display with the count.
/// - `style` (RenderStyle): Style for the count text.
fn push_status_count(
    summary: &mut Vec<StyledText>,
    count: usize,
    label: &'static str,
    style: RenderStyle,
) {
    if count == 0 {
        return;
    }

    summary.push(styled_text(" · ", style_dim()));
    summary.push(styled_text(format!("{count} {label}"), style));
}

/// Builds a styled text value for a render cell.
///
/// # Arguments
/// - `text` (impl Into<String>): Text content.
/// - `style` (RenderStyle): Style applied to the text.
///
/// # Returns
/// - `StyledText`: Styled text value.
fn styled_text(text: impl Into<String>, style: RenderStyle) -> StyledText {
    StyledText {
        text: text.into(),
        style,
    }
}

/// Returns a dim render style.
///
/// # Returns
/// - `RenderStyle`: Dim style.
fn style_dim() -> RenderStyle {
    RenderStyle::builder().dim().build()
}

/// Returns a bold render style.
///
/// # Returns
/// - `RenderStyle`: Bold style.
fn style_bold() -> RenderStyle {
    RenderStyle::builder().bold().build()
}

/// Returns a cyan bold render style.
///
/// # Returns
/// - `RenderStyle`: Cyan bold style.
fn style_cyan_bold() -> RenderStyle {
    RenderStyle::builder().fg(RenderColor::Cyan).bold().build()
}

/// Returns a green render style.
///
/// # Returns
/// - `RenderStyle`: Green style.
fn style_green() -> RenderStyle {
    RenderStyle::builder().fg(RenderColor::Green).build()
}

/// Returns a red render style.
///
/// # Returns
/// - `RenderStyle`: Red style.
fn style_red() -> RenderStyle {
    RenderStyle::builder().fg(RenderColor::Red).build()
}
