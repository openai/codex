//! This module contains a type that can make `Send + !Sync` types `Sync` by
//! disallowing all immutable access to the value.
//!
//! A similar primitive is provided in the `sync_wrapper` crate.

use std::any::Any;

pub(crate) struct SyncWrapper<T> {
    value: T,
}

// safety: The SyncWrapper being send allows you to send the inner value across
// thread boundaries.
unsafe impl<T: Send> Send for SyncWrapper<T> {}

// safety: An immutable reference to a SyncWrapper is useless, so moving such an
// immutable reference across threads is safe.
unsafe impl<T> Sync for SyncWrapper<T> {}

impl<T> SyncWrapper<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value }
    }

    pub(crate) fn into_inner(self) -> T {
        self.value
    }
}

impl SyncWrapper<Box<dyn Any + Send>> {
    /// Attempt to downcast using `Any::downcast_ref()` to a type that is known to be `Sync`.
    pub(crate) fn downcast_ref_sync<T: Any + Sync>(&self) -> Option<&T> {
        // SAFETY: if the downcast fails, the inner value is not touched,
        // so no thread-safety violation can occur.
        self.value.downcast_ref()
    }
}
