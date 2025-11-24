//! Async executors.
//!
//! This crate provides two reference executors that trade performance for
//! functionality. They should be considered reference executors that are "good
//! enough" for most use cases. For more specialized use cases, consider writing
//! your own executor on top of [`async-task`].
//!
//! [`async-task`]: https://crates.io/crates/async-task
//!
//! # Examples
//!
//! ```
//! use async_executor::Executor;
//! use futures_lite::future;
//!
//! // Create a new executor.
//! let ex = Executor::new();
//!
//! // Spawn a task.
//! let task = ex.spawn(async {
//!     println!("Hello world");
//! });
//!
//! // Run the executor until the task completes.
//! future::block_on(ex.run(task));
//! ```

#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    clippy::undocumented_unsafe_blocks
)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![allow(clippy::unused_unit)] // false positive fixed in Rust 1.89

use std::fmt;
use std::marker::PhantomData;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, TryLockError};
use std::task::{Context, Poll, Waker};

use async_task::{Builder, Runnable};
use concurrent_queue::ConcurrentQueue;
use futures_lite::{future, prelude::*};
use pin_project_lite::pin_project;
use slab::Slab;

#[cfg(feature = "static")]
mod static_executors;

#[doc(no_inline)]
pub use async_task::{FallibleTask, Task};
#[cfg(feature = "static")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "static"))))]
pub use static_executors::*;

/// An async executor.
///
/// # Examples
///
/// A multi-threaded executor:
///
/// ```
/// use async_channel::unbounded;
/// use async_executor::Executor;
/// use easy_parallel::Parallel;
/// use futures_lite::future;
///
/// let ex = Executor::new();
/// let (signal, shutdown) = unbounded::<()>();
///
/// Parallel::new()
///     // Run four executor threads.
///     .each(0..4, |_| future::block_on(ex.run(shutdown.recv())))
///     // Run the main future on the current thread.
///     .finish(|| future::block_on(async {
///         println!("Hello world!");
///         drop(signal);
///     }));
/// ```
pub struct Executor<'a> {
    /// The executor state.
    state: AtomicPtr<State>,

    /// Makes the `'a` lifetime invariant.
    _marker: PhantomData<std::cell::UnsafeCell<&'a ()>>,
}

// SAFETY: Executor stores no thread local state that can be accessed via other thread.
unsafe impl Send for Executor<'_> {}
// SAFETY: Executor internally synchronizes all of it's operations internally.
unsafe impl Sync for Executor<'_> {}

impl UnwindSafe for Executor<'_> {}
impl RefUnwindSafe for Executor<'_> {}

impl fmt::Debug for Executor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_executor(self, "Executor", f)
    }
}

