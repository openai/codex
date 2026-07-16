use super::PreviousSectionState;
use super::WorldStateSection;
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

    fn snapshot(&self) -> Self::Snapshot {
        self.enabled
    }

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
    ) -> Option<Box<dyn ContextualUserFragment>> {
        if !self.enabled
            || matches!(previous, PreviousSectionState::Known(previous) if *previous)
            || matches!(previous, PreviousSectionState::Unknown)
        {
            return None;
        }

        Some(Box::new(EnvironmentsInstructions))
    }
}

#[cfg(test)]
#[path = "environments_instructions_tests.rs"]
mod tests;
