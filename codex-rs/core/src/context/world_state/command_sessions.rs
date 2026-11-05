//! Bounded command snapshots appended only when state changes.
//! Live process state is authoritative; persisted snapshots only suppress duplicate updates.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

const MAX_SESSIONS: usize = 16;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum CommandSessionStatus {
    Running,
    Exited,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CommandSession {
    pub(crate) session_id: i32,
    pub(crate) status: CommandSessionStatus,
    pub(crate) in_use: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CommandSessionsState {
    sessions: BTreeMap<i32, CommandSession>,
    omitted: usize,
    #[serde(default)]
    pub(crate) polling_unavailable: bool,
}

impl CommandSessionsState {
    pub(crate) fn new(sessions: impl Iterator<Item = CommandSession>) -> Self {
        let mut state = Self::default();
        for session in sessions {
            state.sessions.insert(session.session_id, session);
            if state.sessions.len() > MAX_SESSIONS {
                state.sessions.pop_last();
                state.omitted += 1;
            }
        }
        state
    }
}

impl WorldStateSection for CommandSessionsState {
    const ID: &'static str = "command_sessions";
    type Snapshot = Self;

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        role == "developer" && Self::matches_text(text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        if matches!(previous, PreviousSectionState::Known(previous) if previous == self) {
            return (None, Vec::new());
        }
        (
            Some(self.clone()),
            vec![WorldStateUpdate::fragment(self.clone())],
        )
    }
}

/// Clears stale handles when disabled, or preserves live handles when polling is unavailable.
#[derive(Default)]
pub(crate) struct DisabledCommandSessionsState(pub(crate) CommandSessionsState);

impl WorldStateSection for DisabledCommandSessionsState {
    const ID: &'static str = CommandSessionsState::ID;
    type Snapshot = CommandSessionsState;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        CommandSessionsState::matches_retained_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        match previous {
            PreviousSectionState::Absent if self.0.sessions.is_empty() => (None, Vec::new()),
            PreviousSectionState::Absent
            | PreviousSectionState::Unknown
            | PreviousSectionState::Known(_) => self.0.render_diff(previous),
        }
    }
}

impl ContextualUserFragment for CommandSessionsState {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("exec.command_sessions".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<command_sessions>", "</command_sessions>")
    }

    fn body(&self) -> ContentItem {
        // Only harness-assigned IDs and fixed lifecycle labels belong in this
        // developer fragment. At most 16 entries keep it below 2 KiB.
        let mut text = String::from(
            "Current command sessions for this thread (supersedes earlier snapshots). Exited sessions retain results for write_stdin. In-use sessions are temporarily busy with a tool interaction or result finalization.\n",
        );
        if self.polling_unavailable {
            text.push_str("write_stdin is unavailable with the current tool set.\n");
        }
        if self.sessions.is_empty() && self.omitted == 0 {
            text.push_str("None.");
        }
        for session in self.sessions.values() {
            let id = session.session_id;
            let status = match session.status {
                CommandSessionStatus::Running => "running",
                CommandSessionStatus::Exited => "exited; poll to collect",
            };
            let in_use = if session.in_use { "; in use" } else { "" };
            text.push_str(&format!("{id}: {status}{in_use}\n"));
        }
        if self.omitted > 0 {
            let omitted = self.omitted;
            text.push_str(&format!(
                "{omitted} additional sessions omitted; this snapshot is incomplete."
            ));
        }
        input_text(format!("\n{}\n", text.trim_end()))
    }
}