impl<'a> Executor<'a> {
    /// Creates a new executor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::Executor;
    ///
    /// let ex = Executor::new();
    /// ```
    pub const fn new() -> Executor<'a> {
        Executor {
            state: AtomicPtr::new(std::ptr::null_mut()),
            _marker: PhantomData,
        }
    }

    /// Returns `true` if there are no unfinished tasks.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::Executor;
    ///
    /// let ex = Executor::new();
    /// assert!(ex.is_empty());
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// assert!(!ex.is_empty());
    ///
    /// assert!(ex.try_tick());
    /// assert!(ex.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.state().active().is_empty()
    }

    /// Spawns a task onto the executor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::Executor;
    ///
    /// let ex = Executor::new();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// ```
    pub fn spawn<T: Send + 'a>(&self, future: impl Future<Output = T> + Send + 'a) -> Task<T> {
        let mut active = self.state().active();

        // SAFETY: `T` and the future are `Send`.
        unsafe { self.spawn_inner(future, &mut active) }
    }

    /// Spawns many tasks onto the executor.
    ///
    /// As opposed to the [`spawn`] method, this locks the executor's inner task lock once and
    /// spawns all of the tasks in one go. With large amounts of tasks this can improve
    /// contention.
    ///
    /// For very large numbers of tasks the lock is occasionally dropped and re-acquired to
    /// prevent runner thread starvation. It is assumed that the iterator provided does not
    /// block; blocking iterators can lock up the internal mutex and therefore the entire
    /// executor.
    ///
    /// ## Example
    ///
    /// ```
    /// use async_executor::Executor;
    /// use futures_lite::{stream, prelude::*};
    /// use std::future::ready;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut ex = Executor::new();
    ///
    /// let futures = [
    ///     ready(1),
    ///     ready(2),
    ///     ready(3)
    /// ];
    ///
    /// // Spawn all of the futures onto the executor at once.
    /// let mut tasks = vec![];
    /// ex.spawn_many(futures, &mut tasks);
    ///
    /// // Await all of them.
    /// let results = ex.run(async move {
    ///     stream::iter(tasks).then(|x| x).collect::<Vec<_>>().await
    /// }).await;
    /// assert_eq!(results, [1, 2, 3]);
    /// # });
    /// ```
    ///
    /// [`spawn`]: Executor::spawn
    pub fn spawn_many<T: Send + 'a, F: Future<Output = T> + Send + 'a>(
        &self,
        futures: impl IntoIterator<Item = F>,
        handles: &mut impl Extend<Task<F::Output>>,
    ) {
        let mut active = Some(self.state().active());

        // Convert the futures into tasks.
        let tasks = futures.into_iter().enumerate().map(move |(i, future)| {
            // SAFETY: `T` and the future are `Send`.
            let task = unsafe { self.spawn_inner(future, active.as_mut().unwrap()) };

            // Yield the lock every once in a while to ease contention.
            if i.wrapping_sub(1) % 500 == 0 {
                drop(active.take());
                active = Some(self.state().active());
            }

            task
        });

        // Push the tasks to the user's collection.
        handles.extend(tasks);
    }

    /// Spawn a future while holding the inner lock.
    ///
    /// # Safety
    ///
    /// If this is an `Executor`, `F` and `T` must be `Send`.
    unsafe fn spawn_inner<T: 'a>(
        &self,
        future: impl Future<Output = T> + 'a,
        active: &mut Slab<Waker>,
    ) -> Task<T> {
        // Remove the task from the set of active tasks when the future finishes.
        let entry = active.vacant_entry();
        let index = entry.key();
        let state = self.state_as_arc();
        let future = AsyncCallOnDrop::new(future, move || drop(state.active().try_remove(index)));

        // Create the task and register it in the set of active tasks.
        //
        // SAFETY:
        //
        // If `future` is not `Send`, this must be a `LocalExecutor` as per this
        // function's unsafe precondition. Since `LocalExecutor` is `!Sync`,
        // `try_tick`, `tick` and `run` can only be called from the origin
        // thread of the `LocalExecutor`. Similarly, `spawn` can only  be called
        // from the origin thread, ensuring that `future` and the executor share
        // the same origin thread. The `Runnable` can be scheduled from other
        // threads, but because of the above `Runnable` can only be called or
        // dropped on the origin thread.
        //
        // `future` is not `'static`, but we make sure that the `Runnable` does
        // not outlive `'a`. When the executor is dropped, the `active` field is
        // drained and all of the `Waker`s are woken. Then, the queue inside of
        // the `Executor` is drained of all of its runnables. This ensures that
        // runnables are dropped and this precondition is satisfied.
        //
        // `self.schedule()` is `Send`, `Sync` and `'static`, as checked below.
        // Therefore we do not need to worry about what is done with the
        // `Waker`.
        let (runnable, task) = Builder::new()
            .propagate_panic(true)
            .spawn_unchecked(|()| future, self.schedule());
        entry.insert(runnable.waker());

        runnable.schedule();
        task
    }

    /// Attempts to run a task if at least one is scheduled.
    ///
    /// Running a scheduled task means simply polling its future once.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::Executor;
    ///
    /// let ex = Executor::new();
    /// assert!(!ex.try_tick()); // no tasks to run
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// assert!(ex.try_tick()); // a task was found
    /// ```
    pub fn try_tick(&self) -> bool {
        self.state().try_tick()
    }

    /// Runs a single task.
    ///
    /// Running a task means simply polling its future once.
    ///
    /// If no tasks are scheduled when this method is called, it will wait until one is scheduled.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::Executor;
    /// use futures_lite::future;
    ///
    /// let ex = Executor::new();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// future::block_on(ex.tick()); // runs the task
    /// ```
    pub async fn tick(&self) {
        self.state().tick().await;
    }

    /// Runs the executor until the given future completes.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::Executor;
    /// use futures_lite::future;
    ///
    /// let ex = Executor::new();
    ///
    /// let task = ex.spawn(async { 1 + 2 });
    /// let res = future::block_on(ex.run(async { task.await * 2 }));
    ///
    /// assert_eq!(res, 6);
    /// ```
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        self.state().run(future).await
    }

    /// Returns a function that schedules a runnable task when it gets woken up.
    fn schedule(&self) -> impl Fn(Runnable) + Send + Sync + 'static {
        let state = self.state_as_arc();

        // TODO: If possible, push into the current local queue and notify the ticker.
        move |runnable| {
            let result = state.queue.push(runnable);
            debug_assert!(result.is_ok()); // Since we use unbounded queue, push will never fail.
            state.notify();
        }
    }

    /// Returns a pointer to the inner state.
    #[inline]
    fn state_ptr(&self) -> *const State {
        #[cold]
        fn alloc_state(atomic_ptr: &AtomicPtr<State>) -> *mut State {
            let state = Arc::new(State::new());
            // TODO: Switch this to use cast_mut once the MSRV can be bumped past 1.65
            let ptr = Arc::into_raw(state) as *mut State;
            if let Err(actual) = atomic_ptr.compare_exchange(
                std::ptr::null_mut(),
                ptr,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                // SAFETY: This was just created from Arc::into_raw.
                drop(unsafe { Arc::from_raw(ptr) });
                actual
            } else {
                ptr
            }
        }

        let mut ptr = self.state.load(Ordering::Acquire);
        if ptr.is_null() {
            ptr = alloc_state(&self.state);
        }
        ptr
    }

    /// Returns a reference to the inner state.
    #[inline]
    fn state(&self) -> &State {
        // SAFETY: So long as an Executor lives, it's state pointer will always be valid
        // when accessed through state_ptr.
        unsafe { &*self.state_ptr() }
    }

    // Clones the inner state Arc
    #[inline]
    fn state_as_arc(&self) -> Arc<State> {
        // SAFETY: So long as an Executor lives, it's state pointer will always be a valid
        // Arc when accessed through state_ptr.
        let arc = unsafe { Arc::from_raw(self.state_ptr()) };
        let clone = arc.clone();
        std::mem::forget(arc);
        clone
    }
}

