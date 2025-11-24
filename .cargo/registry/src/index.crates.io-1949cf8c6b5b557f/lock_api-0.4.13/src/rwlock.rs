// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

#[cfg(feature = "arc_lock")]
use alloc::sync::Arc;
#[cfg(feature = "arc_lock")]
use core::mem::ManuallyDrop;
#[cfg(feature = "arc_lock")]
use core::ptr;

#[cfg(feature = "owning_ref")]
use owning_ref::StableAddress;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Basic operations for a reader-writer lock.
///
/// Types implementing this trait can be used by `RwLock` to form a safe and
/// fully-functioning `RwLock` type.
///
/// # Safety
///
/// Implementations of this trait must ensure that the `RwLock` is actually
/// exclusive: an exclusive lock can't be acquired while an exclusive or shared
/// lock exists, and a shared lock can't be acquire while an exclusive lock
/// exists.
pub unsafe trait RawRwLock {
    /// Initial value for an unlocked `RwLock`.
    // A “non-constant” const item is a legacy way to supply an initialized value to downstream
    // static items. Can hopefully be replaced with `const fn new() -> Self` at some point.
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    /// Marker type which determines whether a lock guard should be `Send`. Use
    /// one of the `GuardSend` or `GuardNoSend` helper types here.
    type GuardMarker;

    /// Acquires a shared lock, blocking the current thread until it is able to do so.
    fn lock_shared(&self);

    /// Attempts to acquire a shared lock without blocking.
    fn try_lock_shared(&self) -> bool;

    /// Releases a shared lock.
    ///
    /// # Safety
    ///
    /// This method may only be called if a shared lock is held in the current context.
    unsafe fn unlock_shared(&self);

    /// Acquires an exclusive lock, blocking the current thread until it is able to do so.
    fn lock_exclusive(&self);

    /// Attempts to acquire an exclusive lock without blocking.
    fn try_lock_exclusive(&self) -> bool;

    /// Releases an exclusive lock.
    ///
    /// # Safety
    ///
    /// This method may only be called if an exclusive lock is held in the current context.
    unsafe fn unlock_exclusive(&self);

    /// Checks if this `RwLock` is currently locked in any way.
    #[inline]
    fn is_locked(&self) -> bool {
        let acquired_lock = self.try_lock_exclusive();
        if acquired_lock {
            // Safety: A lock was successfully acquired above.
            unsafe {
                self.unlock_exclusive();
            }
        }
        !acquired_lock
    }

    /// Check if this `RwLock` is currently exclusively locked.
    fn is_locked_exclusive(&self) -> bool {
        let acquired_lock = self.try_lock_shared();
        if acquired_lock {
            // Safety: A shared lock was successfully acquired above.
            unsafe {
                self.unlock_shared();
            }
        }
        !acquired_lock
    }
}

/// Additional methods for `RwLock`s which support fair unlocking.
///
/// Fair unlocking means that a lock is handed directly over to the next waiting
/// thread if there is one, without giving other threads the opportunity to
/// "steal" the lock in the meantime. This is typically slower than unfair
/// unlocking, but may be necessary in certain circumstances.
pub unsafe trait RawRwLockFair: RawRwLock {
    /// Releases a shared lock using a fair unlock protocol.
    ///
    /// # Safety
    ///
    /// This method may only be called if a shared lock is held in the current context.
    unsafe fn unlock_shared_fair(&self);

    /// Releases an exclusive lock using a fair unlock protocol.
    ///
    /// # Safety
    ///
    /// This method may only be called if an exclusive lock is held in the current context.
    unsafe fn unlock_exclusive_fair(&self);

    /// Temporarily yields a shared lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_shared_fair` followed
    /// by `lock_shared`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    ///
    /// # Safety
    ///
    /// This method may only be called if a shared lock is held in the current context.
    unsafe fn bump_shared(&self) {
        self.unlock_shared_fair();
        self.lock_shared();
    }

    /// Temporarily yields an exclusive lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_exclusive_fair` followed
    /// by `lock_exclusive`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    ///
    /// # Safety
    ///
    /// This method may only be called if an exclusive lock is held in the current context.
    unsafe fn bump_exclusive(&self) {
        self.unlock_exclusive_fair();
        self.lock_exclusive();
    }
}

/// Additional methods for `RwLock`s which support atomically downgrading an
/// exclusive lock to a shared lock.
pub unsafe trait RawRwLockDowngrade: RawRwLock {
    /// Atomically downgrades an exclusive lock into a shared lock without
    /// allowing any thread to take an exclusive lock in the meantime.
    ///
    /// # Safety
    ///
    /// This method may only be called if an exclusive lock is held in the current context.
    unsafe fn downgrade(&self);
}

/// Additional methods for `RwLock`s which support locking with timeouts.
///
/// The `Duration` and `Instant` types are specified as associated types so that
/// this trait is usable even in `no_std` environments.
pub unsafe trait RawRwLockTimed: RawRwLock {
    /// Duration type used for `try_lock_for`.
    type Duration;

    /// Instant type used for `try_lock_until`.
    type Instant;

    /// Attempts to acquire a shared lock until a timeout is reached.
    fn try_lock_shared_for(&self, timeout: Self::Duration) -> bool;

    /// Attempts to acquire a shared lock until a timeout is reached.
    fn try_lock_shared_until(&self, timeout: Self::Instant) -> bool;

    /// Attempts to acquire an exclusive lock until a timeout is reached.
    fn try_lock_exclusive_for(&self, timeout: Self::Duration) -> bool;

    /// Attempts to acquire an exclusive lock until a timeout is reached.
    fn try_lock_exclusive_until(&self, timeout: Self::Instant) -> bool;
}

/// Additional methods for `RwLock`s which support recursive read locks.
///
/// These are guaranteed to succeed without blocking if
/// another read lock is held at the time of the call. This allows a thread
/// to recursively lock a `RwLock`. However using this method can cause
/// writers to starve since readers no longer block if a writer is waiting
/// for the lock.
pub unsafe trait RawRwLockRecursive: RawRwLock {
    /// Acquires a shared lock without deadlocking in case of a recursive lock.
    fn lock_shared_recursive(&self);

    /// Attempts to acquire a shared lock without deadlocking in case of a recursive lock.
    fn try_lock_shared_recursive(&self) -> bool;
}

/// Additional methods for `RwLock`s which support recursive read locks and timeouts.
pub unsafe trait RawRwLockRecursiveTimed: RawRwLockRecursive + RawRwLockTimed {
    /// Attempts to acquire a shared lock until a timeout is reached, without
    /// deadlocking in case of a recursive lock.
    fn try_lock_shared_recursive_for(&self, timeout: Self::Duration) -> bool;

    /// Attempts to acquire a shared lock until a timeout is reached, without
    /// deadlocking in case of a recursive lock.
    fn try_lock_shared_recursive_until(&self, timeout: Self::Instant) -> bool;
}

/// Additional methods for `RwLock`s which support atomically upgrading a shared
/// lock to an exclusive lock.
///
/// This requires acquiring a special "upgradable read lock" instead of a
/// normal shared lock. There may only be one upgradable lock at any time,
/// otherwise deadlocks could occur when upgrading.
pub unsafe trait RawRwLockUpgrade: RawRwLock {
    /// Acquires an upgradable lock, blocking the current thread until it is able to do so.
    fn lock_upgradable(&self);

    /// Attempts to acquire an upgradable lock without blocking.
    fn try_lock_upgradable(&self) -> bool;

    /// Releases an upgradable lock.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn unlock_upgradable(&self);

    /// Upgrades an upgradable lock to an exclusive lock.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn upgrade(&self);

    /// Attempts to upgrade an upgradable lock to an exclusive lock without
    /// blocking.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn try_upgrade(&self) -> bool;
}

/// Additional methods for `RwLock`s which support upgradable locks and fair
/// unlocking.
pub unsafe trait RawRwLockUpgradeFair: RawRwLockUpgrade + RawRwLockFair {
    /// Releases an upgradable lock using a fair unlock protocol.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn unlock_upgradable_fair(&self);

    /// Temporarily yields an upgradable lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_upgradable_fair` followed
    /// by `lock_upgradable`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn bump_upgradable(&self) {
        self.unlock_upgradable_fair();
        self.lock_upgradable();
    }
}

/// Additional methods for `RwLock`s which support upgradable locks and lock
/// downgrading.
pub unsafe trait RawRwLockUpgradeDowngrade: RawRwLockUpgrade + RawRwLockDowngrade {
    /// Downgrades an upgradable lock to a shared lock.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn downgrade_upgradable(&self);

