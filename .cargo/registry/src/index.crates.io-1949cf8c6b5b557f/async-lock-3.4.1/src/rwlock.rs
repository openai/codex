use core::cell::UnsafeCell;
use core::fmt;
use core::mem::{self, ManuallyDrop};
use core::ops::{Deref, DerefMut};
use core::ptr::{self, NonNull};

use alloc::sync::Arc;

pub(crate) mod futures;
mod raw;

use self::futures::{
    Read, ReadArc, UpgradableRead, UpgradableReadArc, Upgrade, UpgradeArc, Write, WriteArc,
};
use self::raw::{RawRwLock, RawUpgrade};

/// An async reader-writer lock.
///
/// This type of lock allows multiple readers or one writer at any point in time.
///
/// The locking strategy is write-preferring, which means writers are never starved.
/// Releasing a write lock wakes the next blocked reader and the next blocked writer.
///
/// # Examples
///
/// ```
/// # futures_lite::future::block_on(async {
/// use async_lock::RwLock;
///
/// let lock = RwLock::new(5);
///
/// // Multiple read locks can be held at a time.
/// let r1 = lock.read().await;
/// let r2 = lock.read().await;
/// assert_eq!(*r1, 5);
/// assert_eq!(*r2, 5);
/// drop((r1, r2));
///
/// // Only one write lock can be held at a time.
/// let mut w = lock.write().await;
/// *w += 1;
/// assert_eq!(*w, 6);
/// # })
/// ```
pub struct RwLock<T: ?Sized> {
    /// The underlying locking implementation.
    /// Doesn't depend on `T`.
    raw: RawRwLock,

    /// The inner value.
    value: UnsafeCell<T>,
}

