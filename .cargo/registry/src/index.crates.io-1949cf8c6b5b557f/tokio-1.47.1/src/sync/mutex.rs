#![cfg_attr(not(feature = "sync"), allow(unreachable_pub, dead_code))]

use crate::sync::batch_semaphore as semaphore;
#[cfg(all(tokio_unstable, feature = "tracing"))]
use crate::util::trace;

use std::cell::UnsafeCell;
use std::error::Error;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::{fmt, mem, ptr};

/// An asynchronous `Mutex`-like type.
///
/// This type acts similarly to [`std::sync::Mutex`], with two major
/// differences: [`lock`] is an async method so does not block, and the lock
/// guard is designed to be held across `.await` points.
///
/// Tokio's Mutex operates on a guaranteed FIFO basis.
/// This means that the order in which tasks call the [`lock`] method is
/// the exact order in which they will acquire the lock.
///
/// # Which kind of mutex should you use?
///
/// Contrary to popular belief, it is ok and often preferred to use the ordinary
/// [`Mutex`][std] from the standard library in asynchronous code.
///
/// The feature that the async mutex offers over the blocking mutex is the
/// ability to keep it locked across an `.await` point. This makes the async
/// mutex more expensive than the blocking mutex, so the blocking mutex should
/// be preferred in the cases where it can be used. The primary use case for the
/// async mutex is to provide shared mutable access to IO resources such as a
/// database connection. If the value behind the mutex is just data, it's
/// usually appropriate to use a blocking mutex such as the one in the standard
/// library or [`parking_lot`].
///
/// Note that, although the compiler will not prevent the std `Mutex` from holding
/// its guard across `.await` points in situations where the task is not movable
/// between threads, this virtually never leads to correct concurrent code in
/// practice as it can easily lead to deadlocks.
///
/// A common pattern is to wrap the `Arc<Mutex<...>>` in a struct that provides
/// non-async methods for performing operations on the data within, and only
/// lock the mutex inside these methods. The [mini-redis] example provides an
/// illustration of this pattern.
///
/// Additionally, when you _do_ want shared access to an IO resource, it is
/// often better to spawn a task to manage the IO resource, and to use message
/// passing to communicate with that task.
///
/// [std]: std::sync::Mutex
/// [`parking_lot`]: https://docs.rs/parking_lot
/// [mini-redis]: https://github.com/tokio-rs/mini-redis/blob/master/src/db.rs
///
/// # Examples:
///
/// ```rust,no_run
/// use tokio::sync::Mutex;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() {
///     let data1 = Arc::new(Mutex::new(0));
///     let data2 = Arc::clone(&data1);
///
///     tokio::spawn(async move {
///         let mut lock = data2.lock().await;
///         *lock += 1;
///     });
///
///     let mut lock = data1.lock().await;
///     *lock += 1;
/// }
/// ```
///
///
/// ```rust,no_run
/// use tokio::sync::Mutex;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() {
///     let count = Arc::new(Mutex::new(0));
///
///     for i in 0..5 {
///         let my_count = Arc::clone(&count);
///         tokio::spawn(async move {
///             for j in 0..10 {
///                 let mut lock = my_count.lock().await;
///                 *lock += 1;
///                 println!("{} {} {}", i, j, lock);
///             }
///         });
///     }
///
///     loop {
///         if *count.lock().await >= 50 {
///             break;
///         }
///     }
///     println!("Count hit 50.");
/// }
/// ```
/// There are a few things of note here to pay attention to in this example.
/// 1. The mutex is wrapped in an [`Arc`] to allow it to be shared across
///    threads.
/// 2. Each spawned task obtains a lock and releases it on every iteration.
/// 3. Mutation of the data protected by the Mutex is done by de-referencing
///    the obtained lock as seen on lines 13 and 20.
///
/// Tokio's Mutex works in a simple FIFO (first in, first out) style where all
/// calls to [`lock`] complete in the order they were performed. In that way the
/// Mutex is "fair" and predictable in how it distributes the locks to inner
/// data. Locks are released and reacquired after every iteration, so basically,
/// each thread goes to the back of the line after it increments the value once.
/// Note that there's some unpredictability to the timing between when the
/// threads are started, but once they are going they alternate predictably.
/// Finally, since there is only a single valid lock at any given time, there is
/// no possibility of a race condition when mutating the inner value.
///
/// Note that in contrast to [`std::sync::Mutex`], this implementation does not
/// poison the mutex when a thread holding the [`MutexGuard`] panics. In such a
/// case, the mutex will be unlocked. If the panic is caught, this might leave
/// the data protected by the mutex in an inconsistent state.
///
/// [`Mutex`]: struct@Mutex
/// [`MutexGuard`]: struct@MutexGuard
/// [`Arc`]: struct@std::sync::Arc
/// [`std::sync::Mutex`]: struct@std::sync::Mutex
/// [`Send`]: trait@std::marker::Send
/// [`lock`]: method@Mutex::lock
pub struct Mutex<T: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    s: semaphore::Semaphore,
    c: UnsafeCell<T>,
}

