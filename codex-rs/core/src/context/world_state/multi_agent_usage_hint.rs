use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextualUserFragment;
use crate::context::MultiAgentRoleInstructions;
use crate::context::MultiAgentUsageHint;

/// Configured or model-owned multi-agent instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct MultiAgentUsageHintState {
    instructions: MultiAgentRoleInstructions,
    pub(super) fingerprint: WorldStateHash,
}

impl MultiAgentUsageHintState {
    pub(crate) fn new(instructions: MultiAgentRoleInstructions) -> Self {
        let fingerprint = WorldStateHash::from_fragment(&instructions);
        Self {
            instructions,
            fingerprint,
        }
    }
}

impl WorldStateSection for MultiAgentUsageHintState {
    const ID: &'static str = "multi_agent_usage_hint";
    type Snapshot = WorldStateHash;

    fn matches_current_legacy_fragment(&self, role: &str, text: &str) -> bool {
        role == self.instructions.role() && text == self.instructions.render()
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let fragment: Option<Box<dyn ContextualUserFragment>> = match previous {
            PreviousSectionState::Known(previous) if previous == &self.fingerprint => {
                return (None, Vec::new());
            }
            PreviousSectionState::Unknown => None,
            PreviousSectionState::Known(_) | PreviousSectionState::Absent => {
                if self.instructions.markers().0.is_empty() {
                    Some(Box::new(MultiAgentUsageHint::new(
                        &self.instructions.body(),
                    )))
                } else {
                    Some(Box::new(self.instructions.clone()))
                }
            }
        };
        (
            Some(self.fingerprint.clone()),
            WorldStateUpdate::optional_standalone_boxed_fragment(fragment),
        )
    }
}
