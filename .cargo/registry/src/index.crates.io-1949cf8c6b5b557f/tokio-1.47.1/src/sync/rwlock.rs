use crate::sync::batch_semaphore::{Semaphore, TryAcquireError};
use crate::sync::mutex::TryLockError;
#[cfg(all(tokio_unstable, feature = "tracing"))]
use crate::util::trace;
use std::cell::UnsafeCell;
use std::marker;
use std::marker::PhantomData;
use std::sync::Arc;

pub(crate) mod owned_read_guard;
pub(crate) mod owned_write_guard;
pub(crate) mod owned_write_guard_mapped;
pub(crate) mod read_guard;
pub(crate) mod write_guard;
pub(crate) mod write_guard_mapped;
pub(crate) use owned_read_guard::OwnedRwLockReadGuard;
pub(crate) use owned_write_guard::OwnedRwLockWriteGuard;
pub(crate) use owned_write_guard_mapped::OwnedRwLockMappedWriteGuard;
pub(crate) use read_guard::RwLockReadGuard;
pub(crate) use write_guard::RwLockWriteGuard;
pub(crate) use write_guard_mapped::RwLockMappedWriteGuard;

#[cfg(not(loom))]
const MAX_READS: u32 = u32::MAX >> 3;

#[cfg(loom)]
const MAX_READS: u32 = 10;

/// An asynchronous reader-writer lock.
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. The write portion of this lock typically allows modification
/// of the underlying data (exclusive access) and the read portion of this lock
/// typically allows for read-only access (shared access).
///
/// In comparison, a [`Mutex`] does not distinguish between readers or writers
/// that acquire the lock, therefore causing any tasks waiting for the lock to
/// become available to yield. An `RwLock` will allow any number of readers to
/// acquire the lock as long as a writer is not holding the lock.
///
/// The priority policy of Tokio's read-write lock is _fair_ (or
/// [_write-preferring_]), in order to ensure that readers cannot starve
/// writers. Fairness is ensured using a first-in, first-out queue for the tasks
/// awaiting the lock; if a task that wishes to acquire the write lock is at the
/// head of the queue, read locks will not be given out until the write lock has
/// been released. This is in contrast to the Rust standard library's
/// `std::sync::RwLock`, where the priority policy is dependent on the
/// operating system's implementation.
///
/// The type parameter `T` represents the data that this lock protects. It is
/// required that `T` satisfies [`Send`] to be shared across threads. The RAII guards
/// returned from the locking methods implement [`Deref`](trait@std::ops::Deref)
/// (and [`DerefMut`](trait@std::ops::DerefMut)
/// for the `write` methods) to allow access to the content of the lock.
///
/// # Examples
///
/// ```
/// use tokio::sync::RwLock;
///
/// #[tokio::main]
/// async fn main() {
///     let lock = RwLock::new(5);
///
///     // many reader locks can be held at once
///     {
///         let r1 = lock.read().await;
///         let r2 = lock.read().await;
///         assert_eq!(*r1, 5);
///         assert_eq!(*r2, 5);
///     } // read locks are dropped at this point
///
///     // only one write lock may be held, however
///     {
///         let mut w = lock.write().await;
///         *w += 1;
///         assert_eq!(*w, 6);
///     } // write lock is dropped here
/// }
/// ```
///
/// [`Mutex`]: struct@super::Mutex
/// [`RwLock`]: struct@RwLock
/// [`RwLockReadGuard`]: struct@RwLockReadGuard
/// [`RwLockWriteGuard`]: struct@RwLockWriteGuard
/// [`Send`]: trait@std::marker::Send
/// [_write-preferring_]: https://en.wikipedia.org/wiki/Readers%E2%80%93writer_lock#Priority_policies
pub struct RwLock<T: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,

    // maximum number of concurrent readers
    mr: u32,

    //semaphore to coordinate read and write access to T
    s: Semaphore,

    //inner data T
    c: UnsafeCell<T>,
}