impl Drop for Executor<'_> {
    fn drop(&mut self) {
        let ptr = *self.state.get_mut();
        if ptr.is_null() {
            return;
        }

        // SAFETY: As ptr is not null, it was allocated via Arc::new and converted
        // via Arc::into_raw in state_ptr.
        let state = unsafe { Arc::from_raw(ptr) };

        let mut active = state.active();
        for w in active.drain() {
            w.wake();
        }
        drop(active);

        while state.queue.pop().is_ok() {}
    }
}

impl<'a> Default for Executor<'a> {
    fn default() -> Executor<'a> {
        Executor::new()
    }
}

/// A thread-local executor.
///
/// The executor can only be run on the thread that created it.
///
/// # Examples
///
/// ```
/// use async_executor::LocalExecutor;
/// use futures_lite::future;
///
/// let local_ex = LocalExecutor::new();
///
/// future::block_on(local_ex.run(async {
///     println!("Hello world!");
/// }));
/// ```
pub struct LocalExecutor<'a> {
    /// The inner executor.
    inner: Executor<'a>,

    /// Makes the type `!Send` and `!Sync`.
    _marker: PhantomData<Rc<()>>,
}

impl UnwindSafe for LocalExecutor<'_> {}
impl RefUnwindSafe for LocalExecutor<'_> {}

