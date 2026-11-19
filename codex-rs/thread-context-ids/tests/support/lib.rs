//! Fresh thread context allocators for tests that do not exercise history recovery.

use codex_thread_context_ids::ThreadContextIds;

/// Creates an independent UUID-backed allocator.
pub fn thread_context_ids() -> ThreadContextIds {
    ThreadContextIds::for_thread()
}