/// A handle to a held `Mutex`. The guard can be held across any `.await` point
/// as it is [`Send`].
///
/// As long as you have this guard, you have exclusive access to the underlying
/// `T`. The guard internally borrows the `Mutex`, so the mutex will not be
/// dropped while a guard exists.
///
/// The lock is automatically released whenever the guard is dropped, at which
/// point `lock` will succeed yet again.
#[clippy::has_significant_drop]
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, T: ?Sized> {
    // When changing the fields in this struct, make sure to update the
    // `skip_drop` method.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    lock: &'a Mutex<T>,
}

/// An owned handle to a held `Mutex`.
///
/// This guard is only available from a `Mutex` that is wrapped in an [`Arc`]. It
/// is identical to `MutexGuard`, except that rather than borrowing the `Mutex`,
/// it clones the `Arc`, incrementing the reference count. This means that
/// unlike `MutexGuard`, it will have the `'static` lifetime.
///
/// As long as you have this guard, you have exclusive access to the underlying
/// `T`. The guard internally keeps a reference-counted pointer to the original
/// `Mutex`, so even if the lock goes away, the guard remains valid.
///
/// The lock is automatically released whenever the guard is dropped, at which
/// point `lock` will succeed yet again.
///
/// [`Arc`]: std::sync::Arc
#[clippy::has_significant_drop]
pub struct OwnedMutexGuard<T: ?Sized> {
    // When changing the fields in this struct, make sure to update the
    // `skip_drop` method.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    lock: Arc<Mutex<T>>,
}

/// A handle to a held `Mutex` that has had a function applied to it via [`MutexGuard::map`].
///
/// This can be used to hold a subfield of the protected data.
///
/// [`MutexGuard::map`]: method@MutexGuard::map
#[clippy::has_significant_drop]
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MappedMutexGuard<'a, T: ?Sized> {
    // When changing the fields in this struct, make sure to update the
    // `skip_drop` method.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    s: &'a semaphore::Semaphore,
    data: *mut T,
    // Needed to tell the borrow checker that we are holding a `&mut T`
    marker: PhantomData<&'a mut T>,
}

/// A owned handle to a held `Mutex` that has had a function applied to it via
/// [`OwnedMutexGuard::map`].
///
/// This can be used to hold a subfield of the protected data.
///
/// [`OwnedMutexGuard::map`]: method@OwnedMutexGuard::map
#[clippy::has_significant_drop]
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct OwnedMappedMutexGuard<T: ?Sized, U: ?Sized = T> {
    // When changing the fields in this struct, make sure to update the
    // `skip_drop` method.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    data: *mut U,
    lock: Arc<Mutex<T>>,
}

/// A helper type used when taking apart a `MutexGuard` without running its
/// Drop implementation.
#[allow(dead_code)] // Unused fields are still used in Drop.
struct MutexGuardInner<'a, T: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    lock: &'a Mutex<T>,
}

/// A helper type used when taking apart a `OwnedMutexGuard` without running
/// its Drop implementation.
struct OwnedMutexGuardInner<T: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    lock: Arc<Mutex<T>>,
}

/// A helper type used when taking apart a `MappedMutexGuard` without running
/// its Drop implementation.
#[allow(dead_code)] // Unused fields are still used in Drop.
struct MappedMutexGuardInner<'a, T: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    s: &'a semaphore::Semaphore,
    data: *mut T,
}

/// A helper type used when taking apart a `OwnedMappedMutexGuard` without running
/// its Drop implementation.
#[allow(dead_code)] // Unused fields are still used in Drop.
struct OwnedMappedMutexGuardInner<T: ?Sized, U: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    data: *mut U,
    lock: Arc<Mutex<T>>,
}

// As long as T: Send, it's fine to send and share Mutex<T> between threads.
// If T was not Send, sending and sharing a Mutex<T> would be bad, since you can
// access T through Mutex<T>.
unsafe impl<T> Send for Mutex<T> where T: ?Sized + Send {}
unsafe impl<T> Sync for Mutex<T> where T: ?Sized + Send {}
unsafe impl<T> Sync for MutexGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Sync for OwnedMutexGuard<T> where T: ?Sized + Send + Sync {}
unsafe impl<'a, T> Sync for MappedMutexGuard<'a, T> where T: ?Sized + Sync + 'a {}
unsafe impl<'a, T> Send for MappedMutexGuard<'a, T> where T: ?Sized + Send + 'a {}

