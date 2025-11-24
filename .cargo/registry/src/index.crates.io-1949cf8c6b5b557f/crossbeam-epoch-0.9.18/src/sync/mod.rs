//! Synchronization primitives.

pub(crate) mod list;
#[cfg(feature = "std")]
#[cfg(not(crossbeam_loom))]
pub(crate) mod once_lock;
pub(crate) mod queue;
