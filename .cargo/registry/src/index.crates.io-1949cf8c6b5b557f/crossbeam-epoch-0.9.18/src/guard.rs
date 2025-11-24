use core::fmt;
use core::mem;

use crate::atomic::Shared;
use crate::collector::Collector;
use crate::deferred::Deferred;
use crate::internal::Local;

/// A guard that keeps the current thread pinned.
///
/// # Pinning
///
/// The current thread is pinned by calling [`pin`], which returns a new guard:
///
/// ```
/// use crossbeam_epoch as epoch;
///
/// // It is often convenient to prefix a call to `pin` with a `&` in order to create a reference.
/// // This is not really necessary, but makes passing references to the guard a bit easier.
/// let guard = &epoch::pin();
/// ```
///
/// When a guard gets dropped, the current thread is automatically unpinned.
///
/// # Pointers on the stack
///
/// Having a guard allows us to create pointers on the stack to heap-allocated objects.
/// For example:
///
/// ```
/// use crossbeam_epoch::{self as epoch, Atomic};
/// use std::sync::atomic::Ordering::SeqCst;
///
/// // Create a heap-allocated number.
/// let a = Atomic::new(777);
///
/// // Pin the current thread.
/// let guard = &epoch::pin();
///
/// // Load the heap-allocated object and create pointer `p` on the stack.
/// let p = a.load(SeqCst, guard);
///
/// // Dereference the pointer and print the value:
/// if let Some(num) = unsafe { p.as_ref() } {
///     println!("The number is {}.", num);
/// }
/// # unsafe { drop(a.into_owned()); } // avoid leak
/// ```
///
/// # Multiple guards
///
/// Pinning is reentrant and it is perfectly legal to create multiple guards. In that case, the
/// thread will actually be pinned only when the first guard is created and unpinned when the last
/// one is dropped:
///
/// ```
/// use crossbeam_epoch as epoch;
///
/// let guard1 = epoch::pin();
/// let guard2 = epoch::pin();
/// assert!(epoch::is_pinned());
/// drop(guard1);
/// assert!(epoch::is_pinned());
/// drop(guard2);
/// assert!(!epoch::is_pinned());
/// ```
///
/// [`pin`]: super::pin
pub struct Guard {
    pub(crate) local: *const Local,
}

impl Guard {
    /// Stores a function so that it can be executed at some point after all currently pinned
    /// threads get unpinned.
    ///
    /// This method first stores `f` into the thread-local (or handle-local) cache. If this cache
    /// becomes full, some functions are moved into the global cache. At the same time, some
    /// functions from both local and global caches may get executed in order to incrementally
    /// clean up the caches as they fill up.
    ///
    /// There is no guarantee when exactly `f` will be executed. The only guarantee is that it
    /// won't be executed until all currently pinned threads get unpinned. In theory, `f` might
    /// never run, but the epoch-based garbage collection will make an effort to execute it
    /// reasonably soon.
    ///
    /// If this method is called from an [`unprotected`] guard, the function will simply be
    /// executed immediately.
    pub fn defer<F, R>(&self, f: F)
    where
        F: FnOnce() -> R,
        F: Send + 'static,
    {
        unsafe {
            self.defer_unchecked(f);
        }
    }