impl fmt::Debug for LocalExecutor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_executor(&self.inner, "LocalExecutor", f)
    }
}

impl<'a> LocalExecutor<'a> {
    /// Creates a single-threaded executor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    ///
    /// let local_ex = LocalExecutor::new();
    /// ```
    pub const fn new() -> LocalExecutor<'a> {
        LocalExecutor {
            inner: Executor::new(),
            _marker: PhantomData,
        }
    }

    /// Returns `true` if there are no unfinished tasks.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    ///
    /// let local_ex = LocalExecutor::new();
    /// assert!(local_ex.is_empty());
    ///
    /// let task = local_ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// assert!(!local_ex.is_empty());
    ///
    /// assert!(local_ex.try_tick());
    /// assert!(local_ex.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner().is_empty()
    }

    /// Spawns a task onto the executor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    ///
    /// let local_ex = LocalExecutor::new();
    ///
    /// let task = local_ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// ```
    pub fn spawn<T: 'a>(&self, future: impl Future<Output = T> + 'a) -> Task<T> {
        let mut active = self.inner().state().active();

        // SAFETY: This executor is not thread safe, so the future and its result
        //         cannot be sent to another thread.
        unsafe { self.inner().spawn_inner(future, &mut active) }
    }

    /// Spawns many tasks onto the executor.
    ///
    /// As opposed to the [`spawn`] method, this locks the executor's inner task lock once and
    /// spawns all of the tasks in one go. With large amounts of tasks this can improve
    /// contention.
    ///
    /// It is assumed that the iterator provided does not block; blocking iterators can lock up
    /// the internal mutex and therefore the entire executor. Unlike [`Executor::spawn`], the
    /// mutex is not released, as there are no other threads that can poll this executor.
    ///
    /// ## Example
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    /// use futures_lite::{stream, prelude::*};
    /// use std::future::ready;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut ex = LocalExecutor::new();
    ///
    /// let futures = [
    ///     ready(1),
    ///     ready(2),
    ///     ready(3)
    /// ];
    ///
    /// // Spawn all of the futures onto the executor at once.
    /// let mut tasks = vec![];
    /// ex.spawn_many(futures, &mut tasks);
    ///
    /// // Await all of them.
    /// let results = ex.run(async move {
    ///     stream::iter(tasks).then(|x| x).collect::<Vec<_>>().await
    /// }).await;
    /// assert_eq!(results, [1, 2, 3]);
    /// # });
    /// ```
    ///
    /// [`spawn`]: LocalExecutor::spawn
    /// [`Executor::spawn_many`]: Executor::spawn_many
    pub fn spawn_many<T: 'a, F: Future<Output = T> + 'a>(
        &self,
        futures: impl IntoIterator<Item = F>,
        handles: &mut impl Extend<Task<F::Output>>,
    ) {
        let mut active = self.inner().state().active();

        // Convert all of the futures to tasks.
        let tasks = futures.into_iter().map(|future| {
            // SAFETY: This executor is not thread safe, so the future and its result
            //         cannot be sent to another thread.
            unsafe { self.inner().spawn_inner(future, &mut active) }

            // As only one thread can spawn or poll tasks at a time, there is no need
            // to release lock contention here.
        });

        // Push them to the user's collection.
        handles.extend(tasks);
    }

    /// Attempts to run a task if at least one is scheduled.
    ///
    /// Running a scheduled task means simply polling its future once.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    ///
    /// let ex = LocalExecutor::new();
    /// assert!(!ex.try_tick()); // no tasks to run
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// assert!(ex.try_tick()); // a task was found
    /// ```
    pub fn try_tick(&self) -> bool {
        self.inner().try_tick()
    }

    /// Runs a single task.
    ///
    /// Running a task means simply polling its future once.
    ///
    /// If no tasks are scheduled when this method is called, it will wait until one is scheduled.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    /// use futures_lite::future;
    ///
    /// let ex = LocalExecutor::new();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// future::block_on(ex.tick()); // runs the task
    /// ```
    pub async fn tick(&self) {
        self.inner().tick().await
    }

    /// Runs the executor until the given future completes.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    /// use futures_lite::future;
    ///
    /// let local_ex = LocalExecutor::new();
    ///
    /// let task = local_ex.spawn(async { 1 + 2 });
    /// let res = future::block_on(local_ex.run(async { task.await * 2 }));
    ///
    /// assert_eq!(res, 6);
    /// ```
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        self.inner().run(future).await
    }

    /// Returns a reference to the inner executor.
    fn inner(&self) -> &Executor<'a> {
        &self.inner
    }
}

