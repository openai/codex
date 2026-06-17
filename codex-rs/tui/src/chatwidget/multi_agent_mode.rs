use super::*;

pub(super) const CASCADE_USAGE: &str = "Usage: /cascade [on|off]";

impl ChatWidget {
    pub(super) fn multi_agent_mode_for_turn(&self) -> Option<MultiAgentMode> {
        self.pending_multi_agent_mode
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
                "Cascade: explicit requests only",
                "Use /cascade on to let Codex delegate proactively.",
            ),
            MultiAgentMode::Proactive => (
                "Cascade: proactive",
                "Use /cascade off to require an explicit delegation request.",
            ),
        };
        self.add_info_message(message.to_string(), Some(hint.to_string()));
    }
}
