use super::batch_semaphore as ll; // low level implementation
use super::{AcquireError, TryAcquireError};
#[cfg(all(tokio_unstable, feature = "tracing"))]
use crate::util::trace;
use std::sync::Arc;

/// Counting semaphore performing asynchronous permit acquisition.
///
/// A semaphore maintains a set of permits. Permits are used to synchronize
/// access to a shared resource. A semaphore differs from a mutex in that it
/// can allow more than one concurrent caller to access the shared resource at a
/// time.
///
/// When `acquire` is called and the semaphore has remaining permits, the
/// function immediately returns a permit. However, if no remaining permits are
/// available, `acquire` (asynchronously) waits until an outstanding permit is
/// dropped. At this point, the freed permit is assigned to the caller.
///
/// This `Semaphore` is fair, which means that permits are given out in the order
/// they were requested. This fairness is also applied when `acquire_many` gets
/// involved, so if a call to `acquire_many` at the front of the queue requests
/// more permits than currently available, this can prevent a call to `acquire`
/// from completing, even if the semaphore has enough permits complete the call
/// to `acquire`.
///
/// To use the `Semaphore` in a poll function, you can use the [`PollSemaphore`]
/// utility.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use tokio::sync::{Semaphore, TryAcquireError};
///
/// #[tokio::main]
/// async fn main() {
///     let semaphore = Semaphore::new(3);
///
///     let a_permit = semaphore.acquire().await.unwrap();
///     let two_permits = semaphore.acquire_many(2).await.unwrap();
///
///     assert_eq!(semaphore.available_permits(), 0);
///
///     let permit_attempt = semaphore.try_acquire();
///     assert_eq!(permit_attempt.err(), Some(TryAcquireError::NoPermits));
/// }
/// ```
///
/// ## Limit the number of simultaneously opened files in your program
///
/// Most operating systems have limits on the number of open file
/// handles. Even in systems without explicit limits, resource constraints
/// implicitly set an upper bound on the number of open files. If your
/// program attempts to open a large number of files and exceeds this
/// limit, it will result in an error.
///
/// This example uses a Semaphore with 100 permits. By acquiring a permit from
/// the Semaphore before accessing a file, you ensure that your program opens
/// no more than 100 files at a time. When trying to open the 101st
/// file, the program will wait until a permit becomes available before
/// proceeding to open another file.
/// ```
/// use std::io::Result;
/// use tokio::fs::File;
/// use tokio::sync::Semaphore;
/// use tokio::io::AsyncWriteExt;
///
/// static PERMITS: Semaphore = Semaphore::const_new(100);
///
/// async fn write_to_file(message: &[u8]) -> Result<()> {
///     let _permit = PERMITS.acquire().await.unwrap();
///     let mut buffer = File::create("example.txt").await?;
///     buffer.write_all(message).await?;
///     Ok(()) // Permit goes out of scope here, and is available again for acquisition
/// }
/// ```
///
/// ## Limit the number of outgoing requests being sent at the same time
///
/// In some scenarios, it might be required to limit the number of outgoing
/// requests being sent in parallel. This could be due to limits of a consumed
/// API or the network resources of the system the application is running on.
///
/// This example uses an `Arc<Semaphore>` with 10 permits. Each task spawned is
/// given a reference to the semaphore by cloning the `Arc<Semaphore>`. Before
/// a task sends a request, it must acquire a permit from the semaphore by
/// calling [`Semaphore::acquire`]. This ensures that at most 10 requests are
/// sent in parallel at any given time. After a task has sent a request, it
/// drops the permit to allow other tasks to send requests.
///
/// ```
/// use std::sync::Arc;
/// use tokio::sync::Semaphore;
///
/// #[tokio::main]
/// async fn main() {
///     // Define maximum number of parallel requests.
///     let semaphore = Arc::new(Semaphore::new(10));
///     // Spawn many tasks that will send requests.
///     let mut jhs = Vec::new();
///     for task_id in 0..100 {
///         let semaphore = semaphore.clone();
///         let jh = tokio::spawn(async move {
///             // Acquire permit before sending request.
///             let _permit = semaphore.acquire().await.unwrap();
///             // Send the request.
///             let response = send_request(task_id).await;
///             // Drop the permit after the request has been sent.
///             drop(_permit);
///             // Handle response.
///             // ...
///
///             response
///         });
///         jhs.push(jh);
///     }
///     // Collect responses from tasks.
///     let mut responses = Vec::new();
///     for jh in jhs {
///         let response = jh.await.unwrap();
///         responses.push(response);
///     }
///     // Process responses.
///     // ...
/// }
/// # async fn send_request(task_id: usize) {
/// #     // Send request.
/// # }
/// ```
///
/// ## Limit the number of incoming requests being handled at the same time
///
/// Similar to limiting the number of simultaneously opened files, network handles
/// are a limited resource. Allowing an unbounded amount of requests to be processed
/// could result in a denial-of-service, among many other issues.
///
/// This example uses an `Arc<Semaphore>` instead of a global variable.
/// To limit the number of requests that can be processed at the time,
/// we acquire a permit for each task before spawning it. Once acquired,
/// a new task is spawned; and once finished, the permit is dropped inside
/// of the task to allow others to spawn. Permits must be acquired via
/// [`Semaphore::acquire_owned`] to be movable across the task boundary.
/// (Since our semaphore is not a global variable — if it was, then `acquire` would be enough.)
///
/// ```no_run
/// use std::sync::Arc;
/// use tokio::sync::Semaphore;
/// use tokio::net::TcpListener;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let semaphore = Arc::new(Semaphore::new(3));
///     let listener = TcpListener::bind("127.0.0.1:8080").await?;
///
///     loop {
///         // Acquire permit before accepting the next socket.
///         //
///         // We use `acquire_owned` so that we can move `permit` into
///         // other tasks.
///         let permit = semaphore.clone().acquire_owned().await.unwrap();
///         let (mut socket, _) = listener.accept().await?;
///
///         tokio::spawn(async move {
///             // Do work using the socket.
///             handle_connection(&mut socket).await;
///             // Drop socket while the permit is still live.
///             drop(socket);
///             // Drop the permit, so more tasks can be created.
///             drop(permit);
///         });
///     }
/// }
/// # async fn handle_connection(_socket: &mut tokio::net::TcpStream) {
/// #   // Do work
/// # }
/// ```
///
/// ## Prevent tests from running in parallel
///
/// By default, Rust runs tests in the same file in parallel. However, in some
/// cases, running two tests in parallel may lead to problems. For example, this
/// can happen when tests use the same database.
///
/// Consider the following scenario:
/// 1. `test_insert`: Inserts a key-value pair into the database, then retrieves
///    the value using the same key to verify the insertion.
/// 2. `test_update`: Inserts a key, then updates the key to a new value and
///    verifies that the value has been accurately updated.
/// 3. `test_others`: A third test that doesn't modify the database state. It
///    can run in parallel with the other tests.
///
/// In this example, `test_insert` and `test_update` need to run in sequence to
/// work, but it doesn't matter which test runs first. We can leverage a
/// semaphore with a single permit to address this challenge.
///
/// ```
/// # use tokio::sync::Mutex;
/// # use std::collections::BTreeMap;
/// # struct Database {
/// #   map: Mutex<BTreeMap<String, i32>>,
/// # }
/// # impl Database {
/// #    pub const fn setup() -> Database {
/// #        Database {
/// #            map: Mutex::const_new(BTreeMap::new()),
/// #        }
/// #    }
/// #    pub async fn insert(&self, key: &str, value: i32) {
/// #        self.map.lock().await.insert(key.to_string(), value);
/// #    }
/// #    pub async fn update(&self, key: &str, value: i32) {
/// #        self.map.lock().await
/// #            .entry(key.to_string())
/// #            .and_modify(|origin| *origin = value);
/// #    }
/// #    pub async fn delete(&self, key: &str) {
/// #        self.map.lock().await.remove(key);
/// #    }
/// #    pub async fn get(&self, key: &str) -> i32 {
/// #        *self.map.lock().await.get(key).unwrap()
/// #    }
/// # }
/// use tokio::sync::Semaphore;
///
/// // Initialize a static semaphore with only one permit, which is used to
/// // prevent test_insert and test_update from running in parallel.
/// static PERMIT: Semaphore = Semaphore::const_new(1);
///
/// // Initialize the database that will be used by the subsequent tests.
/// static DB: Database = Database::setup();
///
/// #[tokio::test]
/// # async fn fake_test_insert() {}
/// async fn test_insert() {
///     // Acquire permit before proceeding. Since the semaphore has only one permit,
///     // the test will wait if the permit is already acquired by other tests.
///     let permit = PERMIT.acquire().await.unwrap();
///
///     // Do the actual test stuff with database
///
///     // Insert a key-value pair to database
///     let (key, value) = ("name", 0);
///     DB.insert(key, value).await;
///
///     // Verify that the value has been inserted correctly.
///     assert_eq!(DB.get(key).await, value);
///
///     // Undo the insertion, so the database is empty at the end of the test.
///     DB.delete(key).await;
///
///     // Drop permit. This allows the other test to start running.
///     drop(permit);
/// }
///
/// #[tokio::test]
/// # async fn fake_test_update() {}
/// async fn test_update() {
///     // Acquire permit before proceeding. Since the semaphore has only one permit,
///     // the test will wait if the permit is already acquired by other tests.
///     let permit = PERMIT.acquire().await.unwrap();
///
///     // Do the same insert.
///     let (key, value) = ("name", 0);
///     DB.insert(key, value).await;
///
///     // Update the existing value with a new one.
///     let new_value = 1;
///     DB.update(key, new_value).await;
///
///     // Verify that the value has been updated correctly.
///     assert_eq!(DB.get(key).await, new_value);
///
///     // Undo any modificattion.
///     DB.delete(key).await;
///
///     // Drop permit. This allows the other test to start running.
///     drop(permit);
/// }
///
/// #[tokio::test]
/// # async fn fake_test_others() {}
/// async fn test_others() {
///     // This test can run in parallel with test_insert and test_update,
///     // so it does not use PERMIT.
/// }
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// #   test_insert().await;
/// #   test_update().await;
/// #   test_others().await;
/// # }
/// ```
///
/// ## Rate limiting using a token bucket
///
/// This example showcases the [`add_permits`] and [`SemaphorePermit::forget`] methods.
///
/// Many applications and systems have constraints on the rate at which certain
/// operations should occur. Exceeding this rate can result in suboptimal
/// performance or even errors.
///
/// This example implements rate limiting using a [token bucket]. A token bucket is a form of rate
/// limiting that doesn't kick in immediately, to allow for short bursts of incoming requests that
/// arrive at the same time.
///
/// With a token bucket, each incoming request consumes a token, and the tokens are refilled at a
/// certain rate that defines the rate limit. When a burst of requests arrives, tokens are
/// immediately given out until the bucket is empty. Once the bucket is empty, requests will have to
/// wait for new tokens to be added.
///
/// Unlike the example that limits how many requests can be handled at the same time, we do not add
/// tokens back when we finish handling a request. Instead, tokens are added only by a timer task.
///
/// Note that this implementation is suboptimal when the duration is small, because it consumes a
/// lot of cpu constantly looping and sleeping.
///
/// [token bucket]: https://en.wikipedia.org/wiki/Token_bucket
/// [`add_permits`]: crate::sync::Semaphore::add_permits
/// [`SemaphorePermit::forget`]: crate::sync::SemaphorePermit::forget
/// ```
/// use std::sync::Arc;
/// use tokio::sync::Semaphore;
/// use tokio::time::{interval, Duration};
///
/// struct TokenBucket {
///     sem: Arc<Semaphore>,
///     jh: tokio::task::JoinHandle<()>,
/// }
///
/// impl TokenBucket {
///     fn new(duration: Duration, capacity: usize) -> Self {
///         let sem = Arc::new(Semaphore::new(capacity));
///
///         // refills the tokens at the end of each interval
///         let jh = tokio::spawn({
///             let sem = sem.clone();
///             let mut interval = interval(duration);
///             interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
///
///             async move {
///                 loop {
///                     interval.tick().await;
///
///                     if sem.available_permits() < capacity {
///                         sem.add_permits(1);
///                     }
///                 }
///             }
///         });
///
///         Self { jh, sem }
///     }
///
///     async fn acquire(&self) {
///         // This can return an error if the semaphore is closed, but we
///         // never close it, so this error can never happen.
///         let permit = self.sem.acquire().await.unwrap();
///         // To avoid releasing the permit back to the semaphore, we use
///         // the `SemaphorePermit::forget` method.
///         permit.forget();
///     }
/// }
///
/// impl Drop for TokenBucket {
///     fn drop(&mut self) {
///         // Kill the background task so it stops taking up resources when we
///         // don't need it anymore.
///         self.jh.abort();
///     }
/// }
///
/// #[tokio::main]
/// # async fn _hidden() {}
/// # #[tokio::main(flavor = "current_thread", start_paused = true)]
/// async fn main() {
///     let capacity = 5;
///     let update_interval = Duration::from_secs_f32(1.0 / capacity as f32);
///     let bucket = TokenBucket::new(update_interval, capacity);
///
///     for _ in 0..5 {
///         bucket.acquire().await;
///
///         // do the operation
///     }
/// }
/// ```
///
/// [`PollSemaphore`]: https://docs.rs/tokio-util/latest/tokio_util/sync/struct.PollSemaphore.html
/// [`Semaphore::acquire_owned`]: crate::sync::Semaphore::acquire_owned
#[derive(Debug)]
pub struct Semaphore {
    /// The low level semaphore
    ll_sem: ll::Semaphore,
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
}