unsafe impl<T: Send + ?Sized> Send for RwLock<T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    const_fn! {
        const_if: #[cfg(not(loom))];
        /// Creates a new reader-writer lock.
        ///
        /// # Examples
        ///
        /// ```
        /// use async_lock::RwLock;
        ///
        /// let lock = RwLock::new(0);
        /// ```
        #[must_use]
        #[inline]
        pub const fn new(t: T) -> RwLock<T> {
            RwLock {
                raw: RawRwLock::new(),
                value: UnsafeCell::new(t),
            }
        }
    }

    /// Unwraps the lock and returns the inner value.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// assert_eq!(lock.into_inner(), 5);
    /// ```
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }

    /// Attempts to acquire an an owned, reference-counted read lock.
    ///
    /// If a read lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.read_arc().await;
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_read_arc().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn try_read_arc(self: &Arc<Self>) -> Option<RwLockReadGuardArc<T>> {
        if self.raw.try_read() {
            let arc = self.clone();

            // SAFETY: we previously acquired a read lock.
            Some(unsafe { RwLockReadGuardArc::from_arc(arc) })
        } else {
            None
        }
    }

    /// Acquires an owned, reference-counted read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also concurrent attempts
    /// to acquire a write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.read_arc().await;
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_read_arc().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn read_arc<'a>(self: &'a Arc<Self>) -> ReadArc<'a, T> {
        ReadArc::new(self.raw.read(), self)
    }

    /// Acquires an owned, reference-counted read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also concurrent attempts
    /// to acquire a write lock.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`read_arc`][`RwLock::read_arc`] method,
    /// this method will block the current thread until the read lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a lock can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use async_lock::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.read_arc_blocking();
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_read().is_some());
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn read_arc_blocking(self: &Arc<Self>) -> RwLockReadGuardArc<T> {
        self.read_arc().wait()
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Attempts to acquire a read lock.
    ///
    /// If a read lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.read().await;
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_read().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        if self.raw.try_read() {
            Some(RwLockReadGuard {
                lock: &self.raw,
                value: self.value.get(),
            })
        } else {
            None
        }
    }

    /// Acquires a read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also concurrent attempts
    /// to acquire a write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.read().await;
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_read().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn read(&self) -> Read<'_, T> {
        Read::new(self.raw.read(), self.value.get())
    }

    /// Acquires a read lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Note that attempts to acquire a read lock will block if there are also concurrent attempts
    /// to acquire a write lock.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`read`][`RwLock::read`] method,
    /// this method will block the current thread until the read lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a lock can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.read_blocking();
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_read().is_some());
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn read_blocking(&self) -> RwLockReadGuard<'_, T> {
        self.read().wait()
    }

    /// Attempts to acquire a read lock with the possibility to upgrade to a write lock.
    ///
    /// If a read lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// Upgradable read lock reserves the right to be upgraded to a write lock, which means there
    /// can be at most one upgradable read lock at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read().await;
    /// assert_eq!(*reader, 1);
    /// assert_eq!(*lock.try_read().unwrap(), 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuard::upgrade(reader).await;
    /// *writer = 2;
    /// # })
    /// ```
    #[inline]
    pub fn try_upgradable_read(&self) -> Option<RwLockUpgradableReadGuard<'_, T>> {
        if self.raw.try_upgradable_read() {
            Some(RwLockUpgradableReadGuard {
                lock: &self.raw,
                value: self.value.get(),
            })
        } else {
            None
        }
    }

    /// Acquires a read lock with the possibility to upgrade to a write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Upgradable read lock reserves the right to be upgraded to a write lock, which means there
    /// can be at most one upgradable read lock at a time.
    ///
    /// Note that attempts to acquire an upgradable read lock will block if there are concurrent
    /// attempts to acquire another upgradable read lock or a write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read().await;
    /// assert_eq!(*reader, 1);
    /// assert_eq!(*lock.try_read().unwrap(), 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuard::upgrade(reader).await;
    /// *writer = 2;
    /// # })
    /// ```
    #[inline]
    pub fn upgradable_read(&self) -> UpgradableRead<'_, T> {
        UpgradableRead::new(self.raw.upgradable_read(), self.value.get())
    }

    /// Attempts to acquire a read lock with the possibility to upgrade to a write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Upgradable read lock reserves the right to be upgraded to a write lock, which means there
    /// can be at most one upgradable read lock at a time.
    ///
    /// Note that attempts to acquire an upgradable read lock will block if there are concurrent
    /// attempts to acquire another upgradable read lock or a write lock.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`upgradable_read`][`RwLock::upgradable_read`]
    /// method, this method will block the current thread until the read lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a lock can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read_blocking();
    /// assert_eq!(*reader, 1);
    /// assert_eq!(*lock.try_read().unwrap(), 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuard::upgrade_blocking(reader);
    /// *writer = 2;
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn upgradable_read_blocking(&self) -> RwLockUpgradableReadGuard<'_, T> {
        self.upgradable_read().wait()
    }

    /// Attempts to acquire an owned, reference-counted read lock
    /// with the possibility to upgrade to a write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Upgradable read lock reserves the right to be upgraded to a write lock, which means there
    /// can be at most one upgradable read lock at a time.
    ///
    /// Note that attempts to acquire an upgradable read lock will block if there are concurrent
    /// attempts to acquire another upgradable read lock or a write lock.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`upgradable_read_arc`][`RwLock::upgradable_read_arc`]
    /// method, this method will block the current thread until the read lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a lock can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc_blocking();
    /// assert_eq!(*reader, 1);
    /// assert_eq!(*lock.try_read().unwrap(), 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuardArc::upgrade_blocking(reader);
    /// *writer = 2;
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn upgradable_read_arc_blocking(self: &Arc<Self>) -> RwLockUpgradableReadGuardArc<T> {
        self.upgradable_read_arc().wait()
    }

    /// Attempts to acquire an owned, reference-counted read lock with the possibility to
    /// upgrade to a write lock.
    ///
    /// If a read lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// Upgradable read lock reserves the right to be upgraded to a write lock, which means there
    /// can be at most one upgradable read lock at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc().await;
    /// assert_eq!(*reader, 1);
    /// assert_eq!(*lock.try_read_arc().unwrap(), 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuardArc::upgrade(reader).await;
    /// *writer = 2;
    /// # })
    /// ```
    #[inline]
    pub fn try_upgradable_read_arc(self: &Arc<Self>) -> Option<RwLockUpgradableReadGuardArc<T>> {
        if self.raw.try_upgradable_read() {
            Some(RwLockUpgradableReadGuardArc { lock: self.clone() })
        } else {
            None
        }
    }

    /// Acquires an owned, reference-counted read lock with the possibility
    /// to upgrade to a write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// Upgradable read lock reserves the right to be upgraded to a write lock, which means there
    /// can be at most one upgradable read lock at a time.
    ///
    /// Note that attempts to acquire an upgradable read lock will block if there are concurrent
    /// attempts to acquire another upgradable read lock or a write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc().await;
    /// assert_eq!(*reader, 1);
    /// assert_eq!(*lock.try_read_arc().unwrap(), 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuardArc::upgrade(reader).await;
    /// *writer = 2;
    /// # })
    /// ```
    #[inline]
    pub fn upgradable_read_arc<'a>(self: &'a Arc<Self>) -> UpgradableReadArc<'a, T> {
        UpgradableReadArc::new(self.raw.upgradable_read(), self)
    }

    /// Attempts to acquire a write lock.
    ///
    /// If a write lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// assert!(lock.try_write().is_some());
    /// let reader = lock.read().await;
    /// assert!(lock.try_write().is_none());
    /// # })
    /// ```
    #[inline]
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        if self.raw.try_write() {
            Some(RwLockWriteGuard {
                lock: &self.raw,
                value: self.value.get(),
            })
        } else {
            None
        }
    }

    /// Acquires a write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let writer = lock.write().await;
    /// assert!(lock.try_read().is_none());
    /// # })
    /// ```
    #[inline]
    pub fn write(&self) -> Write<'_, T> {
        Write::new(self.raw.write(), self.value.get())
    }

    /// Acquires a write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`write`] method, this method will
    /// block the current thread until the write lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a lock can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let writer = lock.write_blocking();
    /// assert!(lock.try_read().is_none());
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn write_blocking(&self) -> RwLockWriteGuard<'_, T> {
        self.write().wait()
    }

    /// Attempts to acquire an owned, reference-counted write lock.
    ///
    /// If a write lock could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// assert!(lock.try_write_arc().is_some());
    /// let reader = lock.read_arc().await;
    /// assert!(lock.try_write_arc().is_none());
    /// # })
    /// ```
    #[inline]
    pub fn try_write_arc(self: &Arc<Self>) -> Option<RwLockWriteGuardArc<T>> {
        if self.raw.try_write() {
            Some(RwLockWriteGuardArc { lock: self.clone() })
        } else {
            None
        }
    }

    /// Acquires an owned, reference-counted write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let writer = lock.write_arc().await;
    /// assert!(lock.try_read_arc().is_none());
    /// # })
    /// ```
    #[inline]
    pub fn write_arc<'a>(self: &'a Arc<Self>) -> WriteArc<'a, T> {
        WriteArc::new(self.raw.write(), self)
    }

    /// Acquires an owned, reference-counted write lock.
    ///
    /// Returns a guard that releases the lock when dropped.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`write_arc`][RwLock::write_arc] method, this method will
    /// block the current thread until the write lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a lock can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use async_lock::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let writer = lock.write_arc_blocking();
    /// assert!(lock.try_read().is_none());
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn write_arc_blocking(self: &Arc<Self>) -> RwLockWriteGuardArc<T> {
        self.write_arc().wait()
    }

    /// Returns a mutable reference to the inner value.
    ///
    /// Since this call borrows the lock mutably, no actual locking takes place. The mutable borrow
    /// statically guarantees no locks exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::RwLock;
    ///
    /// let mut lock = RwLock::new(1);
    ///
    /// *lock.get_mut() = 2;
    /// assert_eq!(*lock.read().await, 2);
    /// # })
    /// ```
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Locked;
        impl fmt::Debug for Locked {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<locked>")
            }
        }

        match self.try_read() {
            None => f.debug_struct("RwLock").field("value", &Locked).finish(),
            Some(guard) => f.debug_struct("RwLock").field("value", &&*guard).finish(),
        }
    }
}

