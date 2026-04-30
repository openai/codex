//! Helpers for rendering and navigating multi-agent state in the TUI.
//!
//! This module owns the shared presentation contracts for multi-agent history rows, `/agent` picker
//! entries, and the fast-switch keyboard shortcuts. Higher-level coordination, such as deciding
//! which thread becomes active or when a thread closes, stays in [`crate::app::App`].

use crate::history_cell::PlainHistoryCell;
use crate::render::line_utils::prefix_lines;
use crate::text_formatting::truncate_text;
use codex_app_server_protocol::CollabAgentState;
use codex_app_server_protocol::CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
#[cfg(target_os = "macos")]
use crossterm::event::KeyEventKind;
#[cfg(target_os = "macos")]
use crossterm::event::KeyModifiers;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::collections::HashSet;

const COLLAB_PROMPT_PREVIEW_GRAPHEMES: usize = 160;
const COLLAB_AGENT_ERROR_PREVIEW_GRAPHEMES: usize = 160;
const COLLAB_AGENT_RESPONSE_PREVIEW_GRAPHEMES: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentPickerThreadEntry {
    /// Human-friendly nickname shown in picker rows and footer labels.
    pub(crate) agent_nickname: Option<String>,
    /// Agent type shown in brackets when present, for example `worker`.
    pub(crate) agent_role: Option<String>,
    /// Whether the thread has emitted a close event and should render dimmed.
    pub(crate) is_closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AgentMetadata {
    /// Human-friendly nickname shown in rendered tool-call rows.
    pub(crate) agent_nickname: Option<String>,
    /// Agent type shown in brackets when present, for example `worker`.
    pub(crate) agent_role: Option<String>,
}

#[derive(Clone, Copy)]
struct AgentLabel<'a> {
    thread_id: Option<ThreadId>,
    nickname: Option<&'a str>,
    role: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnRequestSummary {
    pub(crate) model: String,
    pub(crate) reasoning_effort: ReasoningEffortConfig,
}

pub(crate) fn agent_picker_status_dot_spans(is_closed: bool) -> Vec<Span<'static>> {
    let dot = if is_closed {
        "•".into()
    } else {
        "•".green()
    };
    vec![dot, " ".into()]
}

pub(crate) fn format_agent_picker_item_name(
    agent_nickname: Option<&str>,
    agent_role: Option<&str>,
    is_primary: bool,
) -> String {
    if is_primary {
        return "Main [default]".to_string();
    }

    let agent_nickname = agent_nickname
        .map(str::trim)
        .filter(|nickname| !nickname.is_empty());
    let agent_role = agent_role.map(str::trim).filter(|role| !role.is_empty());
    match (agent_nickname, agent_role) {
        (Some(agent_nickname), Some(agent_role)) => format!("{agent_nickname} [{agent_role}]"),
        (Some(agent_nickname), None) => agent_nickname.to_string(),
        (None, Some(agent_role)) => format!("[{agent_role}]"),
        (None, None) => "Agent".to_string(),
    }
}

pub(crate) fn previous_agent_shortcut() -> crate::key_hint::KeyBinding {
    crate::key_hint::alt(KeyCode::Left)
}

pub(crate) fn next_agent_shortcut() -> crate::key_hint::KeyBinding {
    crate::key_hint::alt(KeyCode::Right)
}

/// Matches the canonical "previous agent" binding plus platform-specific fallbacks that keep agent
/// navigation working when enhanced key reporting is unavailable.
pub(crate) fn previous_agent_shortcut_matches(
    key_event: KeyEvent,
    allow_word_motion_fallback: bool,
) -> bool {
    previous_agent_shortcut().is_press(key_event)
        || previous_agent_word_motion_fallback(key_event, allow_word_motion_fallback)
}

/// Matches the canonical "next agent" binding plus platform-specific fallbacks that keep agent
/// navigation working when enhanced key reporting is unavailable.
pub(crate) fn next_agent_shortcut_matches(
    key_event: KeyEvent,
    allow_word_motion_fallback: bool,
) -> bool {
    next_agent_shortcut().is_press(key_event)
        || next_agent_word_motion_fallback(key_event, allow_word_motion_fallback)
}

