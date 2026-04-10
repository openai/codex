use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;

use crate::ThreadStoreResult;

/// Live append handle for a thread.
#[async_trait]
pub trait ThreadRecorder: Send + Sync {
    /// Returns the thread id this recorder appends to.
    fn thread_id(&self) -> ThreadId;

    /// Queues items for persistence according to this recorder's filtering policy.
    async fn record_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()>;

    /// Materializes the thread if persistence is lazy, then persists all queued items.
    async fn persist(&self) -> ThreadStoreResult<()>;

    /// Flushes all queued items and returns once they are durable/readable.
    async fn flush(&self) -> ThreadStoreResult<()>;

    /// Flushes pending items and closes the recorder.
    async fn shutdown(&self) -> ThreadStoreResult<()>;
}
