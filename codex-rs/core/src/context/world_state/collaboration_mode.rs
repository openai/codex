use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;

/// Collaboration-mode instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct CollaborationModeState {
    mode: ModeKind,
    instructions: String,
}

impl CollaborationModeState {
    pub(crate) fn from_collaboration_mode(collaboration_mode: &CollaborationMode) -> Option<Self> {
        collaboration_mode
            .settings
            .developer_instructions
            .clone()
            .filter(|instructions| !instructions.is_empty())
            .map(|instructions| Self {
                mode: collaboration_mode.mode,
                instructions,
            })
    }
}

impl WorldStateSection for CollaborationModeState {
    const ID: &'static str = "collaboration_mode";
    type Snapshot = ModeKind;

    fn snapshot(&self) -> Self::Snapshot {
        self.mode
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && CollaborationModeInstructions::matches_text(text)
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
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &self.mode)
            || matches!(previous, PreviousSectionState::Unknown)
        {
            return None;
        }

        Some(Box::new(CollaborationModeInstructions {
            instructions: self.instructions.clone(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CollaborationModeInstructions {
    instructions: String,
}

impl ContextualUserFragment for CollaborationModeInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (COLLABORATION_MODE_OPEN_TAG, COLLABORATION_MODE_CLOSE_TAG)
    }

    fn body(&self) -> String {
        self.instructions.clone()
    }
}

#[cfg(test)]
#[path = "collaboration_mode_tests.rs"]
mod tests;
