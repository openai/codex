use crate::tab_status::TabStatus;
use crate::tab_status::set_tab_status;

/// Tracks the OSC 21337 state emitted by the bottom pane.
pub(super) struct TabStatusState {
    enabled: bool,
    last_status: Option<TabStatus>,
}

impl TabStatusState {
    pub(super) fn new() -> Self {
        Self {
            enabled: true,
            last_status: None,
        }
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(super) fn refresh(&mut self, desired: TabStatus) {
        if !self.enabled || self.last_status == Some(desired) {
            return;
        }
        if let Err(err) = set_tab_status(desired) {
            tracing::debug!(error = %err, "failed to set tab status");
            return;
        }
        self.last_status = Some(desired);
    }

    #[cfg(test)]
    pub(super) fn last_status(&self) -> Option<TabStatus> {
        self.last_status
    }
}
