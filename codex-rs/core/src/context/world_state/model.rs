use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::ModelSwitchInstructions;

/// Model identity and the instructions needed when that identity changes.
#[derive(Clone, Debug)]
pub(crate) struct ModelInstructionsState {
    model: String,
    previous_model: Option<String>,
    instructions: String,
}

impl ModelInstructionsState {
    pub(crate) fn new(model: &str, previous_model: Option<&str>, instructions: String) -> Self {
        Self {
            model: model.to_string(),
            previous_model: previous_model.map(str::to_string),
            instructions,
        }
    }
}

impl WorldStateSection for ModelInstructionsState {
    const ID: &'static str = "model";
    type Snapshot = String;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && ModelSwitchInstructions::matches_text(text)
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
        let current = self.model.clone();
        let model_changed = match previous {
            PreviousSectionState::Known(previous) => previous != &self.model,
            PreviousSectionState::Unknown | PreviousSectionState::Absent => self
                .previous_model
                .as_deref()
                .is_some_and(|previous| previous != self.model),
        };

        let fragment: Option<Box<dyn ContextualUserFragment>> =
            (model_changed && !self.instructions.is_empty()).then(|| {
                Box::new(ModelSwitchInstructions::new(self.instructions.clone()))
                    as Box<dyn ContextualUserFragment>
            });
        (Some(current), fragment)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
