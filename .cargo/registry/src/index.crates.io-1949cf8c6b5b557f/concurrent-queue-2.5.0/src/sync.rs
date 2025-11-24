//! Synchronization facade to choose between `core` primitives and `loom` primitives.

#[cfg(all(feature = "portable-atomic", not(loom)))]
mod sync_impl {
    pub(crate) use core::cell;
    pub(crate) use portable_atomic as atomic;

    #[cfg(not(feature = "std"))]
    pub(crate) use atomic::hint::spin_loop;

    #[cfg(feature = "std")]
    pub(crate) use std::thread::yield_now;
}

#[cfg(all(not(feature = "portable-atomic"), not(loom)))]
mod sync_impl {
    pub(crate) use core::cell;
    pub(crate) use core::sync::atomic;

    #[cfg(not(feature = "std"))]
    #[inline]
    pub(crate) fn spin_loop() {
        #[allow(deprecated)]
        atomic::spin_loop_hint();
    }

    #[cfg(feature = "std")]
    pub(crate) use std::thread::yield_now;
}

#[cfg(loom)]
mod sync_impl {
    pub(crate) use loom::cell;

    pub(crate) mod atomic {
        pub(crate) use loom::sync::atomic::*;
    }

    #[cfg(not(feature = "std"))]
    pub(crate) use loom::hint::spin_loop;
    #[cfg(feature = "std")]
    pub(crate) use loom::thread::yield_now;
}

pub(crate) use sync_impl::*;

/// Notify the CPU that we are currently busy-waiting.
#[inline]
pub(crate) fn busy_wait() {
    #[cfg(feature = "std")]
    yield_now();

    #[cfg(not(feature = "std"))]
    spin_loop();
}

#[cfg(loom)]
pub(crate) mod prelude {}

#[cfg(not(loom))]
pub(crate) mod prelude {
    use super::{atomic, cell};

    /// Emulate `loom::UnsafeCell`'s API.
    pub(crate) trait UnsafeCellExt {
        type Value;

        fn with_mut<R, F>(&self, f: F) -> R
        where
            F: FnOnce(*mut Self::Value) -> R;
    }

    impl<T> UnsafeCellExt for cell::UnsafeCell<T> {
        type Value = T;

        fn with_mut<R, F>(&self, f: F) -> R
        where
            F: FnOnce(*mut Self::Value) -> R,
        {
            f(self.get())
        }
    }

    /// Emulate `loom::Atomic*`'s API.
    pub(crate) trait AtomicExt {
        type Value;

        fn with_mut<R, F>(&mut self, f: F) -> R
        where
            F: FnOnce(&mut Self::Value) -> R;
    }

    impl AtomicExt for atomic::AtomicUsize {
        type Value = usize;

        fn with_mut<R, F>(&mut self, f: F) -> R
        where
            F: FnOnce(&mut Self::Value) -> R,
        {
            f(self.get_mut())
        }
    }

    impl<T> AtomicExt for atomic::AtomicPtr<T> {
        type Value = *mut T;

        fn with_mut<R, F>(&mut self, f: F) -> R
        where
            F: FnOnce(&mut Self::Value) -> R,
        {
            f(self.get_mut())
        }
    }
}
