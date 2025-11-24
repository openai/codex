// Copyright 2018 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::{
    mutex::{RawMutex, RawMutexFair, RawMutexTimed},
    GuardNoSend,
};
use core::{
    cell::{Cell, UnsafeCell},
    fmt,
    marker::PhantomData,
    mem,
    num::NonZeroUsize,
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

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

/// Helper trait which returns a non-zero thread ID.
///
/// The simplest way to implement this trait is to return the address of a
/// thread-local variable.
///
/// # Safety
///
/// Implementations of this trait must ensure that no two active threads share
/// the same thread ID. However the ID of a thread that has exited can be
/// re-used since that thread is no longer active.
pub unsafe trait GetThreadId {
    /// Initial value.
    // A “non-constant” const item is a legacy way to supply an initialized value to downstream
    // static items. Can hopefully be replaced with `const fn new() -> Self` at some point.
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    /// Returns a non-zero thread ID which identifies the current thread of
    /// execution.
    fn nonzero_thread_id(&self) -> NonZeroUsize;
}

/// A raw mutex type that wraps another raw mutex to provide reentrancy.
///
/// Although this has the same methods as the [`RawMutex`] trait, it does
/// not implement it, and should not be used in the same way, since this
/// mutex can successfully acquire a lock multiple times in the same thread.
/// Only use this when you know you want a raw mutex that can be locked
/// reentrantly; you probably want [`ReentrantMutex`] instead.
pub struct RawReentrantMutex<R, G> {
    owner: AtomicUsize,
    lock_count: Cell<usize>,
    mutex: R,
    get_thread_id: G,
}

unsafe impl<R: RawMutex + Send, G: GetThreadId + Send> Send for RawReentrantMutex<R, G> {}
unsafe impl<R: RawMutex + Sync, G: GetThreadId + Sync> Sync for RawReentrantMutex<R, G> {}

impl<R: RawMutex, G: GetThreadId> RawReentrantMutex<R, G> {
    /// Initial value for an unlocked mutex.
    #[allow(clippy::declare_interior_mutable_const)]
    pub const INIT: Self = RawReentrantMutex {
        owner: AtomicUsize::new(0),
        lock_count: Cell::new(0),
        mutex: R::INIT,
        get_thread_id: G::INIT,
    };

    #[inline]
    fn lock_internal<F: FnOnce() -> bool>(&self, try_lock: F) -> bool {
        let id = self.get_thread_id.nonzero_thread_id().get();
        if self.owner.load(Ordering::Relaxed) == id {
            self.lock_count.set(
                self.lock_count
                    .get()
                    .checked_add(1)
                    .expect("ReentrantMutex lock count overflow"),
            );
        } else {
            if !try_lock() {
                return false;
            }
            self.owner.store(id, Ordering::Relaxed);
            debug_assert_eq!(self.lock_count.get(), 0);
            self.lock_count.set(1);
        }
        true
    }

    /// Acquires this mutex, blocking if it's held by another thread.
    #[inline]
    pub fn lock(&self) {
        self.lock_internal(|| {
            self.mutex.lock();
            true
        });
    }

    /// Attempts to acquire this mutex without blocking. Returns `true`
    /// if the lock was successfully acquired and `false` otherwise.
    #[inline]
    pub fn try_lock(&self) -> bool {
        self.lock_internal(|| self.mutex.try_lock())
    }

    /// Unlocks this mutex. The inner mutex may not be unlocked if
    /// this mutex was acquired previously in the current thread.
    ///
    /// # Safety
    ///
    /// This method may only be called if the mutex is held by the current thread.
    #[inline]
    pub unsafe fn unlock(&self) {
        let lock_count = self.lock_count.get() - 1;
        self.lock_count.set(lock_count);
        if lock_count == 0 {
            self.owner.store(0, Ordering::Relaxed);
            self.mutex.unlock();
        }
    }

    /// Checks whether the mutex is currently locked.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.mutex.is_locked()
    }

    /// Checks whether the mutex is currently held by the current thread.
    #[inline]
    pub fn is_owned_by_current_thread(&self) -> bool {
        let id = self.get_thread_id.nonzero_thread_id().get();
        self.owner.load(Ordering::Relaxed) == id
    }
}

