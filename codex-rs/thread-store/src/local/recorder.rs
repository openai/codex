use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::RolloutRecorder;

use crate::ThreadRecorder;
use crate::ThreadStoreResult;

use super::helpers::io_error;

pub(crate) struct RolloutThreadRecorder {
    pub(crate) thread_id: ThreadId,
    pub(crate) inner: RolloutRecorder,
}

#[async_trait]
impl ThreadRecorder for RolloutThreadRecorder {
    fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    async fn record_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()> {
        self.inner.record_items(items).await.map_err(io_error)
    }

    async fn persist(&self) -> ThreadStoreResult<()> {
        self.inner.persist().await.map_err(io_error)
    }

    async fn flush(&self) -> ThreadStoreResult<()> {
        self.inner.flush().await.map_err(io_error)
    }

    async fn shutdown(&self) -> ThreadStoreResult<()> {
        self.inner.shutdown().await.map_err(io_error)
    }
}