/// A permit from the semaphore.
///
/// This type is created by the [`acquire`] method.
///
/// [`acquire`]: crate::sync::Semaphore::acquire()
#[must_use]
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct SemaphorePermit<'a> {
    sem: &'a Semaphore,
    permits: u32,
}

/// An owned permit from the semaphore.
///
/// This type is created by the [`acquire_owned`] method.
///
/// [`acquire_owned`]: crate::sync::Semaphore::acquire_owned()
#[must_use]
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct OwnedSemaphorePermit {
    sem: Arc<Semaphore>,
    permits: u32,
}

#[test]
#[cfg(not(loom))]
fn bounds() {
    fn check_unpin<T: Unpin>() {}
    // This has to take a value, since the async fn's return type is unnameable.
    fn check_send_sync_val<T: Send + Sync>(_t: T) {}
    fn check_send_sync<T: Send + Sync>() {}
    check_unpin::<Semaphore>();
    check_unpin::<SemaphorePermit<'_>>();
    check_send_sync::<Semaphore>();

    let semaphore = Semaphore::new(0);
    check_send_sync_val(semaphore.acquire());
}

impl Semaphore {
    /// The maximum number of permits which a semaphore can hold. It is `usize::MAX >> 3`.
    ///
    /// Exceeding this limit typically results in a panic.
    pub const MAX_PERMITS: usize = super::batch_semaphore::Semaphore::MAX_PERMITS;

