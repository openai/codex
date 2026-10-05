//! Records fixed base instructions alongside tool declarations at the start of each window.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::BaseInstructionsFragment;

pub(crate) struct BaseInstructionsState(pub(crate) String);

impl WorldStateSection for BaseInstructionsState {
    const ID: &'static str = "base_instructions";
    type Snapshot = WorldStateHash;

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let fragment = BaseInstructionsFragment(self.0.clone());
        let hash = WorldStateHash::from_fragment(&fragment);
        let updates = WorldStateUpdate::optional_prefix_boxed_fragment(
            (matches!(previous, PreviousSectionState::Absent) && !self.0.is_empty())
                .then(|| Box::new(fragment) as _),
        );
        (Some(hash), updates)
    }
}
