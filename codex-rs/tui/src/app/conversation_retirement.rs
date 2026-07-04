//! Bounded retirement tombstones for conversations removed from the visible pane set.
//!
//! Tombstones reject late app-server work from a closed pane without permanently preventing an
//! authoritative resume of the same thread.

use super::*;

const RETIRED_THREAD_ID_CAPACITY: usize = 32;

impl App {
    pub(super) fn is_thread_retired(&self, thread_id: &ThreadId) -> bool {
        self.retired_thread_ids.contains(thread_id)
    }

    pub(super) fn retire_thread(&mut self, thread_id: ThreadId) {
        if self.is_thread_retired(&thread_id) {
            return;
        }
        if self.retired_thread_ids.len() >= RETIRED_THREAD_ID_CAPACITY {
            self.retired_thread_ids.pop_front();
        }
        self.retired_thread_ids.push_back(thread_id);
    }

    pub(super) fn restore_thread(&mut self, thread_id: ThreadId) {
        self.retired_thread_ids
            .retain(|retired_thread_id| *retired_thread_id != thread_id);
    }
}

#[cfg(test)]
#[path = "conversation_retirement_tests.rs"]
mod tests;