impl<T> From<T> for RwLock<T> {
    #[inline]
    fn from(val: T) -> RwLock<T> {
        RwLock::new(val)
    }
}

impl<T: Default> Default for RwLock<T> {
    #[inline]
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

/// A guard that releases the read lock when dropped.
#[clippy::has_significant_drop]
pub struct RwLockReadGuard<'a, T: ?Sized> {
    /// Reference to underlying locking implementation.
    /// Doesn't depend on `T`.
    lock: &'a RawRwLock,

    /// Pointer to the value protected by the lock. Covariant in `T`.
    value: *const T,
}

unsafe impl<T: Sync + ?Sized> Send for RwLockReadGuard<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for RwLockReadGuard<'_, T> {}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping a read guard.
        unsafe {
            self.lock.read_unlock();
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for RwLockReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for RwLockReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

/// An owned, reference-counting guard that releases the read lock when dropped.
#[clippy::has_significant_drop]
pub struct RwLockReadGuardArc<T> {
    /// **WARNING**: This doesn't actually point to a `T`!
    /// It points to a `RwLock<T>`, via a pointer obtained with `Arc::into_raw`.
    /// We lie for covariance.
    lock: NonNull<T>,
}

unsafe impl<T: Send + Sync> Send for RwLockReadGuardArc<T> {}
unsafe impl<T: Send + Sync> Sync for RwLockReadGuardArc<T> {}

impl<T> RwLockReadGuardArc<T> {
    /// Constructs the underlying `Arc` back from the underlying `RwLock`.
    ///
    /// # Safety
    ///
    /// Both the returned `Arc` and the guard will decrement their reference
    /// counts on drop! So one of the two must be forgotten.
    #[inline]
    unsafe fn inner_arc(guard: &Self) -> ManuallyDrop<Arc<RwLock<T>>> {
        ManuallyDrop::new(Arc::from_raw(guard.lock.as_ptr().cast()))
    }

    /// Constructs a guard from the underlying `Arc`.
    ///
    /// # Safety
    ///
    /// A read lock must be acquired before calling this.
    #[inline]
    unsafe fn from_arc(arc: Arc<RwLock<T>>) -> Self {
        let ptr = Arc::into_raw(arc);

        Self {
            lock: NonNull::new(ptr as *mut RwLock<T> as *mut T).unwrap(),
        }
    }
}

impl<T> Drop for RwLockReadGuardArc<T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are in `drop`, decrementing the reference count
        // on purpose.
        // We hold a read lock on the `RwLock`.
        unsafe {
            let arc = ManuallyDrop::into_inner(Self::inner_arc(self));
            arc.raw.read_unlock();
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for RwLockReadGuardArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display> fmt::Display for RwLockReadGuardArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T> Deref for RwLockReadGuardArc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: we use `ManuallyDrop` to avoid double-drop.
        // We hold a read lock on the `RwLock`.
        unsafe {
            let arc = Self::inner_arc(self);
            &*arc.value.get()
        }
    }
}