unsafe impl<T, U> Sync for OwnedMappedMutexGuard<T, U>
where
    T: ?Sized + Send + Sync,
    U: ?Sized + Send + Sync,
{
}
unsafe impl<T, U> Send for OwnedMappedMutexGuard<T, U>
where
    T: ?Sized + Send,
    U: ?Sized + Send,
{
}

/// Error returned from the [`Mutex::try_lock`], [`RwLock::try_read`] and
/// [`RwLock::try_write`] functions.
///
/// `Mutex::try_lock` operation will only fail if the mutex is already locked.
///
/// `RwLock::try_read` operation will only fail if the lock is currently held
/// by an exclusive writer.
///
/// `RwLock::try_write` operation will only fail if the lock is currently held
/// by any reader or by an exclusive writer.
///
/// [`Mutex::try_lock`]: Mutex::try_lock
/// [`RwLock::try_read`]: fn@super::RwLock::try_read
/// [`RwLock::try_write`]: fn@super::RwLock::try_write
#[derive(Debug)]
pub struct TryLockError(pub(super) ());

impl fmt::Display for TryLockError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "operation would block")
    }
}

impl Error for TryLockError {}

#[test]
#[cfg(not(loom))]
fn bounds() {
    fn check_send<T: Send>() {}
    fn check_unpin<T: Unpin>() {}
    // This has to take a value, since the async fn's return type is unnameable.
    fn check_send_sync_val<T: Send + Sync>(_t: T) {}
    fn check_send_sync<T: Send + Sync>() {}
    fn check_static<T: 'static>() {}
    fn check_static_val<T: 'static>(_t: T) {}

    check_send::<MutexGuard<'_, u32>>();
    check_send::<OwnedMutexGuard<u32>>();
    check_unpin::<Mutex<u32>>();
    check_send_sync::<Mutex<u32>>();
    check_static::<OwnedMutexGuard<u32>>();

    let mutex = Mutex::new(1);
    check_send_sync_val(mutex.lock());
    let arc_mutex = Arc::new(Mutex::new(1));
    check_send_sync_val(arc_mutex.clone().lock_owned());
    check_static_val(arc_mutex.lock_owned());
}