#[cfg(target_os = "macos")]
fn previous_agent_word_motion_fallback(
    key_event: KeyEvent,
    allow_word_motion_fallback: bool,
) -> bool {
    // Some terminals, especially on macOS, send Option+b/f as word-motion keys instead of
    // Option+arrow events unless enhanced keyboard reporting is enabled. Callers should only
    // enable this fallback when the composer is empty so draft editing retains the expected
    // word-wise motion behavior.
    allow_word_motion_fallback
        && matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }
        )
}

#[cfg(not(target_os = "macos"))]
fn previous_agent_word_motion_fallback(
    _key_event: KeyEvent,
    _allow_word_motion_fallback: bool,
) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn next_agent_word_motion_fallback(key_event: KeyEvent, allow_word_motion_fallback: bool) -> bool {
    // Some terminals, especially on macOS, send Option+b/f as word-motion keys instead of
    // Option+arrow events unless enhanced keyboard reporting is enabled. Callers should only
    // enable this fallback when the composer is empty so draft editing retains the expected
    // word-wise motion behavior.
    allow_word_motion_fallback
        && matches!(
            key_event,
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::ALT,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }
        )
}

#[cfg(not(target_os = "macos"))]
fn next_agent_word_motion_fallback(
    _key_event: KeyEvent,
    _allow_word_motion_fallback: bool,
) -> bool {
    false
}

pub(crate) fn spawn_request_summary(item: &ThreadItem) -> Option<SpawnRequestSummary> {
    match item {
        ThreadItem::CollabAgentToolCall {
            tool: CollabAgentTool::SpawnAgent,
            model: Some(model),
            reasoning_effort: Some(reasoning_effort),
            ..
        } => Some(SpawnRequestSummary {
            model: model.clone(),
            reasoning_effort: *reasoning_effort,
        }),
        _ => None,
    }
}

pub(crate) fn tool_call_history_cell(
    item: &ThreadItem,
    cached_spawn_request: Option<&SpawnRequestSummary>,
    mut agent_metadata: impl FnMut(ThreadId) -> AgentMetadata,
) -> Option<PlainHistoryCell> {
    let ThreadItem::CollabAgentToolCall {
        tool,
        status,
        receiver_thread_ids,
        prompt,
        agents_states,
        ..
    } = item
    else {
        return None;
    };

    let first_receiver = receiver_thread_ids
        .first()
        .and_then(|id| parse_thread_id(id));
    let prompt = prompt.as_deref().unwrap_or_default();

    match tool {
        CollabAgentTool::SpawnAgent => {
            if matches!(status, CollabAgentToolCallStatus::InProgress) {
                return None;
            }
            let fallback_spawn_request = spawn_request_summary(item);
            let spawn_request = cached_spawn_request.or(fallback_spawn_request.as_ref());
            Some(spawn_end(
                first_receiver,
                prompt,
                spawn_request,
                &mut agent_metadata,
            ))
        }
        CollabAgentTool::SendInput => {
            if matches!(status, CollabAgentToolCallStatus::InProgress) {
                return None;
            }
            first_receiver.map(|receiver_thread_id| {
                interaction_end(receiver_thread_id, prompt, &mut agent_metadata)
            })
        }
        CollabAgentTool::ResumeAgent => first_receiver.map(|receiver_thread_id| {
            if matches!(status, CollabAgentToolCallStatus::InProgress) {
                resume_begin(receiver_thread_id, &mut agent_metadata)
            } else {
                let state = first_agent_state(receiver_thread_ids, agents_states);
                resume_end(
                    receiver_thread_id,
                    state,
                    "Agent resume failed",
                    &mut agent_metadata,
                )
            }
        }),
        CollabAgentTool::Wait => {
            if matches!(status, CollabAgentToolCallStatus::InProgress) {
                Some(waiting_begin(receiver_thread_ids, &mut agent_metadata))
            } else {
                Some(waiting_end(
                    receiver_thread_ids,
                    agents_states,
                    &mut agent_metadata,
                ))
            }
        }
        CollabAgentTool::CloseAgent => {
            if matches!(status, CollabAgentToolCallStatus::InProgress) {
                return None;
            }
            first_receiver
                .map(|receiver_thread_id| close_end(receiver_thread_id, &mut agent_metadata))
        }
    }
}