impl<'a> Default for LocalExecutor<'a> {
    fn default() -> LocalExecutor<'a> {
        LocalExecutor::new()
    }
}

/// The state of a executor.
struct State {
    /// The global queue.
    queue: ConcurrentQueue<Runnable>,

    /// Local queues created by runners.
    local_queues: RwLock<Vec<Arc<ConcurrentQueue<Runnable>>>>,

    /// Set to `true` when a sleeping ticker is notified or no tickers are sleeping.
    notified: AtomicBool,

    /// A list of sleeping tickers.
    sleepers: Mutex<Sleepers>,

    /// Currently active tasks.
    active: Mutex<Slab<Waker>>,
}

impl State {
    /// Creates state for a new executor.
    const fn new() -> State {
        State {
            queue: ConcurrentQueue::unbounded(),
            local_queues: RwLock::new(Vec::new()),
            notified: AtomicBool::new(true),
            sleepers: Mutex::new(Sleepers {
                count: 0,
                wakers: Vec::new(),
                free_ids: Vec::new(),
            }),
            active: Mutex::new(Slab::new()),
        }
    }

    /// Returns a reference to currently active tasks.
    fn active(&self) -> MutexGuard<'_, Slab<Waker>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notifies a sleeping ticker.
    #[inline]
    fn notify(&self) {
        if self
            .notified
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let waker = self
                .sleepers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .notify();
            if let Some(w) = waker {
                w.wake();
            }
        }
    }

    pub(crate) fn try_tick(&self) -> bool {
        match self.queue.pop() {
            Err(_) => false,
            Ok(runnable) => {
                // Notify another ticker now to pick up where this ticker left off, just in case
                // running the task takes a long time.
                self.notify();

                // Run the task.
                runnable.run();
                true
            }
        }
    }

    pub(crate) async fn tick(&self) {
        let runnable = Ticker::new(self).runnable().await;
        runnable.run();
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        let mut runner = Runner::new(self);
        let mut rng = fastrand::Rng::new();

        // A future that runs tasks forever.
        let run_forever = async {
            loop {
                for _ in 0..200 {
                    let runnable = runner.runnable(&mut rng).await;
                    runnable.run();
                }
                future::yield_now().await;
            }
        };

        // Run `future` and `run_forever` concurrently until `future` completes.
        future.or(run_forever).await
    }
}

/// A list of sleeping tickers.
struct Sleepers {
    /// Number of sleeping tickers (both notified and unnotified).
    count: usize,

    /// IDs and wakers of sleeping unnotified tickers.
    ///
    /// A sleeping ticker is notified when its waker is missing from this list.
    wakers: Vec<(usize, Waker)>,

    /// Reclaimed IDs.
    free_ids: Vec<usize>,
}

impl Sleepers {
    /// Inserts a new sleeping ticker.
    fn insert(&mut self, waker: &Waker) -> usize {
        let id = match self.free_ids.pop() {
            Some(id) => id,
            None => self.count + 1,
        };
        self.count += 1;
        self.wakers.push((id, waker.clone()));
        id
    }