impl<T: ?Sized> Mutex<T> {
    /// Creates a new lock in an unlocked state ready for use.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    ///
    /// let lock = Mutex::new(5);
    /// ```
    #[track_caller]
    pub fn new(t: T) -> Self
    where
        T: Sized,
    {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = {
            let location = std::panic::Location::caller();

            tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "Mutex",
                kind = "Sync",
                loc.file = location.file(),
                loc.line = location.line(),
                loc.col = location.column(),
            )
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let s = resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = false,
            );
            semaphore::Semaphore::new(1)
        });

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        let s = semaphore::Semaphore::new(1);

        Self {
            c: UnsafeCell::new(t),
            s,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span,
        }
    }

    /// Creates a new lock in an unlocked state ready for use.
    ///
    /// When using the `tracing` [unstable feature], a `Mutex` created with
    /// `const_new` will not be instrumented. As such, it will not be visible
    /// in [`tokio-console`]. Instead, [`Mutex::new`] should be used to create
    /// an instrumented object if that is needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    ///
    /// static LOCK: Mutex<i32> = Mutex::const_new(5);
    /// ```
    ///
    /// [`tokio-console`]: https://github.com/tokio-rs/console
    /// [unstable feature]: crate#unstable-features
    #[cfg(not(all(loom, test)))]
    pub const fn const_new(t: T) -> Self
    where
        T: Sized,
    {
        Self {
            c: UnsafeCell::new(t),
            s: semaphore::Semaphore::const_new(1),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Locks this mutex, causing the current task to yield until the lock has
    /// been acquired.  When the lock has been acquired, function returns a
    /// [`MutexGuard`].
    ///
    /// If the mutex is available to be acquired immediately, then this call
    /// will typically not yield to the runtime. However, this is not guaranteed
    /// under all circumstances.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute locks in the order they
    /// were requested. Cancelling a call to `lock` makes you lose your place in
    /// the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mutex = Mutex::new(1);
    ///
    ///     let mut n = mutex.lock().await;
    ///     *n = 2;
    /// }
    /// ```
    pub async fn lock(&self) -> MutexGuard<'_, T> {
        let acquire_fut = async {
            self.acquire().await;

            MutexGuard {
                lock: self,
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: self.resource_span.clone(),
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let acquire_fut = trace::async_op(
            move || acquire_fut,
            self.resource_span.clone(),
            "Mutex::lock",
            "poll",
            false,
        );

        #[allow(clippy::let_and_return)] // this lint triggers when disabling tracing
        let guard = acquire_fut.await;

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = true,
            );
        });

        guard
    }

    /// Blockingly locks this `Mutex`. When the lock has been acquired, function returns a
    /// [`MutexGuard`].
    ///
    /// This method is intended for use cases where you
    /// need to use this mutex in asynchronous code as well as in synchronous code.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    ///
    ///   - If you find yourself in an asynchronous execution context and needing
    ///     to call some (synchronous) function which performs one of these
    ///     `blocking_` operations, then consider wrapping that call inside
    ///     [`spawn_blocking()`][crate::runtime::Handle::spawn_blocking]
    ///     (or [`block_in_place()`][crate::task::block_in_place]).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mutex =  Arc::new(Mutex::new(1));
    ///     let lock = mutex.lock().await;
    ///
    ///     let mutex1 = Arc::clone(&mutex);
    ///     let blocking_task = tokio::task::spawn_blocking(move || {
    ///         // This shall block until the `lock` is released.
    ///         let mut n = mutex1.blocking_lock();
    ///         *n = 2;
    ///     });
    ///
    ///     assert_eq!(*lock, 1);
    ///     // Release the lock.
    ///     drop(lock);
    ///
    ///     // Await the completion of the blocking task.
    ///     blocking_task.await.unwrap();
    ///
    ///     // Assert uncontended.
    ///     let n = mutex.try_lock().unwrap();
    ///     assert_eq!(*n, 2);
    /// }
    ///
    /// ```
    #[track_caller]
    #[cfg(feature = "sync")]
    #[cfg_attr(docsrs, doc(alias = "lock_blocking"))]
    pub fn blocking_lock(&self) -> MutexGuard<'_, T> {
        crate::future::block_on(self.lock())
    }

    /// Blockingly locks this `Mutex`. When the lock has been acquired, function returns an
    /// [`OwnedMutexGuard`].
    ///
    /// This method is identical to [`Mutex::blocking_lock`], except that the returned
    /// guard references the `Mutex` with an [`Arc`] rather than by borrowing
    /// it. Therefore, the `Mutex` must be wrapped in an `Arc` to call this
    /// method, and the guard will live for the `'static` lifetime, as it keeps
    /// the `Mutex` alive by holding an `Arc`.
    ///
    /// # Panics
    ///
    /// This function panics if called within an asynchronous execution context.
    ///
    ///   - If you find yourself in an asynchronous execution context and needing
    ///     to call some (synchronous) function which performs one of these
    ///     `blocking_` operations, then consider wrapping that call inside
    ///     [`spawn_blocking()`][crate::runtime::Handle::spawn_blocking]
    ///     (or [`block_in_place()`][crate::task::block_in_place]).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mutex =  Arc::new(Mutex::new(1));
    ///     let lock = mutex.lock().await;
    ///
    ///     let mutex1 = Arc::clone(&mutex);
    ///     let blocking_task = tokio::task::spawn_blocking(move || {
    ///         // This shall block until the `lock` is released.
    ///         let mut n = mutex1.blocking_lock_owned();
    ///         *n = 2;
    ///     });
    ///
    ///     assert_eq!(*lock, 1);
    ///     // Release the lock.
    ///     drop(lock);
    ///
    ///     // Await the completion of the blocking task.
    ///     blocking_task.await.unwrap();
    ///
    ///     // Assert uncontended.
    ///     let n = mutex.try_lock().unwrap();
    ///     assert_eq!(*n, 2);
    /// }
    ///
    /// ```
    #[track_caller]
    #[cfg(feature = "sync")]
    pub fn blocking_lock_owned(self: Arc<Self>) -> OwnedMutexGuard<T> {
        crate::future::block_on(self.lock_owned())
    }

    /// Locks this mutex, causing the current task to yield until the lock has
    /// been acquired. When the lock has been acquired, this returns an
    /// [`OwnedMutexGuard`].
    ///
    /// If the mutex is available to be acquired immediately, then this call
    /// will typically not yield to the runtime. However, this is not guaranteed
    /// under all circumstances.
    ///
    /// This method is identical to [`Mutex::lock`], except that the returned
    /// guard references the `Mutex` with an [`Arc`] rather than by borrowing
    /// it. Therefore, the `Mutex` must be wrapped in an `Arc` to call this
    /// method, and the guard will live for the `'static` lifetime, as it keeps
    /// the `Mutex` alive by holding an `Arc`.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute locks in the order they
    /// were requested. Cancelling a call to `lock_owned` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    /// use std::sync::Arc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mutex = Arc::new(Mutex::new(1));
    ///
    ///     let mut n = mutex.clone().lock_owned().await;
    ///     *n = 2;
    /// }
    /// ```
    ///
    /// [`Arc`]: std::sync::Arc
    pub async fn lock_owned(self: Arc<Self>) -> OwnedMutexGuard<T> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = self.resource_span.clone();

        let acquire_fut = async {
            self.acquire().await;

            OwnedMutexGuard {
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: self.resource_span.clone(),
                lock: self,
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let acquire_fut = trace::async_op(
            move || acquire_fut,
            resource_span,
            "Mutex::lock_owned",
            "poll",
            false,
        );

        #[allow(clippy::let_and_return)] // this lint triggers when disabling tracing
        let guard = acquire_fut.await;

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = true,
            );
        });

        guard
    }

    async fn acquire(&self) {
        crate::trace::async_trace_leaf().await;

        self.s.acquire(1).await.unwrap_or_else(|_| {
            // The semaphore was closed. but, we never explicitly close it, and
            // we own it exclusively, which means that this can never happen.
            unreachable!()
        });
    }

    /// Attempts to acquire the lock, and returns [`TryLockError`] if the
    /// lock is currently held somewhere else.
    ///
    /// [`TryLockError`]: TryLockError
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    /// # async fn dox() -> Result<(), tokio::sync::TryLockError> {
    ///
    /// let mutex = Mutex::new(1);
    ///
    /// let n = mutex.try_lock()?;
    /// assert_eq!(*n, 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, TryLockError> {
        match self.s.try_acquire(1) {
            Ok(()) => {
                let guard = MutexGuard {
                    lock: self,
                    #[cfg(all(tokio_unstable, feature = "tracing"))]
                    resource_span: self.resource_span.clone(),
                };

                #[cfg(all(tokio_unstable, feature = "tracing"))]
                self.resource_span.in_scope(|| {
                    tracing::trace!(
                        target: "runtime::resource::state_update",
                        locked = true,
                    );
                });

                Ok(guard)
            }
            Err(_) => Err(TryLockError(())),
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `Mutex` mutably, no actual locking needs to
    /// take place -- the mutable borrow statically guarantees no locks exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    ///
    /// fn main() {
    ///     let mut mutex = Mutex::new(1);
    ///
    ///     let n = mutex.get_mut();
    ///     *n = 2;
    /// }
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        unsafe {
            // Safety: This is https://github.com/rust-lang/rust/pull/76936
            &mut *self.c.get()
        }
    }

    /// Attempts to acquire the lock, and returns [`TryLockError`] if the lock
    /// is currently held somewhere else.
    ///
    /// This method is identical to [`Mutex::try_lock`], except that the
    /// returned  guard references the `Mutex` with an [`Arc`] rather than by
    /// borrowing it. Therefore, the `Mutex` must be wrapped in an `Arc` to call
    /// this method, and the guard will live for the `'static` lifetime, as it
    /// keeps the `Mutex` alive by holding an `Arc`.
    ///
    /// [`TryLockError`]: TryLockError
    /// [`Arc`]: std::sync::Arc
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    /// use std::sync::Arc;
    /// # async fn dox() -> Result<(), tokio::sync::TryLockError> {
    ///
    /// let mutex = Arc::new(Mutex::new(1));
    ///
    /// let n = mutex.clone().try_lock_owned()?;
    /// assert_eq!(*n, 1);
    /// # Ok(())
    /// # }
    pub fn try_lock_owned(self: Arc<Self>) -> Result<OwnedMutexGuard<T>, TryLockError> {
        match self.s.try_acquire(1) {
            Ok(()) => {
                let guard = OwnedMutexGuard {
                    #[cfg(all(tokio_unstable, feature = "tracing"))]
                    resource_span: self.resource_span.clone(),
                    lock: self,
                };

                #[cfg(all(tokio_unstable, feature = "tracing"))]
                guard.resource_span.in_scope(|| {
                    tracing::trace!(
                        target: "runtime::resource::state_update",
                        locked = true,
                    );
                });

                Ok(guard)
            }
            Err(_) => Err(TryLockError(())),
        }
    }

    /// Consumes the mutex, returning the underlying data.
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Mutex;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mutex = Mutex::new(1);
    ///
    ///     let n = mutex.into_inner();
    ///     assert_eq!(n, 1);
    /// }
    /// ```
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.c.into_inner()
    }
}