/// A guard that releases the upgradable read lock when dropped.
#[clippy::has_significant_drop]
pub struct RwLockUpgradableReadGuard<'a, T: ?Sized> {
    /// Reference to underlying locking implementation.
    /// Doesn't depend on `T`.
    /// This guard holds a lock on the witer mutex!
    lock: &'a RawRwLock,

    /// Pointer to the value protected by the lock. Invariant in `T`
    /// as the upgradable lock could provide write access.
    value: *mut T,
}

impl<T: ?Sized> Drop for RwLockUpgradableReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping an upgradable read guard.
        unsafe {
            self.lock.upgradable_read_unlock();
        }
    }
}

unsafe impl<T: Send + Sync + ?Sized> Send for RwLockUpgradableReadGuard<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for RwLockUpgradableReadGuard<'_, T> {}

impl<'a, T: ?Sized> RwLockUpgradableReadGuard<'a, T> {
    /// Downgrades into a regular reader guard.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read().await;
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_upgradable_read().is_none());
    ///
    /// let reader = RwLockUpgradableReadGuard::downgrade(reader);
    ///
    /// assert!(lock.try_upgradable_read().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn downgrade(guard: Self) -> RwLockReadGuard<'a, T> {
        let upgradable = ManuallyDrop::new(guard);

        // SAFETY: `guard` is an upgradable read lock.
        unsafe {
            upgradable.lock.downgrade_upgradable_read();
        };

        RwLockReadGuard {
            lock: upgradable.lock,
            value: upgradable.value,
        }
    }

    /// Attempts to upgrade into a write lock.
    ///
    /// If a write lock could not be acquired at this time, then [`None`] is returned. Otherwise,
    /// an upgraded guard is returned that releases the write lock when dropped.
    ///
    /// This function can only fail if there are other active read locks.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read().await;
    /// assert_eq!(*reader, 1);
    ///
    /// let reader2 = lock.read().await;
    /// let reader = RwLockUpgradableReadGuard::try_upgrade(reader).unwrap_err();
    ///
    /// drop(reader2);
    /// let writer = RwLockUpgradableReadGuard::try_upgrade(reader).unwrap();
    /// # })
    /// ```
    #[inline]
    pub fn try_upgrade(guard: Self) -> Result<RwLockWriteGuard<'a, T>, Self> {
        // If there are no readers, grab the write lock.
        // SAFETY: `guard` is an upgradable read guard
        if unsafe { guard.lock.try_upgrade() } {
            let reader = ManuallyDrop::new(guard);

            Ok(RwLockWriteGuard {
                lock: reader.lock,
                value: reader.value,
            })
        } else {
            Err(guard)
        }
    }

    /// Upgrades into a write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read().await;
    /// assert_eq!(*reader, 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuard::upgrade(reader).await;
    /// *writer = 2;
    /// # })
    /// ```
    #[inline]
    pub fn upgrade(guard: Self) -> Upgrade<'a, T> {
        let reader = ManuallyDrop::new(guard);

        Upgrade::new(
            // SAFETY: `reader` is an upgradable read guard
            unsafe { reader.lock.upgrade() },
            reader.value,
        )
    }

    /// Upgrades into a write lock.
    ///
    /// # Blocking
    ///
    /// This function will block the current thread until it is able to acquire the write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let reader = lock.upgradable_read_blocking();
    /// assert_eq!(*reader, 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuard::upgrade_blocking(reader);
    /// *writer = 2;
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn upgrade_blocking(guard: Self) -> RwLockWriteGuard<'a, T> {
        RwLockUpgradableReadGuard::upgrade(guard).wait()
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for RwLockUpgradableReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for RwLockUpgradableReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for RwLockUpgradableReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