    /// Re-inserts a sleeping ticker's waker if it was notified.
    ///
    /// Returns `true` if the ticker was notified.
    fn update(&mut self, id: usize, waker: &Waker) -> bool {
        for item in &mut self.wakers {
            if item.0 == id {
                item.1.clone_from(waker);
                return false;
            }
        }

        self.wakers.push((id, waker.clone()));
        true
    }

    /// Removes a previously inserted sleeping ticker.
    ///
    /// Returns `true` if the ticker was notified.
    fn remove(&mut self, id: usize) -> bool {
        self.count -= 1;
        self.free_ids.push(id);

        for i in (0..self.wakers.len()).rev() {
            if self.wakers[i].0 == id {
                self.wakers.remove(i);
                return false;
            }
        }
        true
    }

    /// Returns `true` if a sleeping ticker is notified or no tickers are sleeping.
    fn is_notified(&self) -> bool {
        self.count == 0 || self.count > self.wakers.len()
    }

    /// Returns notification waker for a sleeping ticker.
    ///
    /// If a ticker was notified already or there are no tickers, `None` will be returned.
    fn notify(&mut self) -> Option<Waker> {
        if self.wakers.len() == self.count {
            self.wakers.pop().map(|item| item.1)
        } else {
            None
        }
    }
}

/// Runs task one by one.
struct Ticker<'a> {
    /// The executor state.
    state: &'a State,

    /// Set to a non-zero sleeper ID when in sleeping state.
    ///
    /// States a ticker can be in:
    /// 1) Woken.
    ///    2a) Sleeping and unnotified.
    ///    2b) Sleeping and notified.
    sleeping: usize,
}

impl Ticker<'_> {
    /// Creates a ticker.
    fn new(state: &State) -> Ticker<'_> {
        Ticker { state, sleeping: 0 }
    }

    /// Moves the ticker into sleeping and unnotified state.
    ///
    /// Returns `false` if the ticker was already sleeping and unnotified.
    fn sleep(&mut self, waker: &Waker) -> bool {
        let mut sleepers = self
            .state
            .sleepers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        match self.sleeping {
            // Move to sleeping state.
            0 => {
                self.sleeping = sleepers.insert(waker);
            }

            // Already sleeping, check if notified.
            id => {
                if !sleepers.update(id, waker) {
                    return false;
                }
            }
        }

        self.state
            .notified
            .store(sleepers.is_notified(), Ordering::Release);

        true
    }

    /// Moves the ticker into woken state.
    fn wake(&mut self) {
        if self.sleeping != 0 {
            let mut sleepers = self
                .state
                .sleepers
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            sleepers.remove(self.sleeping);

            self.state
                .notified
                .store(sleepers.is_notified(), Ordering::Release);
        }
        self.sleeping = 0;
    }

    /// Waits for the next runnable task to run.
    async fn runnable(&mut self) -> Runnable {
        self.runnable_with(|| self.state.queue.pop().ok()).await
    }

    /// Waits for the next runnable task to run, given a function that searches for a task.
    async fn runnable_with(&mut self, mut search: impl FnMut() -> Option<Runnable>) -> Runnable {
        future::poll_fn(|cx| {
            loop {
                match search() {
                    None => {
                        // Move to sleeping and unnotified state.
                        if !self.sleep(cx.waker()) {
                            // If already sleeping and unnotified, return.
                            return Poll::Pending;
                        }
                    }
                    Some(r) => {
                        // Wake up.
                        self.wake();

                        // Notify another ticker now to pick up where this ticker left off, just in
                        // case running the task takes a long time.
                        self.state.notify();

                        return Poll::Ready(r);
                    }
                }
            }
        })
        .await
    }
}

impl Drop for Ticker<'_> {
    fn drop(&mut self) {
        // If this ticker is in sleeping state, it must be removed from the sleepers list.
        if self.sleeping != 0 {
            let mut sleepers = self
                .state
                .sleepers
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let notified = sleepers.remove(self.sleeping);

            self.state
                .notified
                .store(sleepers.is_notified(), Ordering::Release);

            // If this ticker was notified, then notify another ticker.
            if notified {
                drop(sleepers);
                self.state.notify();
            }
        }
    }
}

