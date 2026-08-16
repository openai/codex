use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use codex_protocol::ThreadId;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;

use crate::ThreadMetadataPatch;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

type PendingThreadMetadataEntry = Arc<Mutex<Option<ThreadMetadataPatch>>>;

/// In-memory metadata staged before a reserved thread starts.
///
/// The registry lock is held only while looking up or removing an entry. Never await an entry
/// lock while holding the registry lock. Callers may hold an entry lock before the live-writer
/// lock.
#[derive(Clone, Default)]
pub(super) struct PendingThreadMetadataRegistry {
    entries: Arc<Mutex<HashMap<ThreadId, PendingThreadMetadataEntry>>>,
}

pub(super) type LockedPendingThreadMetadata = OwnedMutexGuard<Option<ThreadMetadataPatch>>;

impl PendingThreadMetadataRegistry {
    pub(super) async fn stage(
        &self,
        thread_id: ThreadId,
        patch: ThreadMetadataPatch,
    ) -> ThreadStoreResult<()> {
        if patch.is_empty() {
            return Err(ThreadStoreError::InvalidRequest {
                message: "pending thread metadata cannot be empty".to_string(),
            });
        }
        let mut entries = self.entries.lock().await;
        match entries.entry(thread_id) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(Mutex::new(Some(patch))));
                Ok(())
            }
            Entry::Occupied(_) => Err(ThreadStoreError::InvalidRequest {
                message: format!("pending thread metadata already exists: {thread_id}"),
            }),
        }
    }

    pub(super) async fn remove(&self, thread_id: ThreadId) {
        self.entries.lock().await.remove(&thread_id);
    }

    pub(super) async fn lock(&self, thread_id: ThreadId) -> Option<LockedPendingThreadMetadata> {
        let entry = self.entries.lock().await.get(&thread_id).cloned()?;
        Some(entry.lock_owned().await)
    }
}
