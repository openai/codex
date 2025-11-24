#[cfg(not(feature = "tokio"))]
pub(crate) use async_lock::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
#[cfg(feature = "tokio")]
pub(crate) use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// An abstraction over async semaphore API.
#[cfg(not(feature = "tokio"))]
pub(crate) struct Semaphore(async_lock::Semaphore);
#[cfg(feature = "tokio")]
pub(crate) struct Semaphore(tokio::sync::Semaphore);

impl Semaphore {
    pub const fn new(permits: usize) -> Self {
        #[cfg(not(feature = "tokio"))]
        let semaphore = async_lock::Semaphore::new(permits);
        #[cfg(feature = "tokio")]
        let semaphore = tokio::sync::Semaphore::const_new(permits);

        Self(semaphore)
    }

    pub async fn acquire(&self) -> SemaphorePermit<'_> {
        #[cfg(not(feature = "tokio"))]
        {
            self.0.acquire().await
        }
        #[cfg(feature = "tokio")]
        {
            // SAFETY: Since we never explicitly close the sempaphore, `acquire` can't fail.
            self.0.acquire().await.unwrap()
        }
    }
}

#[cfg(not(feature = "tokio"))]
pub(crate) type SemaphorePermit<'a> = async_lock::SemaphoreGuard<'a>;
#[cfg(feature = "tokio")]
pub(crate) type SemaphorePermit<'a> = tokio::sync::SemaphorePermit<'a>;