impl<R: RawMutexFair, G: GetThreadId> RawReentrantMutex<R, G> {
    /// Unlocks this mutex using a fair unlock protocol. The inner mutex
    /// may not be unlocked if this mutex was acquired previously in the
    /// current thread.
    ///
    /// # Safety
    ///
    /// This method may only be called if the mutex is held by the current thread.
    #[inline]
    pub unsafe fn unlock_fair(&self) {
        let lock_count = self.lock_count.get() - 1;
        self.lock_count.set(lock_count);
        if lock_count == 0 {
            self.owner.store(0, Ordering::Relaxed);
            self.mutex.unlock_fair();
        }
    }

    /// Temporarily yields the mutex to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_fair` followed
    /// by `lock`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    ///
    /// # Safety
    ///
    /// This method may only be called if the mutex is held by the current thread.
    #[inline]
    pub unsafe fn bump(&self) {
        if self.lock_count.get() == 1 {
            let id = self.owner.load(Ordering::Relaxed);
            self.owner.store(0, Ordering::Relaxed);
            self.lock_count.set(0);
            self.mutex.bump();
            self.owner.store(id, Ordering::Relaxed);
            self.lock_count.set(1);
        }
    }
}

impl<R: RawMutexTimed, G: GetThreadId> RawReentrantMutex<R, G> {
    /// Attempts to acquire this lock until a timeout is reached.
    #[inline]
    pub fn try_lock_until(&self, timeout: R::Instant) -> bool {
        self.lock_internal(|| self.mutex.try_lock_until(timeout))
    }

    /// Attempts to acquire this lock until a timeout is reached.
    #[inline]
    pub fn try_lock_for(&self, timeout: R::Duration) -> bool {
        self.lock_internal(|| self.mutex.try_lock_for(timeout))
    }
}

/// A mutex which can be recursively locked by a single thread.
///
/// This type is identical to `Mutex` except for the following points:
///
/// - Locking multiple times from the same thread will work correctly instead of
///   deadlocking.
/// - `ReentrantMutexGuard` does not give mutable references to the locked data.
///   Use a `RefCell` if you need this.
///
/// See [`Mutex`](crate::Mutex) for more details about the underlying mutex
/// primitive.
pub struct ReentrantMutex<R, G, T: ?Sized> {
    raw: RawReentrantMutex<R, G>,
    data: UnsafeCell<T>,
}

unsafe impl<R: RawMutex + Send, G: GetThreadId + Send, T: ?Sized + Send> Send
    for ReentrantMutex<R, G, T>
{
}
unsafe impl<R: RawMutex + Sync, G: GetThreadId + Sync, T: ?Sized + Send> Sync
    for ReentrantMutex<R, G, T>
{
}

impl<R: RawMutex, G: GetThreadId, T> ReentrantMutex<R, G, T> {
    /// Creates a new reentrant mutex in an unlocked state ready for use.
    #[cfg(has_const_fn_trait_bound)]
    #[inline]
    pub const fn new(val: T) -> ReentrantMutex<R, G, T> {
        ReentrantMutex {
            data: UnsafeCell::new(val),
            raw: RawReentrantMutex {
                owner: AtomicUsize::new(0),
                lock_count: Cell::new(0),
                mutex: R::INIT,
                get_thread_id: G::INIT,
            },
        }
    }

    /// Creates a new reentrant mutex in an unlocked state ready for use.
    #[cfg(not(has_const_fn_trait_bound))]
    #[inline]
    pub fn new(val: T) -> ReentrantMutex<R, G, T> {
        ReentrantMutex {
            data: UnsafeCell::new(val),
            raw: RawReentrantMutex {
                owner: AtomicUsize::new(0),
                lock_count: Cell::new(0),
                mutex: R::INIT,
                get_thread_id: G::INIT,
            },
        }
    }

    /// Consumes this mutex, returning the underlying data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<R, G, T> ReentrantMutex<R, G, T> {
    /// Creates a new reentrant mutex based on a pre-existing raw mutex and a
    /// helper to get the thread ID.
    #[inline]
    pub const fn from_raw(raw_mutex: R, get_thread_id: G, val: T) -> ReentrantMutex<R, G, T> {
        ReentrantMutex {
            data: UnsafeCell::new(val),
            raw: RawReentrantMutex {
                owner: AtomicUsize::new(0),
                lock_count: Cell::new(0),
                mutex: raw_mutex,
                get_thread_id,
            },
        }
    }