impl<T> From<T> for Mutex<T> {
    fn from(s: T) -> Self {
        Self::new(s)
    }
}

impl<T> Default for Mutex<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> std::fmt::Debug for Mutex<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Mutex");
        match self.try_lock() {
            Ok(inner) => d.field("data", &&*inner),
            Err(_) => d.field("data", &format_args!("<locked>")),
        };
        d.finish()
    }
}

// === impl MutexGuard ===

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    fn skip_drop(self) -> MutexGuardInner<'a, T> {
        let me = mem::ManuallyDrop::new(self);
        // SAFETY: This duplicates the `resource_span` and then forgets the
        // original. In the end, we have not duplicated or forgotten any values.
        MutexGuardInner {
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: unsafe { std::ptr::read(&me.resource_span) },
            lock: me.lock,
        }
    }

    /// Makes a new [`MappedMutexGuard`] for a component of the locked data.
    ///
    /// This operation cannot fail as the [`MutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `MutexGuard::map(...)`. A method
    /// would interfere with methods of the same name on the contents of the locked data.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{Mutex, MutexGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let foo = Mutex::new(Foo(1));
    ///
    /// {
    ///     let mut mapped = MutexGuard::map(foo.lock().await, |f| &mut f.0);
    ///     *mapped = 2;
    /// }
    ///
    /// assert_eq!(Foo(2), *foo.lock().await);
    /// # }
    /// ```
    ///
    /// [`MutexGuard`]: struct@MutexGuard
    /// [`MappedMutexGuard`]: struct@MappedMutexGuard
    #[inline]
    pub fn map<U, F>(mut this: Self, f: F) -> MappedMutexGuard<'a, U>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> &mut U,
    {
        let data = f(&mut *this) as *mut U;
        let inner = this.skip_drop();
        MappedMutexGuard {
            s: &inner.lock.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        }
    }

    /// Attempts to make a new [`MappedMutexGuard`] for a component of the locked data. The
    /// original guard is returned if the closure returns `None`.
    ///
    /// This operation cannot fail as the [`MutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `MutexGuard::try_map(...)`. A
    /// method would interfere with methods of the same name on the contents of the locked data.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{Mutex, MutexGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let foo = Mutex::new(Foo(1));
    ///
    /// {
    ///     let mut mapped = MutexGuard::try_map(foo.lock().await, |f| Some(&mut f.0))
    ///         .expect("should not fail");
    ///     *mapped = 2;
    /// }
    ///
    /// assert_eq!(Foo(2), *foo.lock().await);
    /// # }
    /// ```
    ///
    /// [`MutexGuard`]: struct@MutexGuard
    /// [`MappedMutexGuard`]: struct@MappedMutexGuard
    #[inline]
    pub fn try_map<U, F>(mut this: Self, f: F) -> Result<MappedMutexGuard<'a, U>, Self>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let data = match f(&mut *this) {
            Some(data) => data as *mut U,
            None => return Err(this),
        };
        let inner = this.skip_drop();
        Ok(MappedMutexGuard {
            s: &inner.lock.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        })
    }

    /// Returns a reference to the original `Mutex`.
    ///
    /// ```
    /// use tokio::sync::{Mutex, MutexGuard};
    ///
    /// async fn unlock_and_relock<'l>(guard: MutexGuard<'l, u32>) -> MutexGuard<'l, u32> {
    ///     println!("1. contains: {:?}", *guard);
    ///     let mutex = MutexGuard::mutex(&guard);
    ///     drop(guard);
    ///     let guard = mutex.lock().await;
    ///     println!("2. contains: {:?}", *guard);
    ///     guard
    /// }
    /// #
    /// # #[tokio::main]
    /// # async fn main() {
    /// #     let mutex = Mutex::new(0u32);
    /// #     let guard = mutex.lock().await;
    /// #     let _guard = unlock_and_relock(guard).await;
    /// # }
    /// ```
    #[inline]
    pub fn mutex(this: &Self) -> &'a Mutex<T> {
        this.lock
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.s.release(1);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = false,
            );
        });
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.c.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.c.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