    /// Stores a function so that it can be executed at some point after all currently pinned
    /// threads get unpinned.
    ///
    /// This method first stores `f` into the thread-local (or handle-local) cache. If this cache
    /// becomes full, some functions are moved into the global cache. At the same time, some
    /// functions from both local and global caches may get executed in order to incrementally
    /// clean up the caches as they fill up.
    ///
    /// There is no guarantee when exactly `f` will be executed. The only guarantee is that it
    /// won't be executed until all currently pinned threads get unpinned. In theory, `f` might
    /// never run, but the epoch-based garbage collection will make an effort to execute it
    /// reasonably soon.
    ///
    /// If this method is called from an [`unprotected`] guard, the function will simply be
    /// executed immediately.
    ///
    /// # Safety
    ///
    /// The given function must not hold reference onto the stack. It is highly recommended that
    /// the passed function is **always** marked with `move` in order to prevent accidental
    /// borrows.
    ///
    /// ```
    /// use crossbeam_epoch as epoch;
    ///
    /// let guard = &epoch::pin();
    /// let message = "Hello!";
    /// unsafe {
    ///     // ALWAYS use `move` when sending a closure into `defer_unchecked`.
    ///     guard.defer_unchecked(move || {
    ///         println!("{}", message);
    ///     });
    /// }
    /// ```
    ///
    /// Apart from that, keep in mind that another thread may execute `f`, so anything accessed by
    /// the closure must be `Send`.
    ///
    /// We intentionally didn't require `F: Send`, because Rust's type systems usually cannot prove
    /// `F: Send` for typical use cases. For example, consider the following code snippet, which
    /// exemplifies the typical use case of deferring the deallocation of a shared reference:
    ///
    /// ```ignore
    /// let shared = Owned::new(7i32).into_shared(guard);
    /// guard.defer_unchecked(move || shared.into_owned()); // `Shared` is not `Send`!
    /// ```
    ///
    /// While `Shared` is not `Send`, it's safe for another thread to call the deferred function,
    /// because it's called only after the grace period and `shared` is no longer shared with other
    /// threads. But we don't expect type systems to prove this.
    ///
    /// # Examples
    ///
    /// When a heap-allocated object in a data structure becomes unreachable, it has to be
    /// deallocated. However, the current thread and other threads may be still holding references
    /// on the stack to that same object. Therefore it cannot be deallocated before those references
    /// get dropped. This method can defer deallocation until all those threads get unpinned and
    /// consequently drop all their references on the stack.
    ///
    /// ```
    /// use crossbeam_epoch::{self as epoch, Atomic, Owned};
    /// use std::sync::atomic::Ordering::SeqCst;
    ///
    /// let a = Atomic::new("foo");
    ///
    /// // Now suppose that `a` is shared among multiple threads and concurrently
    /// // accessed and modified...
    ///
    /// // Pin the current thread.
    /// let guard = &epoch::pin();
    ///
    /// // Steal the object currently stored in `a` and swap it with another one.
    /// let p = a.swap(Owned::new("bar").into_shared(guard), SeqCst, guard);
    ///
    /// if !p.is_null() {
    ///     // The object `p` is pointing to is now unreachable.
    ///     // Defer its deallocation until all currently pinned threads get unpinned.
    ///     unsafe {
    ///         // ALWAYS use `move` when sending a closure into `defer_unchecked`.
    ///         guard.defer_unchecked(move || {
    ///             println!("{} is now being deallocated.", p.deref());
    ///             // Now we have unique access to the object pointed to by `p` and can turn it
    ///             // into an `Owned`. Dropping the `Owned` will deallocate the object.
    ///             drop(p.into_owned());
    ///         });
    ///     }
    /// }
    /// # unsafe { drop(a.into_owned()); } // avoid leak
    /// ```
    pub unsafe fn defer_unchecked<F, R>(&self, f: F)
    where
        F: FnOnce() -> R,
    {
        if let Some(local) = self.local.as_ref() {
            local.defer(Deferred::new(move || drop(f())), self);
        } else {
            drop(f());
        }
    }

    /// Stores a destructor for an object so that it can be deallocated and dropped at some point
    /// after all currently pinned threads get unpinned.
    ///
    /// This method first stores the destructor into the thread-local (or handle-local) cache. If
    /// this cache becomes full, some destructors are moved into the global cache. At the same
    /// time, some destructors from both local and global caches may get executed in order to
    /// incrementally clean up the caches as they fill up.
    ///
    /// There is no guarantee when exactly the destructor will be executed. The only guarantee is
    /// that it won't be executed until all currently pinned threads get unpinned. In theory, the
    /// destructor might never run, but the epoch-based garbage collection will make an effort to
    /// execute it reasonably soon.
    ///
    /// If this method is called from an [`unprotected`] guard, the destructor will simply be
    /// executed immediately.
    ///
    /// # Safety
    ///
    /// The object must not be reachable by other threads anymore, otherwise it might be still in
    /// use when the destructor runs.
    ///
    /// Apart from that, keep in mind that another thread may execute the destructor, so the object
    /// must be sendable to other threads.
    ///
    /// We intentionally didn't require `T: Send`, because Rust's type systems usually cannot prove
    /// `T: Send` for typical use cases. For example, consider the following code snippet, which
    /// exemplifies the typical use case of deferring the deallocation of a shared reference:
    ///
    /// ```ignore
    /// let shared = Owned::new(7i32).into_shared(guard);
    /// guard.defer_destroy(shared); // `Shared` is not `Send`!
    /// ```
    ///
    /// While `Shared` is not `Send`, it's safe for another thread to call the destructor, because
    /// it's called only after the grace period and `shared` is no longer shared with other
    /// threads. But we don't expect type systems to prove this.
    ///
    /// # Examples
    ///
    /// When a heap-allocated object in a data structure becomes unreachable, it has to be
    /// deallocated. However, the current thread and other threads may be still holding references
    /// on the stack to that same object. Therefore it cannot be deallocated before those references
    /// get dropped. This method can defer deallocation until all those threads get unpinned and
    /// consequently drop all their references on the stack.
    ///
    /// ```
    /// use crossbeam_epoch::{self as epoch, Atomic, Owned};
    /// use std::sync::atomic::Ordering::SeqCst;
    ///
    /// let a = Atomic::new("foo");
    ///
    /// // Now suppose that `a` is shared among multiple threads and concurrently
    /// // accessed and modified...
    ///
    /// // Pin the current thread.
    /// let guard = &epoch::pin();
    ///
    /// // Steal the object currently stored in `a` and swap it with another one.
    /// let p = a.swap(Owned::new("bar").into_shared(guard), SeqCst, guard);
    ///
    /// if !p.is_null() {
    ///     // The object `p` is pointing to is now unreachable.
    ///     // Defer its deallocation until all currently pinned threads get unpinned.
    ///     unsafe {
    ///         guard.defer_destroy(p);
    ///     }
    /// }
    /// # unsafe { drop(a.into_owned()); } // avoid leak
    /// ```
    pub unsafe fn defer_destroy<T>(&self, ptr: Shared<'_, T>) {
        self.defer_unchecked(move || ptr.into_owned());
    }

