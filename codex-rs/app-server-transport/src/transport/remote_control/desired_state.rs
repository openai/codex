use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteControlDesiredState {
    // `Unknown` exists only on plain startup before auth and enrollment scope resolve. Persisted
    // `1` is `Enabled { persistence_preference: Some(true) }`; `0`, `NULL`, or no row are
    // `Disabled`. Runtime-only enable is `Enabled { persistence_preference: None }`, so new rows
    // keep `NULL`; durable RPC enable uses `Some(true)`, so new rows get `1`. Disabled sessions do
    // not create enrollments.
    Unknown,
    Disabled,
    Enabled {
        persistence_preference: Option<bool>,
    },
}
impl RemoteControlDesiredState {
    pub(super) fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
}

pub(super) async fn acquire_persistence_lock(lock: &Semaphore) -> SemaphorePermit<'_> {
    lock.acquire().await.unwrap_or_else(|_| unreachable!())
}