    /// Downgrades an exclusive lock to an upgradable lock.
    ///
    /// # Safety
    ///
    /// This method may only be called if an exclusive lock is held in the current context.
    unsafe fn downgrade_to_upgradable(&self);
}

/// Additional methods for `RwLock`s which support upgradable locks and locking
/// with timeouts.
pub unsafe trait RawRwLockUpgradeTimed: RawRwLockUpgrade + RawRwLockTimed {
    /// Attempts to acquire an upgradable lock until a timeout is reached.
    fn try_lock_upgradable_for(&self, timeout: Self::Duration) -> bool;

    /// Attempts to acquire an upgradable lock until a timeout is reached.
    fn try_lock_upgradable_until(&self, timeout: Self::Instant) -> bool;

    /// Attempts to upgrade an upgradable lock to an exclusive lock until a
    /// timeout is reached.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn try_upgrade_for(&self, timeout: Self::Duration) -> bool;

    /// Attempts to upgrade an upgradable lock to an exclusive lock until a
    /// timeout is reached.
    ///
    /// # Safety
    ///
    /// This method may only be called if an upgradable lock is held in the current context.
    unsafe fn try_upgrade_until(&self, timeout: Self::Instant) -> bool;
}

/// A reader-writer lock
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. The write portion of this lock typically allows modification
/// of the underlying data (exclusive access) and the read portion of this lock
/// typically allows for read-only access (shared access).
///
/// The type parameter `T` represents the data that this lock protects. It is
/// required that `T` satisfies `Send` to be shared across threads and `Sync` to
/// allow concurrent access through readers. The RAII guards returned from the
/// locking methods implement `Deref` (and `DerefMut` for the `write` methods)
/// to allow access to the contained of the lock.
pub struct RwLock<R, T: ?Sized> {
    raw: R,
    data: UnsafeCell<T>,
}

// Copied and modified from serde
#[cfg(feature = "serde")]
impl<R, T> Serialize for RwLock<R, T>
where
    R: RawRwLock,
    T: Serialize + ?Sized,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.read().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, R, T> Deserialize<'de> for RwLock<R, T>
where
    R: RawRwLock,
    T: Deserialize<'de> + ?Sized,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(RwLock::new)
    }
}

unsafe impl<R: RawRwLock + Send, T: ?Sized + Send> Send for RwLock<R, T> {}
unsafe impl<R: RawRwLock + Sync, T: ?Sized + Send + Sync> Sync for RwLock<R, T> {}

impl<R: RawRwLock, T> RwLock<R, T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    #[cfg(has_const_fn_trait_bound)]
    #[inline]
    pub const fn new(val: T) -> RwLock<R, T> {
        RwLock {
            data: UnsafeCell::new(val),
            raw: R::INIT,
        }
    }

    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    #[cfg(not(has_const_fn_trait_bound))]
    #[inline]
    pub fn new(val: T) -> RwLock<R, T> {
        RwLock {
            data: UnsafeCell::new(val),
            raw: R::INIT,
        }
    }

    /// Consumes this `RwLock`, returning the underlying data.
    #[inline]
    #[allow(unused_unsafe)]
    pub fn into_inner(self) -> T {
        unsafe { self.data.into_inner() }
    }
}

impl<R, T> RwLock<R, T> {
    /// Creates a new new instance of an `RwLock<T>` based on a pre-existing
    /// `RawRwLock<T>`.
    #[inline]
    pub const fn from_raw(raw_rwlock: R, val: T) -> RwLock<R, T> {
        RwLock {
            data: UnsafeCell::new(val),
            raw: raw_rwlock,
        }
    }

    /// Creates a new new instance of an `RwLock<T>` based on a pre-existing
    /// `RawRwLock<T>`.
    ///
    /// This allows creating a `RwLock<T>` in a constant context on stable
    /// Rust.
    ///
    /// This method is a legacy alias for [`from_raw`](Self::from_raw).
    #[inline]
    pub const fn const_new(raw_rwlock: R, val: T) -> RwLock<R, T> {
        Self::from_raw(raw_rwlock, val)
    }
}