/// A worker in a work-stealing executor.
///
/// This is just a ticker that also has an associated local queue for improved cache locality.
struct Runner<'a> {
    /// The executor state.
    state: &'a State,

    /// Inner ticker.
    ticker: Ticker<'a>,

    /// The local queue.
    local: Arc<ConcurrentQueue<Runnable>>,

    /// Bumped every time a runnable task is found.
    ticks: usize,
}

impl Runner<'_> {
    /// Creates a runner and registers it in the executor state.
    fn new(state: &State) -> Runner<'_> {
        let runner = Runner {
            state,
            ticker: Ticker::new(state),
            local: Arc::new(ConcurrentQueue::bounded(512)),
            ticks: 0,
        };
        state
            .local_queues
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(runner.local.clone());
        runner
    }

    /// Waits for the next runnable task to run.
    async fn runnable(&mut self, rng: &mut fastrand::Rng) -> Runnable {
        let runnable = self
            .ticker
            .runnable_with(|| {
                // Try the local queue.
                if let Ok(r) = self.local.pop() {
                    return Some(r);
                }

                // Try stealing from the global queue.
                if let Ok(r) = self.state.queue.pop() {
                    steal(&self.state.queue, &self.local);
                    return Some(r);
                }

                // Try stealing from other runners.
                if let Ok(local_queues) = self.state.local_queues.try_read() {
                    // Pick a random starting point in the iterator list and rotate the list.
                    let n = local_queues.len();
                    let start = rng.usize(..n);
                    let iter = local_queues
                        .iter()
                        .chain(local_queues.iter())
                        .skip(start)
                        .take(n);

                    // Remove this runner's local queue.
                    let iter = iter.filter(|local| !Arc::ptr_eq(local, &self.local));

                    // Try stealing from each local queue in the list.
                    for local in iter {
                        steal(local, &self.local);
                        if let Ok(r) = self.local.pop() {
                            return Some(r);
                        }
                    }
                }

                None
            })
            .await;

        // Bump the tick counter.
        self.ticks = self.ticks.wrapping_add(1);

        if self.ticks % 64 == 0 {
            // Steal tasks from the global queue to ensure fair task scheduling.
            steal(&self.state.queue, &self.local);
        }

        runnable
    }
}

impl Drop for Runner<'_> {
    fn drop(&mut self) {
        // Remove the local queue.
        self.state
            .local_queues
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|local| !Arc::ptr_eq(local, &self.local));

        // Re-schedule remaining tasks in the local queue.
        while let Ok(r) = self.local.pop() {
            r.schedule();
        }
    }
}

/// Steals some items from one queue into another.
fn steal<T>(src: &ConcurrentQueue<T>, dest: &ConcurrentQueue<T>) {
    // Half of `src`'s length rounded up.
    let mut count = (src.len() + 1) / 2;

    if count > 0 {
        // Don't steal more than fits into the queue.
        if let Some(cap) = dest.capacity() {
            count = count.min(cap - dest.len());
        }

        // Steal tasks.
        for _ in 0..count {
            if let Ok(t) = src.pop() {
                assert!(dest.push(t).is_ok());
            } else {
                break;
            }
        }
    }
}

/// Debug implementation for `Executor` and `LocalExecutor`.
fn debug_executor(executor: &Executor<'_>, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // Get a reference to the state.
    let ptr = executor.state.load(Ordering::Acquire);
    if ptr.is_null() {
        // The executor has not been initialized.
        struct Uninitialized;

        impl fmt::Debug for Uninitialized {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<uninitialized>")
            }
        }

        return f.debug_tuple(name).field(&Uninitialized).finish();
    }

    // SAFETY: If the state pointer is not null, it must have been
    // allocated properly by Arc::new and converted via Arc::into_raw
    // in state_ptr.
    let state = unsafe { &*ptr };

    debug_state(state, name, f)
}