    /// Creates a new semaphore with the initial number of permits.
    ///
    /// Panics if `permits` exceeds [`Semaphore::MAX_PERMITS`].
    #[track_caller]
    pub fn new(permits: usize) -> Self {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = {
            let location = std::panic::Location::caller();

            tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "Semaphore",
                kind = "Sync",
                loc.file = location.file(),
                loc.line = location.line(),
                loc.col = location.column(),
                inherits_child_attrs = true,
            )
        };

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let ll_sem = resource_span.in_scope(|| ll::Semaphore::new(permits));

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        let ll_sem = ll::Semaphore::new(permits);

        Self {
            ll_sem,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span,
        }
    }

    /// Creates a new semaphore with the initial number of permits.
    ///
    /// When using the `tracing` [unstable feature], a `Semaphore` created with
    /// `const_new` will not be instrumented. As such, it will not be visible
    /// in [`tokio-console`]. Instead, [`Semaphore::new`] should be used to
    /// create an instrumented object if that is needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Semaphore;
    ///
    /// static SEM: Semaphore = Semaphore::const_new(10);
    /// ```
    ///
    /// [`tokio-console`]: https://github.com/tokio-rs/console
    /// [unstable feature]: crate#unstable-features
    #[cfg(not(all(loom, test)))]
    pub const fn const_new(permits: usize) -> Self {
        Self {
            ll_sem: ll::Semaphore::const_new(permits),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Creates a new closed semaphore with 0 permits.
    pub(crate) fn new_closed() -> Self {
        Self {
            ll_sem: ll::Semaphore::new_closed(),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Creates a new closed semaphore with 0 permits.
    #[cfg(not(all(loom, test)))]
    pub(crate) const fn const_new_closed() -> Self {
        Self {
            ll_sem: ll::Semaphore::const_new_closed(),
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: tracing::Span::none(),
        }
    }

    /// Returns the current number of available permits.
    pub fn available_permits(&self) -> usize {
        self.ll_sem.available_permits()
    }

    /// Adds `n` new permits to the semaphore.
    ///
    /// The maximum number of permits is [`Semaphore::MAX_PERMITS`], and this function will panic if the limit is exceeded.
    pub fn add_permits(&self, n: usize) {
        self.ll_sem.release(n);
    }

    /// Decrease a semaphore's permits by a maximum of `n`.
    ///
    /// If there are insufficient permits and it's not possible to reduce by `n`,
    /// return the number of permits that were actually reduced.
    pub fn forget_permits(&self, n: usize) -> usize {
        self.ll_sem.forget_permits(n)
    }

    /// Acquires a permit from the semaphore.
    ///
    /// If the semaphore has been closed, this returns an [`AcquireError`].
    /// Otherwise, this returns a [`SemaphorePermit`] representing the
    /// acquired permit.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute permits in the order they
    /// were requested. Cancelling a call to `acquire` makes you lose your place
    /// in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Semaphore;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let semaphore = Semaphore::new(2);
    ///
    ///     let permit_1 = semaphore.acquire().await.unwrap();
    ///     assert_eq!(semaphore.available_permits(), 1);
    ///
    ///     let permit_2 = semaphore.acquire().await.unwrap();
    ///     assert_eq!(semaphore.available_permits(), 0);
    ///
    ///     drop(permit_1);
    ///     assert_eq!(semaphore.available_permits(), 1);
    /// }
    /// ```
    ///
    /// [`AcquireError`]: crate::sync::AcquireError
    /// [`SemaphorePermit`]: crate::sync::SemaphorePermit
    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, AcquireError> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let inner = trace::async_op(
            || self.ll_sem.acquire(1),
            self.resource_span.clone(),
            "Semaphore::acquire",
            "poll",
            true,
        );
        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        let inner = self.ll_sem.acquire(1);

        inner.await?;
        Ok(SemaphorePermit {
            sem: self,
            permits: 1,
        })
    }

    /// Acquires `n` permits from the semaphore.
    ///
    /// If the semaphore has been closed, this returns an [`AcquireError`].
    /// Otherwise, this returns a [`SemaphorePermit`] representing the
    /// acquired permits.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute permits in the order they
    /// were requested. Cancelling a call to `acquire_many` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Semaphore;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let semaphore = Semaphore::new(5);
    ///
    ///     let permit = semaphore.acquire_many(3).await.unwrap();
    ///     assert_eq!(semaphore.available_permits(), 2);
    /// }
    /// ```
    ///
    /// [`AcquireError`]: crate::sync::AcquireError
    /// [`SemaphorePermit`]: crate::sync::SemaphorePermit
    pub async fn acquire_many(&self, n: u32) -> Result<SemaphorePermit<'_>, AcquireError> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        trace::async_op(
            || self.ll_sem.acquire(n as usize),
            self.resource_span.clone(),
            "Semaphore::acquire_many",
            "poll",
            true,
        )
        .await?;

        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        self.ll_sem.acquire(n as usize).await?;

        Ok(SemaphorePermit {
            sem: self,
            permits: n,
        })
    }

    /// Tries to acquire a permit from the semaphore.
    ///
    /// If the semaphore has been closed, this returns a [`TryAcquireError::Closed`]
    /// and a [`TryAcquireError::NoPermits`] if there are no permits left. Otherwise,
    /// this returns a [`SemaphorePermit`] representing the acquired permits.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{Semaphore, TryAcquireError};
    ///
    /// # fn main() {
    /// let semaphore = Semaphore::new(2);
    ///
    /// let permit_1 = semaphore.try_acquire().unwrap();
    /// assert_eq!(semaphore.available_permits(), 1);
    ///
    /// let permit_2 = semaphore.try_acquire().unwrap();
    /// assert_eq!(semaphore.available_permits(), 0);
    ///
    /// let permit_3 = semaphore.try_acquire();
    /// assert_eq!(permit_3.err(), Some(TryAcquireError::NoPermits));
    /// # }
    /// ```
    ///
    /// [`TryAcquireError::Closed`]: crate::sync::TryAcquireError::Closed
    /// [`TryAcquireError::NoPermits`]: crate::sync::TryAcquireError::NoPermits
    /// [`SemaphorePermit`]: crate::sync::SemaphorePermit
    pub fn try_acquire(&self) -> Result<SemaphorePermit<'_>, TryAcquireError> {
        match self.ll_sem.try_acquire(1) {
            Ok(()) => Ok(SemaphorePermit {
                sem: self,
                permits: 1,
            }),
            Err(e) => Err(e),
        }
    }

    /// Tries to acquire `n` permits from the semaphore.
    ///
    /// If the semaphore has been closed, this returns a [`TryAcquireError::Closed`]
    /// and a [`TryAcquireError::NoPermits`] if there are not enough permits left.
    /// Otherwise, this returns a [`SemaphorePermit`] representing the acquired permits.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{Semaphore, TryAcquireError};
    ///
    /// # fn main() {
    /// let semaphore = Semaphore::new(4);
    ///
    /// let permit_1 = semaphore.try_acquire_many(3).unwrap();
    /// assert_eq!(semaphore.available_permits(), 1);
    ///
    /// let permit_2 = semaphore.try_acquire_many(2);
    /// assert_eq!(permit_2.err(), Some(TryAcquireError::NoPermits));
    /// # }
    /// ```
    ///
    /// [`TryAcquireError::Closed`]: crate::sync::TryAcquireError::Closed
    /// [`TryAcquireError::NoPermits`]: crate::sync::TryAcquireError::NoPermits
    /// [`SemaphorePermit`]: crate::sync::SemaphorePermit
    pub fn try_acquire_many(&self, n: u32) -> Result<SemaphorePermit<'_>, TryAcquireError> {
        match self.ll_sem.try_acquire(n as usize) {
            Ok(()) => Ok(SemaphorePermit {
                sem: self,
                permits: n,
            }),
            Err(e) => Err(e),
        }
    }

    /// Acquires a permit from the semaphore.
    ///
    /// The semaphore must be wrapped in an [`Arc`] to call this method.
    /// If the semaphore has been closed, this returns an [`AcquireError`].
    /// Otherwise, this returns a [`OwnedSemaphorePermit`] representing the
    /// acquired permit.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute permits in the order they
    /// were requested. Cancelling a call to `acquire_owned` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let semaphore = Arc::new(Semaphore::new(3));
    ///     let mut join_handles = Vec::new();
    ///
    ///     for _ in 0..5 {
    ///         let permit = semaphore.clone().acquire_owned().await.unwrap();
    ///         join_handles.push(tokio::spawn(async move {
    ///             // perform task...
    ///             // explicitly own `permit` in the task
    ///             drop(permit);
    ///         }));
    ///     }
    ///
    ///     for handle in join_handles {
    ///         handle.await.unwrap();
    ///     }
    /// }
    /// ```
    ///
    /// [`Arc`]: std::sync::Arc
    /// [`AcquireError`]: crate::sync::AcquireError
    /// [`OwnedSemaphorePermit`]: crate::sync::OwnedSemaphorePermit
    pub async fn acquire_owned(self: Arc<Self>) -> Result<OwnedSemaphorePermit, AcquireError> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let inner = trace::async_op(
            || self.ll_sem.acquire(1),
            self.resource_span.clone(),
            "Semaphore::acquire_owned",
            "poll",
            true,
        );
        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        let inner = self.ll_sem.acquire(1);

        inner.await?;
        Ok(OwnedSemaphorePermit {
            sem: self,
            permits: 1,
        })
    }

    /// Acquires `n` permits from the semaphore.
    ///
    /// The semaphore must be wrapped in an [`Arc`] to call this method.
    /// If the semaphore has been closed, this returns an [`AcquireError`].
    /// Otherwise, this returns a [`OwnedSemaphorePermit`] representing the
    /// acquired permit.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute permits in the order they
    /// were requested. Cancelling a call to `acquire_many_owned` makes you lose
    /// your place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let semaphore = Arc::new(Semaphore::new(10));
    ///     let mut join_handles = Vec::new();
    ///
    ///     for _ in 0..5 {
    ///         let permit = semaphore.clone().acquire_many_owned(2).await.unwrap();
    ///         join_handles.push(tokio::spawn(async move {
    ///             // perform task...
    ///             // explicitly own `permit` in the task
    ///             drop(permit);
    ///         }));
    ///     }
    ///
    ///     for handle in join_handles {
    ///         handle.await.unwrap();
    ///     }
    /// }
    /// ```
    ///
    /// [`Arc`]: std::sync::Arc
    /// [`AcquireError`]: crate::sync::AcquireError
    /// [`OwnedSemaphorePermit`]: crate::sync::OwnedSemaphorePermit
    pub async fn acquire_many_owned(
        self: Arc<Self>,
        n: u32,
    ) -> Result<OwnedSemaphorePermit, AcquireError> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let inner = trace::async_op(
            || self.ll_sem.acquire(n as usize),
            self.resource_span.clone(),
            "Semaphore::acquire_many_owned",
            "poll",
            true,
        );
        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        let inner = self.ll_sem.acquire(n as usize);

        inner.await?;
        Ok(OwnedSemaphorePermit {
            sem: self,
            permits: n,
        })
    }

    /// Tries to acquire a permit from the semaphore.
    ///
    /// The semaphore must be wrapped in an [`Arc`] to call this method. If
    /// the semaphore has been closed, this returns a [`TryAcquireError::Closed`]
    /// and a [`TryAcquireError::NoPermits`] if there are no permits left.
    /// Otherwise, this returns a [`OwnedSemaphorePermit`] representing the
    /// acquired permit.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::{Semaphore, TryAcquireError};
    ///
    /// # fn main() {
    /// let semaphore = Arc::new(Semaphore::new(2));
    ///
    /// let permit_1 = Arc::clone(&semaphore).try_acquire_owned().unwrap();
    /// assert_eq!(semaphore.available_permits(), 1);
    ///
    /// let permit_2 = Arc::clone(&semaphore).try_acquire_owned().unwrap();
    /// assert_eq!(semaphore.available_permits(), 0);
    ///
    /// let permit_3 = semaphore.try_acquire_owned();
    /// assert_eq!(permit_3.err(), Some(TryAcquireError::NoPermits));
    /// # }
    /// ```
    ///
    /// [`Arc`]: std::sync::Arc
    /// [`TryAcquireError::Closed`]: crate::sync::TryAcquireError::Closed
    /// [`TryAcquireError::NoPermits`]: crate::sync::TryAcquireError::NoPermits
    /// [`OwnedSemaphorePermit`]: crate::sync::OwnedSemaphorePermit
    pub fn try_acquire_owned(self: Arc<Self>) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        match self.ll_sem.try_acquire(1) {
            Ok(()) => Ok(OwnedSemaphorePermit {
                sem: self,
                permits: 1,
            }),
            Err(e) => Err(e),
        }
    }

    /// Tries to acquire `n` permits from the semaphore.
    ///
    /// The semaphore must be wrapped in an [`Arc`] to call this method. If
    /// the semaphore has been closed, this returns a [`TryAcquireError::Closed`]
    /// and a [`TryAcquireError::NoPermits`] if there are no permits left.
    /// Otherwise, this returns a [`OwnedSemaphorePermit`] representing the
    /// acquired permit.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::{Semaphore, TryAcquireError};
    ///
    /// # fn main() {
    /// let semaphore = Arc::new(Semaphore::new(4));
    ///
    /// let permit_1 = Arc::clone(&semaphore).try_acquire_many_owned(3).unwrap();
    /// assert_eq!(semaphore.available_permits(), 1);
    ///
    /// let permit_2 = semaphore.try_acquire_many_owned(2);
    /// assert_eq!(permit_2.err(), Some(TryAcquireError::NoPermits));
    /// # }
    /// ```
    ///
    /// [`Arc`]: std::sync::Arc
    /// [`TryAcquireError::Closed`]: crate::sync::TryAcquireError::Closed
    /// [`TryAcquireError::NoPermits`]: crate::sync::TryAcquireError::NoPermits
    /// [`OwnedSemaphorePermit`]: crate::sync::OwnedSemaphorePermit
    pub fn try_acquire_many_owned(
        self: Arc<Self>,
        n: u32,
    ) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        match self.ll_sem.try_acquire(n as usize) {
            Ok(()) => Ok(OwnedSemaphorePermit {
                sem: self,
                permits: n,
            }),
            Err(e) => Err(e),
        }
    }

    /// Closes the semaphore.
    ///
    /// This prevents the semaphore from issuing new permits and notifies all pending waiters.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Semaphore;
    /// use std::sync::Arc;
    /// use tokio::sync::TryAcquireError;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let semaphore = Arc::new(Semaphore::new(1));
    ///     let semaphore2 = semaphore.clone();
    ///
    ///     tokio::spawn(async move {
    ///         let permit = semaphore.acquire_many(2).await;
    ///         assert!(permit.is_err());
    ///         println!("waiter received error");
    ///     });
    ///
    ///     println!("closing semaphore");
    ///     semaphore2.close();
    ///
    ///     // Cannot obtain more permits
    ///     assert_eq!(semaphore2.try_acquire().err(), Some(TryAcquireError::Closed))
    /// }
    /// ```
    pub fn close(&self) {
        self.ll_sem.close();
    }

    /// Returns true if the semaphore is closed
    pub fn is_closed(&self) -> bool {
        self.ll_sem.is_closed()
    }
}