    /// Clears up the thread-local cache of deferred functions by executing them or moving into the
    /// global cache.
    ///
    /// Call this method after deferring execution of a function if you want to get it executed as
    /// soon as possible. Flushing will make sure it is residing in in the global cache, so that
    /// any thread has a chance of taking the function and executing it.
    ///
    /// If this method is called from an [`unprotected`] guard, it is a no-op (nothing happens).
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_epoch as epoch;
    ///
    /// let guard = &epoch::pin();
    /// guard.defer(move || {
    ///     println!("This better be printed as soon as possible!");
    /// });
    /// guard.flush();
    /// ```
    pub fn flush(&self) {
        if let Some(local) = unsafe { self.local.as_ref() } {
            local.flush(self);
        }
    }

    /// Unpins and then immediately re-pins the thread.
    ///
    /// This method is useful when you don't want delay the advancement of the global epoch by
    /// holding an old epoch. For safety, you should not maintain any guard-based reference across
    /// the call (the latter is enforced by `&mut self`). The thread will only be repinned if this
    /// is the only active guard for the current thread.
    ///
    /// If this method is called from an [`unprotected`] guard, then the call will be just no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_epoch::{self as epoch, Atomic};
    /// use std::sync::atomic::Ordering::SeqCst;
    ///
    /// let a = Atomic::new(777);
    /// let mut guard = epoch::pin();
    /// {
    ///     let p = a.load(SeqCst, &guard);
    ///     assert_eq!(unsafe { p.as_ref() }, Some(&777));
    /// }
    /// guard.repin();
    /// {
    ///     let p = a.load(SeqCst, &guard);
    ///     assert_eq!(unsafe { p.as_ref() }, Some(&777));
    /// }
    /// # unsafe { drop(a.into_owned()); } // avoid leak
    /// ```
    pub fn repin(&mut self) {
        if let Some(local) = unsafe { self.local.as_ref() } {
            local.repin();
        }
    }

    /// Temporarily unpins the thread, executes the given function and then re-pins the thread.
    ///
    /// This method is useful when you need to perform a long-running operation (e.g. sleeping)
    /// and don't need to maintain any guard-based reference across the call (the latter is enforced
    /// by `&mut self`). The thread will only be unpinned if this is the only active guard for the
    /// current thread.
    ///
    /// If this method is called from an [`unprotected`] guard, then the passed function is called
    /// directly without unpinning the thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_epoch::{self as epoch, Atomic};
    /// use std::sync::atomic::Ordering::SeqCst;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let a = Atomic::new(777);
    /// let mut guard = epoch::pin();
    /// {
    ///     let p = a.load(SeqCst, &guard);
    ///     assert_eq!(unsafe { p.as_ref() }, Some(&777));
    /// }
    /// guard.repin_after(|| thread::sleep(Duration::from_millis(50)));
    /// {
    ///     let p = a.load(SeqCst, &guard);
    ///     assert_eq!(unsafe { p.as_ref() }, Some(&777));
    /// }
    /// # unsafe { drop(a.into_owned()); } // avoid leak
    /// ```
    pub fn repin_after<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // Ensure the Guard is re-pinned even if the function panics
        struct ScopeGuard(*const Local);
        impl Drop for ScopeGuard {
            fn drop(&mut self) {
                if let Some(local) = unsafe { self.0.as_ref() } {
                    mem::forget(local.pin());
                    local.release_handle();
                }
            }
        }