    /// Creates a new reentrant mutex based on a pre-existing raw mutex and a
    /// helper to get the thread ID.
    ///
    /// This allows creating a reentrant mutex in a constant context on stable
    /// Rust.
    ///
    /// This method is a legacy alias for [`from_raw`](Self::from_raw).
    #[inline]
    pub const fn const_new(raw_mutex: R, get_thread_id: G, val: T) -> ReentrantMutex<R, G, T> {
        Self::from_raw(raw_mutex, get_thread_id, val)
    }
}

impl<R: RawMutex, G: GetThreadId, T: ?Sized> ReentrantMutex<R, G, T> {
    /// Creates a new `ReentrantMutexGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds the lock.
    ///
    /// Calling this function when a guard has already been produced is undefined behaviour unless
    /// the guard was forgotten with `mem::forget`.
    #[inline]
    pub unsafe fn make_guard_unchecked(&self) -> ReentrantMutexGuard<'_, R, G, T> {
        ReentrantMutexGuard {
            remutex: &self,
            marker: PhantomData,
        }
    }

    /// Acquires a reentrant mutex, blocking the current thread until it is able
    /// to do so.
    ///
    /// If the mutex is held by another thread then this function will block the
    /// local thread until it is available to acquire the mutex. If the mutex is
    /// already held by the current thread then this function will increment the
    /// lock reference count and return immediately. Upon returning,
    /// the thread is the only thread with the mutex held. An RAII guard is
    /// returned to allow scoped unlock of the lock. When the guard goes out of
    /// scope, the mutex will be unlocked.
    #[inline]
    pub fn lock(&self) -> ReentrantMutexGuard<'_, R, G, T> {
        self.raw.lock();
        // SAFETY: The lock is held, as required.
        unsafe { self.make_guard_unchecked() }
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then `None` is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the
    /// guard is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_lock(&self) -> Option<ReentrantMutexGuard<'_, R, G, T>> {
        if self.raw.try_lock() {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_guard_unchecked() })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `ReentrantMutex` mutably, no actual locking needs to
    /// take place---the mutable borrow statically guarantees no locks exist.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    /// Checks whether the mutex is currently locked.
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// Checks whether the mutex is currently held by the current thread.
    #[inline]
    pub fn is_owned_by_current_thread(&self) -> bool {
        self.raw.is_owned_by_current_thread()
    }

    /// Forcibly unlocks the mutex.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `ReentrantMutexGuard` object alive, for example when
    /// dealing with FFI.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread logically owns a
    /// `ReentrantMutexGuard` but that guard has be discarded using `mem::forget`.
    /// Behavior is undefined if a mutex is unlocked when not locked.
    #[inline]
    pub unsafe fn force_unlock(&self) {
        self.raw.unlock();
    }

    /// Returns the underlying raw mutex object.
    ///
    /// Note that you will most likely need to import the `RawMutex` trait from
    /// `lock_api` to be able to call functions on the raw mutex.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it allows unlocking a mutex while
    /// still holding a reference to a `ReentrantMutexGuard`.
    #[inline]
    pub unsafe fn raw(&self) -> &R {
        &self.raw.mutex
    }

    /// Returns a raw pointer to the underlying data.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `ReentrantMutexGuard` object alive, for example
    /// when dealing with FFI.
    ///
    /// # Safety
    ///
    /// You must ensure that there are no data races when dereferencing the
    /// returned pointer, for example if the current thread logically owns a
    /// `ReentrantMutexGuard` but that guard has been discarded using
    /// `mem::forget`.
    #[inline]
    pub fn data_ptr(&self) -> *mut T {
        self.data.get()
    }

    /// Creates a new `ArcReentrantMutexGuard` without checking if the lock is held.
    ///
    /// # Safety
    ///
    /// This method must only be called if the thread logically holds the lock.
    ///
    /// Calling this function when a guard has already been produced is undefined behaviour unless
    /// the guard was forgotten with `mem::forget`.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub unsafe fn make_arc_guard_unchecked(self: &Arc<Self>) -> ArcReentrantMutexGuard<R, G, T> {
        ArcReentrantMutexGuard {
            remutex: self.clone(),
            marker: PhantomData,
        }
    }

    /// Acquires a reentrant mutex through an `Arc`.
    ///
    /// This method is similar to the `lock` method; however, it requires the `ReentrantMutex` to be inside of an
    /// `Arc` and the resulting mutex guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn lock_arc(self: &Arc<Self>) -> ArcReentrantMutexGuard<R, G, T> {
        self.raw.lock();
        // SAFETY: locking guarantee is upheld
        unsafe { self.make_arc_guard_unchecked() }
    }

    /// Attempts to acquire a reentrant mutex through an `Arc`.
    ///
    /// This method is similar to the `try_lock` method; however, it requires the `ReentrantMutex` to be inside
    /// of an `Arc` and the resulting mutex guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_lock_arc(self: &Arc<Self>) -> Option<ArcReentrantMutexGuard<R, G, T>> {
        if self.raw.try_lock() {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawMutexFair, G: GetThreadId, T: ?Sized> ReentrantMutex<R, G, T> {
    /// Forcibly unlocks the mutex using a fair unlock protocol.
    ///
    /// This is useful when combined with `mem::forget` to hold a lock without
    /// the need to maintain a `ReentrantMutexGuard` object alive, for example when
    /// dealing with FFI.
    ///
    /// # Safety
    ///
    /// This method must only be called if the current thread logically owns a
    /// `ReentrantMutexGuard` but that guard has be discarded using `mem::forget`.
    /// Behavior is undefined if a mutex is unlocked when not locked.
    #[inline]
    pub unsafe fn force_unlock_fair(&self) {
        self.raw.unlock_fair();
    }
}

impl<R: RawMutexTimed, G: GetThreadId, T: ?Sized> ReentrantMutex<R, G, T> {
    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired, then
    /// `None` is returned. Otherwise, an RAII guard is returned. The lock will
    /// be unlocked when the guard is dropped.
    #[inline]
    pub fn try_lock_for(&self, timeout: R::Duration) -> Option<ReentrantMutexGuard<'_, R, G, T>> {
        if self.raw.try_lock_for(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired, then
    /// `None` is returned. Otherwise, an RAII guard is returned. The lock will
    /// be unlocked when the guard is dropped.
    #[inline]
    pub fn try_lock_until(&self, timeout: R::Instant) -> Option<ReentrantMutexGuard<'_, R, G, T>> {
        if self.raw.try_lock_until(timeout) {
            // SAFETY: The lock is held, as required.
            Some(unsafe { self.make_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this lock until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_lock_for` method; however, it requires the `ReentrantMutex` to be
    /// inside of an `Arc` and the resulting mutex guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_lock_arc_for(
        self: &Arc<Self>,
        timeout: R::Duration,
    ) -> Option<ArcReentrantMutexGuard<R, G, T>> {
        if self.raw.try_lock_for(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this lock until a timeout is reached, through an `Arc`.
    ///
    /// This method is similar to the `try_lock_until` method; however, it requires the `ReentrantMutex` to be
    /// inside of an `Arc` and the resulting mutex guard has no lifetime requirements.
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_lock_arc_until(
        self: &Arc<Self>,
        timeout: R::Instant,
    ) -> Option<ArcReentrantMutexGuard<R, G, T>> {
        if self.raw.try_lock_until(timeout) {
            // SAFETY: locking guarantee is upheld
            Some(unsafe { self.make_arc_guard_unchecked() })
        } else {
            None
        }
    }
}

impl<R: RawMutex, G: GetThreadId, T: ?Sized + Default> Default for ReentrantMutex<R, G, T> {
    #[inline]
    fn default() -> ReentrantMutex<R, G, T> {
        ReentrantMutex::new(Default::default())
    }
}

impl<R: RawMutex, G: GetThreadId, T> From<T> for ReentrantMutex<R, G, T> {
    #[inline]
    fn from(t: T) -> ReentrantMutex<R, G, T> {
        ReentrantMutex::new(t)
    }
}

impl<R: RawMutex, G: GetThreadId, T: ?Sized + fmt::Debug> fmt::Debug for ReentrantMutex<R, G, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f
                .debug_struct("ReentrantMutex")
                .field("data", &&*guard)
                .finish(),
            None => {
                struct LockedPlaceholder;
                impl fmt::Debug for LockedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<locked>")
                    }
                }

                f.debug_struct("ReentrantMutex")
                    .field("data", &LockedPlaceholder)
                    .finish()
            }
        }
    }
}

// Copied and modified from serde
#[cfg(feature = "serde")]
impl<R, G, T> Serialize for ReentrantMutex<R, G, T>
where
    R: RawMutex,
    G: GetThreadId,
    T: Serialize + ?Sized,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.lock().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, R, G, T> Deserialize<'de> for ReentrantMutex<R, G, T>
where
    R: RawMutex,
    G: GetThreadId,
    T: Deserialize<'de> + ?Sized,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(ReentrantMutex::new)
    }
}

/// An RAII implementation of a "scoped lock" of a reentrant mutex. When this structure
/// is dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// `Deref` implementation.
#[clippy::has_significant_drop]
#[must_use = "if unused the ReentrantMutex will immediately unlock"]
pub struct ReentrantMutexGuard<'a, R: RawMutex, G: GetThreadId, T: ?Sized> {
    remutex: &'a ReentrantMutex<R, G, T>,
    marker: PhantomData<(&'a T, GuardNoSend)>,
}

unsafe impl<'a, R: RawMutex + Sync + 'a, G: GetThreadId + Sync + 'a, T: ?Sized + Sync + 'a> Sync
    for ReentrantMutexGuard<'a, R, G, T>
{
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> ReentrantMutexGuard<'a, R, G, T> {
    /// Returns a reference to the original `ReentrantMutex` object.
    pub fn remutex(s: &Self) -> &'a ReentrantMutex<R, G, T> {
        s.remutex
    }

    /// Makes a new `MappedReentrantMutexGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `ReentrantMutexGuard` passed
    /// in already locked the mutex.
    ///
    /// This is an associated function that needs to be
    /// used as `ReentrantMutexGuard::map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedReentrantMutexGuard<'a, R, G, U>
    where
        F: FnOnce(&T) -> &U,
    {
        let raw = &s.remutex.raw;
        let data = f(unsafe { &*s.remutex.data.get() });
        mem::forget(s);
        MappedReentrantMutexGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make  a new `MappedReentrantMutexGuard` for a component of the
    /// locked data. The original guard is return if the closure returns `None`.
    ///
    /// This operation cannot fail as the `ReentrantMutexGuard` passed
    /// in already locked the mutex.
    ///
    /// This is an associated function that needs to be
    /// used as `ReentrantMutexGuard::try_map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn try_map<U: ?Sized, F>(
        s: Self,
        f: F,
    ) -> Result<MappedReentrantMutexGuard<'a, R, G, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        let raw = &s.remutex.raw;
        let data = match f(unsafe { &*s.remutex.data.get() }) {
            Some(data) => data,
            None => return Err(s),
        };
        mem::forget(s);
        Ok(MappedReentrantMutexGuard {
            raw,
            data,
            marker: PhantomData,
        })
    }

    /// Temporarily unlocks the mutex to execute the given function.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the mutex.
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: A ReentrantMutexGuard always holds the lock.
        unsafe {
            s.remutex.raw.unlock();
        }
        defer!(s.remutex.raw.lock());
        f()
    }
}

impl<'a, R: RawMutexFair + 'a, G: GetThreadId + 'a, T: ?Sized + 'a>
    ReentrantMutexGuard<'a, R, G, T>
{
    /// Unlocks the mutex using a fair unlock protocol.
    ///
    /// By default, mutexes are unfair and allow the current thread to re-lock
    /// the mutex before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the mutex for a long time. This is the
    /// default because it allows much higher throughput as it avoids forcing a
    /// context switch on every mutex unlock. This can result in one thread
    /// acquiring a mutex many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `ReentrantMutexGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: A ReentrantMutexGuard always holds the lock
        unsafe {
            s.remutex.raw.unlock_fair();
        }
        mem::forget(s);
    }

    /// Temporarily unlocks the mutex to execute the given function.
    ///
    /// The mutex is unlocked a fair unlock protocol.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the mutex.
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: A ReentrantMutexGuard always holds the lock
        unsafe {
            s.remutex.raw.unlock_fair();
        }
        defer!(s.remutex.raw.lock());
        f()
    }

    /// Temporarily yields the mutex to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `unlock_fair` followed
    /// by `lock`, however it can be much more efficient in the case where there
    /// are no waiting threads.
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: A ReentrantMutexGuard always holds the lock
        unsafe {
            s.remutex.raw.bump();
        }
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> Deref
    for ReentrantMutexGuard<'a, R, G, T>
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.remutex.data.get() }
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> Drop
    for ReentrantMutexGuard<'a, R, G, T>
{
    #[inline]
    fn drop(&mut self) {
        // Safety: A ReentrantMutexGuard always holds the lock.
        unsafe {
            self.remutex.raw.unlock();
        }
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug
    for ReentrantMutexGuard<'a, R, G, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for ReentrantMutexGuard<'a, R, G, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> StableAddress
    for ReentrantMutexGuard<'a, R, G, T>
{
}

/// An RAII mutex guard returned by the `Arc` locking operations on `ReentrantMutex`.
///
/// This is similar to the `ReentrantMutexGuard` struct, except instead of using a reference to unlock the
/// `Mutex` it uses an `Arc<ReentrantMutex>`. This has several advantages, most notably that it has an `'static`
/// lifetime.
#[cfg(feature = "arc_lock")]
#[clippy::has_significant_drop]
#[must_use = "if unused the ReentrantMutex will immediately unlock"]
pub struct ArcReentrantMutexGuard<R: RawMutex, G: GetThreadId, T: ?Sized> {
    remutex: Arc<ReentrantMutex<R, G, T>>,
    marker: PhantomData<GuardNoSend>,
}

#[cfg(feature = "arc_lock")]
impl<R: RawMutex, G: GetThreadId, T: ?Sized> ArcReentrantMutexGuard<R, G, T> {
    /// Returns a reference to the `ReentrantMutex` this object is guarding, contained in its `Arc`.
    pub fn remutex(s: &Self) -> &Arc<ReentrantMutex<R, G, T>> {
        &s.remutex
    }

    /// Temporarily unlocks the mutex to execute the given function.
    ///
    /// This is safe because `&mut` guarantees that there exist no other
    /// references to the data protected by the mutex.
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: A ReentrantMutexGuard always holds the lock.
        unsafe {
            s.remutex.raw.unlock();
        }
        defer!(s.remutex.raw.lock());
        f()
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawMutexFair, G: GetThreadId, T: ?Sized> ArcReentrantMutexGuard<R, G, T> {
    /// Unlocks the mutex using a fair unlock protocol.
    ///
    /// This is functionally identical to the `unlock_fair` method on [`ReentrantMutexGuard`].
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: A ReentrantMutexGuard always holds the lock
        unsafe {
            s.remutex.raw.unlock_fair();
        }

        // SAFETY: ensure that the Arc's refcount is decremented
        let mut s = ManuallyDrop::new(s);
        unsafe { ptr::drop_in_place(&mut s.remutex) };
    }

    /// Temporarily unlocks the mutex to execute the given function.
    ///
    /// This is functionally identical to the `unlocked_fair` method on [`ReentrantMutexGuard`].
    #[inline]
    pub fn unlocked_fair<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // Safety: A ReentrantMutexGuard always holds the lock
        unsafe {
            s.remutex.raw.unlock_fair();
        }
        defer!(s.remutex.raw.lock());
        f()
    }

    /// Temporarily yields the mutex to a waiting thread if there is one.
    ///
    /// This is functionally equivalent to the `bump` method on [`ReentrantMutexGuard`].
    #[inline]
    pub fn bump(s: &mut Self) {
        // Safety: A ReentrantMutexGuard always holds the lock
        unsafe {
            s.remutex.raw.bump();
        }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawMutex, G: GetThreadId, T: ?Sized> Deref for ArcReentrantMutexGuard<R, G, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.remutex.data.get() }
    }
}

#[cfg(feature = "arc_lock")]
impl<R: RawMutex, G: GetThreadId, T: ?Sized> Drop for ArcReentrantMutexGuard<R, G, T> {
    #[inline]
    fn drop(&mut self) {
        // Safety: A ReentrantMutexGuard always holds the lock.
        unsafe {
            self.remutex.raw.unlock();
        }
    }
}

/// An RAII mutex guard returned by `ReentrantMutexGuard::map`, which can point to a
/// subfield of the protected data.
///
/// The main difference between `MappedReentrantMutexGuard` and `ReentrantMutexGuard` is that the
/// former doesn't support temporarily unlocking and re-locking, since that
/// could introduce soundness issues if the locked object is modified by another
/// thread.
#[clippy::has_significant_drop]
#[must_use = "if unused the ReentrantMutex will immediately unlock"]
pub struct MappedReentrantMutexGuard<'a, R: RawMutex, G: GetThreadId, T: ?Sized> {
    raw: &'a RawReentrantMutex<R, G>,
    data: *const T,
    marker: PhantomData<&'a T>,
}

