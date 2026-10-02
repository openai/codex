use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextualUserFragment;
use crate::context::EnvironmentsInstructions;

/// Whether generic execution-environment guidance should be visible to the model.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EnvironmentsInstructionsState {
    enabled: bool,
}

impl EnvironmentsInstructionsState {
    pub(crate) fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl WorldStateSection for EnvironmentsInstructionsState {
    const ID: &'static str = "environments_instructions";
    type Snapshot = bool;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && EnvironmentsInstructions::matches_text(text)
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
    ) -> SectionTransition<Self::Snapshot> {
        let current = self.enabled;
        if !self.enabled
            || matches!(previous, PreviousSectionState::Known(previous) if *previous)
            || matches!(previous, PreviousSectionState::Unknown)
        {
            return (Some(current), Vec::new());
        }

        (
            Some(current),
            vec![WorldStateUpdate::fragment(EnvironmentsInstructions)],
        )
    }
}

#[cfg(test)]
#[path = "environments_instructions_tests.rs"]
mod tests;
