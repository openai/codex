use super::*;

pub(super) const MULTI_AGENT_MODE_USAGE: &str = "Usage: /multi-agent [on|off]";

impl ChatWidget {
    pub(super) fn multi_agent_mode_enabled(&self) -> bool {
        self.config.features.enabled(Feature::MultiAgentV2)
            && self.config.multi_agent_v2.usage_hint_enabled
            && self
                .config
                .multi_agent_v2
                .root_agent_usage_hint_text
                .is_some()
    }

    pub(super) fn multi_agent_mode_for_turn(&self) -> Option<MultiAgentMode> {
        if self.multi_agent_mode_enabled() {
            self.pending_multi_agent_mode
        } else {
            None
        }
    }

    pub(super) fn set_multi_agent_mode_from_ui(&mut self, mode: MultiAgentMode) {
        self.multi_agent_mode = mode;
        self.pending_multi_agent_mode = Some(mode);
        self.show_multi_agent_mode();
    }

    pub(super) fn apply_server_multi_agent_mode(&mut self, mode: MultiAgentMode) {
        match self.pending_multi_agent_mode {
            Some(pending) if pending == mode => {
                self.multi_agent_mode = mode;
                self.pending_multi_agent_mode = None;
            }
            Some(_) => {}
            None => self.multi_agent_mode = mode,
        }
    }

    pub(super) fn show_multi_agent_mode(&mut self) {
        let (message, hint) = match self.multi_agent_mode {
            MultiAgentMode::ExplicitRequestOnly => (
                "Multi-agent delegation: explicit requests only",
                "Use /multi-agent on to let Codex delegate proactively.",
            ),
            MultiAgentMode::Proactive => (
                "Multi-agent delegation: proactive",
                "Use /multi-agent off to require an explicit delegation request.",
            ),
        };
        self.add_info_message(message.to_string(), Some(hint.to_string()));
    }
}