fn spawn_end(
    new_thread_id: Option<ThreadId>,
    prompt: &str,
    spawn_request: Option<&SpawnRequestSummary>,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    let title = match new_thread_id {
        Some(thread_id) => title_with_agent(
            "Spawned",
            agent_label(thread_id, &agent_metadata(thread_id)),
            spawn_request,
        ),
        None => title_text("Agent spawn failed"),
    };

    let mut details = Vec::new();
    if let Some(line) = prompt_line(prompt) {
        details.push(line);
    }
    collab_event(title, details)
}

fn interaction_end(
    receiver_thread_id: ThreadId,
    prompt: &str,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    let title = title_with_agent(
        "Sent input to",
        agent_label(receiver_thread_id, &agent_metadata(receiver_thread_id)),
        /*spawn_request*/ None,
    );

    let mut details = Vec::new();
    if let Some(line) = prompt_line(prompt) {
        details.push(line);
    }
    collab_event(title, details)
}

fn waiting_begin(
    receiver_thread_ids: &[String],
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    let receiver_agents = receiver_thread_ids
        .iter()
        .filter_map(|thread_id| parse_thread_id(thread_id))
        .map(|thread_id| (thread_id, agent_metadata(thread_id)))
        .collect::<Vec<_>>();

    let title = match receiver_agents.as_slice() {
        [(thread_id, metadata)] => title_with_agent(
            "Waiting for",
            agent_label(*thread_id, metadata),
            /*spawn_request*/ None,
        ),
        [] => title_text("Waiting for agents"),
        _ => title_text(format!("Waiting for {} agents", receiver_agents.len())),
    };

    let details = if receiver_agents.len() > 1 {
        receiver_agents
            .iter()
            .map(|(thread_id, metadata)| agent_label_line(agent_label(*thread_id, metadata)))
            .collect()
    } else {
        Vec::new()
    };

    collab_event(title, details)
}

fn waiting_end(
    receiver_thread_ids: &[String],
    agents_states: &std::collections::HashMap<String, CollabAgentState>,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    let details = wait_complete_lines(receiver_thread_ids, agents_states, agent_metadata);
    collab_event(title_text("Finished waiting"), details)
}

fn close_end(
    receiver_thread_id: ThreadId,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    collab_event(
        title_with_agent(
            "Closed",
            agent_label(receiver_thread_id, &agent_metadata(receiver_thread_id)),
            /*spawn_request*/ None,
        ),
        Vec::new(),
    )
}

fn resume_begin(
    receiver_thread_id: ThreadId,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    collab_event(
        title_with_agent(
            "Resuming",
            agent_label(receiver_thread_id, &agent_metadata(receiver_thread_id)),
            /*spawn_request*/ None,
        ),
        Vec::new(),
    )
}

fn resume_end(
    receiver_thread_id: ThreadId,
    status: Option<&CollabAgentState>,
    fallback_error: &str,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> PlainHistoryCell {
    collab_event(
        title_with_agent(
            "Resumed",
            agent_label(receiver_thread_id, &agent_metadata(receiver_thread_id)),
            /*spawn_request*/ None,
        ),
        vec![status_summary_line(status, fallback_error)],
    )
}

fn collab_event(title: Line<'static>, details: Vec<Line<'static>>) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = vec![title];
    if !details.is_empty() {
        lines.extend(prefix_lines(details, "  └ ".dim(), "    ".into()));
    }
    PlainHistoryCell::new(lines)
}

fn title_text(title: impl Into<String>) -> Line<'static> {
    title_spans_line(vec![Span::from(title.into()).bold()])
}

fn title_with_agent(
    prefix: &str,
    agent: AgentLabel<'_>,
    spawn_request: Option<&SpawnRequestSummary>,
) -> Line<'static> {
    let mut spans = vec![Span::from(format!("{prefix} ")).bold()];
    spans.extend(agent_label_spans(agent));
    spans.extend(spawn_request_spans(spawn_request));
    title_spans_line(spans)
}

