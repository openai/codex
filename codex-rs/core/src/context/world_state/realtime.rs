use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::RealtimeEndInstructions;
use crate::context::RealtimeStartInstructions;
use crate::context::RealtimeStartWithInstructions;
use serde::Deserialize;
use serde::Serialize;

/// The realtime conversation state currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct RealtimeState {
    snapshot: RealtimeSnapshot,
    start_instructions: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct RealtimeSnapshot {
    active: bool,
}

impl RealtimeState {
    pub(crate) fn new(active: bool, start_instructions: Option<&str>) -> Self {
        Self {
            snapshot: RealtimeSnapshot { active },
            start_instructions: start_instructions.map(str::to_string),
        }
    }

    fn render_start(&self) -> Box<dyn ContextualUserFragment> {
        match self.start_instructions.as_deref() {
            Some(instructions) => Box::new(RealtimeStartWithInstructions::new(instructions)),
            None => Box::new(RealtimeStartInstructions),
        }
    }

    fn render_transition(&self, previous_active: bool) -> Option<Box<dyn ContextualUserFragment>> {
        match (previous_active, self.snapshot.active) {
            (false, true) => Some(self.render_start()),
            (true, false) => Some(Box::new(RealtimeEndInstructions::new("inactive"))),
            (false, false) | (true, true) => None,
        }
    }
}

impl WorldStateSection for RealtimeState {
    const ID: &'static str = "realtime";
    type Snapshot = RealtimeSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer"
            && RealtimeStartInstructions::matches_text(text)
            && !RealtimeEndInstructions::matches_text(text)
    }

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        Self::matches_legacy_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        match previous {
            PreviousSectionState::Known(previous) if previous == &self.snapshot => None,
            PreviousSectionState::Known(previous) => self.render_transition(previous.active),
            PreviousSectionState::Absent | PreviousSectionState::Unknown
                if self.snapshot.active =>
            {
                Some(self.render_start())
            }
            PreviousSectionState::Absent | PreviousSectionState::Unknown => None,
        }
    }
}

#[cfg(test)]
#[path = "realtime_tests.rs"]
mod tests;