impl<'a> SemaphorePermit<'a> {
    /// Forgets the permit **without** releasing it back to the semaphore.
    /// This can be used to reduce the amount of permits available from a
    /// semaphore.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// let sem = Arc::new(Semaphore::new(10));
    /// {
    ///     let permit = sem.try_acquire_many(5).unwrap();
    ///     assert_eq!(sem.available_permits(), 5);
    ///     permit.forget();
    /// }
    ///
    /// // Since we forgot the permit, available permits won't go back to its initial value
    /// // even after the permit is dropped.
    /// assert_eq!(sem.available_permits(), 5);
    /// ```
    pub fn forget(mut self) {
        self.permits = 0;
    }

    /// Merge two [`SemaphorePermit`] instances together, consuming `other`
    /// without releasing the permits it holds.
    ///
    /// Permits held by both `self` and `other` are released when `self` drops.
    ///
    /// # Panics
    ///
    /// This function panics if permits from different [`Semaphore`] instances
    /// are merged.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// let sem = Arc::new(Semaphore::new(10));
    /// let mut permit = sem.try_acquire().unwrap();
    ///
    /// for _ in 0..9 {
    ///     let _permit = sem.try_acquire().unwrap();
    ///     // Merge individual permits into a single one.
    ///     permit.merge(_permit)
    /// }
    ///
    /// assert_eq!(sem.available_permits(), 0);
    ///
    /// // Release all permits in a single batch.
    /// drop(permit);
    ///
    /// assert_eq!(sem.available_permits(), 10);
    /// ```
    #[track_caller]
    pub fn merge(&mut self, mut other: Self) {
        assert!(
            std::ptr::eq(self.sem, other.sem),
            "merging permits from different semaphore instances"
        );
        self.permits += other.permits;
        other.permits = 0;
    }