fn title_spans_line(mut spans: Vec<Span<'static>>) -> Line<'static> {
    let mut title = Vec::with_capacity(spans.len() + 1);
    title.push(Span::from("• ").dim());
    title.append(&mut spans);
    title.into()
}

fn parse_thread_id(thread_id: &str) -> Option<ThreadId> {
    ThreadId::from_string(thread_id).ok()
}

fn agent_label(thread_id: ThreadId, metadata: &AgentMetadata) -> AgentLabel<'_> {
    AgentLabel {
        thread_id: Some(thread_id),
        nickname: metadata.agent_nickname.as_deref(),
        role: metadata.agent_role.as_deref(),
    }
}

fn agent_label_line(agent: AgentLabel<'_>) -> Line<'static> {
    agent_label_spans(agent).into()
}

fn agent_label_spans(agent: AgentLabel<'_>) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let nickname = agent
        .nickname
        .map(str::trim)
        .filter(|nickname| !nickname.is_empty());
    let role = agent.role.map(str::trim).filter(|role| !role.is_empty());

    if let Some(nickname) = nickname {
        spans.push(Span::from(nickname.to_string()).cyan().bold());
    } else if let Some(thread_id) = agent.thread_id {
        spans.push(Span::from(thread_id.to_string()).cyan());
    } else {
        spans.push(Span::from("agent").cyan());
    }

    if let Some(role) = role {
        spans.push(Span::from(" ").dim());
        spans.push(Span::from(format!("[{role}]")));
    }

    spans
}

fn spawn_request_spans(spawn_request: Option<&SpawnRequestSummary>) -> Vec<Span<'static>> {
    let Some(spawn_request) = spawn_request else {
        return Vec::new();
    };

    let model = spawn_request.model.trim();
    if model.is_empty() && spawn_request.reasoning_effort == ReasoningEffortConfig::default() {
        return Vec::new();
    }

    let details = if model.is_empty() {
        format!("({})", spawn_request.reasoning_effort)
    } else {
        format!("({model} {})", spawn_request.reasoning_effort)
    };

    vec![Span::from(" ").dim(), Span::from(details).magenta()]
}

fn prompt_line(prompt: &str) -> Option<Line<'static>> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Line::from(Span::from(truncate_text(
            trimmed,
            COLLAB_PROMPT_PREVIEW_GRAPHEMES,
        ))))
    }
}

fn wait_complete_lines(
    receiver_thread_ids: &[String],
    agents_states: &std::collections::HashMap<String, CollabAgentState>,
    agent_metadata: &mut impl FnMut(ThreadId) -> AgentMetadata,
) -> Vec<Line<'static>> {
    let mut seen = HashSet::new();
    let mut entries = receiver_thread_ids
        .iter()
        .filter_map(|thread_id| {
            let parsed_thread_id = parse_thread_id(thread_id)?;
            let status = agents_states.get(thread_id)?;
            seen.insert(parsed_thread_id);
            Some((parsed_thread_id, agent_metadata(parsed_thread_id), status))
        })
        .collect::<Vec<_>>();

    let mut extras = agents_states
        .iter()
        .filter_map(|(thread_id, status)| {
            let parsed_thread_id = parse_thread_id(thread_id)?;
            (!seen.contains(&parsed_thread_id))
                .then(|| (parsed_thread_id, agent_metadata(parsed_thread_id), status))
        })
        .collect::<Vec<_>>();
    extras.sort_by(|left, right| left.0.to_string().cmp(&right.0.to_string()));
    entries.extend(extras);

    if entries.is_empty() {
        vec![Line::from(Span::from("No agents completed yet"))]
    } else {
        entries
            .into_iter()
            .map(|(thread_id, metadata, status)| {
                let mut spans = agent_label_spans(agent_label(thread_id, &metadata));
                spans.push(Span::from(": ").dim());
                spans.extend(status_summary_spans(status));
                spans.into()
            })
            .collect()
    }
}

fn first_agent_state<'a>(
    receiver_thread_ids: &[String],
    agents_states: &'a std::collections::HashMap<String, CollabAgentState>,
) -> Option<&'a CollabAgentState> {
    receiver_thread_ids
        .iter()
        .find_map(|thread_id| agents_states.get(thread_id))
        .or_else(|| {
            agents_states
                .iter()
                .min_by(|left, right| left.0.cmp(right.0))
                .map(|(_, status)| status)
        })
}

fn status_summary_line(status: Option<&CollabAgentState>, fallback_error: &str) -> Line<'static> {
    match status {
        Some(status) => status_summary_spans(status).into(),
        None => error_summary_spans(fallback_error).into(),
    }
}