/// Debug implementation for `Executor` and `LocalExecutor`.
fn debug_state(state: &State, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// Debug wrapper for the number of active tasks.
    struct ActiveTasks<'a>(&'a Mutex<Slab<Waker>>);

    impl fmt::Debug for ActiveTasks<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.0.try_lock() {
                Ok(lock) => fmt::Debug::fmt(&lock.len(), f),
                Err(TryLockError::WouldBlock) => f.write_str("<locked>"),
                Err(TryLockError::Poisoned(err)) => fmt::Debug::fmt(&err.into_inner().len(), f),
            }
        }
    }

    /// Debug wrapper for the local runners.
    struct LocalRunners<'a>(&'a RwLock<Vec<Arc<ConcurrentQueue<Runnable>>>>);

    impl fmt::Debug for LocalRunners<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.0.try_read() {
                Ok(lock) => f
                    .debug_list()
                    .entries(lock.iter().map(|queue| queue.len()))
                    .finish(),
                Err(TryLockError::WouldBlock) => f.write_str("<locked>"),
                Err(TryLockError::Poisoned(_)) => f.write_str("<poisoned>"),
            }
        }
    }

    /// Debug wrapper for the sleepers.
    struct SleepCount<'a>(&'a Mutex<Sleepers>);

    impl fmt::Debug for SleepCount<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.0.try_lock() {
                Ok(lock) => fmt::Debug::fmt(&lock.count, f),
                Err(TryLockError::WouldBlock) => f.write_str("<locked>"),
                Err(TryLockError::Poisoned(_)) => f.write_str("<poisoned>"),
            }
        }
    }

    f.debug_struct(name)
        .field("active", &ActiveTasks(&state.active))
        .field("global_tasks", &state.queue.len())
        .field("local_runners", &LocalRunners(&state.local_queues))
        .field("sleepers", &SleepCount(&state.sleepers))
        .finish()
}

/// Runs a closure when dropped.
struct CallOnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

pin_project! {
    /// A wrapper around a future, running a closure when dropped.
    struct AsyncCallOnDrop<Fut, Cleanup: FnMut()> {
        #[pin]
        future: Fut,
        cleanup: CallOnDrop<Cleanup>,
    }
}

impl<Fut, Cleanup: FnMut()> AsyncCallOnDrop<Fut, Cleanup> {
    fn new(future: Fut, cleanup: Cleanup) -> Self {
        Self {
            future,
            cleanup: CallOnDrop(cleanup),
        }
    }
}

impl<Fut: Future, Cleanup: FnMut()> Future for AsyncCallOnDrop<Fut, Cleanup> {
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}

fn _ensure_send_and_sync() {
    use futures_lite::future::pending;

    fn is_send<T: Send>(_: T) {}
    fn is_sync<T: Sync>(_: T) {}
    fn is_static<T: 'static>(_: T) {}

    is_send::<Executor<'_>>(Executor::new());
    is_sync::<Executor<'_>>(Executor::new());

    let ex = Executor::new();
    is_send(ex.run(pending::<()>()));
    is_sync(ex.run(pending::<()>()));
    is_send(ex.tick());
    is_sync(ex.tick());
    is_send(ex.schedule());
    is_sync(ex.schedule());
    is_static(ex.schedule());

    /// ```compile_fail
    /// use async_executor::LocalExecutor;
    /// use futures_lite::future::pending;
    ///
    /// fn is_send<T: Send>(_: T) {}
    /// fn is_sync<T: Sync>(_: T) {}
    ///
    /// is_send::<LocalExecutor<'_>>(LocalExecutor::new());
    /// is_sync::<LocalExecutor<'_>>(LocalExecutor::new());
    ///
    /// let ex = LocalExecutor::new();
    /// is_send(ex.run(pending::<()>()));
    /// is_sync(ex.run(pending::<()>()));
    /// is_send(ex.tick());
    /// is_sync(ex.tick());
    /// ```
    fn _negative_test() {}
}