// === impl OwnedMutexGuard ===

impl<T: ?Sized> OwnedMutexGuard<T> {
    fn skip_drop(self) -> OwnedMutexGuardInner<T> {
        let me = mem::ManuallyDrop::new(self);
        // SAFETY: This duplicates the values in every field of the guard, then
        // forgets the originals, so in the end no value is duplicated.
        unsafe {
            OwnedMutexGuardInner {
                lock: ptr::read(&me.lock),
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: ptr::read(&me.resource_span),
            }
        }
    }

    /// Makes a new [`OwnedMappedMutexGuard`] for a component of the locked data.
    ///
    /// This operation cannot fail as the [`OwnedMutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `OwnedMutexGuard::map(...)`. A method
    /// would interfere with methods of the same name on the contents of the locked data.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{Mutex, OwnedMutexGuard};
    /// use std::sync::Arc;
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let foo = Arc::new(Mutex::new(Foo(1)));
    ///
    /// {
    ///     let mut mapped = OwnedMutexGuard::map(foo.clone().lock_owned().await, |f| &mut f.0);
    ///     *mapped = 2;
    /// }
    ///
    /// assert_eq!(Foo(2), *foo.lock().await);
    /// # }
    /// ```
    ///
    /// [`OwnedMutexGuard`]: struct@OwnedMutexGuard
    /// [`OwnedMappedMutexGuard`]: struct@OwnedMappedMutexGuard
    #[inline]
    pub fn map<U, F>(mut this: Self, f: F) -> OwnedMappedMutexGuard<T, U>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> &mut U,
    {
        let data = f(&mut *this) as *mut U;
        let inner = this.skip_drop();
        OwnedMappedMutexGuard {
            data,
            lock: inner.lock,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        }
    }

