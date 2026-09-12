//! Bounded overview previews from existing reads and already-delivered events.
//! Observing activity never attaches to a thread or fetches additional history.
//! Message previews retain Markdown until the details panel's width is known.

use super::App;
use super::ThreadBufferedEvent;
use super::ThreadEventAttachment;
use super::agents_overview_view::AgentsOverviewGroup;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::collections::HashMap;

const PREVIEW_CHARS: usize = 512;

pub(super) fn preview_agent_message(text: &str) -> String {
    preview_markdown(&crate::markdown::unwrap_markdown_fences(text))
}

pub(super) fn preview_markdown(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .take(PREVIEW_CHARS)
        .collect()
}

#[derive(Clone, Default)]
pub(super) struct AgentsOverviewDetails {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) usage_lines: Vec<Line<'static>>,
    pub(super) last_message: Option<(String, AbsolutePathBuf)>,
}

pub(super) fn preview_text(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .filter(|ch| !ch.is_control())
        .take(PREVIEW_CHARS)
        .collect()
}

#[derive(Default)]
pub(super) struct AgentsOverviewActivity {
    reasoning: Option<ReasoningPreview>,
    last_message: Option<String>,
}

struct ReasoningPreview {
    turn_id: String,
    item_id: String,
    summary_index: i64,
    buffer: String,
    header: Option<String>,
}

impl App {
    pub(super) fn track_agents_overview_activity(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) {
        if !self.agents_overview.threads.contains_key(&thread_id) {
            return;
        }
        let activity = self.agents_overview.activity.entry(thread_id).or_default();
        let changed = match notification {
            ServerNotification::ReasoningSummaryTextDelta(delta) => {
                let had_header = activity
                    .reasoning
                    .as_ref()
                    .is_some_and(|reasoning| reasoning.header.is_some());
                if activity.reasoning.as_ref().is_none_or(|reasoning| {
                    reasoning.turn_id != delta.turn_id
                        || reasoning.item_id != delta.item_id
                        || reasoning.summary_index != delta.summary_index
                }) {
                    activity.reasoning = Some(ReasoningPreview {
                        turn_id: delta.turn_id.clone(),
                        item_id: delta.item_id.clone(),
                        summary_index: delta.summary_index,
                        buffer: String::new(),
                        header: None,
                    });
                }
                let Some(reasoning) = activity.reasoning.as_mut() else {
                    return;
                };
                if reasoning.header.is_some() {
                    return;
                }
                let remaining = PREVIEW_CHARS.saturating_sub(reasoning.buffer.chars().count());
                reasoning.buffer.extend(delta.delta.chars().take(remaining));
                reasoning.header = crate::chatwidget::extract_first_bold(&reasoning.buffer)
                    .map(|header| preview_text(&header));
                had_header || reasoning.header.is_some()
            }
            ServerNotification::ItemCompleted(ItemCompletedNotification {
                item: ThreadItem::AgentMessage { text, .. },
                ..
            }) => {
                activity.last_message = Some(preview_agent_message(text));
                activity.reasoning = None;
                true
            }
            ServerNotification::TurnStarted(_)
            | ServerNotification::TurnCompleted(_)
            | ServerNotification::ReasoningSummaryPartAdded(_)
            | ServerNotification::ItemStarted(_) => activity.reasoning.take().is_some(),
            ServerNotification::ThreadStatusChanged(status)
                if !matches!(status.status, ThreadStatus::Active { .. }) =>
            {
                activity.reasoning.take().is_some()
            }
            _ => false,
        };
        if changed {
            self.repaint_agents_overview();
        }
    }

