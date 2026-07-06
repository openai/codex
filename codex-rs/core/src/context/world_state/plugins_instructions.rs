use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::AvailablePluginsInstructions;
use crate::context::ContextualUserFragment;

/// Whether generic plugin usage guidance should be visible to the model.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PluginsInstructionsState {
    available: bool,
}

impl PluginsInstructionsState {
    pub(crate) fn new(available: bool) -> Self {
        Self { available }
    }
}

impl WorldStateSection for PluginsInstructionsState {
    const ID: &'static str = "plugins_instructions";
    type Snapshot = bool;

    fn snapshot(&self) -> Self::Snapshot {
        self.available
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && AvailablePluginsInstructions::matches_text(text)
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

        Some(Box::new(AvailablePluginsInstructions))
    }
}

#[cfg(test)]
#[path = "plugins_instructions_tests.rs"]
mod tests;
