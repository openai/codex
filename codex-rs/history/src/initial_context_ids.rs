//! Initializes the thread allocator and active window from rollout history.

use codex_protocol::ContextWindowId;
use codex_thread_context_ids::ThreadContextIds;

use crate::InitialHistory;
use crate::RolloutItem;

/// Ready-to-use allocator and the window it starts or resumes.
pub struct InitialContextIds {
    pub context_ids: ThreadContextIds,
    pub initial_window_id: ContextWindowId,
}

impl InitialHistory {
    /// Initializes the allocator and active window from the history the thread will replay.
    /// On resume, use the snapshot validated by the rollout writer. For fixtures use
    /// `codex_thread_context_ids_test_support::thread_context_ids()`.
    pub fn initialize_context_ids(&self) -> InitialContextIds {
        // A resume already owns its window.
        let resumed_window = match self {
            InitialHistory::Resumed(resumed) => {
                resumed
                    .history
                    .iter()
                    .rev()
                    .find_map(|record| match record {
                        RolloutItem::Compacted(checkpoint) => {
                            checkpoint.window_id.as_ref()?.validated()
                        }
                        RolloutItem::SessionMeta(meta)
                            if meta.meta.id == resumed.conversation_id =>
                        {
                            meta.meta.context_window.as_ref()?.window_id.validated()
                        }
                        _ => None,
                    })
            }
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => None,
        };
        let context_ids = context_ids_from_rollout(self.get_rollout_items());
        let initial_window_id = resumed_window.unwrap_or_else(|| context_ids.next_window());
        InitialContextIds {
            context_ids,
            initial_window_id,
        }
    }
}

/// Creates an allocator without allocating a window, for isolated rollout reconstruction.
pub fn context_ids_from_rollout(_records: &[RolloutItem]) -> ThreadContextIds {
    ThreadContextIds::for_thread()
}
