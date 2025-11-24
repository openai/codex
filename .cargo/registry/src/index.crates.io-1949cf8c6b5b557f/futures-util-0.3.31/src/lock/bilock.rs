//! Futures-powered synchronization primitives.

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering::SeqCst;
use core::{fmt, ptr};
#[cfg(feature = "bilock")]
use futures_core::future::Future;
use futures_core::task::{Context, Poll, Waker};

/// A type of futures-powered synchronization primitive which is a mutex between
/// two possible owners.
///
/// This primitive is not as generic as a full-blown mutex but is sufficient for
/// many use cases where there are only two possible owners of a resource. The
/// implementation of `BiLock` can be more optimized for just the two possible
/// owners.
///
/// Note that it's possible to use this lock through a poll-style interface with
/// the `poll_lock` method but you can also use it as a future with the `lock`
/// method that consumes a `BiLock` and returns a future that will resolve when
/// it's locked.
///
/// A `BiLock` is typically used for "split" operations where data which serves
/// two purposes wants to be split into two to be worked with separately. For
/// example a TCP stream could be both a reader and a writer or a framing layer
/// could be both a stream and a sink for messages. A `BiLock` enables splitting
/// these two and then using each independently in a futures-powered fashion.
///
/// This type is only available when the `bilock` feature of this
/// library is activated.
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
pub struct BiLock<T> {
    arc: Arc<Inner<T>>,
}

#[derive(Debug)]
struct Inner<T> {
    state: AtomicPtr<Waker>,
    value: Option<UnsafeCell<T>>,
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

impl<T> BiLock<T> {
    /// Creates a new `BiLock` protecting the provided data.
    ///
    /// Two handles to the lock are returned, and these are the only two handles
    /// that will ever be available to the lock. These can then be sent to separate
    /// tasks to be managed there.
    ///
    /// The data behind the bilock is considered to be pinned, which allows `Pin`
    /// references to locked data. However, this means that the locked value
    /// will only be available through `Pin<&mut T>` (not `&mut T`) unless `T` is `Unpin`.
    /// Similarly, reuniting the lock and extracting the inner value is only
    /// possible when `T` is `Unpin`.
    pub fn new(t: T) -> (Self, Self) {
        let arc = Arc::new(Inner {
            state: AtomicPtr::new(ptr::null_mut()),
            value: Some(UnsafeCell::new(t)),
        });

        (Self { arc: arc.clone() }, Self { arc })
    }

    /// Attempt to acquire this lock, returning `Pending` if it can't be
    /// acquired.
    ///
    /// This function will acquire the lock in a nonblocking fashion, returning
    /// immediately if the lock is already held. If the lock is successfully
    /// acquired then `Poll::Ready` is returned with a value that represents
    /// the locked value (and can be used to access the protected data). The
    /// lock is unlocked when the returned `BiLockGuard` is dropped.
    ///
    /// If the lock is already held then this function will return
    /// `Poll::Pending`. In this case the current task will also be scheduled
    /// to receive a notification when the lock would otherwise become
    /// available.
    ///
    /// # Panics
    ///
    /// This function will panic if called outside the context of a future's
    /// task.
    pub fn poll_lock(&self, cx: &mut Context<'_>) -> Poll<BiLockGuard<'_, T>> {
        let mut waker = None;
        loop {
            let n = self.arc.state.swap(invalid_ptr(1), SeqCst);
            match n as usize {
                // Woohoo, we grabbed the lock!
                0 => return Poll::Ready(BiLockGuard { bilock: self }),

                // Oops, someone else has locked the lock
                1 => {}

                // A task was previously blocked on this lock, likely our task,
                // so we need to update that task.
                _ => unsafe {
                    let mut prev = Box::from_raw(n);
                    *prev = cx.waker().clone();
                    waker = Some(prev);
                },
            }

            // type ascription for safety's sake!
            let me: Box<Waker> = waker.take().unwrap_or_else(|| Box::new(cx.waker().clone()));
            let me = Box::into_raw(me);

            match self.arc.state.compare_exchange(invalid_ptr(1), me, SeqCst, SeqCst) {
                // The lock is still locked, but we've now parked ourselves, so
                // just report that we're scheduled to receive a notification.
                Ok(_) => return Poll::Pending,

                // Oops, looks like the lock was unlocked after our swap above
                // and before the compare_exchange. Deallocate what we just
                // allocated and go through the loop again.
                Err(n) if n.is_null() => unsafe {
                    waker = Some(Box::from_raw(me));
                },

                // The top of this loop set the previous state to 1, so if we
                // failed the CAS above then it's because the previous value was
                // *not* zero or one. This indicates that a task was blocked,
                // but we're trying to acquire the lock and there's only one
                // other reference of the lock, so it should be impossible for
                // that task to ever block itself.
                Err(n) => panic!("invalid state: {}", n as usize),
            }
        }
    }

    /// Perform a "blocking lock" of this lock, consuming this lock handle and
    /// returning a future to the acquired lock.
    ///
    /// This function consumes the `BiLock<T>` and returns a sentinel future,
    /// `BiLockAcquire<T>`. The returned future will resolve to
    /// `BiLockAcquired<T>` which represents a locked lock similarly to
    /// `BiLockGuard<T>`.
    ///
    /// Note that the returned future will never resolve to an error.
    #[cfg(feature = "bilock")]
    #[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
    pub fn lock(&self) -> BiLockAcquire<'_, T> {
        BiLockAcquire { bilock: self }
    }