    /// Attempts to make a new [`OwnedMappedMutexGuard`] for a component of the locked data. The
    /// original guard is returned if the closure returns `None`.
    ///
    /// This operation cannot fail as the [`OwnedMutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `OwnedMutexGuard::try_map(...)`. A
    /// method would interfere with methods of the same name on the contents of the locked data.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{Mutex, OwnedMutexGuard};
    /// use std::sync::Arc;
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let foo = Arc::new(Mutex::new(Foo(1)));
    ///
    /// {
    ///     let mut mapped = OwnedMutexGuard::try_map(foo.clone().lock_owned().await, |f| Some(&mut f.0))
    ///         .expect("should not fail");
    ///     *mapped = 2;
    /// }
    ///
    /// assert_eq!(Foo(2), *foo.lock().await);
    /// # }
    /// ```
    ///
    /// [`OwnedMutexGuard`]: struct@OwnedMutexGuard
    /// [`OwnedMappedMutexGuard`]: struct@OwnedMappedMutexGuard
    #[inline]
    pub fn try_map<U, F>(mut this: Self, f: F) -> Result<OwnedMappedMutexGuard<T, U>, Self>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let data = match f(&mut *this) {
            Some(data) => data as *mut U,
            None => return Err(this),
        };
        let inner = this.skip_drop();
        Ok(OwnedMappedMutexGuard {
            data,
            lock: inner.lock,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        })
    }

    /// Returns a reference to the original `Arc<Mutex>`.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::{Mutex, OwnedMutexGuard};
    ///
    /// async fn unlock_and_relock(guard: OwnedMutexGuard<u32>) -> OwnedMutexGuard<u32> {
    ///     println!("1. contains: {:?}", *guard);
    ///     let mutex: Arc<Mutex<u32>> = OwnedMutexGuard::mutex(&guard).clone();
    ///     drop(guard);
    ///     let guard = mutex.lock_owned().await;
    ///     println!("2. contains: {:?}", *guard);
    ///     guard
    /// }
    /// #
    /// # #[tokio::main]
    /// # async fn main() {
    /// #     let mutex = Arc::new(Mutex::new(0u32));
    /// #     let guard = mutex.lock_owned().await;
    /// #     unlock_and_relock(guard).await;
    /// # }
    /// ```
    #[inline]
    pub fn mutex(this: &Self) -> &Arc<Mutex<T>> {
        &this.lock
    }
}

impl<T: ?Sized> Drop for OwnedMutexGuard<T> {
    fn drop(&mut self) {
        self.lock.s.release(1);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = false,
            );
        });
    }
}

impl<T: ?Sized> Deref for OwnedMutexGuard<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.c.get() }
    }
}

impl<T: ?Sized> DerefMut for OwnedMutexGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.c.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for OwnedMutexGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for OwnedMutexGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

// === impl MappedMutexGuard ===