#[test]
#[cfg(not(loom))]
fn bounds() {
    fn check_send<T: Send>() {}
    fn check_sync<T: Sync>() {}
    fn check_unpin<T: Unpin>() {}
    // This has to take a value, since the async fn's return type is unnameable.
    fn check_send_sync_val<T: Send + Sync>(_t: T) {}

    check_send::<RwLock<u32>>();
    check_sync::<RwLock<u32>>();
    check_unpin::<RwLock<u32>>();

    check_send::<RwLockReadGuard<'_, u32>>();
    check_sync::<RwLockReadGuard<'_, u32>>();
    check_unpin::<RwLockReadGuard<'_, u32>>();

    check_send::<OwnedRwLockReadGuard<u32, i32>>();
    check_sync::<OwnedRwLockReadGuard<u32, i32>>();
    check_unpin::<OwnedRwLockReadGuard<u32, i32>>();

    check_send::<RwLockWriteGuard<'_, u32>>();
    check_sync::<RwLockWriteGuard<'_, u32>>();
    check_unpin::<RwLockWriteGuard<'_, u32>>();

    check_send::<RwLockMappedWriteGuard<'_, u32>>();
    check_sync::<RwLockMappedWriteGuard<'_, u32>>();
    check_unpin::<RwLockMappedWriteGuard<'_, u32>>();

    check_send::<OwnedRwLockWriteGuard<u32>>();
    check_sync::<OwnedRwLockWriteGuard<u32>>();
    check_unpin::<OwnedRwLockWriteGuard<u32>>();

    check_send::<OwnedRwLockMappedWriteGuard<u32, i32>>();
    check_sync::<OwnedRwLockMappedWriteGuard<u32, i32>>();
    check_unpin::<OwnedRwLockMappedWriteGuard<u32, i32>>();

    let rwlock = Arc::new(RwLock::new(0));
    check_send_sync_val(rwlock.read());
    check_send_sync_val(Arc::clone(&rwlock).read_owned());
    check_send_sync_val(rwlock.write());
    check_send_sync_val(Arc::clone(&rwlock).write_owned());
}

// As long as T: Send + Sync, it's fine to send and share RwLock<T> between threads.
// If T were not Send, sending and sharing a RwLock<T> would be bad, since you can access T through
// RwLock<T>.
unsafe impl<T> Send for RwLock<T> where T: ?Sized + Send {}
unsafe impl<T> Sync for RwLock<T> where T: ?Sized + Send + Sync {}
// NB: These impls need to be explicit since we're storing a raw pointer.
// Safety: Stores a raw pointer to `T`, so if `T` is `Sync`, the lock guard over
// `T` is `Send`.
unsafe impl<T> Send for RwLockReadGuard<'_, T> where T: ?Sized + Sync {}
unsafe impl<T> Sync for RwLockReadGuard<'_, T> where T: ?Sized + Send + Sync {}
// T is required to be `Send` because an OwnedRwLockReadGuard can be used to drop the value held in
// the RwLock, unlike RwLockReadGuard.
unsafe impl<T, U> Send for OwnedRwLockReadGuard<T, U>
where
    T: ?Sized + Send + Sync,
    U: ?Sized + Sync,
{
}
unsafe impl<T, U> Sync for OwnedRwLockReadGuard<T, U>
where
    T: ?Sized + Send + Sync,
    U: ?Sized + Send + Sync,
{
}
unsafe impl<T> Sync for RwLockWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Sync for OwnedRwLockWriteGuard<T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Sync for RwLockMappedWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T, U> Sync for OwnedRwLockMappedWriteGuard<T, U>
where
    T: ?Sized + Send + Sync,
    U: ?Sized + Send + Sync,
{
}
// Safety: Stores a raw pointer to `T`, so if `T` is `Sync`, the lock guard over
// `T` is `Send` - but since this is also provides mutable access, we need to
// make sure that `T` is `Send` since its value can be sent across thread
// boundaries.
unsafe impl<T> Send for RwLockWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Send for OwnedRwLockWriteGuard<T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Send for RwLockMappedWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T, U> Send for OwnedRwLockMappedWriteGuard<T, U>
where
    T: ?Sized + Send + Sync,
    U: ?Sized + Send + Sync,
{
}