impl<R: RawRwLock, T: ?Sized> RwLock<R, T> {
    /// Creates a new `RwLockReadGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds a read lock.
    ///
    /// This function does not increment the read count of the lock. Calling this function when a
    /// guard has already been produced is undefined behaviour unless the guard was forgotten
    /// with `mem::forget`.
    #[inline]
    pub unsafe fn make_read_guard_unchecked(&self) -> RwLockReadGuard<'_, R, T> {
        RwLockReadGuard {
            rwlock: self,
            marker: PhantomData,
        }
    }

    /// Creates a new `RwLockReadGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds a write lock.
    ///
    /// Calling this function when a guard has already been produced is undefined behaviour unless
    /// the guard was forgotten with `mem::forget`.
    #[inline]
    pub unsafe fn make_write_guard_unchecked(&self) -> RwLockWriteGuard<'_, R, T> {
        RwLockWriteGuard {
            rwlock: self,
            marker: PhantomData,
        }
    }

    /// Locks this `RwLock` with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns.
    ///
    /// Note that attempts to recursively acquire a read lock on a `RwLock` when
    /// the current thread already holds one may result in a deadlock.
    ///
    /// Returns an RAII guard which will release this thread's shared access
    /// once it is dropped.
    #[inline]
    pub fn read(&self) -> RwLockReadGuard<'_, R, T> {
        self.raw.lock_shared();
        // SAFETY: The lock is held, as required.
        unsafe { self.make_read_guard_unchecked() }
    }

    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access could not be granted at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access
    /// when it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, R, T>> {
        if self.raw.try_lock_shared() {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock`
    /// when dropped.
    #[inline]
    pub fn write(&self) -> RwLockWriteGuard<'_, R, T> {
        self.raw.lock_exclusive();
        // SAFETY: The lock is held, as required.
        unsafe { self.make_write_guard_unchecked() }
    }

    /// Attempts to lock this `RwLock` with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when
    /// it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, R, T>> {
        if self.raw.try_lock_exclusive() {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_write_guard_unchecked() })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `RwLock` mutably, no actual locking needs to
    /// take place---the mutable borrow statically guarantees no locks exist.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    /// Checks whether this `RwLock` is currently locked in any way.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// Check if this `RwLock` is currently exclusively locked.
    #[inline]
    pub fn is_locked_exclusive(&self) -> bool {
        self.raw.is_locked_exclusive()
    }

    /// Forcibly unlocks a read lock.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `RwLockReadGuard` object alive, for example when
    /// dealing with FFI.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread logically owns a
    /// `RwLockReadGuard` but that guard has be discarded using `mem::forget`.
    /// Behavior is undefined if a rwlock is read-unlocked when not read-locked.
    #[inline]
    pub unsafe fn force_unlock_read(&self) {
        self.raw.unlock_shared();
    }

    /// Forcibly unlocks a write lock.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `RwLockWriteGuard` object alive, for example when
    /// dealing with FFI.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread logically owns a
    /// `RwLockWriteGuard` but that guard has be discarded using `mem::forget`.
    /// Behavior is undefined if a rwlock is write-unlocked when not write-locked.
    #[inline]
    pub unsafe fn force_unlock_write(&self) {
        self.raw.unlock_exclusive();
    }

    /// Returns the underlying raw reader-writer lock object.
    ///
    /// Note that you will most likely need to import the `RawRwLock` trait from
    /// `lock_api` to be able to call functions on the raw
    /// reader-writer lock.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it allows unlocking a mutex while
    /// still holding a reference to a lock guard.
    pub unsafe fn raw(&self) -> &R {
        &self.raw
    }

    /// Returns a raw pointer to the underlying data.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `RwLockReadGuard` or `RwLockWriteGuard` object
    /// alive, for example when dealing with FFI.
    ///
    /// # Safety
    ///
    /// You must ensure that there are no data races when dereferencing the
    /// returned pointer, for example if the current thread logically owns a
    /// `RwLockReadGuard` or `RwLockWriteGuard` but that guard has been discarded
    /// using `mem::forget`.
    #[inline]
    pub fn data_ptr(&self) -> *mut T {
        self.data.get()
    }

    /// Creates a new `RwLockReadGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds a read lock.
    ///
    /// This function does not increment the read count of the lock. Calling this function when a
    /// guard has already been produced is undefined behaviour unless the guard was forgotten
    /// with `mem::forget`.`
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub unsafe fn make_arc_read_guard_unchecked(self: &Arc<Self>) -> ArcRwLockReadGuard<R, T> {
        ArcRwLockReadGuard {
            rwlock: self.clone(),
            marker: PhantomData,
        }
    }

    /// Creates a new `RwLockWriteGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds a write lock.
    ///
    /// Calling this function when a guard has already been produced is undefined behaviour unless
    /// the guard was forgotten with `mem::forget`.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub unsafe fn make_arc_write_guard_unchecked(self: &Arc<Self>) -> ArcRwLockWriteGuard<R, T> {
        ArcRwLockWriteGuard {
            rwlock: self.clone(),
            marker: PhantomData,
        }
    }

    /// Locks this `RwLock` with read access, through an `Arc`.
    ///
    /// This method is similar to the `read` method; however, it requires the `RwLock` to be inside of an `Arc`
    /// and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn read_arc(self: &Arc<Self>) -> ArcRwLockReadGuard<R, T> {
        self.raw.lock_shared();
        // SAFETY: locking guarantee is upheld
        unsafe { self.make_arc_read_guard_unchecked() }
    }

    /// Attempts to lock this `RwLock` with read access, through an `Arc`.
    ///
    /// This method is similar to the `try_read` method; however, it requires the `RwLock` to be inside of an
    /// `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_read_arc(self: &Arc<Self>) -> Option<ArcRwLockReadGuard<R, T>> {
        if self.raw.try_lock_shared() {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Locks this `RwLock` with write access, through an `Arc`.
    ///
    /// This method is similar to the `write` method; however, it requires the `RwLock` to be inside of an `Arc`
    /// and the resulting write guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn write_arc(self: &Arc<Self>) -> ArcRwLockWriteGuard<R, T> {
        self.raw.lock_exclusive();
        // SAFETY: locking guarantee is upheld
        unsafe { self.make_arc_write_guard_unchecked() }
    }

    /// Attempts to lock this `RwLock` with writ access, through an `Arc`.
    ///
    /// This method is similar to the `try_write` method; however, it requires the `RwLock` to be inside of an
    /// `Arc` and the resulting write guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_write_arc(self: &Arc<Self>) -> Option<ArcRwLockWriteGuard<R, T>> {
        if self.raw.try_lock_exclusive() {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_write_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawRwLockFair, T: ?Sized> RwLock<R, T> {
    /// Forcibly unlocks a read lock using a fair unlock protocol.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `RwLockReadGuard` object alive, for example when
    /// dealing with FFI.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread logically owns a
    /// `RwLockReadGuard` but that guard has be discarded using `mem::forget`.
    /// Behavior is undefined if a rwlock is read-unlocked when not read-locked.
    #[inline]
    pub unsafe fn force_unlock_read_fair(&self) {
        self.raw.unlock_shared_fair();
    }

    /// Forcibly unlocks a write lock using a fair unlock protocol.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `RwLockWriteGuard` object alive, for example when
    /// dealing with FFI.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread logically owns a
    /// `RwLockWriteGuard` but that guard has be discarded using `mem::forget`.
    /// Behavior is undefined if a rwlock is write-unlocked when not write-locked.
    #[inline]
    pub unsafe fn force_unlock_write_fair(&self) {
        self.raw.unlock_exclusive_fair();
    }
}

impl<R: RawRwLockTimed, T: ?Sized> RwLock<R, T> {
    /// Attempts to acquire this `RwLock` with shared read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    #[inline]
    pub fn try_read_for(&self, timeout: R::Duration) -> Option<RwLockReadGuard<'_, R, T>> {
        if self.raw.try_lock_shared_for(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with shared read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    #[inline]
    pub fn try_read_until(&self, timeout: R::Instant) -> Option<RwLockReadGuard<'_, R, T>> {
        if self.raw.try_lock_shared_until(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with exclusive write access until a
    /// timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the exclusive access when it is dropped.
    #[inline]
    pub fn try_write_for(&self, timeout: R::Duration) -> Option<RwLockWriteGuard<'_, R, T>> {
        if self.raw.try_lock_exclusive_for(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_write_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with exclusive write access until a
    /// timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the exclusive access when it is dropped.
    #[inline]
    pub fn try_write_until(&self, timeout: R::Instant) -> Option<RwLockWriteGuard<'_, R, T>> {
        if self.raw.try_lock_exclusive_until(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_write_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with read access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_read_for` method; however, it requires the `RwLock` to be inside of an
    /// `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_read_arc_for(
        self: &Arc<Self>,
        timeout: R::Duration,
    ) -> Option<ArcRwLockReadGuard<R, T>> {
        if self.raw.try_lock_shared_for(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with read access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_read_until` method; however, it requires the `RwLock` to be inside of
    /// an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_read_arc_until(
        self: &Arc<Self>,
        timeout: R::Instant,
    ) -> Option<ArcRwLockReadGuard<R, T>> {
        if self.raw.try_lock_shared_until(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with write access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_write_for` method; however, it requires the `RwLock` to be inside of
    /// an `Arc` and the resulting write guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_write_arc_for(
        self: &Arc<Self>,
        timeout: R::Duration,
    ) -> Option<ArcRwLockWriteGuard<R, T>> {
        if self.raw.try_lock_exclusive_for(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_write_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with read access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_write_until` method; however, it requires the `RwLock` to be inside of
    /// an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_write_arc_until(
        self: &Arc<Self>,
        timeout: R::Instant,
    ) -> Option<ArcRwLockWriteGuard<R, T>> {
        if self.raw.try_lock_exclusive_until(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_write_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawRwLockRecursive, T: ?Sized> RwLock<R, T> {
    /// Locks this `RwLock` with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns.
    ///
    /// Unlike `read`, this method is guaranteed to succeed without blocking if
    /// another read lock is held at the time of the call. This allows a thread
    /// to recursively lock a `RwLock`. However using this method can cause
    /// writers to starve since readers no longer block if a writer is waiting
    /// for the lock.
    ///
    /// Returns an RAII guard which will release this thread's shared access
    /// once it is dropped.
    #[inline]
    pub fn read_recursive(&self) -> RwLockReadGuard<'_, R, T> {
        self.raw.lock_shared_recursive();
        // SAFETY: The lock is held, as required.
        unsafe { self.make_read_guard_unchecked() }
    }

    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access could not be granted at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access
    /// when it is dropped.
    ///
    /// This method is guaranteed to succeed if another read lock is held at the
    /// time of the call. See the documentation for `read_recursive` for details.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_read_recursive(&self) -> Option<RwLockReadGuard<'_, R, T>> {
        if self.raw.try_lock_shared_recursive() {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Locks this `RwLock` with shared read access, through an `Arc`.
    ///
    /// This method is similar to the `read_recursive` method; however, it requires the `RwLock` to be inside of
    /// an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn read_arc_recursive(self: &Arc<Self>) -> ArcRwLockReadGuard<R, T> {
        self.raw.lock_shared_recursive();
        // SAFETY: locking guarantee is upheld
        unsafe { self.make_arc_read_guard_unchecked() }
    }

    /// Attempts to lock this `RwLock` with shared read access, through an `Arc`.
    ///
    /// This method is similar to the `try_read_recursive` method; however, it requires the `RwLock` to be inside
    /// of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_read_recursive_arc(self: &Arc<Self>) -> Option<ArcRwLockReadGuard<R, T>> {
        if self.raw.try_lock_shared_recursive() {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_read_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawRwLockRecursiveTimed, T: ?Sized> RwLock<R, T> {
    /// Attempts to acquire this `RwLock` with shared read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    ///
    /// This method is guaranteed to succeed without blocking if another read
    /// lock is held at the time of the call. See the documentation for
    /// `read_recursive` for details.
    #[inline]
    pub fn try_read_recursive_for(
        &self,
        timeout: R::Duration,
    ) -> Option<RwLockReadGuard<'_, R, T>> {
        if self.raw.try_lock_shared_recursive_for(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with shared read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    #[inline]
    pub fn try_read_recursive_until(
        &self,
        timeout: R::Instant,
    ) -> Option<RwLockReadGuard<'_, R, T>> {
        if self.raw.try_lock_shared_recursive_until(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to lock this `RwLock` with read access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_read_recursive_for` method; however, it requires the `RwLock` to be
    /// inside of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_read_arc_recursive_for(
        self: &Arc<Self>,
        timeout: R::Duration,
    ) -> Option<ArcRwLockReadGuard<R, T>> {
        if self.raw.try_lock_shared_recursive_for(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_read_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to lock this `RwLock` with read access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_read_recursive_until` method; however, it requires the `RwLock` to be
    /// inside of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_read_arc_recursive_until(
        self: &Arc<Self>,
        timeout: R::Instant,
    ) -> Option<ArcRwLockReadGuard<R, T>> {
        if self.raw.try_lock_shared_recursive_until(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_read_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawRwLockUpgrade, T: ?Sized> RwLock<R, T> {
    /// Creates a new `RwLockUpgradableReadGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds an upgradable read lock.
    ///
    /// This function does not increment the read count of the lock. Calling this function when a
    /// guard has already been produced is undefined behaviour unless the guard was forgotten
    /// with `mem::forget`.
    #[inline]
    pub unsafe fn make_upgradable_guard_unchecked(&self) -> RwLockUpgradableReadGuard<'_, R, T> {
        RwLockUpgradableReadGuard {
            rwlock: self,
            marker: PhantomData,
        }
    }

    /// Locks this `RwLock` with upgradable read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers or other
    /// upgradable reads which hold the lock. There may be other readers currently
    /// inside the lock when this method returns.
    ///
    /// Returns an RAII guard which will release this thread's shared access
    /// once it is dropped.
    #[inline]
    pub fn upgradable_read(&self) -> RwLockUpgradableReadGuard<'_, R, T> {
        self.raw.lock_upgradable();
        // SAFETY: The lock is held, as required.
        unsafe { self.make_upgradable_guard_unchecked() }
    }

    /// Attempts to acquire this `RwLock` with upgradable read access.
    ///
    /// If the access could not be granted at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access
    /// when it is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_upgradable_read(&self) -> Option<RwLockUpgradableReadGuard<'_, R, T>> {
        if self.raw.try_lock_upgradable() {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_upgradable_guard_unchecked() })
        } else {
            None
        }
    }

    /// Creates a new `ArcRwLockUpgradableReadGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds an upgradable read lock.
    ///
    /// This function does not increment the read count of the lock. Calling this function when a
    /// guard has already been produced is undefined behaviour unless the guard was forgotten
    /// with `mem::forget`.`
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub unsafe fn make_upgradable_arc_guard_unchecked(
        self: &Arc<Self>,
    ) -> ArcRwLockUpgradableReadGuard<R, T> {
        ArcRwLockUpgradableReadGuard {
            rwlock: self.clone(),
            marker: PhantomData,
        }
    }

    /// Locks this `RwLock` with upgradable read access, through an `Arc`.
    ///
    /// This method is similar to the `upgradable_read` method; however, it requires the `RwLock` to be
    /// inside of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn upgradable_read_arc(self: &Arc<Self>) -> ArcRwLockUpgradableReadGuard<R, T> {
        self.raw.lock_upgradable();
        // SAFETY: locking guarantee is upheld
        unsafe { self.make_upgradable_arc_guard_unchecked() }
    }

    /// Attempts to lock this `RwLock` with upgradable read access, through an `Arc`.
    ///
    /// This method is similar to the `try_upgradable_read` method; however, it requires the `RwLock` to be
    /// inside of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_upgradable_read_arc(self: &Arc<Self>) -> Option<ArcRwLockUpgradableReadGuard<R, T>> {
        if self.raw.try_lock_upgradable() {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_upgradable_arc_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawRwLockUpgradeTimed, T: ?Sized> RwLock<R, T> {
    /// Attempts to acquire this `RwLock` with upgradable read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    #[inline]
    pub fn try_upgradable_read_for(
        &self,
        timeout: R::Duration,
    ) -> Option<RwLockUpgradableReadGuard<'_, R, T>> {
        if self.raw.try_lock_upgradable_for(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_upgradable_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this `RwLock` with upgradable read access until a timeout
    /// is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned. Otherwise, an RAII guard is returned which will
    /// release the shared access when it is dropped.
    #[inline]
    pub fn try_upgradable_read_until(
        &self,
        timeout: R::Instant,
    ) -> Option<RwLockUpgradableReadGuard<'_, R, T>> {
        if self.raw.try_lock_upgradable_until(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_upgradable_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to lock this `RwLock` with upgradable access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_upgradable_read_for` method; however, it requires the `RwLock` to be
    /// inside of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_upgradable_read_arc_for(
        self: &Arc<Self>,
        timeout: R::Duration,
    ) -> Option<ArcRwLockUpgradableReadGuard<R, T>> {
        if self.raw.try_lock_upgradable_for(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_upgradable_arc_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to lock this `RwLock` with upgradable access until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_upgradable_read_until` method; however, it requires the `RwLock` to be
    /// inside of an `Arc` and the resulting read guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_upgradable_read_arc_until(
        self: &Arc<Self>,
        timeout: R::Instant,
    ) -> Option<ArcRwLockUpgradableReadGuard<R, T>> {
        if self.raw.try_lock_upgradable_until(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_upgradable_arc_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawRwLock, T: ?Sized + Default> Default for RwLock<R, T> {
    #[inline]
    fn default() -> RwLock<R, T> {
        RwLock::new(Default::default())
    }
}

impl<R: RawRwLock, T> From<T> for RwLock<R, T> {
    #[inline]
    fn from(t: T) -> RwLock<R, T> {
        RwLock::new(t)
    }
}

impl<R: RawRwLock, T: ?Sized + fmt::Debug> fmt::Debug for RwLock<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RwLock");
        match self.try_read() {
            Some(guard) => d.field("data", &&*guard),
            None => {
                // Additional format_args! here is to remove quotes around <locked> in debug output.
                d.field("data", &format_args!("<locked>"))
            }
        };
        d.finish()
    }
}

/// RAII structure used to release the shared read access of a lock when
/// dropped.
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockReadGuard<'a, R: RawRwLock, T: ?Sized> {
    rwlock: &'a RwLock<R, T>,
    marker: PhantomData<(&'a T, R::GuardMarker)>,
}

unsafe impl<R: RawRwLock + Sync, T: Sync + ?Sized> Sync for RwLockReadGuard<'_, R, T> {}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> RwLockReadGuard<'a, R, T> {
    /// Returns a reference to the original reader-writer lock object.
    pub fn rwlock(s: &Self) -> &'a RwLock<R, T> {
        s.rwlock
    }

    /// Make a new `MappedRwLockReadGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `RwLockReadGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockReadGuard::map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedRwLockReadGuard<'a, R, U>
    where
        F: FnOnce(&T) -> &U,
    {
        let raw = &s.rwlock.raw;
        let data = f(unsafe { &*s.rwlock.data.get() });
        mem::forget(s);
        MappedRwLockReadGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make  a new `MappedRwLockReadGuard` for a component of the
    /// locked data. Returns the original guard if the closure returns `None`.
    ///
    /// This operation cannot fail as the `RwLockReadGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockReadGuard::try_map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn try_map<U: ?Sized, F>(s: Self, f: F) -> Result<MappedRwLockReadGuard<'a, R, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        let raw = &s.rwlock.raw;
        let data = match f(unsafe { &*s.rwlock.data.get() }) {
            Some(data) => data,
            None => return Err(s),
        };
        mem::forget(s);
        Ok(MappedRwLockReadGuard {
            raw,
            data,
            marker: PhantomData,
        })
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the `RwLock`.
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_shared();
        }
        defer!(s.rwlock.raw.lock_shared());
        f()
    }
}

impl<'a, R: RawRwLockFair + 'a, T: ?Sized + 'a> RwLockReadGuard<'a, R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// By default, `RwLock` is unfair and allow the current thread to re-lock
    /// the `RwLock` before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the `RwLock` for a long time. This is
    /// the default because it allows much higher throughput as it avoids
    /// forcing a context switch on every `RwLock` unlock. This can result in one
    /// thread acquiring a `RwLock` many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `RwLockReadGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_shared_fair();
        }
        mem::forget(s);
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// The `RwLock` is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the `RwLock`.
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_shared_fair();
        }
        defer!(s.rwlock.raw.lock_shared());
        f()
    }

    /// Temporarily yields the `RwLock` to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_fair` followed
    /// by `read`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.bump_shared();
        }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Deref for RwLockReadGuard<'a, R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Drop for RwLockReadGuard<'a, R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            self.rwlock.raw.unlock_shared();
        }
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug for RwLockReadGuard<'a, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for RwLockReadGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> StableAddress for RwLockReadGuard<'a, R, T> {}

