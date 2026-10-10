//! Notification previews bypass focus filtering without reviving a failed backend.

use super::Tui;
use codex_config::types::NotificationCondition;

impl Tui {
    pub(crate) fn notify_test(&mut self, message: &str) -> bool {
        let condition = self.notification_condition;
        self.notification_condition = NotificationCondition::Always;
        let sent = self.notify(message);
        self.notification_condition = condition;
        sent
    }
}

#[cfg(test)]
#[path = "notification_preview_tests.rs"]
mod tests;