fn status_summary_spans(status: &CollabAgentState) -> Vec<Span<'static>> {
    match status.status {
        CollabAgentStatus::PendingInit => vec![Span::from("Pending init").cyan()],
        CollabAgentStatus::Running => vec![Span::from("Running").cyan().bold()],
        // Allow `.yellow()`
        #[allow(clippy::disallowed_methods)]
        CollabAgentStatus::Interrupted => vec![Span::from("Interrupted").yellow()],
        CollabAgentStatus::Completed => {
            let mut spans = vec![Span::from("Completed").green()];
            if let Some(message) = status.message.as_ref() {
                let message_preview = truncate_text(
                    &message.split_whitespace().collect::<Vec<_>>().join(" "),
                    COLLAB_AGENT_RESPONSE_PREVIEW_GRAPHEMES,
                );
                if !message_preview.is_empty() {
                    spans.push(Span::from(" - ").dim());
                    spans.push(Span::from(message_preview));
                }
            }
            spans
        }
        CollabAgentStatus::Errored => {
            error_summary_spans(status.message.as_deref().unwrap_or("Agent errored"))
        }
        CollabAgentStatus::Shutdown => vec![Span::from("Shutdown")],
        CollabAgentStatus::NotFound => vec![Span::from("Not found").red()],
    }
}

