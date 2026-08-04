use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::CollaborationModeMessages;
use codex_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;

/// Collaboration-mode instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct CollaborationModeState {
    mode: ModeKind,
    model: String,
    instructions: Option<String>,
}

impl CollaborationModeState {
    pub(crate) fn from_collaboration_mode(
        collaboration_mode: &CollaborationMode,
        catalog_messages: Option<&CollaborationModeMessages>,
    ) -> Self {
        let catalog_instructions =
            catalog_messages.and_then(|messages| match collaboration_mode.mode {
                ModeKind::Default => messages.default.as_ref(),
                ModeKind::Plan => messages.plan.as_ref(),
            });

        Self {
            mode: collaboration_mode.mode,
            model: collaboration_mode.settings.model.clone(),
            instructions: catalog_instructions.cloned().or_else(|| {
                collaboration_mode
                    .settings
                    .developer_instructions
                    .clone()
                    .filter(|instructions| !instructions.is_empty())
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum CollaborationModeSnapshot {
    Current { mode: ModeKind, model: String },
    Legacy(ModeKind),
}

impl WorldStateSection for CollaborationModeState {
    const ID: &'static str = "collaboration_mode";
    type Snapshot = CollaborationModeSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        CollaborationModeSnapshot::Current {
            mode: self.mode,
            model: self.model.clone(),
        }
    }

    fn should_persist(&self) -> bool {
        self.instructions.is_some()
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
        if matches!(
            previous,
            PreviousSectionState::Known(CollaborationModeSnapshot::Current { mode, model })
                if *mode == self.mode && model == &self.model
        ) || matches!(previous, PreviousSectionState::Unknown)
            || (self.instructions.is_none() && matches!(previous, PreviousSectionState::Absent))
        {
            return None;
        }

        Some(Box::new(CollaborationModeInstructions {
            instructions: self.instructions.clone().unwrap_or_default(),
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
