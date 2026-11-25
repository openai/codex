//! Retain observed subagent outcomes across later idle/unloaded notifications.
//! Only explicit work starts clear an outcome; idle alone never implies completion.

use super::Overview;
use codex_app_server_protocol::CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SubAgentActivityKind;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    Starting,
    Running,
    Waiting,
    Idle,
    Completed,
    Stopped,
    Failed,
    Closed,
    Unavailable,
}

impl State {
    pub(super) fn active(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Waiting)
    }

    pub(super) fn supersedes(self, previous: Option<Self>, start_hint: bool) -> bool {
        // Spawn activity can arrive after the child's own completion notification.
        !previous.is_some_and(|previous| {
            (previous.done()
                && (matches!(self, Self::Idle | Self::Unavailable) || start_hint && !self.done()))
                || (matches!(previous, Self::Running | Self::Waiting) && self == Self::Starting)
        })
    }

    pub(super) fn done(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Stopped | Self::Failed | Self::Closed
        )
    }

    pub(super) fn label(self) -> (&'static str, &'static str) {
        match self {
            Self::Starting => ("●", "Starting"),
            Self::Running => ("●", "Running"),
            Self::Waiting => ("●", "Waiting"),
            Self::Idle => ("○", "Idle"),
            Self::Completed => ("○", "Completed"),
            Self::Stopped => ("○", "Stopped"),
            Self::Failed => ("!", "Failed"),
            Self::Closed => ("○", "Closed"),
            Self::Unavailable => ("○", "Unavailable"),
        }
    }
}

pub(super) fn thread_status(status: &ThreadStatus) -> State {
    match status {
        ThreadStatus::Active { active_flags } if !active_flags.is_empty() => State::Waiting,
        ThreadStatus::Active { .. } => State::Running,
        ThreadStatus::Idle => State::Idle,
        ThreadStatus::SystemError => State::Failed,
        ThreadStatus::NotLoaded => State::Unavailable,
    }
}

pub(super) fn observe(event: &ServerNotification, overview: &mut Overview) -> bool {
    let Overview {
        members,
        states,
        restored_outcomes,
        ..
    } = overview;
    let item = match event {
        ServerNotification::ItemStarted(e) => Some(&e.item.payload),
        ServerNotification::ItemCompleted(e) => Some(&e.item.payload),
        _ => None,
    };
    let mut changed = false;
    let mut update = |raw: &str, state: State, start_hint| {
        let Ok(id) = ThreadId::from_string(raw) else {
            return;
        };
        if !members.contains(&id) || !state.supersedes(states.get(&id).copied(), start_hint) {
            return;
        }
        restored_outcomes.remove(&id);
        changed |= states.insert(id, state) != Some(state);
    };
    match event {
        ServerNotification::ThreadStarted(e) => {
            update(
                &e.thread.id,
                thread_status(&e.thread.status),
                /*start_hint*/ false,
            )
        }
        ServerNotification::ThreadStatusChanged(e) => {
            update(
                &e.thread_id,
                thread_status(&e.status),
                /*start_hint*/ false,
            )
        }
        ServerNotification::ThreadClosed(e) => {
            update(&e.thread_id, State::Unavailable, /*start_hint*/ false)
        }
        ServerNotification::TurnStarted(e) => {
            update(&e.thread_id, State::Running, /*start_hint*/ false)
        }
        ServerNotification::TurnCompleted(e) => update(
            &e.thread_id,
            match e.turn.status {
                TurnStatus::Completed => State::Completed,
                TurnStatus::Interrupted => State::Stopped,
                TurnStatus::Failed => State::Failed,
                TurnStatus::InProgress => State::Running,
            },
            /*start_hint*/ false,
        ),
        _ => {}
    }
    if let Some(item) = item {
        visit_item(item, update);
    }
    changed
}

pub(super) fn visit_item(item: &ThreadItemPayload, mut update: impl FnMut(&str, State, bool)) {
    match item {
        ThreadItemPayload::SubAgentActivity {
            agent_thread_id,
            kind,
            ..
        } => match kind {
            SubAgentActivityKind::Started => {
                update(agent_thread_id, State::Running, /*start_hint*/ true)
            }
            SubAgentActivityKind::Completed => {
                update(agent_thread_id, State::Completed, /*start_hint*/ false)
            }
            SubAgentActivityKind::Interrupted => {
                update(agent_thread_id, State::Stopped, /*start_hint*/ false)
            }
            // Ordinary mailbox delivery emits this too; it need not start a turn.
            SubAgentActivityKind::Interacted => {}
        },
        ThreadItemPayload::CollabAgentToolCall {
            agents_states,
            tool,
            ..
        } => {
            for (id, state) in agents_states {
                update(
                    id,
                    match state.status {
                        CollabAgentStatus::PendingInit => State::Starting,
                        CollabAgentStatus::Running => State::Running,
                        CollabAgentStatus::Interrupted => State::Stopped,
                        CollabAgentStatus::Completed => State::Completed,
                        CollabAgentStatus::Errored => State::Failed,
                        CollabAgentStatus::Shutdown => State::Closed,
                        CollabAgentStatus::NotFound => State::Unavailable,
                    },
                    *tool == CollabAgentTool::SpawnAgent,
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "sidebar_agent_status_tests.rs"]
mod tests;