    /// Returns `true` only if the other `BiLock<T>` originated from the same call to `BiLock::new`.
    pub fn is_pair_of(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.arc, &other.arc)
    }

    /// Attempts to put the two "halves" of a `BiLock<T>` back together and
    /// recover the original value. Succeeds only if the two `BiLock<T>`s
    /// originated from the same call to `BiLock::new`.
    pub fn reunite(self, other: Self) -> Result<T, ReuniteError<T>>
    where
        T: Unpin,
    {
        if self.is_pair_of(&other) {
            drop(other);
            let inner = Arc::try_unwrap(self.arc)
                .ok()
                .expect("futures: try_unwrap failed in BiLock<T>::reunite");
            Ok(unsafe { inner.into_value() })
        } else {
            Err(ReuniteError(self, other))
        }
    }

    fn unlock(&self) {
        let n = self.arc.state.swap(ptr::null_mut(), SeqCst);
        match n as usize {
            // we've locked the lock, shouldn't be possible for us to see an
            // unlocked lock.
            0 => panic!("invalid unlocked state"),

            // Ok, no one else tried to get the lock, we're done.
            1 => {}

            // Another task has parked themselves on this lock, let's wake them
            // up as its now their turn.
            _ => unsafe {
                Box::from_raw(n).wake();
            },
        }
    }
}

impl<T: Unpin> Inner<T> {
    unsafe fn into_value(mut self) -> T {
        self.value.take().unwrap().into_inner()
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        assert!(self.state.load(SeqCst).is_null());
    }
}

/// Error indicating two `BiLock<T>`s were not two halves of a whole, and
/// thus could not be `reunite`d.
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
pub struct ReuniteError<T>(pub BiLock<T>, pub BiLock<T>);

impl<T> fmt::Debug for ReuniteError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ReuniteError").field(&"...").finish()
    }
}

impl<T> fmt::Display for ReuniteError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tried to reunite two BiLocks that don't form a pair")
    }
}

#[cfg(feature = "std")]
impl<T: core::any::Any> std::error::Error for ReuniteError<T> {}

/// Returned RAII guard from the `poll_lock` method.
///
/// This structure acts as a sentinel to the data in the `BiLock<T>` itself,
/// implementing `Deref` and `DerefMut` to `T`. When dropped, the lock will be
/// unlocked.
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
pub struct BiLockGuard<'a, T> {
    bilock: &'a BiLock<T>,
}

// We allow parallel access to T via Deref, so Sync bound is also needed here.
unsafe impl<T: Send + Sync> Sync for BiLockGuard<'_, T> {}

impl<T> Deref for BiLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.bilock.arc.value.as_ref().unwrap().get() }
    }
}

impl<T: Unpin> DerefMut for BiLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.bilock.arc.value.as_ref().unwrap().get() }
    }
}

impl<T> BiLockGuard<'_, T> {
    /// Get a mutable pinned reference to the locked value.
    pub fn as_pin_mut(&mut self) -> Pin<&mut T> {
        // Safety: we never allow moving a !Unpin value out of a bilock, nor
        // allow mutable access to it
        unsafe { Pin::new_unchecked(&mut *self.bilock.arc.value.as_ref().unwrap().get()) }
    }
}

impl<T> Drop for BiLockGuard<'_, T> {
    fn drop(&mut self) {
        self.bilock.unlock();
    }
}

/// Future returned by `BiLock::lock` which will resolve when the lock is
/// acquired.
#[cfg(feature = "bilock")]
#[cfg_attr(docsrs, doc(cfg(feature = "bilock")))]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct BiLockAcquire<'a, T> {
    bilock: &'a BiLock<T>,
}

// Pinning is never projected to fields
#[cfg(feature = "bilock")]
impl<T> Unpin for BiLockAcquire<'_, T> {}

#[cfg(feature = "bilock")]
impl<'a, T> Future for BiLockAcquire<'a, T> {
    type Output = BiLockGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.bilock.poll_lock(cx)
    }
}

// Based on core::ptr::invalid_mut. Equivalent to `addr as *mut T`, but is strict-provenance compatible.
#[allow(clippy::useless_transmute)]
#[inline]
fn invalid_ptr<T>(addr: usize) -> *mut T {
    // SAFETY: every valid integer is also a valid pointer (as long as you don't dereference that
    // pointer).
    unsafe { core::mem::transmute(addr) }
}
