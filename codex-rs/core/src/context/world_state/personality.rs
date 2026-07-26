use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::PersonalitySpecInstructions;
use codex_protocol::config_types::Personality;
use serde::Deserialize;
use serde::Serialize;

/// Personality instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct PersonalityState {
    snapshot: PersonalitySnapshot,
    previous: Option<PersonalitySnapshot>,
    instructions: Option<String>,
    personality_is_baked: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct PersonalitySnapshot {
    model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    personality: Option<Personality>,
}

impl PersonalityState {
    pub(crate) fn new(
        model: &str,
        personality: Option<Personality>,
        previous_model: Option<&str>,
        previous_personality: Option<Personality>,
        instructions: Option<String>,
        personality_is_baked: bool,
    ) -> Self {
        Self {
            snapshot: PersonalitySnapshot {
                model: model.to_string(),
                personality,
            },
            previous: previous_model.map(|model| PersonalitySnapshot {
                model: model.to_string(),
                personality: previous_personality,
            }),
            instructions,
            personality_is_baked,
        }
    }

    fn render_change(
        &self,
        previous: &PersonalitySnapshot,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        (previous.model == self.snapshot.model && previous.personality != self.snapshot.personality)
            .then_some(self.instructions.as_ref())
            .flatten()
            .map(|instructions| {
                Box::new(PersonalitySpecInstructions::new(instructions.clone()))
                    as Box<dyn ContextualUserFragment>
            })
    }
}

impl WorldStateSection for PersonalityState {
    const ID: &'static str = "personality";
    type Snapshot = PersonalitySnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && PersonalitySpecInstructions::matches_text(text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        match previous {
            PreviousSectionState::Known(previous) => self.render_change(previous),
            PreviousSectionState::Unknown => self
                .previous
                .as_ref()
                .and_then(|previous| self.render_change(previous)),
            PreviousSectionState::Absent => (!self.personality_is_baked
                && self
                    .previous
                    .as_ref()
                    .is_none_or(|previous| previous.model == self.snapshot.model))
            .then_some(self.instructions.as_ref())
            .flatten()
            .map(|instructions| {
                Box::new(PersonalitySpecInstructions::new(instructions.clone()))
                    as Box<dyn ContextualUserFragment>
            }),
        }
    }
}

#[cfg(test)]
#[path = "personality_tests.rs"]
mod tests;
