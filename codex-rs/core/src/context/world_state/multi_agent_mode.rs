use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use super::multi_agent_usage_hint::MultiAgentUsageHintState;
use crate::context::ContextualUserFragment;
use crate::context::multi_agent_mode_instructions::MultiAgentModeInstructions;
use codex_protocol::config_types::MultiAgentMode;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use serde::Deserialize;
use serde::Serialize;

const MULTI_AGENT_MODE_MAX_TOKENS: usize = 400;

/// Effective multi-agent mode currently visible to the model.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MultiAgentModeState {
    mode: Option<MultiAgentMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage_hint_hash: Option<WorldStateHash>,
}

impl MultiAgentModeState {
    pub(crate) fn new(mode: Option<MultiAgentMode>) -> Self {
        Self {
            mode: mode.map(|mode| match mode {
                MultiAgentMode::Custom(hint_text) => MultiAgentMode::Custom(truncate_text(
                    &hint_text,
                    TruncationPolicy::Tokens(MULTI_AGENT_MODE_MAX_TOKENS),
                )),
                mode @ (MultiAgentMode::ExplicitRequestOnly | MultiAgentMode::Proactive) => mode,
            }),
            usage_hint_hash: None,
        }
    }

    pub(crate) fn with_usage_hint(mut self, usage_hint: &MultiAgentUsageHintState) -> Self {
        self.usage_hint_hash = Some(usage_hint.snapshot());
        self
    }
}

impl WorldStateSection for MultiAgentModeState {
    const ID: &'static str = "multi_agent_mode";
    type Snapshot = Self;

    fn snapshot(&self) -> Self::Snapshot {
        self.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && MultiAgentModeInstructions::matches_text(text)
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
        let mode = match (&self.mode, previous) {
            (Some(mode), PreviousSectionState::Known(previous))
                if previous.mode.as_ref() == Some(mode)
                    && previous.usage_hint_hash == self.usage_hint_hash =>
            {
                return None;
            }
            (Some(mode), _) => mode.clone(),
            (None, PreviousSectionState::Known(previous))
                if previous.mode == Some(MultiAgentMode::Proactive) =>
            {
                MultiAgentMode::ExplicitRequestOnly
            }
            (None, PreviousSectionState::Unknown) => MultiAgentMode::ExplicitRequestOnly,
            (None, PreviousSectionState::Absent | PreviousSectionState::Known(_)) => return None,
        };

        MultiAgentModeInstructions::from_mode(mode)
            .map(|instructions| Box::new(instructions) as Box<dyn ContextualUserFragment>)
    }
}

#[cfg(test)]
#[path = "multi_agent_mode_tests.rs"]
mod tests;
