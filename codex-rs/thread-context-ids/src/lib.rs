//! Allocates thread context IDs shared by history and inference.

use codex_protocol::ContextWindowId;
use codex_protocol::ResponseItemId;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
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

    /// Assigns IDs to newly rendered window prefix items before ordinary item allocation.
    pub fn assign_prefix_ids<'a>(&self, _items: impl Iterator<Item = &'a mut ResponseItem>) {
        match &self.0 {
            Allocator::Uuid => {}
        }
    }

    /// Derives IDs for the prompt-only items rebuilt on each request.
    pub fn request_ids(&self, thread: ThreadId) -> RequestIds {
        RequestIds(match &self.0 {
            Allocator::Uuid => RequestAllocator::Uuid(Uuid::new_v5(
                &Uuid::NAMESPACE_OID,
                thread.to_string().as_bytes(),
            )),
        })
    }
}

/// Allocates IDs for the prompt-only items in one request.
pub struct RequestIds(RequestAllocator);

enum RequestAllocator {
    Uuid(Uuid),
}

impl RequestIds {
    pub fn id_by_content(&self, kind: &str, content: &[u8]) -> ResponseItemId {
        match &self.0 {
            RequestAllocator::Uuid(namespace) => {
                ResponseItemId::with_suffix(kind, Uuid::new_v5(namespace, content))
            }
        }
    }
}
