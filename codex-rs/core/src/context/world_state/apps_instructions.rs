use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::AppsInstructions;
use crate::context::ContextualUserFragment;

/// Whether generic Apps usage guidance should be visible to the model.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AppsInstructionsState {
    available: bool,
}

impl AppsInstructionsState {
    pub(crate) fn new(available: bool) -> Self {
        Self { available }
    }
}

impl WorldStateSection for AppsInstructionsState {
    const ID: &'static str = "apps_instructions";
    type Snapshot = bool;

    fn snapshot(&self) -> Self::Snapshot {
        self.available
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && AppsInstructions::matches_text(text)
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
        if !self.available
            || matches!(previous, PreviousSectionState::Known(previous) if *previous)
            || matches!(previous, PreviousSectionState::Unknown)
        {
            return None;
        }

        Some(Box::new(AppsInstructions))
    }
}

#[cfg(test)]
#[path = "apps_instructions_tests.rs"]
mod tests;