impl<T: ?Sized> RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// ```
    #[track_caller]
    pub fn new(value: T) -> RwLock<T>
    where
        T: Sized,
    {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = {
            let location = std::panic::Location::caller();
            let resource_span = tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "RwLock",
                kind = "Sync",
                loc.file = location.file(),
                loc.line = location.line(),
                loc.col = location.column(),
            );

            resource_span.in_scope(|| {
                tracing::trace!(
                    target: "runtime::resource::state_update",
                    max_readers = MAX_READS,
                );

                tracing::trace!(
                    target: "runtime::resource::state_update",
                    write_locked = false,
                );

                tracing::trace!(
                    target: "runtime::resource::state_update",
                    current_readers = 0,
                );
            });

            resource_span
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let s = resource_span.in_scope(|| Semaphore::new(MAX_READS as usize));

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        let s = Semaphore::new(MAX_READS as usize);

        RwLock {
            mr: MAX_READS,
            c: UnsafeCell::new(value),
            s,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span,
        }
    }

    /// Creates a new instance of an `RwLock<T>` which is unlocked
    /// and allows a maximum of `max_reads` concurrent readers.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// let lock = RwLock::with_max_readers(5, 1024);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `max_reads` is more than `u32::MAX >> 3`.
    #[track_caller]
    pub fn with_max_readers(value: T, max_reads: u32) -> RwLock<T>
    where
        T: Sized,
    {
        assert!(
            max_reads <= MAX_READS,
            "a RwLock may not be created with more than {MAX_READS} readers"
        );

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = {
            let location = std::panic::Location::caller();

            let resource_span = tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "RwLock",
                kind = "Sync",
                loc.file = location.file(),
                loc.line = location.line(),
                loc.col = location.column(),
            );

            resource_span.in_scope(|| {
                tracing::trace!(
                    target: "runtime::resource::state_update",
                    max_readers = max_reads,
                );

                tracing::trace!(
                    target: "runtime::resource::state_update",
                    write_locked = false,
                );

                tracing::trace!(
                    target: "runtime::resource::state_update",
                    current_readers = 0,
                );
            });

            resource_span
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let s = resource_span.in_scope(|| Semaphore::new(max_reads as usize));

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        let s = Semaphore::new(max_reads as usize);

        RwLock {
            mr: max_reads,
            c: UnsafeCell::new(value),
            s,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span,
        }
    }

    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    ///
    /// When using the `tracing` [unstable feature], a `RwLock` created with
    /// `const_new` will not be instrumented. As such, it will not be visible
    /// in [`tokio-console`]. Instead, [`RwLock::new`] should be used to create
    /// an instrumented object if that is needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// static LOCK: RwLock<i32> = RwLock::const_new(5);
    /// ```
    ///
    /// [`tokio-console`]: https://github.com/tokio-rs/console
    /// [unstable feature]: crate#unstable-features
    #[cfg(not(all(loom, test)))]
    pub const fn const_new(value: T) -> RwLock<T>
    where
        T: Sized,
    {
        RwLock {
            mr: MAX_READS,
            c: UnsafeCell::new(value),
            s: Semaphore::const_new(MAX_READS as usize),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Creates a new instance of an `RwLock<T>` which is unlocked
    /// and allows a maximum of `max_reads` concurrent readers.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// static LOCK: RwLock<i32> = RwLock::const_with_max_readers(5, 1024);
    /// ```
    #[cfg(not(all(loom, test)))]
    pub const fn const_with_max_readers(value: T, max_reads: u32) -> RwLock<T>
    where
        T: Sized,
    {
        assert!(max_reads <= MAX_READS);

        RwLock {
            mr: max_reads,
            c: UnsafeCell::new(value),
            s: Semaphore::const_new(max_reads as usize),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Locks this `RwLock` with shared read access, causing the current task
    /// to yield until the lock has been acquired.
    ///
    /// The calling task will yield until there are no writers which hold the
    /// lock. There may be other readers inside the lock when the task resumes.
    ///
    /// Note that under the priority policy of [`RwLock`], read locks are not
    /// granted until prior write locks, to prevent starvation. Therefore
    /// deadlock may occur if a read lock is held by the current task, a write
    /// lock attempt is made, and then a subsequent read lock attempt is made
    /// by the current task.
    ///
    /// Returns an RAII guard which will drop this read access of the `RwLock`
    /// when dropped.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute locks in the order they
    /// were requested. Cancelling a call to `read` makes you lose your place in
    /// the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let lock = Arc::new(RwLock::new(1));
    ///     let c_lock = lock.clone();
    ///
    ///     let n = lock.read().await;
    ///     assert_eq!(*n, 1);
    ///
    ///     tokio::spawn(async move {
    ///         // While main has an active read lock, we acquire one too.
    ///         let r = c_lock.read().await;
    ///         assert_eq!(*r, 1);
    ///     }).await.expect("The spawned task has panicked");
    ///
    ///     // Drop the guard after the spawned task finishes.
    ///     drop(n);
    /// }
    /// ```
    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        let acquire_fut = async {
            self.s.acquire(1).await.unwrap_or_else(|_| {
                // The semaphore was closed. but, we never explicitly close it, and we have a
                // handle to it through the Arc, which means that this can never happen.
                unreachable!()
            });

            RwLockReadGuard {
                s: &self.s,
                data: self.c.get(),
                marker: PhantomData,
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: self.resource_span.clone(),
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let acquire_fut = trace::async_op(
            move || acquire_fut,
            self.resource_span.clone(),
            "RwLock::read",
            "poll",
            false,
        );

        #[allow(clippy::let_and_return)] // this lint triggers when disabling tracing
        let guard = acquire_fut.await;

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        guard
    }

    /// Blockingly locks this `RwLock` with shared read access.
    ///
    /// This method is intended for use cases where you
    /// need to use this rwlock in asynchronous code as well as in synchronous code.
    ///
    /// Returns an RAII guard which will drop the read access of this `RwLock` when dropped.
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
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let rwlock = Arc::new(RwLock::new(1));
    ///     let mut write_lock = rwlock.write().await;
    ///
    ///     let blocking_task = tokio::task::spawn_blocking({
    ///         let rwlock = Arc::clone(&rwlock);
    ///         move || {
    ///             // This shall block until the `write_lock` is released.
    ///             let read_lock = rwlock.blocking_read();
    ///             assert_eq!(*read_lock, 0);
    ///         }
    ///     });
    ///
    ///     *write_lock -= 1;
    ///     drop(write_lock); // release the lock.
    ///
    ///     // Await the completion of the blocking task.
    ///     blocking_task.await.unwrap();
    ///
    ///     // Assert uncontended.
    ///     assert!(rwlock.try_write().is_ok());
    /// }
    /// ```
    #[track_caller]
    #[cfg(feature = "sync")]
    pub fn blocking_read(&self) -> RwLockReadGuard<'_, T> {
        crate::future::block_on(self.read())
    }

    /// Locks this `RwLock` with shared read access, causing the current task
    /// to yield until the lock has been acquired.
    ///
    /// The calling task will yield until there are no writers which hold the
    /// lock. There may be other readers inside the lock when the task resumes.
    ///
    /// This method is identical to [`RwLock::read`], except that the returned
    /// guard references the `RwLock` with an [`Arc`] rather than by borrowing
    /// it. Therefore, the `RwLock` must be wrapped in an `Arc` to call this
    /// method, and the guard will live for the `'static` lifetime, as it keeps
    /// the `RwLock` alive by holding an `Arc`.
    ///
    /// Note that under the priority policy of [`RwLock`], read locks are not
    /// granted until prior write locks, to prevent starvation. Therefore
    /// deadlock may occur if a read lock is held by the current task, a write
    /// lock attempt is made, and then a subsequent read lock attempt is made
    /// by the current task.
    ///
    /// Returns an RAII guard which will drop this read access of the `RwLock`
    /// when dropped.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute locks in the order they
    /// were requested. Cancelling a call to `read_owned` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let lock = Arc::new(RwLock::new(1));
    ///     let c_lock = lock.clone();
    ///
    ///     let n = lock.read_owned().await;
    ///     assert_eq!(*n, 1);
    ///
    ///     tokio::spawn(async move {
    ///         // While main has an active read lock, we acquire one too.
    ///         let r = c_lock.read_owned().await;
    ///         assert_eq!(*r, 1);
    ///     }).await.expect("The spawned task has panicked");
    ///
    ///     // Drop the guard after the spawned task finishes.
    ///     drop(n);
    ///}
    /// ```
    pub async fn read_owned(self: Arc<Self>) -> OwnedRwLockReadGuard<T> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = self.resource_span.clone();

        let acquire_fut = async {
            self.s.acquire(1).await.unwrap_or_else(|_| {
                // The semaphore was closed. but, we never explicitly close it, and we have a
                // handle to it through the Arc, which means that this can never happen.
                unreachable!()
            });

            OwnedRwLockReadGuard {
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: self.resource_span.clone(),
                data: self.c.get(),
                lock: self,
                _p: PhantomData,
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let acquire_fut = trace::async_op(
            move || acquire_fut,
            resource_span,
            "RwLock::read_owned",
            "poll",
            false,
        );

        #[allow(clippy::let_and_return)] // this lint triggers when disabling tracing
        let guard = acquire_fut.await;

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        guard
    }

    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access couldn't be acquired immediately, returns [`TryLockError`].
    /// Otherwise, an RAII guard is returned which will release read access
    /// when dropped.
    ///
    /// [`TryLockError`]: TryLockError
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let lock = Arc::new(RwLock::new(1));
    ///     let c_lock = lock.clone();
    ///
    ///     let v = lock.try_read().unwrap();
    ///     assert_eq!(*v, 1);
    ///
    ///     tokio::spawn(async move {
    ///         // While main has an active read lock, we acquire one too.
    ///         let n = c_lock.read().await;
    ///         assert_eq!(*n, 1);
    ///     }).await.expect("The spawned task has panicked");
    ///
    ///     // Drop the guard when spawned task finishes.
    ///     drop(v);
    /// }
    /// ```
    pub fn try_read(&self) -> Result<RwLockReadGuard<'_, T>, TryLockError> {
        match self.s.try_acquire(1) {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => return Err(TryLockError(())),
            Err(TryAcquireError::Closed) => unreachable!(),
        }

        let guard = RwLockReadGuard {
            s: &self.s,
            data: self.c.get(),
            marker: marker::PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: self.resource_span.clone(),
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        Ok(guard)
    }

    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access couldn't be acquired immediately, returns [`TryLockError`].
    /// Otherwise, an RAII guard is returned which will release read access
    /// when dropped.
    ///
    /// This method is identical to [`RwLock::try_read`], except that the
    /// returned guard references the `RwLock` with an [`Arc`] rather than by
    /// borrowing it. Therefore, the `RwLock` must be wrapped in an `Arc` to
    /// call this method, and the guard will live for the `'static` lifetime,
    /// as it keeps the `RwLock` alive by holding an `Arc`.
    ///
    /// [`TryLockError`]: TryLockError
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let lock = Arc::new(RwLock::new(1));
    ///     let c_lock = lock.clone();
    ///
    ///     let v = lock.try_read_owned().unwrap();
    ///     assert_eq!(*v, 1);
    ///
    ///     tokio::spawn(async move {
    ///         // While main has an active read lock, we acquire one too.
    ///         let n = c_lock.read_owned().await;
    ///         assert_eq!(*n, 1);
    ///     }).await.expect("The spawned task has panicked");
    ///
    ///     // Drop the guard when spawned task finishes.
    ///     drop(v);
    /// }
    /// ```
    pub fn try_read_owned(self: Arc<Self>) -> Result<OwnedRwLockReadGuard<T>, TryLockError> {
        match self.s.try_acquire(1) {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => return Err(TryLockError(())),
            Err(TryAcquireError::Closed) => unreachable!(),
        }

        let guard = OwnedRwLockReadGuard {
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: self.resource_span.clone(),
            data: self.c.get(),
            lock: self,
            _p: PhantomData,
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        Ok(guard)
    }

    /// Locks this `RwLock` with exclusive write access, causing the current
    /// task to yield until the lock has been acquired.
    ///
    /// The calling task will yield while other writers or readers currently
    /// have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock`
    /// when dropped.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute locks in the order they
    /// were requested. Cancelling a call to `write` makes you lose your place
    /// in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///   let lock = RwLock::new(1);
    ///
    ///   let mut n = lock.write().await;
    ///   *n = 2;
    ///}
    /// ```
    pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
        let acquire_fut = async {
            self.s.acquire(self.mr as usize).await.unwrap_or_else(|_| {
                // The semaphore was closed. but, we never explicitly close it, and we have a
                // handle to it through the Arc, which means that this can never happen.
                unreachable!()
            });

            RwLockWriteGuard {
                permits_acquired: self.mr,
                s: &self.s,
                data: self.c.get(),
                marker: marker::PhantomData,
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: self.resource_span.clone(),
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let acquire_fut = trace::async_op(
            move || acquire_fut,
            self.resource_span.clone(),
            "RwLock::write",
            "poll",
            false,
        );

        #[allow(clippy::let_and_return)] // this lint triggers when disabling tracing
        let guard = acquire_fut.await;

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = true,
            write_locked.op = "override",
            )
        });

        guard
    }

    /// Blockingly locks this `RwLock` with exclusive write access.
    ///
    /// This method is intended for use cases where you
    /// need to use this rwlock in asynchronous code as well as in synchronous code.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock` when dropped.
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
    /// use tokio::{sync::RwLock};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let rwlock =  Arc::new(RwLock::new(1));
    ///     let read_lock = rwlock.read().await;
    ///
    ///     let blocking_task = tokio::task::spawn_blocking({
    ///         let rwlock = Arc::clone(&rwlock);
    ///         move || {
    ///             // This shall block until the `read_lock` is released.
    ///             let mut write_lock = rwlock.blocking_write();
    ///             *write_lock = 2;
    ///         }
    ///     });
    ///
    ///     assert_eq!(*read_lock, 1);
    ///     // Release the last outstanding read lock.
    ///     drop(read_lock);
    ///
    ///     // Await the completion of the blocking task.
    ///     blocking_task.await.unwrap();
    ///
    ///     // Assert uncontended.
    ///     let read_lock = rwlock.try_read().unwrap();
    ///     assert_eq!(*read_lock, 2);
    /// }
    /// ```
    #[track_caller]
    #[cfg(feature = "sync")]
    pub fn blocking_write(&self) -> RwLockWriteGuard<'_, T> {
        crate::future::block_on(self.write())
    }

    /// Locks this `RwLock` with exclusive write access, causing the current
    /// task to yield until the lock has been acquired.
    ///
    /// The calling task will yield while other writers or readers currently
    /// have access to the lock.
    ///
    /// This method is identical to [`RwLock::write`], except that the returned
    /// guard references the `RwLock` with an [`Arc`] rather than by borrowing
    /// it. Therefore, the `RwLock` must be wrapped in an `Arc` to call this
    /// method, and the guard will live for the `'static` lifetime, as it keeps
    /// the `RwLock` alive by holding an `Arc`.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock`
    /// when dropped.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute locks in the order they
    /// were requested. Cancelling a call to `write_owned` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///   let lock = Arc::new(RwLock::new(1));
    ///
    ///   let mut n = lock.write_owned().await;
    ///   *n = 2;
    ///}
    /// ```
    pub async fn write_owned(self: Arc<Self>) -> OwnedRwLockWriteGuard<T> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = self.resource_span.clone();

        let acquire_fut = async {
            self.s.acquire(self.mr as usize).await.unwrap_or_else(|_| {
                // The semaphore was closed. but, we never explicitly close it, and we have a
                // handle to it through the Arc, which means that this can never happen.
                unreachable!()
            });

            OwnedRwLockWriteGuard {
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: self.resource_span.clone(),
                permits_acquired: self.mr,
                data: self.c.get(),
                lock: self,
                _p: PhantomData,
            }
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let acquire_fut = trace::async_op(
            move || acquire_fut,
            resource_span,
            "RwLock::write_owned",
            "poll",
            false,
        );

        #[allow(clippy::let_and_return)] // this lint triggers when disabling tracing
        let guard = acquire_fut.await;

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = true,
            write_locked.op = "override",
            )
        });

        guard
    }

    /// Attempts to acquire this `RwLock` with exclusive write access.
    ///
    /// If the access couldn't be acquired immediately, returns [`TryLockError`].
    /// Otherwise, an RAII guard is returned which will release write access
    /// when dropped.
    ///
    /// [`TryLockError`]: TryLockError
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let rw = RwLock::new(1);
    ///
    ///     let v = rw.read().await;
    ///     assert_eq!(*v, 1);
    ///
    ///     assert!(rw.try_write().is_err());
    /// }
    /// ```
    pub fn try_write(&self) -> Result<RwLockWriteGuard<'_, T>, TryLockError> {
        match self.s.try_acquire(self.mr as usize) {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => return Err(TryLockError(())),
            Err(TryAcquireError::Closed) => unreachable!(),
        }

        let guard = RwLockWriteGuard {
            permits_acquired: self.mr,
            s: &self.s,
            data: self.c.get(),
            marker: marker::PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: self.resource_span.clone(),
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = true,
            write_locked.op = "override",
            )
        });

        Ok(guard)
    }

    /// Attempts to acquire this `RwLock` with exclusive write access.
    ///
    /// If the access couldn't be acquired immediately, returns [`TryLockError`].
    /// Otherwise, an RAII guard is returned which will release write access
    /// when dropped.
    ///
    /// This method is identical to [`RwLock::try_write`], except that the
    /// returned guard references the `RwLock` with an [`Arc`] rather than by
    /// borrowing it. Therefore, the `RwLock` must be wrapped in an `Arc` to
    /// call this method, and the guard will live for the `'static` lifetime,
    /// as it keeps the `RwLock` alive by holding an `Arc`.
    ///
    /// [`TryLockError`]: TryLockError
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::RwLock;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let rw = Arc::new(RwLock::new(1));
    ///
    ///     let v = Arc::clone(&rw).read_owned().await;
    ///     assert_eq!(*v, 1);
    ///
    ///     assert!(rw.try_write_owned().is_err());
    /// }
    /// ```
    pub fn try_write_owned(self: Arc<Self>) -> Result<OwnedRwLockWriteGuard<T>, TryLockError> {
        match self.s.try_acquire(self.mr as usize) {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => return Err(TryLockError(())),
            Err(TryAcquireError::Closed) => unreachable!(),
        }

        let guard = OwnedRwLockWriteGuard {
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: self.resource_span.clone(),
            permits_acquired: self.mr,
            data: self.c.get(),
            lock: self,
            _p: PhantomData,
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = true,
            write_locked.op = "override",
            )
        });

        Ok(guard)
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `RwLock` mutably, no actual locking needs to
    /// take place -- the mutable borrow statically guarantees no locks exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::RwLock;
    ///
    /// fn main() {
    ///     let mut lock = RwLock::new(1);
    ///
    ///     let n = lock.get_mut();
    ///     *n = 2;
    /// }
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        unsafe {
            // Safety: This is https://github.com/rust-lang/rust/pull/76936
            &mut *self.c.get()
        }
    }

    /// Consumes the lock, returning the underlying data.
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.c.into_inner()
    }
}

impl<T> From<T> for RwLock<T> {
    fn from(s: T) -> Self {
        Self::new(s)
    }
}

impl<T> Default for RwLock<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> std::fmt::Debug for RwLock<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("RwLock");
        match self.try_read() {
            Ok(inner) => d.field("data", &&*inner),
            Err(_) => d.field("data", &format_args!("<locked>")),
        };
        d.finish()
    }
}
