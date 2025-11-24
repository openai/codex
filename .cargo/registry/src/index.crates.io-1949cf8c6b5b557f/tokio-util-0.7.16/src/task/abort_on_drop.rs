//! An [`AbortOnDropHandle`] is like a [`JoinHandle`], except that it
//! will abort the task as soon as it is dropped.

use tokio::task::{AbortHandle, JoinError, JoinHandle};

use std::{
    future::Future,
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};

/// A wrapper around a [`tokio::task::JoinHandle`],
/// which [aborts] the task when it is dropped.
///
/// [aborts]: tokio::task::JoinHandle::abort
#[must_use = "Dropping the handle aborts the task immediately"]
#[derive(Debug)]
pub struct AbortOnDropHandle<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDropHandle<T> {
    fn drop(&mut self) {
        self.0.abort()
    }
}

impl<T> AbortOnDropHandle<T> {
    /// Create an [`AbortOnDropHandle`] from a [`JoinHandle`].
    pub fn new(handle: JoinHandle<T>) -> Self {
        Self(handle)
    }

    /// Abort the task associated with this handle,
    /// equivalent to [`JoinHandle::abort`].
    pub fn abort(&self) {
        self.0.abort()
    }

    /// Checks if the task associated with this handle is finished,
    /// equivalent to [`JoinHandle::is_finished`].
    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }

    /// Returns a new [`AbortHandle`] that can be used to remotely abort this task,
    /// equivalent to [`JoinHandle::abort_handle`].
    pub fn abort_handle(&self) -> AbortHandle {
        self.0.abort_handle()
    }

    /// Cancels aborting on drop and returns the original [`JoinHandle`].
    pub fn detach(self) -> JoinHandle<T> {
        // Avoid invoking `AbortOnDropHandle`'s `Drop` impl
        let this = ManuallyDrop::new(self);
        // SAFETY: `&this.0` is a reference, so it is certainly initialized, and
        // it won't be double-dropped because it's in a `ManuallyDrop`
        unsafe { std::ptr::read(&this.0) }
    }
}

impl<T> Future for AbortOnDropHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl<T> AsRef<JoinHandle<T>> for AbortOnDropHandle<T> {
    fn as_ref(&self) -> &JoinHandle<T> {
        &self.0
    }
}
