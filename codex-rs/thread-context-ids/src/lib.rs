//! Allocates thread context IDs shared by history and inference.

use codex_protocol::ContextWindowId;
use codex_protocol::ResponseItemId;
use uuid::Uuid;

/// Allocates IDs for one thread's context.
#[derive(Debug, Clone)]
pub struct ThreadContextIds(Allocator);

#[derive(Debug, Clone)]
enum Allocator {
    Uuid,
}

impl ThreadContextIds {
    /// Creates a thread allocator. Live sessions should use `InitialHistory::initialize_context_ids`
    /// to initialize the active window. For fixtures use `codex_thread_context_ids_test_support::thread_context_ids()`.
    pub fn for_thread() -> Self {
        Self(Allocator::Uuid)
    }

    pub fn next_item(&self, kind: &str) -> ResponseItemId {
        match &self.0 {
            Allocator::Uuid => ResponseItemId::with_suffix(kind, Uuid::now_v7()),
        }
    }

    pub fn next_window(&self) -> ContextWindowId {
        match &self.0 {
            Allocator::Uuid => ContextWindowId::new_uuid_v7(),
        }
    }
}
