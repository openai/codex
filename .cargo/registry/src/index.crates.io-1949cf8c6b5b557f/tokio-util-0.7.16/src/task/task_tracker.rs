//! Types related to the [`TaskTracker`] collection.
//!
//! See the documentation of [`TaskTracker`] for more information.

use pin_project_lite::pin_project;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{futures::Notified, Notify};

#[cfg(feature = "rt")]
use tokio::{
    runtime::Handle,
    task::{JoinHandle, LocalSet},
};

/// A task tracker used for waiting until tasks exit.
///
/// This is usually used together with [`CancellationToken`] to implement [graceful shutdown]. The
/// `CancellationToken` is used to signal to tasks that they should shut down, and the
/// `TaskTracker` is used to wait for them to finish shutting down.
///
/// The `TaskTracker` will also keep track of a `closed` boolean. This is used to handle the case
/// where the `TaskTracker` is empty, but we don't want to shut down yet. This means that the
/// [`wait`] method will wait until *both* of the following happen at the same time:
///
///  * The `TaskTracker` must be closed using the [`close`] method.
///  * The `TaskTracker` must be empty, that is, all tasks that it is tracking must have exited.
///
/// When a call to [`wait`] returns, it is guaranteed that all tracked tasks have exited and that
/// the destructor of the future has finished running. However, there might be a short amount of
/// time where [`JoinHandle::is_finished`] returns false.
///
/// # Comparison to `JoinSet`
///
/// The main Tokio crate has a similar collection known as [`JoinSet`]. The `JoinSet` type has a
/// lot more features than `TaskTracker`, so `TaskTracker` should only be used when one of its
/// unique features is required:
///
///  1. When tasks exit, a `TaskTracker` will allow the task to immediately free its memory.
///  2. By not closing the `TaskTracker`, [`wait`] will be prevented from returning even if
///     the `TaskTracker` is empty.
///  3. A `TaskTracker` does not require mutable access to insert tasks.
///  4. A `TaskTracker` can be cloned to share it with many tasks.
///
/// The first point is the most important one. A [`JoinSet`] keeps track of the return value of
/// every inserted task. This means that if the caller keeps inserting tasks and never calls
/// [`join_next`], then their return values will keep building up and consuming memory, _even if_
/// most of the tasks have already exited. This can cause the process to run out of memory. With a
/// `TaskTracker`, this does not happen. Once tasks exit, they are immediately removed from the
/// `TaskTracker`.
///
/// Note that unlike [`JoinSet`], dropping a `TaskTracker` does not abort the tasks.
///
/// # Examples
///
/// For more examples, please see the topic page on [graceful shutdown].
///
/// ## Spawn tasks and wait for them to exit
///
/// This is a simple example. For this case, [`JoinSet`] should probably be used instead.
///
/// ```
/// use tokio_util::task::TaskTracker;
///
/// #[tokio::main]
/// async fn main() {
///     let tracker = TaskTracker::new();
///
///     for i in 0..10 {
///         tracker.spawn(async move {
///             println!("Task {} is running!", i);
///         });
///     }
///     // Once we spawned everything, we close the tracker.
///     tracker.close();
///
///     // Wait for everything to finish.
///     tracker.wait().await;
///
///     println!("This is printed after all of the tasks.");
/// }
/// ```
///
/// ## Wait for tasks to exit
///
/// This example shows the intended use-case of `TaskTracker`. It is used together with
/// [`CancellationToken`] to implement graceful shutdown.
/// ```
/// use tokio_util::sync::CancellationToken;
/// use tokio_util::task::TaskTracker;
/// use tokio::time::{self, Duration};
///
/// async fn background_task(num: u64) {
///     for i in 0..10 {
///         time::sleep(Duration::from_millis(100*num)).await;
///         println!("Background task {} in iteration {}.", num, i);
///     }
/// }
///
/// #[tokio::main]
/// # async fn _hidden() {}
/// # #[tokio::main(flavor = "current_thread", start_paused = true)]
/// async fn main() {
///     let tracker = TaskTracker::new();
///     let token = CancellationToken::new();
///
///     for i in 0..10 {
///         let token = token.clone();
///         tracker.spawn(async move {
///             // Use a `tokio::select!` to kill the background task if the token is
///             // cancelled.
///             tokio::select! {
///                 () = background_task(i) => {
///                     println!("Task {} exiting normally.", i);
///                 },
///                 () = token.cancelled() => {
///                     // Do some cleanup before we really exit.
///                     time::sleep(Duration::from_millis(50)).await;
///                     println!("Task {} finished cleanup.", i);
///                 },
///             }
///         });
///     }
///
///     // Spawn a background task that will send the shutdown signal.
///     {
///         let tracker = tracker.clone();
///         tokio::spawn(async move {
///             // Normally you would use something like ctrl-c instead of
///             // sleeping.
///             time::sleep(Duration::from_secs(2)).await;
///             tracker.close();
///             token.cancel();
///         });
///     }
///
///     // Wait for all tasks to exit.
///     tracker.wait().await;
///
///     println!("All tasks have exited now.");
/// }
/// ```
///
/// [`CancellationToken`]: crate::sync::CancellationToken
/// [`JoinHandle::is_finished`]: tokio::task::JoinHandle::is_finished
/// [`JoinSet`]: tokio::task::JoinSet
/// [`close`]: Self::close
/// [`join_next`]: tokio::task::JoinSet::join_next
/// [`wait`]: Self::wait
/// [graceful shutdown]: https://tokio.rs/tokio/topics/shutdown
pub struct TaskTracker {
    inner: Arc<TaskTrackerInner>,
}

