use super::*;

impl ChatWidget {
    pub(super) fn multi_agent_mode_for_turn(&self) -> Option<MultiAgentMode> {
        self.pending_multi_agent_mode
    }

    pub(crate) fn set_multi_agent_mode_from_ui(&mut self, mode: MultiAgentMode) {
        self.multi_agent_mode = mode;
        self.pending_multi_agent_mode = Some(mode);
    }

    pub(super) fn set_multi_agent_mode_available(&mut self, available: bool) {
        self.multi_agent_mode_available = available;
        if !available {
            self.pending_multi_agent_mode = None;
        }
        self.bottom_pane.set_cascade_command_enabled(available);
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
}
