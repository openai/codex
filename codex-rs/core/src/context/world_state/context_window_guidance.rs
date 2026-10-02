use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextWindowGuidance;
use crate::context::ContextualUserFragment;

const REPLACEMENT_NOTICE: &str =
    "This context-window guidance replaces all previously provided context-window guidance.";
const REMOVAL_NOTICE: &str = "The previously provided context-window guidance no longer applies.";

/// Model-visible guidance for managing the current context window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextWindowGuidanceState {
    // Empty string means no guidance.
    message: String,
}

impl ContextWindowGuidanceState {
    pub(crate) fn new(message: Option<&str>) -> Self {
        Self {
            message: message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_default()
                .to_string(),
        }
    }
}

impl WorldStateSection for ContextWindowGuidanceState {
    const ID: &'static str = "context_window_guidance";
    type Snapshot = String;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && ContextWindowGuidance::matches_text(text)
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
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &self.message) {
            return (None, Vec::new());
        }
        let previous_may_contain_guidance = match previous {
            PreviousSectionState::Known(previous) => !previous.is_empty(),
            PreviousSectionState::Unknown => true,
            PreviousSectionState::Absent => false,
        };
        let message = if self.message.is_empty() {
            if !previous_may_contain_guidance {
                return (Some(self.message.clone()), Vec::new());
            }
            REMOVAL_NOTICE.to_string()
        } else if previous_may_contain_guidance {
            format!("{REPLACEMENT_NOTICE}\n\n{}", self.message)
        } else {
            self.message.clone()
        };
        (
            Some(self.message.clone()),
            vec![WorldStateUpdate::fragment(ContextWindowGuidance::new(
                &message,
            ))],
        )
    }
}

#[cfg(test)]
#[path = "context_window_guidance_tests.rs"]
mod tests;