    /// Splits `n` permits from `self` and returns a new [`SemaphorePermit`] instance that holds `n` permits.
    ///
    /// If there are insufficient permits and it's not possible to reduce by `n`, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// let sem = Arc::new(Semaphore::new(3));
    ///
    /// let mut p1 = sem.try_acquire_many(3).unwrap();
    /// let p2 = p1.split(1).unwrap();
    ///
    /// assert_eq!(p1.num_permits(), 2);
    /// assert_eq!(p2.num_permits(), 1);
    /// ```
    pub fn split(&mut self, n: usize) -> Option<Self> {
        let n = u32::try_from(n).ok()?;

        if n > self.permits {
            return None;
        }

        self.permits -= n;

        Some(Self {
            sem: self.sem,
            permits: n,
        })
    }

    /// Returns the number of permits held by `self`.
    pub fn num_permits(&self) -> usize {
        self.permits as usize
    }
}

impl OwnedSemaphorePermit {
    /// Forgets the permit **without** releasing it back to the semaphore.
    /// This can be used to reduce the amount of permits available from a
    /// semaphore.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// let sem = Arc::new(Semaphore::new(10));
    /// {
    ///     let permit = sem.clone().try_acquire_many_owned(5).unwrap();
    ///     assert_eq!(sem.available_permits(), 5);
    ///     permit.forget();
    /// }
    ///
    /// // Since we forgot the permit, available permits won't go back to its initial value
    /// // even after the permit is dropped.
    /// assert_eq!(sem.available_permits(), 5);
    /// ```
    pub fn forget(mut self) {
        self.permits = 0;
    }