/// An RAII rwlock guard returned by the `Arc` locking operations on `RwLock`.
///
/// This is similar to the `RwLockReadGuard` struct, except instead of using a reference to unlock the `RwLock`
/// it uses an `Arc<RwLock>`. This has several advantages, most notably that it has an `'static` lifetime.
#[cfg(feature = "arc_lock")]
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct ArcRwLockReadGuard<R: RawRwLock, T: ?Sized> {
    rwlock: Arc<RwLock<R, T>>,
    marker: PhantomData<R::GuardMarker>,
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> ArcRwLockReadGuard<R, T> {
    /// Returns a reference to the rwlock, contained in its `Arc`.
    pub fn rwlock(s: &Self) -> &Arc<RwLock<R, T>> {
        &s.rwlock
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is functionally identical to the `unlocked` method on [`RwLockReadGuard`].
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_shared();
        }
        defer!(s.rwlock.raw.lock_shared());
        f()
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockFair, T: ?Sized> ArcRwLockReadGuard<R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// This is functionally identical to the `unlock_fair` method on [`RwLockReadGuard`].
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_shared_fair();
        }

        // SAFETY: ensure the Arc has its refcount decremented
        let mut s = ManuallyDrop::new(s);
        unsafe { ptr::drop_in_place(&mut s.rwlock) };
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is functionally identical to the `unlocked_fair` method on [`RwLockReadGuard`].
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_shared_fair();
        }
        defer!(s.rwlock.raw.lock_shared());
        f()
    }

    /// Temporarily yields the `RwLock` to a waiting thread if there is one.
    ///
    /// This is functionally identical to the `bump` method on [`RwLockReadGuard`].
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.bump_shared();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> Deref for ArcRwLockReadGuard<R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> Drop for ArcRwLockReadGuard<R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            self.rwlock.raw.unlock_shared();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: fmt::Debug + ?Sized> fmt::Debug for ArcRwLockReadGuard<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: fmt::Display + ?Sized> fmt::Display for ArcRwLockReadGuard<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockWriteGuard<'a, R: RawRwLock, T: ?Sized> {
    rwlock: &'a RwLock<R, T>,
    marker: PhantomData<(&'a mut T, R::GuardMarker)>,
}