/// An owned, reference-counting guard that releases the upgradable read lock when dropped.
#[clippy::has_significant_drop]
pub struct RwLockUpgradableReadGuardArc<T: ?Sized> {
    /// We want invariance, so no need for pointer tricks.
    lock: Arc<RwLock<T>>,
}

impl<T: ?Sized> Drop for RwLockUpgradableReadGuardArc<T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping an upgradable read guard.
        unsafe {
            self.lock.raw.upgradable_read_unlock();
        }
    }
}

unsafe impl<T: Send + Sync + ?Sized> Send for RwLockUpgradableReadGuardArc<T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for RwLockUpgradableReadGuardArc<T> {}

impl<T: fmt::Debug + ?Sized> fmt::Debug for RwLockUpgradableReadGuardArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for RwLockUpgradableReadGuardArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for RwLockUpgradableReadGuardArc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> RwLockUpgradableReadGuardArc<T> {
    /// Downgrades into a regular reader guard.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc().await;
    /// assert_eq!(*reader, 1);
    ///
    /// assert!(lock.try_upgradable_read_arc().is_none());
    ///
    /// let reader = RwLockUpgradableReadGuardArc::downgrade(reader);
    ///
    /// assert!(lock.try_upgradable_read_arc().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn downgrade(guard: Self) -> RwLockReadGuardArc<T> {
        // SAFETY: we hold an upgradable read lock, which we are downgrading.
        unsafe {
            guard.lock.raw.downgrade_upgradable_read();
        }

        // SAFETY: we just downgraded to a read lock.
        unsafe { RwLockReadGuardArc::from_arc(Self::into_arc(guard)) }
    }
}

