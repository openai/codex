//! Keeps persistent-mode developer instructions current without repeating unchanged context.
//! Mode changes retire prior instructions; callers provide the resolved instruction template.

use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;

const REPLACEMENT_NOTICE: &str = "These persistent-mode instructions replace all previously provided persistent-mode instructions.";
const REMOVAL_NOTICE: &str =
    "The previously provided persistent-mode instructions no longer apply.";

#[derive(Clone, Debug)]
pub(crate) struct PersistentModeState {
    instructions: String,
}

impl ContextualUserFragment for PersistentModeState {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("persistent_mode.instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<persistent_mode>", "</persistent_mode>")
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.instructions)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistentModeSnapshot {
    // Keep an object: JSON null would delete the section and lose its disabled state.
    instructions: Option<WorldStateHash>,
}

impl PersistentModeState {
    pub(crate) fn new(
        reasoning_effort: Option<&ReasoningEffort>,
        instructions_template: &str,
        send_user_message_async_available: bool,
    ) -> Self {
        let instructions = if reasoning_effort == Some(&ReasoningEffort::Persistent) {
            instructions_template.trim().replace(
                "{{ approval_request_channel }}",
                if send_user_message_async_available {
                    " via functions.send_user_message_async"
                } else {
                    ""
                },
            )
        } else {
            String::new()
        };
        Self { instructions }
    }
}

impl WorldStateSection for PersistentModeState {
    const ID: &'static str = "persistent_mode";
    type Snapshot = PersistentModeSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        PersistentModeSnapshot {
            instructions: (!self.instructions.is_empty())
                .then(|| WorldStateHash::from_fragment(self)),
        }
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && Self::matches_text(text)
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
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &self.snapshot())
        {
            return None;
        }
        let previous_had_instructions = match previous {
            PreviousSectionState::Absent => false,
            PreviousSectionState::Unknown => true,
            PreviousSectionState::Known(previous) => previous.instructions.is_some(),
        };
        let instructions = match (self.instructions.as_str(), previous_had_instructions) {
            ("", false) => return None,
            ("", true) => REMOVAL_NOTICE.to_string(),
            (instructions, true) => format!("{REPLACEMENT_NOTICE}\n\n{instructions}"),
            (instructions, false) => instructions.to_string(),
        };
        Some(Box::new(Self { instructions }))
    }
}

#[cfg(test)]
#[path = "persistent_mode_tests.rs"]
mod tests;
