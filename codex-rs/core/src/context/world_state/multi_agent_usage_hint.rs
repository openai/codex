use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::MultiAgentUsageHint;

/// Configured multi-agent instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct MultiAgentUsageHintState {
    instructions: MultiAgentUsageHint,
}

impl MultiAgentUsageHintState {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            instructions: MultiAgentUsageHint::new(text),
        }
    }
}

impl WorldStateSection for MultiAgentUsageHintState {
    const ID: &'static str = "multi_agent_usage_hint";
    type Snapshot = WorldStateHash;

    fn snapshot(&self) -> Self::Snapshot {
        WorldStateHash::from_fragment(&self.instructions)
    }

    fn matches_current_legacy_fragment(&self, role: &str, text: &str) -> bool {
        role == self.instructions.role() && text == self.instructions.render()
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        match previous {
            PreviousSectionState::Known(previous) if previous == &self.snapshot() => None,
            PreviousSectionState::Unknown => None,
            PreviousSectionState::Known(_) | PreviousSectionState::Absent => {
                Some(Box::new(self.instructions.clone()))
            }
        }
    }
}