/// Represents a task tracked by a [`TaskTracker`].
#[must_use]
#[derive(Debug)]
pub struct TaskTrackerToken {
    task_tracker: TaskTracker,
}

struct TaskTrackerInner {
    /// Keeps track of the state.
    ///
    /// The lowest bit is whether the task tracker is closed.
    ///
    /// The rest of the bits count the number of tracked tasks.
    state: AtomicUsize,
    /// Used to notify when the last task exits.
    on_last_exit: Notify,
}

pin_project! {
    /// A future that is tracked as a task by a [`TaskTracker`].
    ///
    /// The associated [`TaskTracker`] cannot complete until this future is dropped.
    ///
    /// This future is returned by [`TaskTracker::track_future`].
    #[must_use = "futures do nothing unless polled"]
    pub struct TrackedFuture<F> {
        #[pin]
        future: F,
        token: TaskTrackerToken,
    }
}

pin_project! {
    /// A future that completes when the [`TaskTracker`] is empty and closed.
    ///
    /// This future is returned by [`TaskTracker::wait`].
    #[must_use = "futures do nothing unless polled"]
    pub struct TaskTrackerWaitFuture<'a> {
        #[pin]
        future: Notified<'a>,
        inner: Option<&'a TaskTrackerInner>,
    }
}

impl TaskTrackerInner {
    #[inline]
    fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            on_last_exit: Notify::new(),
        }
    }

    #[inline]
    fn is_closed_and_empty(&self) -> bool {
        // If empty and closed bit set, then we are done.
        //
        // The acquire load will synchronize with the release store of any previous call to
        // `set_closed` and `drop_task`.
        self.state.load(Ordering::Acquire) == 1
    }

    #[inline]
    fn set_closed(&self) -> bool {
        // The AcqRel ordering makes the closed bit behave like a `Mutex<bool>` for synchronization
        // purposes. We do this because it makes the return value of `TaskTracker::{close,reopen}`
        // more meaningful for the user. Without these orderings, this assert could fail:
        // ```
        // // thread 1
        // some_other_atomic.store(true, Relaxed);
        // tracker.close();
        //
        // // thread 2
        // if tracker.reopen() {
        //     assert!(some_other_atomic.load(Relaxed));
        // }
        // ```
        // However, with the AcqRel ordering, we establish a happens-before relationship from the
        // call to `close` and the later call to `reopen` that returned true.
        let state = self.state.fetch_or(1, Ordering::AcqRel);

        // If there are no tasks, and if it was not already closed:
        if state == 0 {
            self.notify_now();
        }

        (state & 1) == 0
    }

    #[inline]
    fn set_open(&self) -> bool {
        // See `set_closed` regarding the AcqRel ordering.
        let state = self.state.fetch_and(!1, Ordering::AcqRel);
        (state & 1) == 1
    }

    #[inline]
    fn add_task(&self) {
        self.state.fetch_add(2, Ordering::Relaxed);
    }

    #[inline]
    fn drop_task(&self) {
        let state = self.state.fetch_sub(2, Ordering::Release);

        // If this was the last task and we are closed:
        if state == 3 {
            self.notify_now();
        }
    }

    #[cold]
    fn notify_now(&self) {
        // Insert an acquire fence. This matters for `drop_task` but doesn't matter for
        // `set_closed` since it already uses AcqRel.
        //
        // This synchronizes with the release store of any other call to `drop_task`, and with the
        // release store in the call to `set_closed`. That ensures that everything that happened
        // before those other calls to `drop_task` or `set_closed` will be visible after this load,
        // and those things will also be visible to anything woken by the call to `notify_waiters`.
        self.state.load(Ordering::Acquire);

        self.on_last_exit.notify_waiters();
    }
}

