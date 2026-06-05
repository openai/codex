use std::time::Duration;
use std::time::Instant;

use crate::tab_status::TabStatus;
use crate::tab_status::set_tab_status;

const MIN_DETAIL_INTERVAL: Duration = Duration::from_millis(/*millis*/ 250);

/// Tracks the OSC 21337 state emitted by the bottom pane.
pub(super) struct TabStatusState {
    enabled: bool,
    last_status: Option<(TabStatus, Option<String>)>,
    last_emit_at: Option<Instant>,
    current_activity: Option<String>,
    last_activity: Option<String>,
}

impl TabStatusState {
    pub(super) fn new() -> Self {
        Self {
            enabled: true,
            last_status: None,
            last_emit_at: None,
            current_activity: None,
            last_activity: None,
        }
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(super) fn reset_for_new_turn(&mut self) {
        self.current_activity = None;
        self.last_activity = None;
    }

    pub(super) fn set_current_activity(&mut self, activity: Option<String>) -> bool {
        let activity = activity
            .map(|activity| activity.trim().to_string())
            .filter(|activity| !activity.is_empty());
        if self.current_activity == activity {
            return false;
        }
        if activity.is_none()
            && let Some(previous) = self.current_activity.take()
        {
            self.last_activity = Some(previous);
        }
        self.current_activity = activity;
        true
    }

    pub(super) fn current_activity(&self) -> Option<&str> {
        self.current_activity.as_deref()
    }

    pub(super) fn last_activity(&self) -> Option<&str> {
        self.last_activity.as_deref()
    }

    /// Returns the remaining throttle delay before detail may be recomputed.
    /// Status-class transitions always bypass the throttle.
    pub(super) fn refresh_delay(&self, desired_class: TabStatus, now: Instant) -> Option<Duration> {
        if !self.enabled
            || self.last_status.as_ref().map(|(status, _)| *status) != Some(desired_class)
        {
            return None;
        }
        self.last_emit_at.and_then(|last_emit_at| {
            let elapsed = now.saturating_duration_since(last_emit_at);
            MIN_DETAIL_INTERVAL
                .checked_sub(elapsed)
                .filter(|delay| !delay.is_zero())
        })
    }

    pub(super) fn refresh(&mut self, desired: (TabStatus, Option<String>), now: Instant) {
        if !self.enabled {
            return;
        }
        if self.last_status.as_ref() == Some(&desired) {
            return;
        }
        if let Err(err) = set_tab_status(desired.0, desired.1.as_deref()) {
            tracing::debug!(error = %err, "failed to set tab status");
            return;
        }
        self.last_status = Some(desired);
        self.last_emit_at = Some(now);
    }

    #[cfg(test)]
    pub(super) fn last_status(&self) -> Option<(TabStatus, Option<String>)> {
        self.last_status.clone()
    }
}

#[cfg(test)]
#[path = "tab_status_state_tests.rs"]
mod tests;