impl<T: ?Sized> RwLockUpgradableReadGuardArc<T> {
    /// Consumes the lock (without dropping) and returns the underlying `Arc`.
    #[inline]
    fn into_arc(guard: Self) -> Arc<RwLock<T>> {
        let guard = ManuallyDrop::new(guard);
        // SAFETY: `guard` is not used after this
        unsafe { ptr::read(&guard.lock) }
    }

    /// Attempts to upgrade into a write lock.
    ///
    /// If a write lock could not be acquired at this time, then [`None`] is returned. Otherwise,
    /// an upgraded guard is returned that releases the write lock when dropped.
    ///
    /// This function can only fail if there are other active read locks.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc().await;
    /// assert_eq!(*reader, 1);
    ///
    /// let reader2 = lock.read_arc().await;
    /// let reader = RwLockUpgradableReadGuardArc::try_upgrade(reader).unwrap_err();
    ///
    /// drop(reader2);
    /// let writer = RwLockUpgradableReadGuardArc::try_upgrade(reader).unwrap();
    /// # })
    /// ```
    #[inline]
    pub fn try_upgrade(guard: Self) -> Result<RwLockWriteGuardArc<T>, Self> {
        // SAFETY: We hold an upgradable read guard.
        if unsafe { guard.lock.raw.try_upgrade() } {
            Ok(RwLockWriteGuardArc {
                lock: Self::into_arc(guard),
            })
        } else {
            Err(guard)
        }
    }

    /// Upgrades into a write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc().await;
    /// assert_eq!(*reader, 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuardArc::upgrade(reader).await;
    /// *writer = 2;
    /// # })
    /// ```
    #[inline]
    pub fn upgrade(guard: Self) -> UpgradeArc<T> {
        // We need to do some ugly lying about lifetimes;
        // See the comment on the `raw` field of `ArcUpgrade`
        // for an explanation.

        // SAFETY: we hold an upgradable read guard.
        let raw: RawUpgrade<'_> = unsafe { guard.lock.raw.upgrade() };

        // SAFETY: see above explanation.
        let raw: RawUpgrade<'static> = unsafe { mem::transmute(raw) };

        unsafe {
            UpgradeArc::new(
                ManuallyDrop::new(raw),
                ManuallyDrop::new(Self::into_arc(guard)),
            )
        }
    }

    /// Upgrades into a write lock.
    ///
    /// # Blocking
    ///
    /// This function will block the current thread until it is able to acquire the write lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let reader = lock.upgradable_read_arc_blocking();
    /// assert_eq!(*reader, 1);
    ///
    /// let mut writer = RwLockUpgradableReadGuardArc::upgrade_blocking(reader);
    /// *writer = 2;
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn upgrade_blocking(guard: Self) -> RwLockWriteGuardArc<T> {
        RwLockUpgradableReadGuardArc::upgrade(guard).wait()
    }
}

/// A guard that releases the write lock when dropped.
#[clippy::has_significant_drop]
pub struct RwLockWriteGuard<'a, T: ?Sized> {
    /// Reference to underlying locking implementation.
    /// Doesn't depend on `T`.
    /// This guard holds a lock on the witer mutex!
    lock: &'a RawRwLock,

    /// Pointer to the value protected by the lock. Invariant in `T`.
    value: *mut T,
}