unsafe impl<'a, R: RawMutex + Sync + 'a, G: GetThreadId + Sync + 'a, T: ?Sized + Sync + 'a> Sync
    for MappedReentrantMutexGuard<'a, R, G, T>
{
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a>
    MappedReentrantMutexGuard<'a, R, G, T>
{
    /// Makes a new `MappedReentrantMutexGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `MappedReentrantMutexGuard` passed
    /// in already locked the mutex.
    ///
    /// This is an associated function that needs to be
    /// used as `MappedReentrantMutexGuard::map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedReentrantMutexGuard<'a, R, G, U>
    where
        F: FnOnce(&T) -> &U,
    {
        let raw = s.raw;
        let data = f(unsafe { &*s.data });
        mem::forget(s);
        MappedReentrantMutexGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make  a new `MappedReentrantMutexGuard` for a component of the
    /// locked data. The original guard is return if the closure returns `None`.
    ///
    /// This operation cannot fail as the `MappedReentrantMutexGuard` passed
    /// in already locked the mutex.
    ///
    /// This is an associated function that needs to be
    /// used as `MappedReentrantMutexGuard::try_map(...)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    #[inline]
    pub fn try_map<U: ?Sized, F>(
        s: Self,
        f: F,
    ) -> Result<MappedReentrantMutexGuard<'a, R, G, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        let raw = s.raw;
        let data = match f(unsafe { &*s.data }) {
            Some(data) => data,
            None => return Err(s),
        };
        mem::forget(s);
        Ok(MappedReentrantMutexGuard {
            raw,
            data,
            marker: PhantomData,
        })
    }
}

impl<'a, R: RawMutexFair + 'a, G: GetThreadId + 'a, T: ?Sized + 'a>
    MappedReentrantMutexGuard<'a, R, G, T>
{
    /// Unlocks the mutex using a fair unlock protocol.
    ///
    /// By default, mutexes are unfair and allow the current thread to re-lock
    /// the mutex before another has the chance to acquire the lock, even if
    /// that thread has been blocked on the mutex for a long time. This is the
    /// default because it allows much higher throughput as it avoids forcing a
    /// context switch on every mutex unlock. This can result in one thread
    /// acquiring a mutex many more times than other threads.
    ///
    /// However in some cases it can be beneficial to ensure fairness by forcing
    /// the lock to pass on to a waiting thread if there is one. This is done by
    /// using this method instead of dropping the `ReentrantMutexGuard` normally.
    #[inline]
    pub fn unlock_fair(s: Self) {
        // Safety: A MappedReentrantMutexGuard always holds the lock
        unsafe {
            s.raw.unlock_fair();
        }
        mem::forget(s);
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> Deref
    for MappedReentrantMutexGuard<'a, R, G, T>
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> Drop
    for MappedReentrantMutexGuard<'a, R, G, T>
{
    #[inline]
    fn drop(&mut self) {
        // Safety: A MappedReentrantMutexGuard always holds the lock.
        unsafe {
            self.raw.unlock();
        }
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: fmt::Debug + ?Sized + 'a> fmt::Debug
    for MappedReentrantMutexGuard<'a, R, G, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: fmt::Display + ?Sized + 'a> fmt::Display
    for MappedReentrantMutexGuard<'a, R, G, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[cfg(feature = "owning_ref")]
unsafe impl<'a, R: RawMutex + 'a, G: GetThreadId + 'a, T: ?Sized + 'a> StableAddress
    for MappedReentrantMutexGuard<'a, R, G, T>
{
}