unsafe impl<R: RawRwLock + Sync, T: Sync + ?Sized> Sync for RwLockWriteGuard<'_, R, T> {}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> RwLockWriteGuard<'a, R, T> {
    /// Returns a reference to the original reader-writer lock object.
    pub fn rwlock(s: &Self) -> &'a RwLock<R, T> {
        s.rwlock
    }

    /// Make a new `MappedRwLockWriteGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `RwLockWriteGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockWriteGuard::map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedRwLockWriteGuard<'a, R, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let raw = &s.rwlock.raw;
        let data = f(unsafe { &mut *s.rwlock.data.get() });
        mem::forget(s);
        MappedRwLockWriteGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make  a new `MappedRwLockWriteGuard` for a component of the
    /// locked data. The original guard is return if the closure returns `None`.
    ///
    /// This operation cannot fail as the `RwLockWriteGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockWriteGuard::try_map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn try_map<U: ?Sized, F>(s: Self, f: F) -> Result<MappedRwLockWriteGuard<'a, R, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let raw = &s.rwlock.raw;
        let data = match f(unsafe { &mut *s.rwlock.data.get() }) {
            Some(data) => data,
            None => return Err(s),
        };
        mem::forget(s);
        Ok(MappedRwLockWriteGuard {
            raw,
            data,
            marker: PhantomData,
        })
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the `RwLock`.
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockReadGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_exclusive();
        }
        defer!(s.rwlock.raw.lock_exclusive());
        f()
    }
}

impl<'a, R: RawRwLockDowngrade + 'a, T: ?Sized + 'a> RwLockWriteGuard<'a, R, T> {
    /// Atomically downgrades a write lock into a read lock without allowing any
    /// writers to take exclusive access of the lock in the meantime.
    ///
    /// Note that if there are any writers currently waiting to take the lock
    /// then other readers may not be able to acquire the lock even if it was
    /// downgraded.
    pub fn downgrade(s: Self) -> RwLockReadGuard<'a, R, T> {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.downgrade();
        }
        let rwlock = s.rwlock;
        mem::forget(s);
        RwLockReadGuard {
            rwlock,
            marker: PhantomData,
        }
    }
}

impl<'a, R: RawRwLockUpgradeDowngrade + 'a, T: ?Sized + 'a> RwLockWriteGuard<'a, R, T> {
    /// Atomically downgrades a write lock into an upgradable read lock without allowing any
    /// writers to take exclusive access of the lock in the meantime.
    ///
    /// Note that if there are any writers currently waiting to take the lock
    /// then other readers may not be able to acquire the lock even if it was
    /// downgraded.
    pub fn downgrade_to_upgradable(s: Self) -> RwLockUpgradableReadGuard<'a, R, T> {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.downgrade_to_upgradable();
        }
        let rwlock = s.rwlock;
        mem::forget(s);
        RwLockUpgradableReadGuard {
            rwlock,
            marker: PhantomData,
        }
    }
}

impl<'a, R: RawRwLockFair + 'a, T: ?Sized + 'a> RwLockWriteGuard<'a, R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// By default, `RwLock` is unfair and allow the current thread to re-lock
    /// the `RwLock` before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the `RwLock` for a long time. This is
    /// the default because it allows much higher throughput as it avoids
    /// forcing a context switch on every `RwLock` unlock. This can result in one
    /// thread acquiring a `RwLock` many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `RwLockWriteGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.unlock_exclusive_fair();
        }
        mem::forget(s);
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// The `RwLock` is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the `RwLock`.
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.unlock_exclusive_fair();
        }
        defer!(s.rwlock.raw.lock_exclusive());
        f()
    }

    /// Temporarily yields the `RwLock` to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_fair` followed
    /// by `write`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.bump_exclusive();
        }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Deref for RwLockWriteGuard<'a, R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> DerefMut for RwLockWriteGuard<'a, R, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Drop for RwLockWriteGuard<'a, R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            self.rwlock.raw.unlock_exclusive();
        }
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug for RwLockWriteGuard<'a, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for RwLockWriteGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> StableAddress for RwLockWriteGuard<'a, R, T> {}

/// An RAII rwlock guard returned by the `Arc` locking operations on `RwLock`.
/// This is similar to the `RwLockWriteGuard` struct, except instead of using a reference to unlock the `RwLock`
/// it uses an `Arc<RwLock>`. This has several advantages, most notably that it has an `'static` lifetime.
#[cfg(feature = "arc_lock")]
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct ArcRwLockWriteGuard<R: RawRwLock, T: ?Sized> {
    rwlock: Arc<RwLock<R, T>>,
    marker: PhantomData<R::GuardMarker>,
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> ArcRwLockWriteGuard<R, T> {
    /// Returns a reference to the rwlock, contained in its `Arc`.
    pub fn rwlock(s: &Self) -> &Arc<RwLock<R, T>> {
        &s.rwlock
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is functionally equivalent to the `unlocked` method on [`RwLockWriteGuard`].
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockWriteGuard always holds a shared lock.
        unsafe {
            s.rwlock.raw.unlock_exclusive();
        }
        defer!(s.rwlock.raw.lock_exclusive());
        f()
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockDowngrade, T: ?Sized> ArcRwLockWriteGuard<R, T> {
    /// Atomically downgrades a write lock into a read lock without allowing any
    /// writers to take exclusive access of the lock in the meantime.
    ///
    /// This is functionally equivalent to the `downgrade` method on [`RwLockWriteGuard`].
    pub fn downgrade(s: Self) -> ArcRwLockReadGuard<R, T> {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.downgrade();
        }

        // SAFETY: prevent the arc's refcount from changing using ManuallyDrop and ptr::read
        let s = ManuallyDrop::new(s);
        let rwlock = unsafe { ptr::read(&s.rwlock) };

        ArcRwLockReadGuard {
            rwlock,
            marker: PhantomData,
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgradeDowngrade, T: ?Sized> ArcRwLockWriteGuard<R, T> {
    /// Atomically downgrades a write lock into an upgradable read lock without allowing any
    /// writers to take exclusive access of the lock in the meantime.
    ///
    /// This is functionally identical to the `downgrade_to_upgradable` method on [`RwLockWriteGuard`].
    pub fn downgrade_to_upgradable(s: Self) -> ArcRwLockUpgradableReadGuard<R, T> {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.downgrade_to_upgradable();
        }

        // SAFETY: same as above
        let s = ManuallyDrop::new(s);
        let rwlock = unsafe { ptr::read(&s.rwlock) };

        ArcRwLockUpgradableReadGuard {
            rwlock,
            marker: PhantomData,
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockFair, T: ?Sized> ArcRwLockWriteGuard<R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// This is functionally equivalent to the `unlock_fair` method on [`RwLockWriteGuard`].
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.unlock_exclusive_fair();
        }

        // SAFETY: prevent the Arc from leaking memory
        let mut s = ManuallyDrop::new(s);
        unsafe { ptr::drop_in_place(&mut s.rwlock) };
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is functionally equivalent to the `unlocked_fair` method on [`RwLockWriteGuard`].
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.unlock_exclusive_fair();
        }
        defer!(s.rwlock.raw.lock_exclusive());
        f()
    }

    /// Temporarily yields the `RwLock` to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to the `bump` method on [`RwLockWriteGuard`].
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.rwlock.raw.bump_exclusive();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> Deref for ArcRwLockWriteGuard<R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> DerefMut for ArcRwLockWriteGuard<R, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: ?Sized> Drop for ArcRwLockWriteGuard<R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: An RwLockWriteGuard always holds an exclusive lock.
        unsafe {
            self.rwlock.raw.unlock_exclusive();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: fmt::Debug + ?Sized> fmt::Debug for ArcRwLockWriteGuard<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLock, T: fmt::Display + ?Sized> fmt::Display for ArcRwLockWriteGuard<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// RAII structure used to release the upgradable read access of a lock when
/// dropped.
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockUpgradableReadGuard<'a, R: RawRwLockUpgrade, T: ?Sized> {
    rwlock: &'a RwLock<R, T>,
    marker: PhantomData<(&'a T, R::GuardMarker)>,
}

unsafe impl<'a, R: RawRwLockUpgrade + 'a, T: ?Sized + Sync + 'a> Sync
    for RwLockUpgradableReadGuard<'a, R, T>
{
}

impl<'a, R: RawRwLockUpgrade + 'a, T: ?Sized + 'a> RwLockUpgradableReadGuard<'a, R, T> {
    /// Returns a reference to the original reader-writer lock object.
    pub fn rwlock(s: &Self) -> &'a RwLock<R, T> {
        s.rwlock
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the `RwLock`.
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.unlock_upgradable();
        }
        defer!(s.rwlock.raw.lock_upgradable());
        f()
    }

    /// Atomically upgrades an upgradable read lock lock into an exclusive write lock,
    /// blocking the current thread until it can be acquired.
    pub fn upgrade(s: Self) -> RwLockWriteGuard<'a, R, T> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.upgrade();
        }
        let rwlock = s.rwlock;
        mem::forget(s);
        RwLockWriteGuard {
            rwlock,
            marker: PhantomData,
        }
    }

    /// Tries to atomically upgrade an upgradable read lock into an exclusive write lock.
    ///
    /// If the access could not be granted at this time, then the current guard is returned.
    pub fn try_upgrade(s: Self) -> Result<RwLockWriteGuard<'a, R, T>, Self> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        if unsafe { s.rwlock.raw.try_upgrade() } {
            let rwlock = s.rwlock;
            mem::forget(s);
            Ok(RwLockWriteGuard {
                rwlock,
                marker: PhantomData,
            })
        } else {
            Err(s)
        }
    }
}

