pub(crate) use std::sync::atomic::{AtomicU64, Ordering};

/// Alias `AtomicU64` to `StaticAtomicU64`
pub(crate) type StaticAtomicU64 = AtomicU64;
