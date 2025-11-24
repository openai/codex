#[cfg(not(feature = "std"))]
pub use no_std_lock::*;
#[cfg(feature = "std")]
pub use std_lock::*;

#[cfg(feature = "std")]
mod std_lock {
    use std::sync::Mutex as StdMutex;
    pub use std::sync::MutexGuard;

    /// A wrapper around [`std::sync::Mutex`].
    #[derive(Debug)]
    pub struct Mutex<T> {
        inner: StdMutex<T>,
    }

    impl<T> Mutex<T> {
        /// Creates a new mutex in an unlocked state ready for use.
        pub fn new(data: T) -> Self {
            Self {
                inner: StdMutex::new(data),
            }
        }

        /// Acquires the mutex, blocking the current thread until it is able to do so.
        ///
        /// This will return `None` in the case the mutex is poisoned.
        #[inline]
        pub fn lock(&self) -> Option<MutexGuard<'_, T>> {
            self.inner.lock().ok()
        }
    }
}

#[cfg(not(feature = "std"))]
mod no_std_lock {
    use alloc::boxed::Box;
    use core::fmt::Debug;
    use core::ops::DerefMut;

    use crate::sync::Arc;

    /// A no-std compatible wrapper around [`Lock`].
    #[derive(Debug)]
    pub struct Mutex<T> {
        inner: Arc<dyn Lock<T>>,
    }

    impl<T: Send + 'static> Mutex<T> {
        /// Creates a new mutex in an unlocked state ready for use.
        pub fn new<M>(val: T) -> Self
        where
            M: MakeMutex,
            T: Send + 'static,
        {
            Self {
                inner: M::make_mutex(val),
            }
        }

        /// Acquires the mutex, blocking the current thread until it is able to do so.
        ///
        /// This will return `None` in the case the mutex is poisoned.
        #[inline]
        pub fn lock(&self) -> Option<MutexGuard<'_, T>> {
            self.inner.lock().ok()
        }
    }

    /// A lock protecting shared data.
    pub trait Lock<T>: Debug + Send + Sync {
        /// Acquire the lock.
        fn lock(&self) -> Result<MutexGuard<'_, T>, Poisoned>;
    }

    /// A lock builder.
    pub trait MakeMutex {
        /// Create a new mutex.
        fn make_mutex<T>(value: T) -> Arc<dyn Lock<T>>
        where
            T: Send + 'static;
    }

    /// A no-std compatible mutex guard.
    pub type MutexGuard<'a, T> = Box<dyn DerefMut<Target = T> + 'a>;

    /// A marker type used to indicate `Lock::lock` failed due to a poisoned lock.
    pub struct Poisoned;
}