unsafe impl<T: Send + ?Sized> Send for RwLockWriteGuard<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for RwLockWriteGuard<'_, T> {}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping a write lock
        unsafe {
            self.lock.write_unlock();
        }
    }
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    /// Downgrades into a regular reader guard.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockWriteGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut writer = lock.write().await;
    /// *writer += 1;
    ///
    /// assert!(lock.try_read().is_none());
    ///
    /// let reader = RwLockWriteGuard::downgrade(writer);
    /// assert_eq!(*reader, 2);
    ///
    /// assert!(lock.try_read().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn downgrade(guard: Self) -> RwLockReadGuard<'a, T> {
        let write = ManuallyDrop::new(guard);

        // SAFETY: `write` is a write guard
        unsafe {
            write.lock.downgrade_write();
        }

        RwLockReadGuard {
            lock: write.lock,
            value: write.value,
        }
    }

    /// Downgrades into an upgradable reader guard.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{RwLock, RwLockUpgradableReadGuard, RwLockWriteGuard};
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut writer = lock.write().await;
    /// *writer += 1;
    ///
    /// assert!(lock.try_read().is_none());
    ///
    /// let reader = RwLockWriteGuard::downgrade_to_upgradable(writer);
    /// assert_eq!(*reader, 2);
    ///
    /// assert!(lock.try_write().is_none());
    /// assert!(lock.try_read().is_some());
    ///
    /// assert!(RwLockUpgradableReadGuard::try_upgrade(reader).is_ok())
    /// # })
    /// ```
    #[inline]
    pub fn downgrade_to_upgradable(guard: Self) -> RwLockUpgradableReadGuard<'a, T> {
        let write = ManuallyDrop::new(guard);

        // SAFETY: `write` is a write guard
        unsafe {
            write.lock.downgrade_to_upgradable();
        }

        RwLockUpgradableReadGuard {
            lock: write.lock,
            value: write.value,
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for RwLockWriteGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for RwLockWriteGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }
}

/// An owned, reference-counted guard that releases the write lock when dropped.
#[clippy::has_significant_drop]
pub struct RwLockWriteGuardArc<T: ?Sized> {
    lock: Arc<RwLock<T>>,
}

unsafe impl<T: Send + Sync + ?Sized> Send for RwLockWriteGuardArc<T> {}
unsafe impl<T: Send + Sync + ?Sized> Sync for RwLockWriteGuardArc<T> {}

impl<T: ?Sized> Drop for RwLockWriteGuardArc<T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping a write lock.
        unsafe {
            self.lock.raw.write_unlock();
        }
    }
}

impl<T> RwLockWriteGuardArc<T> {
    /// Downgrades into a regular reader guard.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockWriteGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let mut writer = lock.write_arc().await;
    /// *writer += 1;
    ///
    /// assert!(lock.try_read_arc().is_none());
    ///
    /// let reader = RwLockWriteGuardArc::downgrade(writer);
    /// assert_eq!(*reader, 2);
    ///
    /// assert!(lock.try_read_arc().is_some());
    /// # })
    /// ```
    #[inline]
    pub fn downgrade(guard: Self) -> RwLockReadGuardArc<T> {
        // SAFETY: `write` is a write guard
        unsafe {
            guard.lock.raw.downgrade_write();
        }

        // SAFETY: we just downgraded to a read lock
        unsafe { RwLockReadGuardArc::from_arc(Self::into_arc(guard)) }
    }
}

impl<T: ?Sized> RwLockWriteGuardArc<T> {
    /// Consumes the lock (without dropping) and returns the underlying `Arc`.
    #[inline]
    fn into_arc(guard: Self) -> Arc<RwLock<T>> {
        let guard = ManuallyDrop::new(guard);
        // SAFETY: `guard` is not used after this
        unsafe { ptr::read(&guard.lock) }
    }

    /// Downgrades into an upgradable reader guard.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use std::sync::Arc;
    /// use async_lock::{RwLock, RwLockUpgradableReadGuardArc, RwLockWriteGuardArc};
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let mut writer = lock.write_arc().await;
    /// *writer += 1;
    ///
    /// assert!(lock.try_read_arc().is_none());
    ///
    /// let reader = RwLockWriteGuardArc::downgrade_to_upgradable(writer);
    /// assert_eq!(*reader, 2);
    ///
    /// assert!(lock.try_write_arc().is_none());
    /// assert!(lock.try_read_arc().is_some());
    ///
    /// assert!(RwLockUpgradableReadGuardArc::try_upgrade(reader).is_ok())
    /// # })
    /// ```
    #[inline]
    pub fn downgrade_to_upgradable(guard: Self) -> RwLockUpgradableReadGuardArc<T> {
        // SAFETY: `guard` is a write guard
        unsafe {
            guard.lock.raw.downgrade_to_upgradable();
        }

        RwLockUpgradableReadGuardArc {
            lock: Self::into_arc(guard),
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for RwLockWriteGuardArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for RwLockWriteGuardArc<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuardArc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuardArc<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}
