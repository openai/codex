use super::ContextualUserFragment;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::models::ContentItemKind;
use codex_protocol::protocol::MULTI_AGENT_MODE_CLOSE_TAG;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MultiAgentModeInstructions {
    multi_agent_mode: MultiAgentMode,
}

impl MultiAgentModeInstructions {
    pub(super) fn from_mode(multi_agent_mode: MultiAgentMode) -> Option<Self> {
        if matches!(
            &multi_agent_mode,
            MultiAgentMode::Custom(hint_text) if hint_text.is_empty()
        ) {
            return None;
        }

        Some(Self { multi_agent_mode })
    }
}

impl ContextualUserFragment for MultiAgentModeInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("multi_agent.mode_instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (MULTI_AGENT_MODE_OPEN_TAG, MULTI_AGENT_MODE_CLOSE_TAG)
    }

    fn body(&self) -> String {
        // `effective_multi_agent_mode` carries configured and catalog overrides as `Custom`.
        // The other variants explicitly select bundled text.
        let bundled = ResolvedModelMessages::bundled().multi_agent();
        match &self.multi_agent_mode {
            MultiAgentMode::Custom(hint_text) => hint_text.as_str(),
            MultiAgentMode::ExplicitRequestOnly => bundled.explicit.text(),
            MultiAgentMode::Proactive => bundled.proactive.text(),
        }
        .to_owned()
    }
}
