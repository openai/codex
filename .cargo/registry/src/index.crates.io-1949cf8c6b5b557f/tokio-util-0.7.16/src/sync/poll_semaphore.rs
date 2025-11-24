use futures_core::Stream;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore, TryAcquireError};

use super::ReusableBoxFuture;

/// A wrapper around [`Semaphore`] that provides a `poll_acquire` method.
///
/// [`Semaphore`]: tokio::sync::Semaphore
pub struct PollSemaphore {
    semaphore: Arc<Semaphore>,
    permit_fut: Option<(
        u32, // The number of permits requested.
        ReusableBoxFuture<'static, Result<OwnedSemaphorePermit, AcquireError>>,
    )>,
}

impl PollSemaphore {
    /// Create a new `PollSemaphore`.
    pub fn new(semaphore: Arc<Semaphore>) -> Self {
        Self {
            semaphore,
            permit_fut: None,
        }
    }

    /// Closes the semaphore.
    pub fn close(&self) {
        self.semaphore.close();
    }

    /// Obtain a clone of the inner semaphore.
    pub fn clone_inner(&self) -> Arc<Semaphore> {
        self.semaphore.clone()
    }

    /// Get back the inner semaphore.
    pub fn into_inner(self) -> Arc<Semaphore> {
        self.semaphore
    }

    /// Poll to acquire a permit from the semaphore.
    ///
    /// This can return the following values:
    ///
    ///  - `Poll::Pending` if a permit is not currently available.
    ///  - `Poll::Ready(Some(permit))` if a permit was acquired.
    ///  - `Poll::Ready(None)` if the semaphore has been closed.
    ///
    /// When this method returns `Poll::Pending`, the current task is scheduled
    /// to receive a wakeup when a permit becomes available, or when the
    /// semaphore is closed. Note that on multiple calls to `poll_acquire`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup.
    pub fn poll_acquire(&mut self, cx: &mut Context<'_>) -> Poll<Option<OwnedSemaphorePermit>> {
        self.poll_acquire_many(cx, 1)
    }

    /// Poll to acquire many permits from the semaphore.
    ///
    /// This can return the following values:
    ///
    ///  - `Poll::Pending` if a permit is not currently available.
    ///  - `Poll::Ready(Some(permit))` if a permit was acquired.
    ///  - `Poll::Ready(None)` if the semaphore has been closed.
    ///
    /// When this method returns `Poll::Pending`, the current task is scheduled
    /// to receive a wakeup when the permits become available, or when the
    /// semaphore is closed. Note that on multiple calls to `poll_acquire`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup.
    pub fn poll_acquire_many(
        &mut self,
        cx: &mut Context<'_>,
        permits: u32,
    ) -> Poll<Option<OwnedSemaphorePermit>> {
        let permit_future = match self.permit_fut.as_mut() {
            Some((prev_permits, fut)) if *prev_permits == permits => fut,
            Some((old_permits, fut_box)) => {
                // We're requesting a different number of permits, so replace the future
                // and record the new amount.
                let fut = Arc::clone(&self.semaphore).acquire_many_owned(permits);
                fut_box.set(fut);
                *old_permits = permits;
                fut_box
            }
            None => {
                // avoid allocations completely if we can grab a permit immediately
                match Arc::clone(&self.semaphore).try_acquire_many_owned(permits) {
                    Ok(permit) => return Poll::Ready(Some(permit)),
                    Err(TryAcquireError::Closed) => return Poll::Ready(None),
                    Err(TryAcquireError::NoPermits) => {}
                }

                let next_fut = Arc::clone(&self.semaphore).acquire_many_owned(permits);
                &mut self
                    .permit_fut
                    .get_or_insert((permits, ReusableBoxFuture::new(next_fut)))
                    .1
            }
        };

        let result = ready!(permit_future.poll(cx));

        // Assume we'll request the same amount of permits in a subsequent call.
        let next_fut = Arc::clone(&self.semaphore).acquire_many_owned(permits);
        permit_future.set(next_fut);

        match result {
            Ok(permit) => Poll::Ready(Some(permit)),
            Err(_closed) => {
                self.permit_fut = None;
                Poll::Ready(None)
            }
        }
    }

    /// Returns the current number of available permits.
    ///
    /// This is equivalent to the [`Semaphore::available_permits`] method on the
    /// `tokio::sync::Semaphore` type.
    ///
    /// [`Semaphore::available_permits`]: tokio::sync::Semaphore::available_permits
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Adds `n` new permits to the semaphore.
    ///
    /// The maximum number of permits is [`Semaphore::MAX_PERMITS`], and this function
    /// will panic if the limit is exceeded.
    ///
    /// This is equivalent to the [`Semaphore::add_permits`] method on the
    /// `tokio::sync::Semaphore` type.
    ///
    /// [`Semaphore::add_permits`]: tokio::sync::Semaphore::add_permits
    pub fn add_permits(&self, n: usize) {
        self.semaphore.add_permits(n);
    }
}

impl Stream for PollSemaphore {
    type Item = OwnedSemaphorePermit;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<OwnedSemaphorePermit>> {
        Pin::into_inner(self).poll_acquire(cx)
    }
}

impl Clone for PollSemaphore {
    fn clone(&self) -> PollSemaphore {
        PollSemaphore::new(self.clone_inner())
    }
}

impl fmt::Debug for PollSemaphore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollSemaphore")
            .field("semaphore", &self.semaphore)
            .finish()
    }
}

impl AsRef<Semaphore> for PollSemaphore {
    fn as_ref(&self) -> &Semaphore {
        &self.semaphore
    }
}
