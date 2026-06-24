use std::collections::HashMap;

use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::Turn;
use codex_protocol::ThreadId;

/// Tracks the typed item lifecycles emitted by app-server so a reconnecting client can rebuild
/// which materialized items are still active.
#[derive(Default)]
pub(super) struct ActiveItemLifecycleTracker {
    started_by_thread: HashMap<ThreadId, Vec<ItemStartedNotification>>,
}

impl ActiveItemLifecycleTracker {
    pub(super) fn note_notification(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) {
        match notification {
            ServerNotification::ItemStarted(started) => {
                let active_items = self.started_by_thread.entry(thread_id).or_default();
                if let Some(existing) = active_items.iter_mut().find(|existing| {
                    existing.turn_id == started.turn_id && existing.item.id() == started.item.id()
                }) {
                    *existing = started.clone();
                } else {
                    active_items.push(started.clone());
                }
            }
            ServerNotification::ItemCompleted(completed) => {
                if let Some(active_items) = self.started_by_thread.get_mut(&thread_id) {
                    active_items.retain(|started| {
                        started.turn_id != completed.turn_id
                            || started.item.id() != completed.item.id()
                    });
                }
            }
            ServerNotification::TurnCompleted(completed) => {
                self.remove_turn(thread_id, &completed.turn.id);
            }
            ServerNotification::Error(error) if !error.will_retry => {
                self.remove_turn(thread_id, &error.turn_id);
            }
            _ => return,
        }

        if self
            .started_by_thread
            .get(&thread_id)
            .is_some_and(Vec::is_empty)
        {
            self.started_by_thread.remove(&thread_id);
        }
    }

    /// Returns active start notifications with their item payload replaced by the latest
    /// materialized content from the listener's turn snapshot.
    pub(super) fn active_starts_for_turn(
        &self,
        thread_id: ThreadId,
        active_turn: &Turn,
    ) -> Vec<ItemStartedNotification> {
        let Some(active_items) = self.started_by_thread.get(&thread_id) else {
            return Vec::new();
        };
        active_items
            .iter()
            .filter(|started| started.turn_id == active_turn.id)
            .cloned()
            .map(|mut started| {
                if let Some(latest_item) = active_turn
                    .items
                    .iter()
                    .find(|item| item.id() == started.item.id())
                {
                    started.item = latest_item.clone();
                }
                started
            })
            .collect()
    }

    fn remove_turn(&mut self, thread_id: ThreadId, turn_id: &str) {
        if let Some(active_items) = self.started_by_thread.get_mut(&thread_id) {
            active_items.retain(|started| started.turn_id != turn_id);
        }
    }
}

#[cfg(test)]
#[path = "active_item_lifecycle_tests.rs"]
mod tests;