impl<'a, R: RawRwLockUpgradeFair + 'a, T: ?Sized + 'a> RwLockUpgradableReadGuard<'a, R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// By default, `RwLock` is unfair and allow the current thread to re-lock
    /// the `RwLock` before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the `RwLock` for a long time. This is
    /// the default because it allows much higher throughput as it avoids
    /// forcing a context switch on every `RwLock` unlock. This can result in one
    /// thread acquiring a `RwLock` many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `RwLockUpgradableReadGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.unlock_upgradable_fair();
        }
        mem::forget(s);
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// The `RwLock` is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the `RwLock`.
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.unlock_upgradable_fair();
        }
        defer!(s.rwlock.raw.lock_upgradable());
        f()
    }

    /// Temporarily yields the `RwLock` to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_fair` followed
    /// by `upgradable_read`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.bump_upgradable();
        }
    }
}

impl<'a, R: RawRwLockUpgradeDowngrade + 'a, T: ?Sized + 'a> RwLockUpgradableReadGuard<'a, R, T> {
    /// Atomically downgrades an upgradable read lock lock into a shared read lock
    /// without allowing any writers to take exclusive access of the lock in the
    /// meantime.
    ///
    /// Note that if there are any writers currently waiting to take the lock
    /// then other readers may not be able to acquire the lock even if it was
    /// downgraded.
    pub fn downgrade(s: Self) -> RwLockReadGuard<'a, R, T> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.downgrade_upgradable();
        }
        let rwlock = s.rwlock;
        mem::forget(s);
        RwLockReadGuard {
            rwlock,
            marker: PhantomData,
        }
    }

    /// First, atomically upgrades an upgradable read lock lock into an exclusive write lock,
    /// blocking the current thread until it can be acquired.
    ///
    /// Then, calls the provided closure with an exclusive reference to the lock's data.
    ///
    /// Finally, atomically downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `upgrade` which takes the guard by value.
    pub fn with_upgraded<Ret, F: FnOnce(&mut T) -> Ret>(&mut self, f: F) -> Ret {
        unsafe {
            self.rwlock.raw.upgrade();
        }

        // Safety: We just upgraded the lock, so we have mutable access to the data.
        // This will restore the state the lock was in at the start of the function.
        defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

        // Safety: We upgraded the lock, so we have mutable access to the data.
        // When this function returns, whether by drop or panic,
        // the drop guard will downgrade it back to an upgradeable lock.
        f(unsafe { &mut *self.rwlock.data.get() })
    }

    /// First, tries to atomically upgrade an upgradable read lock into an exclusive write lock.
    ///
    /// If the access could not be granted at this time, then `None` is returned.
    ///
    /// Otherwise, calls the provided closure with an exclusive reference to the lock's data,
    /// and finally downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `try_upgrade` which takes the guard by value.
    pub fn try_with_upgraded<Ret, F: FnOnce(&mut T) -> Ret>(&mut self, f: F) -> Option<Ret> {
        if unsafe { self.rwlock.raw.try_upgrade() } {
            // Safety: We just upgraded the lock, so we have mutable access to the data.
            // This will restore the state the lock was in at the start of the function.
            defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

            // Safety: We upgraded the lock, so we have mutable access to the data.
            // When this function returns, whether by drop or panic,
            // the drop guard will downgrade it back to an upgradeable lock.
            Some(f(unsafe { &mut *self.rwlock.data.get() }))
        } else {
            None
        }
    }
}

impl<'a, R: RawRwLockUpgradeTimed + 'a, T: ?Sized + 'a> RwLockUpgradableReadGuard<'a, R, T> {
    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// the current guard is returned.
    pub fn try_upgrade_for(
        s: Self,
        timeout: R::Duration,
    ) -> Result<RwLockWriteGuard<'a, R, T>, Self> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        if unsafe { s.rwlock.raw.try_upgrade_for(timeout) } {
            let rwlock = s.rwlock;
            mem::forget(s);
            Ok(RwLockWriteGuard {
                rwlock,
                marker: PhantomData,
            })
        } else {
            Err(s)
        }
    }

    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// the current guard is returned.
    #[inline]
    pub fn try_upgrade_until(
        s: Self,
        timeout: R::Instant,
    ) -> Result<RwLockWriteGuard<'a, R, T>, Self> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        if unsafe { s.rwlock.raw.try_upgrade_until(timeout) } {
            let rwlock = s.rwlock;
            mem::forget(s);
            Ok(RwLockWriteGuard {
                rwlock,
                marker: PhantomData,
            })
        } else {
            Err(s)
        }
    }
}

impl<'a, R: RawRwLockUpgradeTimed + RawRwLockUpgradeDowngrade + 'a, T: ?Sized + 'a>
    RwLockUpgradableReadGuard<'a, R, T>
{
    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned.
    ///
    /// Otherwise, calls the provided closure with an exclusive reference to the lock's data,
    /// and finally downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `try_upgrade_for` which takes the guard by value.
    pub fn try_with_upgraded_for<Ret, F: FnOnce(&mut T) -> Ret>(
        &mut self,
        timeout: R::Duration,
        f: F,
    ) -> Option<Ret> {
        if unsafe { self.rwlock.raw.try_upgrade_for(timeout) } {
            // Safety: We just upgraded the lock, so we have mutable access to the data.
            // This will restore the state the lock was in at the start of the function.
            defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

            // Safety: We upgraded the lock, so we have mutable access to the data.
            // When this function returns, whether by drop or panic,
            // the drop guard will downgrade it back to an upgradeable lock.
            Some(f(unsafe { &mut *self.rwlock.data.get() }))
        } else {
            None
        }
    }

    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned.
    ///
    /// Otherwise, calls the provided closure with an exclusive reference to the lock's data,
    /// and finally downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `try_upgrade_until` which takes the guard by value.
    pub fn try_with_upgraded_until<Ret, F: FnOnce(&mut T) -> Ret>(
        &mut self,
        timeout: R::Instant,
        f: F,
    ) -> Option<Ret> {
        if unsafe { self.rwlock.raw.try_upgrade_until(timeout) } {
            // Safety: We just upgraded the lock, so we have mutable access to the data.
            // This will restore the state the lock was in at the start of the function.
            defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

            // Safety: We upgraded the lock, so we have mutable access to the data.
            // When this function returns, whether by drop or panic,
            // the drop guard will downgrade it back to an upgradeable lock.
            Some(f(unsafe { &mut *self.rwlock.data.get() }))
        } else {
            None
        }
    }
}

