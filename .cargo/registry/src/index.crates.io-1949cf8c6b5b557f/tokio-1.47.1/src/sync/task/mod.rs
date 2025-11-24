//! Thread-safe task notification primitives.

mod atomic_waker;
pub(crate) use self::atomic_waker::AtomicWaker;