        if let Some(local) = unsafe { self.local.as_ref() } {
            // We need to acquire a handle here to ensure the Local doesn't
            // disappear from under us.
            local.acquire_handle();
            local.unpin();
        }

        let _guard = ScopeGuard(self.local);

        f()
    }

    /// Returns the `Collector` associated with this guard.
    ///
    /// This method is useful when you need to ensure that all guards used with
    /// a data structure come from the same collector.
    ///
    /// If this method is called from an [`unprotected`] guard, then `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_epoch as epoch;
    ///
    /// let guard1 = epoch::pin();
    /// let guard2 = epoch::pin();
    /// assert!(guard1.collector() == guard2.collector());
    /// ```
    pub fn collector(&self) -> Option<&Collector> {
        unsafe { self.local.as_ref().map(|local| local.collector()) }
    }
}

impl Drop for Guard {
    #[inline]
    fn drop(&mut self) {
        if let Some(local) = unsafe { self.local.as_ref() } {
            local.unpin();
        }
    }
}

impl fmt::Debug for Guard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Guard { .. }")
    }
}

/// Returns a reference to a dummy guard that allows unprotected access to [`Atomic`]s.
///
/// This guard should be used in special occasions only. Note that it doesn't actually keep any
/// thread pinned - it's just a fake guard that allows loading from [`Atomic`]s unsafely.
///
/// Note that calling [`defer`] with a dummy guard will not defer the function - it will just
/// execute the function immediately.
///
/// If necessary, it's possible to create more dummy guards by cloning: `unprotected().clone()`.
///
/// # Safety
///
/// Loading and dereferencing data from an [`Atomic`] using this guard is safe only if the
/// [`Atomic`] is not being concurrently modified by other threads.
///
/// # Examples
///
/// ```
/// use crossbeam_epoch::{self as epoch, Atomic};
/// use std::sync::atomic::Ordering::Relaxed;
///
/// let a = Atomic::new(7);
///
/// unsafe {
///     // Load `a` without pinning the current thread.
///     a.load(Relaxed, epoch::unprotected());
///
///     // It's possible to create more dummy guards.
///     let dummy = epoch::unprotected();
///
///     dummy.defer(move || {
///         println!("This gets executed immediately.");
///     });
///
///     // Dropping `dummy` doesn't affect the current thread - it's just a noop.
/// }
/// # unsafe { drop(a.into_owned()); } // avoid leak
/// ```
///
/// The most common use of this function is when constructing or destructing a data structure.
///
/// For example, we can use a dummy guard in the destructor of a Treiber stack because at that
/// point no other thread could concurrently modify the [`Atomic`]s we are accessing.
///
/// If we were to actually pin the current thread during destruction, that would just unnecessarily
/// delay garbage collection and incur some performance cost, so in cases like these `unprotected`
/// is very helpful.
///
/// ```
/// use crossbeam_epoch::{self as epoch, Atomic};
/// use std::mem::ManuallyDrop;
/// use std::sync::atomic::Ordering::Relaxed;
///
/// struct Stack<T> {
///     head: Atomic<Node<T>>,
/// }
///
/// struct Node<T> {
///     data: ManuallyDrop<T>,
///     next: Atomic<Node<T>>,
/// }
///
/// impl<T> Drop for Stack<T> {
///     fn drop(&mut self) {
///         unsafe {
///             // Unprotected load.
///             let mut node = self.head.load(Relaxed, epoch::unprotected());
///
///             while let Some(n) = node.as_ref() {
///                 // Unprotected load.
///                 let next = n.next.load(Relaxed, epoch::unprotected());
///
///                 // Take ownership of the node, then drop its data and deallocate it.
///                 let mut o = node.into_owned();
///                 ManuallyDrop::drop(&mut o.data);
///                 drop(o);
///
///                 node = next;
///             }
///         }
///     }
/// }
/// ```
///
/// [`Atomic`]: super::Atomic
/// [`defer`]: Guard::defer
#[inline]
pub unsafe fn unprotected() -> &'static Guard {
    // An unprotected guard is just a `Guard` with its field `local` set to null.
    // We make a newtype over `Guard` because `Guard` isn't `Sync`, so can't be directly stored in
    // a `static`
    struct GuardWrapper(Guard);
    unsafe impl Sync for GuardWrapper {}
    static UNPROTECTED: GuardWrapper = GuardWrapper(Guard {
        local: core::ptr::null(),
    });
    &UNPROTECTED.0
}