impl TaskTracker {
    /// Creates a new `TaskTracker`.
    ///
    /// The `TaskTracker` will start out as open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TaskTrackerInner::new()),
        }
    }

    /// Waits until this `TaskTracker` is both closed and empty.
    ///
    /// If the `TaskTracker` is already closed and empty when this method is called, then it
    /// returns immediately.
    ///
    /// The `wait` future is resistant against [ABA problems][aba]. That is, if the `TaskTracker`
    /// becomes both closed and empty for a short amount of time, then it is guarantee that all
    /// `wait` futures that were created before the short time interval will trigger, even if they
    /// are not polled during that short time interval.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe.
    ///
    /// However, the resistance against [ABA problems][aba] is lost when using `wait` as the
    /// condition in a `tokio::select!` loop.
    ///
    /// [aba]: https://en.wikipedia.org/wiki/ABA_problem
    #[inline]
    pub fn wait(&self) -> TaskTrackerWaitFuture<'_> {
        TaskTrackerWaitFuture {
            future: self.inner.on_last_exit.notified(),
            inner: if self.inner.is_closed_and_empty() {
                None
            } else {
                Some(&self.inner)
            },
        }
    }

    /// Close this `TaskTracker`.
    ///
    /// This allows [`wait`] futures to complete. It does not prevent you from spawning new tasks.
    ///
    /// Returns `true` if this closed the `TaskTracker`, or `false` if it was already closed.
    ///
    /// [`wait`]: Self::wait
    #[inline]
    pub fn close(&self) -> bool {
        self.inner.set_closed()
    }

    /// Reopen this `TaskTracker`.
    ///
    /// This prevents [`wait`] futures from completing even if the `TaskTracker` is empty.
    ///
    /// Returns `true` if this reopened the `TaskTracker`, or `false` if it was already open.
    ///
    /// [`wait`]: Self::wait
    #[inline]
    pub fn reopen(&self) -> bool {
        self.inner.set_open()
    }

    /// Returns `true` if this `TaskTracker` is [closed](Self::close).
    #[inline]
    #[must_use]
    pub fn is_closed(&self) -> bool {
        (self.inner.state.load(Ordering::Acquire) & 1) != 0
    }

    /// Returns the number of tasks tracked by this `TaskTracker`.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.state.load(Ordering::Acquire) >> 1
    }

    /// Returns `true` if there are no tasks in this `TaskTracker`.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.state.load(Ordering::Acquire) <= 1
    }

    /// Spawn the provided future on the current Tokio runtime, and track it in this `TaskTracker`.
    ///
    /// This is equivalent to `tokio::spawn(tracker.track_future(task))`.
    #[inline]
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn spawn<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::task::spawn(self.track_future(task))
    }

    /// Spawn the provided future on the provided Tokio runtime, and track it in this `TaskTracker`.
    ///
    /// This is equivalent to `handle.spawn(tracker.track_future(task))`.
    #[inline]
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn spawn_on<F>(&self, task: F, handle: &Handle) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        handle.spawn(self.track_future(task))
    }

    /// Spawn the provided future on the current [`LocalSet`], and track it in this `TaskTracker`.
    ///
    /// This is equivalent to `tokio::task::spawn_local(tracker.track_future(task))`.
    ///
    /// [`LocalSet`]: tokio::task::LocalSet
    #[inline]
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn spawn_local<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        tokio::task::spawn_local(self.track_future(task))
    }

    /// Spawn the provided future on the provided [`LocalSet`], and track it in this `TaskTracker`.
    ///
    /// This is equivalent to `local_set.spawn_local(tracker.track_future(task))`.
    ///
    /// [`LocalSet`]: tokio::task::LocalSet
    #[inline]
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn spawn_local_on<F>(&self, task: F, local_set: &LocalSet) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        local_set.spawn_local(self.track_future(task))
    }

    /// Spawn the provided blocking task on the current Tokio runtime, and track it in this `TaskTracker`.
    ///
    /// This is equivalent to `tokio::task::spawn_blocking(tracker.track_future(task))`.
    #[inline]
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg(not(target_family = "wasm"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn spawn_blocking<F, T>(&self, task: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let token = self.token();
        tokio::task::spawn_blocking(move || {
            let res = task();
            drop(token);
            res
        })
    }

    /// Spawn the provided blocking task on the provided Tokio runtime, and track it in this `TaskTracker`.
    ///
    /// This is equivalent to `handle.spawn_blocking(tracker.track_future(task))`.
    #[inline]
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg(not(target_family = "wasm"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn spawn_blocking_on<F, T>(&self, task: F, handle: &Handle) -> JoinHandle<T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let token = self.token();
        handle.spawn_blocking(move || {
            let res = task();
            drop(token);
            res
        })
    }

    /// Track the provided future.
    ///
    /// The returned [`TrackedFuture`] will count as a task tracked by this collection, and will
    /// prevent calls to [`wait`] from returning until the task is dropped.
    ///
    /// The task is removed from the collection when it is dropped, not when [`poll`] returns
    /// [`Poll::Ready`].
    ///
    /// # Examples
    ///
    /// Track a future spawned with [`tokio::spawn`].
    ///
    /// ```
    /// # async fn my_async_fn() {}
    /// use tokio_util::task::TaskTracker;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let tracker = TaskTracker::new();
    ///
    /// tokio::spawn(tracker.track_future(my_async_fn()));
    /// # }
    /// ```
    ///
    /// Track a future spawned on a [`JoinSet`].
    /// ```
    /// # async fn my_async_fn() {}
    /// use tokio::task::JoinSet;
    /// use tokio_util::task::TaskTracker;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let tracker = TaskTracker::new();
    /// let mut join_set = JoinSet::new();
    ///
    /// join_set.spawn(tracker.track_future(my_async_fn()));
    /// # }
    /// ```
    ///
    /// [`JoinSet`]: tokio::task::JoinSet
    /// [`Poll::Pending`]: std::task::Poll::Pending
    /// [`poll`]: std::future::Future::poll
    /// [`wait`]: Self::wait
    #[inline]
    pub fn track_future<F: Future>(&self, future: F) -> TrackedFuture<F> {
        TrackedFuture {
            future,
            token: self.token(),
        }
    }

    /// Creates a [`TaskTrackerToken`] representing a task tracked by this `TaskTracker`.
    ///
    /// This token is a lower-level utility than the spawn methods. Each token is considered to
    /// correspond to a task. As long as the token exists, the `TaskTracker` cannot complete.
    /// Furthermore, the count returned by the [`len`] method will include the tokens in the count.
    ///
    /// Dropping the token indicates to the `TaskTracker` that the task has exited.
    ///
    /// [`len`]: TaskTracker::len
    #[inline]
    pub fn token(&self) -> TaskTrackerToken {
        self.inner.add_task();
        TaskTrackerToken {
            task_tracker: self.clone(),
        }
    }

    /// Returns `true` if both task trackers correspond to the same set of tasks.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_util::task::TaskTracker;
    ///
    /// let tracker_1 = TaskTracker::new();
    /// let tracker_2 = TaskTracker::new();
    /// let tracker_1_clone = tracker_1.clone();
    ///
    /// assert!(TaskTracker::ptr_eq(&tracker_1, &tracker_1_clone));
    /// assert!(!TaskTracker::ptr_eq(&tracker_1, &tracker_2));
    /// ```
    #[inline]
    #[must_use]
    pub fn ptr_eq(left: &TaskTracker, right: &TaskTracker) -> bool {
        Arc::ptr_eq(&left.inner, &right.inner)
    }
}

impl Default for TaskTracker {
    /// Creates a new `TaskTracker`.
    ///
    /// The `TaskTracker` will start out as open.
    #[inline]
    fn default() -> TaskTracker {
        TaskTracker::new()
    }
}

impl Clone for TaskTracker {
    /// Returns a new `TaskTracker` that tracks the same set of tasks.
    ///
    /// Since the new `TaskTracker` shares the same set of tasks, changes to one set are visible in
    /// all other clones.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_util::task::TaskTracker;
    ///
    /// #[tokio::main]
    /// # async fn _hidden() {}
    /// # #[tokio::main(flavor = "current_thread")]
    /// async fn main() {
    ///     let tracker = TaskTracker::new();
    ///     let cloned = tracker.clone();
    ///
    ///     // Spawns on `tracker` are visible in `cloned`.
    ///     tracker.spawn(std::future::pending::<()>());
    ///     assert_eq!(cloned.len(), 1);
    ///
    ///     // Spawns on `cloned` are visible in `tracker`.
    ///     cloned.spawn(std::future::pending::<()>());
    ///     assert_eq!(tracker.len(), 2);
    ///
    ///     // Calling `close` is visible to `cloned`.
    ///     tracker.close();
    ///     assert!(cloned.is_closed());
    ///
    ///     // Calling `reopen` is visible to `tracker`.
    ///     cloned.reopen();
    ///     assert!(!tracker.is_closed());
    /// }
    /// ```
    #[inline]
    fn clone(&self) -> TaskTracker {
        Self {
            inner: self.inner.clone(),
        }
    }
}

fn debug_inner(inner: &TaskTrackerInner, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let state = inner.state.load(Ordering::Acquire);
    let is_closed = (state & 1) != 0;
    let len = state >> 1;

    f.debug_struct("TaskTracker")
        .field("len", &len)
        .field("is_closed", &is_closed)
        .field("inner", &(inner as *const TaskTrackerInner))
        .finish()
}

impl fmt::Debug for TaskTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_inner(&self.inner, f)
    }
}