fn error_summary_spans(error: &str) -> Vec<Span<'static>> {
    let mut spans = vec![Span::from("Error").red()];
    let error_preview = truncate_text(
        &error.split_whitespace().collect::<Vec<_>>().join(" "),
        COLLAB_AGENT_ERROR_PREVIEW_GRAPHEMES,
    );
    if !error_preview.is_empty() {
        spans.push(Span::from(" - ").dim());
        spans.push(Span::from(error_preview));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::HistoryCell;
    #[cfg(target_os = "macos")]
    use crossterm::event::KeyEvent;
    #[cfg(target_os = "macos")]
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use ratatui::style::Modifier;
    use std::collections::HashMap;

    #[test]
    fn collab_events_snapshot() {
        let sender_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000001")
            .expect("valid sender thread id");
        let robie_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("valid robie thread id");
        let bob_id = ThreadId::from_string("00000000-0000-0000-0000-000000000003")
            .expect("valid bob thread id");

        let spawn = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-spawn".to_string(),
                tool: CollabAgentTool::SpawnAgent,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string()],
                prompt: Some("Compute 11! and reply with just the integer result.".to_string()),
                model: Some("gpt-5".to_string()),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                agents_states: HashMap::from([(
                    robie_id.to_string(),
                    agent_state(CollabAgentStatus::PendingInit, /*message*/ None),
                )]),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, bob_id),
        )
        .expect("spawn item renders");

        let send = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-send".to_string(),
                tool: CollabAgentTool::SendInput,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string()],
                prompt: Some("Please continue and return the answer only.".to_string()),
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::from([(
                    robie_id.to_string(),
                    agent_state(CollabAgentStatus::Running, /*message*/ None),
                )]),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, bob_id),
        )
        .expect("send-input item renders");

        let waiting = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-wait".to_string(),
                tool: CollabAgentTool::Wait,
                status: CollabAgentToolCallStatus::InProgress,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string()],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::new(),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, bob_id),
        )
        .expect("wait begin item renders");

        let finished = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-wait".to_string(),
                tool: CollabAgentTool::Wait,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string(), bob_id.to_string()],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::from([
                    (
                        robie_id.to_string(),
                        agent_state(CollabAgentStatus::Completed, Some("39916800")),
                    ),
                    (
                        bob_id.to_string(),
                        agent_state(CollabAgentStatus::Errored, Some("tool timeout")),
                    ),
                ]),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, bob_id),
        )
        .expect("wait end item renders");

        let close = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-close".to_string(),
                tool: CollabAgentTool::CloseAgent,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string()],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::from([(
                    robie_id.to_string(),
                    agent_state(CollabAgentStatus::Completed, Some("39916800")),
                )]),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, bob_id),
        )
        .expect("close item renders");

        let snapshot = [spawn, send, waiting, finished, close]
            .iter()
            .map(cell_to_text)
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_snapshot!("collab_agent_transcript", snapshot);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn agent_shortcut_matches_option_arrow_word_motion_fallbacks_only_when_allowed() {
        assert!(previous_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
            /*allow_word_motion_fallback*/ false,
        ));
        assert!(next_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            /*allow_word_motion_fallback*/ false,
        ));
        assert!(previous_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT),
            /*allow_word_motion_fallback*/ true,
        ));
        assert!(next_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT),
            /*allow_word_motion_fallback*/ true,
        ));
        assert!(!previous_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT),
            /*allow_word_motion_fallback*/ false,
        ));
        assert!(!next_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT),
            /*allow_word_motion_fallback*/ false,
        ));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn agent_shortcut_matches_option_arrows_only() {
        assert!(previous_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Left, crossterm::event::KeyModifiers::ALT,),
            /*allow_word_motion_fallback*/ false
        ));
        assert!(next_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Right, crossterm::event::KeyModifiers::ALT,),
            /*allow_word_motion_fallback*/ false
        ));
        assert!(!previous_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Char('b'), crossterm::event::KeyModifiers::ALT,),
            /*allow_word_motion_fallback*/ false
        ));
        assert!(!next_agent_shortcut_matches(
            KeyEvent::new(KeyCode::Char('f'), crossterm::event::KeyModifiers::ALT,),
            /*allow_word_motion_fallback*/ false
        ));
    }

    #[test]
    fn title_styles_nickname_and_role() {
        let sender_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000001")
            .expect("valid sender thread id");
        let robie_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("valid robie thread id");
        let cell = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-spawn".to_string(),
                tool: CollabAgentTool::SpawnAgent,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string()],
                prompt: Some(String::new()),
                model: Some("gpt-5".to_string()),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                agents_states: HashMap::from([(
                    robie_id.to_string(),
                    agent_state(CollabAgentStatus::PendingInit, /*message*/ None),
                )]),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, ThreadId::new()),
        )
        .expect("spawn item renders");

        let lines = cell.display_lines(/*width*/ 200);
        let title = &lines[0];
        assert_eq!(title.spans[2].content.as_ref(), "Robie");
        assert_eq!(title.spans[2].style.fg, Some(Color::Cyan));
        assert!(title.spans[2].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(title.spans[4].content.as_ref(), "[explorer]");
        assert_eq!(title.spans[4].style.fg, None);
        assert!(!title.spans[4].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(title.spans[6].content.as_ref(), "(gpt-5 high)");
        assert_eq!(title.spans[6].style.fg, Some(Color::Magenta));
    }

    #[test]
    fn collab_resume_interrupted_snapshot() {
        let sender_thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000001")
            .expect("valid sender thread id");
        let robie_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("valid robie thread id");

        let cell = tool_call_history_cell(
            &ThreadItem::CollabAgentToolCall {
                id: "call-resume".to_string(),
                tool: CollabAgentTool::ResumeAgent,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![robie_id.to_string()],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::from([(
                    robie_id.to_string(),
                    agent_state(CollabAgentStatus::Interrupted, /*message*/ None),
                )]),
            },
            /*cached_spawn_request*/ None,
            |thread_id| metadata_for(thread_id, robie_id, ThreadId::new()),
        )
        .expect("resume item renders");

        assert_snapshot!("collab_resume_interrupted", cell_to_text(&cell));
    }

    fn agent_state(status: CollabAgentStatus, message: Option<&str>) -> CollabAgentState {
        CollabAgentState {
            status,
            message: message.map(str::to_string),
        }
    }

    fn metadata_for(thread_id: ThreadId, robie_id: ThreadId, bob_id: ThreadId) -> AgentMetadata {
        if thread_id == robie_id {
            AgentMetadata {
                agent_nickname: Some("Robie".to_string()),
                agent_role: Some("explorer".to_string()),
            }
        } else if thread_id == bob_id {
            AgentMetadata {
                agent_nickname: Some("Bob".to_string()),
                agent_role: Some("worker".to_string()),
            }
        } else {
            AgentMetadata::default()
        }
    }

    fn cell_to_text(cell: &PlainHistoryCell) -> String {
        cell.display_lines(/*width*/ 200)
            .iter()
            .map(line_to_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn line_to_text(line: &Line<'static>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }
}