impl<'a, T: ?Sized> MappedMutexGuard<'a, T> {
    fn skip_drop(self) -> MappedMutexGuardInner<'a, T> {
        let me = mem::ManuallyDrop::new(self);
        MappedMutexGuardInner {
            s: me.s,
            data: me.data,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: unsafe { std::ptr::read(&me.resource_span) },
        }
    }

    /// Makes a new [`MappedMutexGuard`] for a component of the locked data.
    ///
    /// This operation cannot fail as the [`MappedMutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `MappedMutexGuard::map(...)`. A
    /// method would interfere with methods of the same name on the contents of the locked data.
    ///
    /// [`MappedMutexGuard`]: struct@MappedMutexGuard
    #[inline]
    pub fn map<U, F>(mut this: Self, f: F) -> MappedMutexGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let data = f(&mut *this) as *mut U;
        let inner = this.skip_drop();
        MappedMutexGuard {
            s: inner.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        }
    }

    /// Attempts to make a new [`MappedMutexGuard`] for a component of the locked data. The
    /// original guard is returned if the closure returns `None`.
    ///
    /// This operation cannot fail as the [`MappedMutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `MappedMutexGuard::try_map(...)`. A
    /// method would interfere with methods of the same name on the contents of the locked data.
    ///
    /// [`MappedMutexGuard`]: struct@MappedMutexGuard
    #[inline]
    pub fn try_map<U, F>(mut this: Self, f: F) -> Result<MappedMutexGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let data = match f(&mut *this) {
            Some(data) => data as *mut U,
            None => return Err(this),
        };
        let inner = this.skip_drop();
        Ok(MappedMutexGuard {
            s: inner.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        })
    }
}

impl<'a, T: ?Sized> Drop for MappedMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.s.release(1);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = false,
            );
        });
    }
}

impl<'a, T: ?Sized> Deref for MappedMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}

impl<'a, T: ?Sized> DerefMut for MappedMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for MappedMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for MappedMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

// === impl OwnedMappedMutexGuard ===

impl<T: ?Sized, U: ?Sized> OwnedMappedMutexGuard<T, U> {
    fn skip_drop(self) -> OwnedMappedMutexGuardInner<T, U> {
        let me = mem::ManuallyDrop::new(self);
        // SAFETY: This duplicates the values in every field of the guard, then
        // forgets the originals, so in the end no value is duplicated.
        unsafe {
            OwnedMappedMutexGuardInner {
                data: me.data,
                lock: ptr::read(&me.lock),
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: ptr::read(&me.resource_span),
            }
        }
    }

    /// Makes a new [`OwnedMappedMutexGuard`] for a component of the locked data.
    ///
    /// This operation cannot fail as the [`OwnedMappedMutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `OwnedMappedMutexGuard::map(...)`. A method
    /// would interfere with methods of the same name on the contents of the locked data.
    ///
    /// [`OwnedMappedMutexGuard`]: struct@OwnedMappedMutexGuard
    #[inline]
    pub fn map<S, F>(mut this: Self, f: F) -> OwnedMappedMutexGuard<T, S>
    where
        F: FnOnce(&mut U) -> &mut S,
    {
        let data = f(&mut *this) as *mut S;
        let inner = this.skip_drop();
        OwnedMappedMutexGuard {
            data,
            lock: inner.lock,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        }
    }

    /// Attempts to make a new [`OwnedMappedMutexGuard`] for a component of the locked data. The
    /// original guard is returned if the closure returns `None`.
    ///
    /// This operation cannot fail as the [`OwnedMutexGuard`] passed in already locked the mutex.
    ///
    /// This is an associated function that needs to be used as `OwnedMutexGuard::try_map(...)`. A
    /// method would interfere with methods of the same name on the contents of the locked data.
    ///
    /// [`OwnedMutexGuard`]: struct@OwnedMutexGuard
    /// [`OwnedMappedMutexGuard`]: struct@OwnedMappedMutexGuard
    #[inline]
    pub fn try_map<S, F>(mut this: Self, f: F) -> Result<OwnedMappedMutexGuard<T, S>, Self>
    where
        F: FnOnce(&mut U) -> Option<&mut S>,
    {
        let data = match f(&mut *this) {
            Some(data) => data as *mut S,
            None => return Err(this),
        };
        let inner = this.skip_drop();
        Ok(OwnedMappedMutexGuard {
            data,
            lock: inner.lock,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: inner.resource_span,
        })
    }
}

impl<T: ?Sized, U: ?Sized> Drop for OwnedMappedMutexGuard<T, U> {
    fn drop(&mut self) {
        self.lock.s.release(1);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
                target: "runtime::resource::state_update",
                locked = false,
            );
        });
    }
}

impl<T: ?Sized, U: ?Sized> Deref for OwnedMappedMutexGuard<T, U> {
    type Target = U;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized, U: ?Sized> DerefMut for OwnedMappedMutexGuard<T, U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data }
    }
}

impl<T: ?Sized, U: ?Sized + fmt::Debug> fmt::Debug for OwnedMappedMutexGuard<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized, U: ?Sized + fmt::Display> fmt::Display for OwnedMappedMutexGuard<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
