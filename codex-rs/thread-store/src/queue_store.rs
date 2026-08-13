use std::fmt::Display;
use std::future::Future;

use codex_protocol::ThreadId;
use codex_rollout::StateDbHandle;
use codex_state::QueuedUserSubmissionRecord;
use codex_state::SqliteQueueStore;

use crate::MAX_QUEUE_ITEMS;
use crate::ThreadStoreError;
use crate::ThreadStoreFuture;

/// Storage-neutral persistence for ordered, thread-scoped user messages.
pub trait QueueStore: Send + Sync {
    fn enqueue(
        &self,
        thread_id: ThreadId,
        payload: String,
    ) -> ThreadStoreFuture<'_, QueuedUserSubmissionRecord>;

    fn list_page(
        &self,
        thread_id: ThreadId,
        offset: usize,
        limit: usize,
    ) -> ThreadStoreFuture<'_, Vec<QueuedUserSubmissionRecord>>;

    fn update(
        &self,
        thread_id: ThreadId,
        item_id: String,
        payload: String,
    ) -> ThreadStoreFuture<'_, Option<QueuedUserSubmissionRecord>>;

    fn delete(&self, thread_id: ThreadId, item_id: String) -> ThreadStoreFuture<'_, bool>;

    /// Atomically replace queue order with every current item ID exactly once.
    ///
    /// Returns [`ThreadStoreError::InvalidRequest`] when `item_ids` is not a
    /// permutation of the complete queue.
    fn reorder(&self, thread_id: ThreadId, item_ids: Vec<String>) -> ThreadStoreFuture<'_, ()>;
}

/// Adapts the local state runtime to the shared queue-storage interface.
#[derive(Clone)]
pub struct LocalQueueStore {
    state_db: StateDbHandle,
}

impl LocalQueueStore {
    pub fn new(state_db: StateDbHandle) -> Self {
        Self { state_db }
    }

    fn queue(&self) -> &SqliteQueueStore {
        self.state_db.thread_queue()
    }
}

fn queue_future<'a, T, E>(
    future: impl Future<Output = Result<T, E>> + Send + 'a,
) -> ThreadStoreFuture<'a, T>
where
    T: Send + 'a,
    E: Display + Send + 'a,
{
    Box::pin(async move {
        future.await.map_err(|error| ThreadStoreError::Internal {
            message: format!("queue storage failed: {error}"),
        })
    })
}

impl QueueStore for LocalQueueStore {
    fn enqueue(
        &self,
        thread_id: ThreadId,
        payload: String,
    ) -> ThreadStoreFuture<'_, QueuedUserSubmissionRecord> {
        Box::pin(async move {
            self.queue()
                .enqueue(thread_id, &payload)
                .await
                .map_err(|error| match error.downcast_ref::<sqlx::Error>() {
                    Some(sqlx::Error::RowNotFound) => ThreadStoreError::InvalidRequest {
                        message: format!(
                            "queue cannot contain more than {MAX_QUEUE_ITEMS} submissions"
                        ),
                    },
                    _ => ThreadStoreError::Internal {
                        message: format!("queue storage failed: {error}"),
                    },
                })
        })
    }

    fn list_page(
        &self,
        thread_id: ThreadId,
        offset: usize,
        limit: usize,
    ) -> ThreadStoreFuture<'_, Vec<QueuedUserSubmissionRecord>> {
        queue_future(self.queue().list_page(thread_id, offset, limit))
    }

    fn update(
        &self,
        thread_id: ThreadId,
        item_id: String,
        payload: String,
    ) -> ThreadStoreFuture<'_, Option<QueuedUserSubmissionRecord>> {
        queue_future(async move { self.queue().update(thread_id, &item_id, &payload).await })
    }

    fn delete(&self, thread_id: ThreadId, item_id: String) -> ThreadStoreFuture<'_, bool> {
        queue_future(async move { self.queue().delete(thread_id, &item_id).await })
    }

    fn reorder(&self, thread_id: ThreadId, item_ids: Vec<String>) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.queue()
                .reorder(thread_id, &item_ids)
                .await
                .map_err(|error| match error.downcast_ref::<std::io::Error>() {
                    Some(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                        ThreadStoreError::InvalidRequest {
                            message: error.to_string(),
                        }
                    }
                    _ => ThreadStoreError::Internal {
                        message: format!("queue storage failed: {error}"),
                    },
                })
        })
    }
}