    pub(super) fn agents_overview_details(
        &self,
        root: &Thread,
        children: &HashMap<String, Vec<&Thread>>,
    ) -> AgentsOverviewDetails {
        let mut lines = Vec::new();
        // Urgency comes first; on ties, prefer available current detail over history.
        let priority = |thread: &Thread| {
            let group = AgentsOverviewGroup::for_status(&thread.status);
            let has_detail = ThreadId::from_string(&thread.id).is_ok_and(|id| match group {
                AgentsOverviewGroup::NeedsYou => self.agents_overview_request_preview(id).is_some(),
                AgentsOverviewGroup::Working => {
                    self.thread_event_channels
                        .get(&id)
                        .is_none_or(|channel| channel.attachment() == ThreadEventAttachment::Live)
                        && self
                            .agents_overview
                            .activity
                            .get(&id)
                            .and_then(|activity| activity.reasoning.as_ref())
                            .is_some_and(|reasoning| reasoning.header.is_some())
                }
                AgentsOverviewGroup::Ready | AgentsOverviewGroup::Finished => false,
            });
            (group, !has_detail)
        };
        let mut source = root;
        let mut pending = vec![root];
        while let Some(thread) = pending.pop() {
            if priority(thread) < priority(source) {
                source = thread;
            }
            pending.extend(children.get(&thread.id).into_iter().flatten().copied());
            pending.sort_by(|left, right| right.id.cmp(&left.id));
        }
        let Ok(thread_id) = ThreadId::from_string(&source.id) else {
            return AgentsOverviewDetails::default();
        };
        let channel = self.thread_event_channels.get(&thread_id);
        let is_child = source.id != root.id;
        if is_child {
            let name = source
                .agent_nickname
                .as_deref()
                .or(source.name.as_deref())
                .unwrap_or(&source.id);
            lines.push(Line::default());
            lines.push(vec!["Agent: ".dim(), preview_text(name).into()].into());
        }
        if AgentsOverviewGroup::for_status(&source.status) == AgentsOverviewGroup::NeedsYou {
            if !is_child {
                lines.extend([Line::default(), "Needs attention".red().into()]);
            }
            if let Some(request) = self.agents_overview_request_preview(thread_id) {
                lines.push(request.into());
            }
            lines.push("Open task to review.".dim().into());
            match &source.status {
                ThreadStatus::Active { active_flags } => {
                    if active_flags.contains(&ThreadActiveFlag::WaitingOnApproval) {
                        lines.push("Waiting for approval.".into());
                    }
                    if active_flags.contains(&ThreadActiveFlag::WaitingOnUserInput) {
                        lines.push("Waiting for your response.".into());
                    }
                }
                ThreadStatus::SystemError => lines.push("Task encountered an error.".into()),
                ThreadStatus::NotLoaded | ThreadStatus::Idle => {}
            }
        }
        let activity = self.agents_overview.activity.get(&thread_id).filter(|_| {
            channel.is_none_or(|channel| channel.attachment() == ThreadEventAttachment::Live)
        });
        if matches!(source.status, ThreadStatus::Active { .. })
            && let Some(header) = activity
                .and_then(|activity| activity.reasoning.as_ref())
                .and_then(|reasoning| reasoning.header.as_ref())
        {
            if !is_child {
                lines.push(Line::default());
            }
            lines.extend(["Latest activity".dim().into(), header.clone().into()]);
        }
        let last_message = activity
            .and_then(|activity| activity.last_message.as_ref())
            .or_else(|| self.agents_overview.last_messages.get(&thread_id))
            .filter(|message| !message.trim().is_empty())
            .map(|message| (message.clone(), source.cwd.clone()));
        AgentsOverviewDetails {
            lines,
            usage_lines: ThreadId::from_string(&root.id)
                .ok()
                .and_then(|id| self.agents_overview.usage.get(&id))
                .map(super::agents_overview_usage::usage_lines)
                .unwrap_or_default(),
            last_message,
        }
    }

    fn agents_overview_request_preview(&self, thread_id: ThreadId) -> Option<String> {
        self.agents_overview
            .dispatched_requests
            .get(&thread_id)
            .and_then(|requests| requests.iter().find_map(request_preview))
            .or_else(|| {
                let channel = self
                    .thread_event_channels
                    .get(&thread_id)
                    .filter(|channel| channel.attachment() == ThreadEventAttachment::Live)?;
                let store = channel.store.try_lock().ok()?;
                store.buffer.iter().find_map(|event| {
                    if let ThreadBufferedEvent::Request(request) = event
                        && store
                            .pending_interactive_replay
                            .should_replay_snapshot_request(request)
                    {
                        request_preview(request)
                    } else {
                        None
                    }
                })
            })
    }
}

fn request_preview(request: &ServerRequest) -> Option<String> {
    let text = match request {
        ServerRequest::CommandExecutionRequestApproval { params, .. } => {
            params.reason.as_deref().or(params.command.as_deref())
        }
        ServerRequest::FileChangeRequestApproval { params, .. } => params.reason.as_deref(),
        ServerRequest::PermissionsRequestApproval { params, .. } => params.reason.as_deref(),
        ServerRequest::McpServerElicitationRequest { params, .. } => match &params.request {
            McpServerElicitationRequest::Form { message, .. }
            | McpServerElicitationRequest::OpenAiForm { message, .. }
            | McpServerElicitationRequest::OpenAiElicitationForm { message, .. }
            | McpServerElicitationRequest::Url { message, .. } => Some(message.as_str()),
            McpServerElicitationRequest::UserVerification { .. } => None,
        },
        ServerRequest::ToolRequestUserInput { params, .. } => params
            .questions
            .first()
            .map(|question| question.question.as_str()),
        _ => None,
    }?;
    Some(preview_text(text))
}
