use async_trait::async_trait;
use codex_bridge::BridgeTransport;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;

use super::RemoteThreadStore;
use super::append_items_via_bridge;
use crate::ThreadOwner;
use crate::ThreadRecorder;
use crate::ThreadStoreResult;

pub(crate) struct RemoteThreadRecorder<T> {
    thread_id: ThreadId,
    owner: ThreadOwner,
    event_persistence_mode: String,
    store: RemoteThreadStore<T>,
}

impl<T> RemoteThreadRecorder<T> {
    pub(crate) fn new(
        thread_id: ThreadId,
        owner: ThreadOwner,
        event_persistence_mode: String,
        store: RemoteThreadStore<T>,
    ) -> Self {
        Self {
            thread_id,
            owner,
            event_persistence_mode,
            store,
        }
    }
}

#[async_trait]
impl<T> ThreadRecorder for RemoteThreadRecorder<T>
where
    T: BridgeTransport + 'static,
{
    fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    async fn record_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        append_items_via_bridge(
            &self.store,
            self.thread_id,
            self.owner.clone(),
            self.event_persistence_mode.clone(),
            items,
        )
        .await
    }

    async fn persist(&self) -> ThreadStoreResult<()> {
        Ok(())
    }

    async fn flush(&self) -> ThreadStoreResult<()> {
        Ok(())
    }

    async fn shutdown(&self) -> ThreadStoreResult<()> {
        Ok(())
    }
}