    /// Merge two [`OwnedSemaphorePermit`] instances together, consuming `other`
    /// without releasing the permits it holds.
    ///
    /// Permits held by both `self` and `other` are released when `self` drops.
    ///
    /// # Panics
    ///
    /// This function panics if permits from different [`Semaphore`] instances
    /// are merged.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// let sem = Arc::new(Semaphore::new(10));
    /// let mut permit = sem.clone().try_acquire_owned().unwrap();
    ///
    /// for _ in 0..9 {
    ///     let _permit = sem.clone().try_acquire_owned().unwrap();
    ///     // Merge individual permits into a single one.
    ///     permit.merge(_permit)
    /// }
    ///
    /// assert_eq!(sem.available_permits(), 0);
    ///
    /// // Release all permits in a single batch.
    /// drop(permit);
    ///
    /// assert_eq!(sem.available_permits(), 10);
    /// ```
    #[track_caller]
    pub fn merge(&mut self, mut other: Self) {
        assert!(
            Arc::ptr_eq(&self.sem, &other.sem),
            "merging permits from different semaphore instances"
        );
        self.permits += other.permits;
        other.permits = 0;
    }

    /// Splits `n` permits from `self` and returns a new [`OwnedSemaphorePermit`] instance that holds `n` permits.
    ///
    /// If there are insufficient permits and it's not possible to reduce by `n`, returns `None`.
    ///
    /// # Note
    ///
    /// It will clone the owned `Arc<Semaphore>` to construct the new instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Semaphore;
    ///
    /// let sem = Arc::new(Semaphore::new(3));
    ///
    /// let mut p1 = sem.try_acquire_many_owned(3).unwrap();
    /// let p2 = p1.split(1).unwrap();
    ///
    /// assert_eq!(p1.num_permits(), 2);
    /// assert_eq!(p2.num_permits(), 1);
    /// ```
    pub fn split(&mut self, n: usize) -> Option<Self> {
        let n = u32::try_from(n).ok()?;

        if n > self.permits {
            return None;
        }

        self.permits -= n;

        Some(Self {
            sem: self.sem.clone(),
            permits: n,
        })
    }

    /// Returns the [`Semaphore`] from which this permit was acquired.
    pub fn semaphore(&self) -> &Arc<Semaphore> {
        &self.sem
    }

    /// Returns the number of permits held by `self`.
    pub fn num_permits(&self) -> usize {
        self.permits as usize
    }
}

impl Drop for SemaphorePermit<'_> {
    fn drop(&mut self) {
        self.sem.add_permits(self.permits as usize);
    }
}

impl Drop for OwnedSemaphorePermit {
    fn drop(&mut self) {
        self.sem.add_permits(self.permits as usize);
    }
}