impl<'a, R: RawRwLockUpgrade + 'a, T: ?Sized + 'a> Deref for RwLockUpgradableReadGuard<'a, R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<'a, R: RawRwLockUpgrade + 'a, T: ?Sized + 'a> Drop for RwLockUpgradableReadGuard<'a, R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            self.rwlock.raw.unlock_upgradable();
        }
    }
}

impl<'a, R: RawRwLockUpgrade + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug
    for RwLockUpgradableReadGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawRwLockUpgrade + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for RwLockUpgradableReadGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawRwLockUpgrade + 'a, T: ?Sized + 'a> StableAddress
    for RwLockUpgradableReadGuard<'a, R, T>
{
}

/// An RAII rwlock guard returned by the `Arc` locking operations on `RwLock`.
/// This is similar to the `RwLockUpgradableReadGuard` struct, except instead of using a reference to unlock the
/// `RwLock` it uses an `Arc<RwLock>`. This has several advantages, most notably that it has an `'static`
/// lifetime.
#[cfg(feature = "arc_lock")]
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct ArcRwLockUpgradableReadGuard<R: RawRwLockUpgrade, T: ?Sized> {
    rwlock: Arc<RwLock<R, T>>,
    marker: PhantomData<R::GuardMarker>,
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgrade, T: ?Sized> ArcRwLockUpgradableReadGuard<R, T> {
    /// Returns a reference to the rwlock, contained in its original `Arc`.
    pub fn rwlock(s: &Self) -> &Arc<RwLock<R, T>> {
        &s.rwlock
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is functionally identical to the `unlocked` method on [`RwLockUpgradableReadGuard`].
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.unlock_upgradable();
        }
        defer!(s.rwlock.raw.lock_upgradable());
        f()
    }

    /// Atomically upgrades an upgradable read lock lock into an exclusive write lock,
    /// blocking the current thread until it can be acquired.
    pub fn upgrade(s: Self) -> ArcRwLockWriteGuard<R, T> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.upgrade();
        }

        // SAFETY: avoid incrementing or decrementing the refcount using ManuallyDrop and reading the Arc out
        //         of the struct
        let s = ManuallyDrop::new(s);
        let rwlock = unsafe { ptr::read(&s.rwlock) };

        ArcRwLockWriteGuard {
            rwlock,
            marker: PhantomData,
        }
    }

    /// Tries to atomically upgrade an upgradable read lock into an exclusive write lock.
    ///
    /// If the access could not be granted at this time, then the current guard is returned.
    pub fn try_upgrade(s: Self) -> Result<ArcRwLockWriteGuard<R, T>, Self> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        if unsafe { s.rwlock.raw.try_upgrade() } {
            // SAFETY: same as above
            let s = ManuallyDrop::new(s);
            let rwlock = unsafe { ptr::read(&s.rwlock) };

            Ok(ArcRwLockWriteGuard {
                rwlock,
                marker: PhantomData,
            })
        } else {
            Err(s)
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgradeFair, T: ?Sized> ArcRwLockUpgradableReadGuard<R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// This is functionally identical to the `unlock_fair` method on [`RwLockUpgradableReadGuard`].
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.unlock_upgradable_fair();
        }

        // SAFETY: make sure we decrement the refcount properly
        let mut s = ManuallyDrop::new(s);
        unsafe { ptr::drop_in_place(&mut s.rwlock) };
    }

    /// Temporarily unlocks the `RwLock` to execute the given function.
    ///
    /// This is functionally equivalent to the `unlocked_fair` method on [`RwLockUpgradableReadGuard`].
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.unlock_upgradable_fair();
        }
        defer!(s.rwlock.raw.lock_upgradable());
        f()
    }

    /// Temporarily yields the `RwLock` to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `bump` on [`RwLockUpgradableReadGuard`].
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.bump_upgradable();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgradeDowngrade, T: ?Sized> ArcRwLockUpgradableReadGuard<R, T> {
    /// Atomically downgrades an upgradable read lock lock into a shared read lock
    /// without allowing any writers to take exclusive access of the lock in the
    /// meantime.
    ///
    /// Note that if there are any writers currently waiting to take the lock
    /// then other readers may not be able to acquire the lock even if it was
    /// downgraded.
    pub fn downgrade(s: Self) -> ArcRwLockReadGuard<R, T> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            s.rwlock.raw.downgrade_upgradable();
        }

        // SAFETY: use ManuallyDrop and ptr::read to ensure the refcount is not changed
        let s = ManuallyDrop::new(s);
        let rwlock = unsafe { ptr::read(&s.rwlock) };

        ArcRwLockReadGuard {
            rwlock,
            marker: PhantomData,
        }
    }

    /// First, atomically upgrades an upgradable read lock lock into an exclusive write lock,
    /// blocking the current thread until it can be acquired.
    ///
    /// Then, calls the provided closure with an exclusive reference to the lock's data.
    ///
    /// Finally, atomically downgrades the lock back to an upgradable read lock.
    /// The closure's return value is returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `upgrade` which takes the guard by value.
    pub fn with_upgraded<Ret, F: FnOnce(&mut T) -> Ret>(&mut self, f: F) -> Ret {
        unsafe {
            self.rwlock.raw.upgrade();
        }

        // Safety: We just upgraded the lock, so we have mutable access to the data.
        // This will restore the state the lock was in at the start of the function.
        defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

        // Safety: We upgraded the lock, so we have mutable access to the data.
        // When this function returns, whether by drop or panic,
        // the drop guard will downgrade it back to an upgradeable lock.
        f(unsafe { &mut *self.rwlock.data.get() })
    }

    /// First, tries to atomically upgrade an upgradable read lock into an exclusive write lock.
    ///
    /// If the access could not be granted at this time, then `None` is returned.
    ///
    /// Otherwise, calls the provided closure with an exclusive reference to the lock's data,
    /// and finally downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `try_upgrade` which takes the guard by value.
    pub fn try_with_upgraded<Ret, F: FnOnce(&mut T) -> Ret>(&mut self, f: F) -> Option<Ret> {
        if unsafe { self.rwlock.raw.try_upgrade() } {
            // Safety: We just upgraded the lock, so we have mutable access to the data.
            // This will restore the state the lock was in at the start of the function.
            defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

            // Safety: We upgraded the lock, so we have mutable access to the data.
            // When this function returns, whether by drop or panic,
            // the drop guard will downgrade it back to an upgradeable lock.
            Some(f(unsafe { &mut *self.rwlock.data.get() }))
        } else {
            None
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgradeTimed, T: ?Sized> ArcRwLockUpgradableReadGuard<R, T> {
    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// the current guard is returned.
    pub fn try_upgrade_for(
        s: Self,
        timeout: R::Duration,
    ) -> Result<ArcRwLockWriteGuard<R, T>, Self> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        if unsafe { s.rwlock.raw.try_upgrade_for(timeout) } {
            // SAFETY: same as above
            let s = ManuallyDrop::new(s);
            let rwlock = unsafe { ptr::read(&s.rwlock) };

            Ok(ArcRwLockWriteGuard {
                rwlock,
                marker: PhantomData,
            })
        } else {
            Err(s)
        }
    }

    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// the current guard is returned.
    #[inline]
    pub fn try_upgrade_until(
        s: Self,
        timeout: R::Instant,
    ) -> Result<ArcRwLockWriteGuard<R, T>, Self> {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        if unsafe { s.rwlock.raw.try_upgrade_until(timeout) } {
            // SAFETY: same as above
            let s = ManuallyDrop::new(s);
            let rwlock = unsafe { ptr::read(&s.rwlock) };

            Ok(ArcRwLockWriteGuard {
                rwlock,
                marker: PhantomData,
            })
        } else {
            Err(s)
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgradeTimed + RawRwLockUpgradeDowngrade, T: ?Sized>
    ArcRwLockUpgradableReadGuard<R, T>
{
    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned.
    ///
    /// Otherwise, calls the provided closure with an exclusive reference to the lock's data,
    /// and finally downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `try_upgrade_for` which takes the guard by value.
    pub fn try_with_upgraded_for<Ret, F: FnOnce(&mut T) -> Ret>(
        &mut self,
        timeout: R::Duration,
        f: F,
    ) -> Option<Ret> {
        if unsafe { self.rwlock.raw.try_upgrade_for(timeout) } {
            // Safety: We just upgraded the lock, so we have mutable access to the data.
            // This will restore the state the lock was in at the start of the function.
            defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

            // Safety: We upgraded the lock, so we have mutable access to the data.
            // When this function returns, whether by drop or panic,
            // the drop guard will downgrade it back to an upgradeable lock.
            Some(f(unsafe { &mut *self.rwlock.data.get() }))
        } else {
            None
        }
    }

    /// Tries to atomically upgrade an upgradable read lock into an exclusive
    /// write lock, until a timeout is reached.
    ///
    /// If the access could not be granted before the timeout expires, then
    /// `None` is returned.
    ///
    /// Otherwise, calls the provided closure with an exclusive reference to the lock's data,
    /// and finally downgrades the lock back to an upgradable read lock.
    /// The closure's return value is wrapped in `Some` and returned.
    ///
    /// This function only requires a mutable reference to the guard, unlike
    /// `try_upgrade_until` which takes the guard by value.
    pub fn try_with_upgraded_until<Ret, F: FnOnce(&mut T) -> Ret>(
        &mut self,
        timeout: R::Instant,
        f: F,
    ) -> Option<Ret> {
        if unsafe { self.rwlock.raw.try_upgrade_until(timeout) } {
            // Safety: We just upgraded the lock, so we have mutable access to the data.
            // This will restore the state the lock was in at the start of the function.
            defer!(unsafe { self.rwlock.raw.downgrade_to_upgradable() });

            // Safety: We upgraded the lock, so we have mutable access to the data.
            // When this function returns, whether by drop or panic,
            // the drop guard will downgrade it back to an upgradeable lock.
            Some(f(unsafe { &mut *self.rwlock.data.get() }))
        } else {
            None
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgrade, T: ?Sized> Deref for ArcRwLockUpgradableReadGuard<R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.data.get() }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgrade, T: ?Sized> Drop for ArcRwLockUpgradableReadGuard<R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: An RwLockUpgradableReadGuard always holds an upgradable lock.
        unsafe {
            self.rwlock.raw.unlock_upgradable();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgrade, T: fmt::Debug + ?Sized> fmt::Debug
    for ArcRwLockUpgradableReadGuard<R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawRwLockUpgrade, T: fmt::Display + ?Sized> fmt::Display
    for ArcRwLockUpgradableReadGuard<R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// An RAII read lock guard returned by `RwLockReadGuard::map`, which can point to a
/// subfield of the protected data.
///
/// The main difference between `MappedRwLockReadGuard` and `RwLockReadGuard` is that the
/// former doesn't support temporarily unlocking and re-locking, since that
/// could introduce soundness issues if the locked object is modified by another
/// thread.
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct MappedRwLockReadGuard<'a, R: RawRwLock, T: ?Sized> {
    raw: &'a R,
    data: *const T,
    marker: PhantomData<&'a T>,
}

unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + Sync + 'a> Sync for MappedRwLockReadGuard<'a, R, T> {}
unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + Sync + 'a> Send for MappedRwLockReadGuard<'a, R, T> where
    R::GuardMarker: Send
{
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> MappedRwLockReadGuard<'a, R, T> {
    /// Make a new `MappedRwLockReadGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `MappedRwLockReadGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `MappedRwLockReadGuard::map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedRwLockReadGuard<'a, R, U>
    where
        F: FnOnce(&T) -> &U,
    {
        let raw = s.raw;
        let data = f(unsafe { &*s.data });
        mem::forget(s);
        MappedRwLockReadGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make  a new `MappedRwLockReadGuard` for a component of the
    /// locked data. The original guard is return if the closure returns `None`.
    ///
    /// This operation cannot fail as the `MappedRwLockReadGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `MappedRwLockReadGuard::try_map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn try_map<U: ?Sized, F>(s: Self, f: F) -> Result<MappedRwLockReadGuard<'a, R, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        let raw = s.raw;
        let data = match f(unsafe { &*s.data }) {
            Some(data) => data,
            None => return Err(s),
        };
        mem::forget(s);
        Ok(MappedRwLockReadGuard {
            raw,
            data,
            marker: PhantomData,
        })
    }
}

impl<'a, R: RawRwLockFair + 'a, T: ?Sized + 'a> MappedRwLockReadGuard<'a, R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// By default, `RwLock` is unfair and allow the current thread to re-lock
    /// the `RwLock` before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the `RwLock` for a long time. This is
    /// the default because it allows much higher throughput as it avoids
    /// forcing a context switch on every `RwLock` unlock. This can result in one
    /// thread acquiring a `RwLock` many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `MappedRwLockReadGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: A MappedRwLockReadGuard always holds a shared lock.
        unsafe {
            s.raw.unlock_shared_fair();
        }
        mem::forget(s);
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Deref for MappedRwLockReadGuard<'a, R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Drop for MappedRwLockReadGuard<'a, R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: A MappedRwLockReadGuard always holds a shared lock.
        unsafe {
            self.raw.unlock_shared();
        }
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug
    for MappedRwLockReadGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for MappedRwLockReadGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> StableAddress
    for MappedRwLockReadGuard<'a, R, T>
{
}

/// An RAII write lock guard returned by `RwLockWriteGuard::map`, which can point to a
/// subfield of the protected data.
///
/// The main difference between `MappedRwLockWriteGuard` and `RwLockWriteGuard` is that the
/// former doesn't support temporarily unlocking and re-locking, since that
/// could introduce soundness issues if the locked object is modified by another
/// thread.
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct MappedRwLockWriteGuard<'a, R: RawRwLock, T: ?Sized> {
    raw: &'a R,
    data: *mut T,
    marker: PhantomData<&'a mut T>,
}

unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + Sync + 'a> Sync
    for MappedRwLockWriteGuard<'a, R, T>
{
}
unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + Send + 'a> Send for MappedRwLockWriteGuard<'a, R, T> where
    R::GuardMarker: Send
{
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> MappedRwLockWriteGuard<'a, R, T> {
    /// Make a new `MappedRwLockWriteGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `MappedRwLockWriteGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `MappedRwLockWriteGuard::map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedRwLockWriteGuard<'a, R, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let raw = s.raw;
        let data = f(unsafe { &mut *s.data });
        mem::forget(s);
        MappedRwLockWriteGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make  a new `MappedRwLockWriteGuard` for a component of the
    /// locked data. The original guard is return if the closure returns `None`.
    ///
    /// This operation cannot fail as the `MappedRwLockWriteGuard` passed
    /// in already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `MappedRwLockWriteGuard::try_map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn try_map<U: ?Sized, F>(s: Self, f: F) -> Result<MappedRwLockWriteGuard<'a, R, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let raw = s.raw;
        let data = match f(unsafe { &mut *s.data }) {
            Some(data) => data,
            None => return Err(s),
        };
        mem::forget(s);
        Ok(MappedRwLockWriteGuard {
            raw,
            data,
            marker: PhantomData,
        })
    }
}

impl<'a, R: RawRwLockFair + 'a, T: ?Sized + 'a> MappedRwLockWriteGuard<'a, R, T> {
    /// Unlocks the `RwLock` using a fair unlock protocol.
    ///
    /// By default, `RwLock` is unfair and allow the current thread to re-lock
    /// the `RwLock` before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the `RwLock` for a long time. This is
    /// the default because it allows much higher throughput as it avoids
    /// forcing a context switch on every `RwLock` unlock. This can result in one
    /// thread acquiring a `RwLock` many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `MappedRwLockWriteGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: A MappedRwLockWriteGuard always holds an exclusive lock.
        unsafe {
            s.raw.unlock_exclusive_fair();
        }
        mem::forget(s);
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Deref for MappedRwLockWriteGuard<'a, R, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> DerefMut for MappedRwLockWriteGuard<'a, R, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data }
    }
}

impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> Drop for MappedRwLockWriteGuard<'a, R, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: A MappedRwLockWriteGuard always holds an exclusive lock.
        unsafe {
            self.raw.unlock_exclusive();
        }
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug
    for MappedRwLockWriteGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawRwLock + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for MappedRwLockWriteGuard<'a, R, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawRwLock + 'a, T: ?Sized + 'a> StableAddress
    for MappedRwLockWriteGuard<'a, R, T>
{
}