impl TaskTrackerToken {
    /// Returns the [`TaskTracker`] that this token is associated with.
    #[inline]
    #[must_use]
    pub fn task_tracker(&self) -> &TaskTracker {
        &self.task_tracker
    }
}

impl Clone for TaskTrackerToken {
    /// Returns a new `TaskTrackerToken` associated with the same [`TaskTracker`].
    ///
    /// This is equivalent to `token.task_tracker().token()`.
    #[inline]
    fn clone(&self) -> TaskTrackerToken {
        self.task_tracker.token()
    }
}

impl Drop for TaskTrackerToken {
    /// Dropping the token indicates to the [`TaskTracker`] that the task has exited.
    #[inline]
    fn drop(&mut self) {
        self.task_tracker.inner.drop_task();
    }
}

impl<F: Future> Future for TrackedFuture<F> {
    type Output = F::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        self.project().future.poll(cx)
    }
}

impl<F: fmt::Debug> fmt::Debug for TrackedFuture<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrackedFuture")
            .field("future", &self.future)
            .field("task_tracker", self.token.task_tracker())
            .finish()
    }
}

impl<'a> Future for TaskTrackerWaitFuture<'a> {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let me = self.project();

        let inner = match me.inner.as_ref() {
            None => return Poll::Ready(()),
            Some(inner) => inner,
        };

        let ready = inner.is_closed_and_empty() || me.future.poll(cx).is_ready();
        if ready {
            *me.inner = None;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl<'a> fmt::Debug for TaskTrackerWaitFuture<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Helper<'a>(&'a TaskTrackerInner);

        impl fmt::Debug for Helper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                debug_inner(self.0, f)
            }
        }

        f.debug_struct("TaskTrackerWaitFuture")
            .field("future", &self.future)
            .field("task_tracker", &self.inner.map(Helper))
            .finish()
    }
}
