//! Coordinates exclusive access to thread-list mutations and resumes.

use super::internal_error;
use codex_app_server_protocol::JSONRPCErrorError;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

pub(crate) struct ThreadListState {
    semaphore: Semaphore,
}

impl Default for ThreadListState {
    fn default() -> Self {
        Self {
            semaphore: Semaphore::new(/*permits*/ 1),
        }
    }
}

impl ThreadListState {
    pub(super) async fn acquire_exclusive(&self) -> Result<SemaphorePermit<'_>, JSONRPCErrorError> {
        self.semaphore.acquire().await.map_err(|err| {
            internal_error(format!("failed to acquire thread list state permit: {err}"))
        })
    }
}
