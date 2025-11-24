//! Runs `!Send` futures on the current thread.
use crate::loom::cell::UnsafeCell;
use crate::loom::sync::{Arc, Mutex};
#[cfg(tokio_unstable)]
use crate::runtime;
use crate::runtime::task::{
    self, JoinHandle, LocalOwnedTasks, SpawnLocation, Task, TaskHarnessScheduleHooks,
};
use crate::runtime::{context, ThreadId, BOX_FUTURE_THRESHOLD};
use crate::sync::AtomicWaker;
use crate::util::trace::SpawnMeta;
use crate::util::RcCell;

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::mem;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Poll;

use pin_project_lite::pin_project;

cfg_rt! {
    /// A set of tasks which are executed on the same thread.
    ///
    /// In some cases, it is necessary to run one or more futures that do not
    /// implement [`Send`] and thus are unsafe to send between threads. In these
    /// cases, a [local task set] may be used to schedule one or more `!Send`
    /// futures to run together on the same thread.
    ///
    /// For example, the following code will not compile:
    ///
    /// ```rust,compile_fail
    /// use std::rc::Rc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // `Rc` does not implement `Send`, and thus may not be sent between
    ///     // threads safely.
    ///     let nonsend_data = Rc::new("my nonsend data...");
    ///
    ///     let nonsend_data = nonsend_data.clone();
    ///     // Because the `async` block here moves `nonsend_data`, the future is `!Send`.
    ///     // Since `tokio::spawn` requires the spawned future to implement `Send`, this
    ///     // will not compile.
    ///     tokio::spawn(async move {
    ///         println!("{}", nonsend_data);
    ///         // ...
    ///     }).await.unwrap();
    /// }
    /// ```
    ///
    /// # Use with `run_until`
    ///
    /// To spawn `!Send` futures, we can use a local task set to schedule them
    /// on the thread calling [`Runtime::block_on`]. When running inside of the
    /// local task set, we can use [`task::spawn_local`], which can spawn
    /// `!Send` futures. For example:
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use tokio::task;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let nonsend_data = Rc::new("my nonsend data...");
    ///
    ///     // Construct a local task set that can run `!Send` futures.
    ///     let local = task::LocalSet::new();
    ///
    ///     // Run the local task set.
    ///     local.run_until(async move {
    ///         let nonsend_data = nonsend_data.clone();
    ///         // `spawn_local` ensures that the future is spawned on the local
    ///         // task set.
    ///         task::spawn_local(async move {
    ///             println!("{}", nonsend_data);
    ///             // ...
    ///         }).await.unwrap();
    ///     }).await;
    /// }
    /// ```
    /// **Note:** The `run_until` method can only be used in `#[tokio::main]`,
    /// `#[tokio::test]` or directly inside a call to [`Runtime::block_on`]. It
    /// cannot be used inside a task spawned with `tokio::spawn`.
    ///
    /// ## Awaiting a `LocalSet`
    ///
    /// Additionally, a `LocalSet` itself implements `Future`, completing when
    /// *all* tasks spawned on the `LocalSet` complete. This can be used to run
    /// several futures on a `LocalSet` and drive the whole set until they
    /// complete. For example,
    ///
    /// ```rust
    /// use tokio::{task, time};
    /// use std::rc::Rc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let nonsend_data = Rc::new("world");
    ///     let local = task::LocalSet::new();
    ///
    ///     let nonsend_data2 = nonsend_data.clone();
    ///     local.spawn_local(async move {
    ///         // ...
    ///         println!("hello {}", nonsend_data2)
    ///     });
    ///
    ///     local.spawn_local(async move {
    ///         time::sleep(time::Duration::from_millis(100)).await;
    ///         println!("goodbye {}", nonsend_data)
    ///     });
    ///
    ///     // ...
    ///
    ///     local.await;
    /// }
    /// ```
    /// **Note:** Awaiting a `LocalSet` can only be done inside
    /// `#[tokio::main]`, `#[tokio::test]` or directly inside a call to
    /// [`Runtime::block_on`]. It cannot be used inside a task spawned with
    /// `tokio::spawn`.
    ///
    /// ## Use inside `tokio::spawn`
    ///
    /// The two methods mentioned above cannot be used inside `tokio::spawn`, so
    /// to spawn `!Send` futures from inside `tokio::spawn`, we need to do
    /// something else. The solution is to create the `LocalSet` somewhere else,
    /// and communicate with it using an [`mpsc`] channel.
    ///
    /// The following example puts the `LocalSet` inside a new thread.
    /// ```
    /// use tokio::runtime::Builder;
    /// use tokio::sync::{mpsc, oneshot};
    /// use tokio::task::LocalSet;
    ///
    /// // This struct describes the task you want to spawn. Here we include
    /// // some simple examples. The oneshot channel allows sending a response
    /// // to the spawner.
    /// #[derive(Debug)]
    /// enum Task {
    ///     PrintNumber(u32),
    ///     AddOne(u32, oneshot::Sender<u32>),
    /// }
    ///
    /// #[derive(Clone)]
    /// struct LocalSpawner {
    ///    send: mpsc::UnboundedSender<Task>,
    /// }
    ///
    /// impl LocalSpawner {
    ///     pub fn new() -> Self {
    ///         let (send, mut recv) = mpsc::unbounded_channel();
    ///
    ///         let rt = Builder::new_current_thread()
    ///             .enable_all()
    ///             .build()
    ///             .unwrap();
    ///
    ///         std::thread::spawn(move || {
    ///             let local = LocalSet::new();
    ///
    ///             local.spawn_local(async move {
    ///                 while let Some(new_task) = recv.recv().await {
    ///                     tokio::task::spawn_local(run_task(new_task));
    ///                 }
    ///                 // If the while loop returns, then all the LocalSpawner
    ///                 // objects have been dropped.
    ///             });
    ///
    ///             // This will return once all senders are dropped and all
    ///             // spawned tasks have returned.
    ///             rt.block_on(local);
    ///         });
    ///
    ///         Self {
    ///             send,
    ///         }
    ///     }
    ///
    ///     pub fn spawn(&self, task: Task) {
    ///         self.send.send(task).expect("Thread with LocalSet has shut down.");
    ///     }
    /// }
    ///
    /// // This task may do !Send stuff. We use printing a number as an example,
    /// // but it could be anything.
    /// //
    /// // The Task struct is an enum to support spawning many different kinds
    /// // of operations.
    /// async fn run_task(task: Task) {
    ///     match task {
    ///         Task::PrintNumber(n) => {
    ///             println!("{}", n);
    ///         },
    ///         Task::AddOne(n, response) => {
    ///             // We ignore failures to send the response.
    ///             let _ = response.send(n + 1);
    ///         },
    ///     }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let spawner = LocalSpawner::new();
    ///
    ///     let (send, response) = oneshot::channel();
    ///     spawner.spawn(Task::AddOne(10, send));
    ///     let eleven = response.await.unwrap();
    ///     assert_eq!(eleven, 11);
    /// }
    /// ```
    ///
    /// [`Send`]: trait@std::marker::Send
    /// [local task set]: struct@LocalSet
    /// [`Runtime::block_on`]: method@crate::runtime::Runtime::block_on
    /// [`task::spawn_local`]: fn@spawn_local
    /// [`mpsc`]: mod@crate::sync::mpsc
    pub struct LocalSet {
        /// Current scheduler tick.
        tick: Cell<u8>,

        /// State available from thread-local.
        context: Rc<Context>,

        /// This type should not be Send.
        _not_send: PhantomData<*const ()>,
    }
}

/// State available from the thread-local.
struct Context {
    /// State shared between threads.
    shared: Arc<Shared>,

    /// True if a task panicked without being handled and the local set is
    /// configured to shutdown on unhandled panic.
    unhandled_panic: Cell<bool>,
}

/// `LocalSet` state shared between threads.
struct Shared {
    /// # Safety
    ///
    /// This field must *only* be accessed from the thread that owns the
    /// `LocalSet` (i.e., `Thread::current().id() == owner`).
    local_state: LocalState,

    /// Remote run queue sender.
    queue: Mutex<Option<VecDeque<task::Notified<Arc<Shared>>>>>,

    /// Wake the `LocalSet` task.
    waker: AtomicWaker,

    /// How to respond to unhandled task panics.
    #[cfg(tokio_unstable)]
    pub(crate) unhandled_panic: crate::runtime::UnhandledPanic,
}

/// Tracks the `LocalSet` state that must only be accessed from the thread that
/// created the `LocalSet`.
struct LocalState {
    /// The `ThreadId` of the thread that owns the `LocalSet`.
    owner: ThreadId,

    /// Local run queue sender and receiver.
    local_queue: UnsafeCell<VecDeque<task::Notified<Arc<Shared>>>>,

    /// Collection of all active tasks spawned onto this executor.
    owned: LocalOwnedTasks<Arc<Shared>>,
}

pin_project! {
    #[derive(Debug)]
    struct RunUntil<'a, F> {
        local_set: &'a LocalSet,
        #[pin]
        future: F,
    }
}

tokio_thread_local!(static CURRENT: LocalData = const { LocalData {
    ctx: RcCell::new(),
    wake_on_schedule: Cell::new(false),
} });

struct LocalData {
    ctx: RcCell<Context>,
    wake_on_schedule: Cell<bool>,
}

impl LocalData {
    /// Should be called except when we call `LocalSet::enter`.
    /// Especially when we poll a `LocalSet`.
    #[must_use = "dropping this guard will reset the entered state"]
    fn enter(&self, ctx: Rc<Context>) -> LocalDataEnterGuard<'_> {
        let ctx = self.ctx.replace(Some(ctx));
        let wake_on_schedule = self.wake_on_schedule.replace(false);
        LocalDataEnterGuard {
            local_data_ref: self,
            ctx,
            wake_on_schedule,
        }
    }
}

/// A guard for `LocalData::enter()`
struct LocalDataEnterGuard<'a> {
    local_data_ref: &'a LocalData,
    ctx: Option<Rc<Context>>,
    wake_on_schedule: bool,
}

impl<'a> Drop for LocalDataEnterGuard<'a> {
    fn drop(&mut self) {
        self.local_data_ref.ctx.set(self.ctx.take());
        self.local_data_ref
            .wake_on_schedule
            .set(self.wake_on_schedule)
    }
}

cfg_rt! {
    /// Spawns a `!Send` future on the current [`LocalSet`] or [`LocalRuntime`].
    ///
    /// The spawned future will run on the same thread that called `spawn_local`.
    ///
    /// The provided future will start running in the background immediately
    /// when `spawn_local` is called, even if you don't await the returned
    /// `JoinHandle`.
    ///
    /// # Panics
    ///
    /// This function panics if called outside of a [`LocalSet`].
    ///
    /// Note that if [`tokio::spawn`] is used from within a `LocalSet`, the
    /// resulting new task will _not_ be inside the `LocalSet`, so you must use
    /// `spawn_local` if you want to stay within the `LocalSet`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use tokio::task;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let nonsend_data = Rc::new("my nonsend data...");
    ///
    ///     let local = task::LocalSet::new();
    ///
    ///     // Run the local task set.
    ///     local.run_until(async move {
    ///         let nonsend_data = nonsend_data.clone();
    ///         task::spawn_local(async move {
    ///             println!("{}", nonsend_data);
    ///             // ...
    ///         }).await.unwrap();
    ///     }).await;
    /// }
    /// ```
    ///
    /// [`LocalSet`]: struct@crate::task::LocalSet
    /// [`LocalRuntime`]: struct@crate::runtime::LocalRuntime
    /// [`tokio::spawn`]: fn@crate::task::spawn
    #[track_caller]
    pub fn spawn_local<F>(future: F) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let fut_size = std::mem::size_of::<F>();
        if fut_size > BOX_FUTURE_THRESHOLD {
            spawn_local_inner(Box::pin(future), SpawnMeta::new_unnamed(fut_size))
        } else {
            spawn_local_inner(future, SpawnMeta::new_unnamed(fut_size))
        }
    }


    #[track_caller]
    pub(super) fn spawn_local_inner<F>(future: F, meta: SpawnMeta<'_>) -> JoinHandle<F::Output>
    where F: Future + 'static,
          F::Output: 'static
    {
        use crate::runtime::{context, task};

        let mut future = Some(future);

        let res = context::with_current(|handle| {
            Some(if handle.is_local() {
                if !handle.can_spawn_local_on_local_runtime() {
                    return None;
                }

                let future = future.take().unwrap();

                #[cfg(all(
                    tokio_unstable,
                    tokio_taskdump,
                    feature = "rt",
                    target_os = "linux",
                    any(
                        target_arch = "aarch64",
                        target_arch = "x86",
                        target_arch = "x86_64"
                    )
                ))]
                let future = task::trace::Trace::root(future);
                let id = task::Id::next();
                let task = crate::util::trace::task(future, "task", meta, id.as_u64());

                // safety: we have verified that this is a `LocalRuntime` owned by the current thread
                unsafe { handle.spawn_local(task, id, meta.spawned_at) }
            } else {
                match CURRENT.with(|LocalData { ctx, .. }| ctx.get()) {
                    None => panic!("`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime"),
                    Some(cx) => cx.spawn(future.take().unwrap(), meta)
                }
            })
        });

        match res {
            Ok(None) => panic!("Local tasks can only be spawned on a LocalRuntime from the thread the runtime was created on"),
            Ok(Some(join_handle)) => join_handle,
            Err(_) => match CURRENT.with(|LocalData { ctx, .. }| ctx.get()) {
                None => panic!("`spawn_local` called from outside of a `task::LocalSet` or LocalRuntime"),
                Some(cx) => cx.spawn(future.unwrap(), meta)
            }
        }
    }
}

/// Initial queue capacity.
const INITIAL_CAPACITY: usize = 64;

/// Max number of tasks to poll per tick.
const MAX_TASKS_PER_TICK: usize = 61;

/// How often it check the remote queue first.
const REMOTE_FIRST_INTERVAL: u8 = 31;

/// Context guard for `LocalSet`
pub struct LocalEnterGuard {
    ctx: Option<Rc<Context>>,

    /// Distinguishes whether the context was entered or being polled.
    /// When we enter it, the value `wake_on_schedule` is set. In this case
    /// `spawn_local` refers the context, whereas it is not being polled now.
    wake_on_schedule: bool,
}

impl Drop for LocalEnterGuard {
    fn drop(&mut self) {
        CURRENT.with(
            |LocalData {
                 ctx,
                 wake_on_schedule,
             }| {
                ctx.set(self.ctx.take());
                wake_on_schedule.set(self.wake_on_schedule);
            },
        );
    }
}

impl fmt::Debug for LocalEnterGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalEnterGuard").finish()
    }
}

impl LocalSet {
    /// Returns a new local task set.
    pub fn new() -> LocalSet {
        let owner = context::thread_id().expect("cannot create LocalSet during thread shutdown");

        LocalSet {
            tick: Cell::new(0),
            context: Rc::new(Context {
                shared: Arc::new(Shared {
                    local_state: LocalState {
                        owner,
                        owned: LocalOwnedTasks::new(),
                        local_queue: UnsafeCell::new(VecDeque::with_capacity(INITIAL_CAPACITY)),
                    },
                    queue: Mutex::new(Some(VecDeque::with_capacity(INITIAL_CAPACITY))),
                    waker: AtomicWaker::new(),
                    #[cfg(tokio_unstable)]
                    unhandled_panic: crate::runtime::UnhandledPanic::Ignore,
                }),
                unhandled_panic: Cell::new(false),
            }),
            _not_send: PhantomData,
        }
    }

    /// Enters the context of this `LocalSet`.
    ///
    /// The [`spawn_local`] method will spawn tasks on the `LocalSet` whose
    /// context you are inside.
    ///
    /// [`spawn_local`]: fn@crate::task::spawn_local
    pub fn enter(&self) -> LocalEnterGuard {
        CURRENT.with(
            |LocalData {
                 ctx,
                 wake_on_schedule,
                 ..
             }| {
                let ctx = ctx.replace(Some(self.context.clone()));
                let wake_on_schedule = wake_on_schedule.replace(true);
                LocalEnterGuard {
                    ctx,
                    wake_on_schedule,
                }
            },
        )
    }

    /// Spawns a `!Send` task onto the local task set.
    ///
    /// This task is guaranteed to be run on the current thread.
    ///
    /// Unlike the free function [`spawn_local`], this method may be used to
    /// spawn local tasks when the `LocalSet` is _not_ running. The provided
    /// future will start running once the `LocalSet` is next started, even if
    /// you don't await the returned `JoinHandle`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio::task;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let local = task::LocalSet::new();
    ///
    ///     // Spawn a future on the local set. This future will be run when
    ///     // we call `run_until` to drive the task set.
    ///     local.spawn_local(async {
    ///        // ...
    ///     });
    ///
    ///     // Run the local task set.
    ///     local.run_until(async move {
    ///         // ...
    ///     }).await;
    ///
    ///     // When `run` finishes, we can spawn _more_ futures, which will
    ///     // run in subsequent calls to `run_until`.
    ///     local.spawn_local(async {
    ///        // ...
    ///     });
    ///
    ///     local.run_until(async move {
    ///         // ...
    ///     }).await;
    /// }
    /// ```
    /// [`spawn_local`]: fn@spawn_local
    #[track_caller]
    pub fn spawn_local<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let fut_size = mem::size_of::<F>();
        if fut_size > BOX_FUTURE_THRESHOLD {
            self.spawn_named(Box::pin(future), SpawnMeta::new_unnamed(fut_size))
        } else {
            self.spawn_named(future, SpawnMeta::new_unnamed(fut_size))
        }
    }

    /// Runs a future to completion on the provided runtime, driving any local
    /// futures spawned on this task set on the current thread.
    ///
    /// This runs the given future on the runtime, blocking until it is
    /// complete, and yielding its resolved result. Any tasks or timers which
    /// the future spawns internally will be executed on the runtime. The future
    /// may also call [`spawn_local`] to `spawn_local` additional local futures on the
    /// current thread.
    ///
    /// This method should not be called from an asynchronous context.
    ///
    /// # Panics
    ///
    /// This function panics if the executor is at capacity, if the provided
    /// future panics, or if called within an asynchronous execution context.
    ///
    /// # Notes
    ///
    /// Since this function internally calls [`Runtime::block_on`], and drives
    /// futures in the local task set inside that call to `block_on`, the local
    /// futures may not use [in-place blocking]. If a blocking call needs to be
    /// issued from a local task, the [`spawn_blocking`] API may be used instead.
    ///
    /// For example, this will panic:
    /// ```should_panic
    /// use tokio::runtime::Runtime;
    /// use tokio::task;
    ///
    /// let rt  = Runtime::new().unwrap();
    /// let local = task::LocalSet::new();
    /// local.block_on(&rt, async {
    ///     let join = task::spawn_local(async {
    ///         let blocking_result = task::block_in_place(|| {
    ///             // ...
    ///         });
    ///         // ...
    ///     });
    ///     join.await.unwrap();
    /// })
    /// ```
    /// This, however, will not panic:
    /// ```
    /// use tokio::runtime::Runtime;
    /// use tokio::task;
    ///
    /// let rt  = Runtime::new().unwrap();
    /// let local = task::LocalSet::new();
    /// local.block_on(&rt, async {
    ///     let join = task::spawn_local(async {
    ///         let blocking_result = task::spawn_blocking(|| {
    ///             // ...
    ///         }).await;
    ///         // ...
    ///     });
    ///     join.await.unwrap();
    /// })
    /// ```
    ///
    /// [`spawn_local`]: fn@spawn_local
    /// [`Runtime::block_on`]: method@crate::runtime::Runtime::block_on
    /// [in-place blocking]: fn@crate::task::block_in_place
    /// [`spawn_blocking`]: fn@crate::task::spawn_blocking
    #[track_caller]
    #[cfg(feature = "rt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
    pub fn block_on<F>(&self, rt: &crate::runtime::Runtime, future: F) -> F::Output
    where
        F: Future,
    {
        rt.block_on(self.run_until(future))
    }

    /// Runs a future to completion on the local set, returning its output.
    ///
    /// This returns a future that runs the given future with a local set,
    /// allowing it to call [`spawn_local`] to spawn additional `!Send` futures.
    /// Any local futures spawned on the local set will be driven in the
    /// background until the future passed to `run_until` completes. When the future
    /// passed to `run_until` finishes, any local futures which have not completed
    /// will remain on the local set, and will be driven on subsequent calls to
    /// `run_until` or when [awaiting the local set] itself.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe when `future` is cancel safe.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio::task;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     task::LocalSet::new().run_until(async {
    ///         task::spawn_local(async move {
    ///             // ...
    ///         }).await.unwrap();
    ///         // ...
    ///     }).await;
    /// }
    /// ```
    ///
    /// [`spawn_local`]: fn@spawn_local
    /// [awaiting the local set]: #awaiting-a-localset
    pub async fn run_until<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        let run_until = RunUntil {
            future,
            local_set: self,
        };
        run_until.await
    }

    #[track_caller]
    pub(in crate::task) fn spawn_named<F>(
        &self,
        future: F,
        meta: SpawnMeta<'_>,
    ) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        self.spawn_named_inner(future, meta)
    }

    #[track_caller]
    fn spawn_named_inner<F>(&self, future: F, meta: SpawnMeta<'_>) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let handle = self.context.spawn(future, meta);

        // Because a task was spawned from *outside* the `LocalSet`, wake the
        // `LocalSet` future to execute the new task, if it hasn't been woken.
        //
        // Spawning via the free fn `spawn` does not require this, as it can
        // only be called from *within* a future executing on the `LocalSet` —
        // in that case, the `LocalSet` must already be awake.
        self.context.shared.waker.wake();
        handle
    }

    /// Ticks the scheduler, returning whether the local future needs to be
    /// notified again.
    fn tick(&self) -> bool {
        for _ in 0..MAX_TASKS_PER_TICK {
            // Make sure we didn't hit an unhandled panic
            assert!(!self.context.unhandled_panic.get(), "a spawned task panicked and the LocalSet is configured to shutdown on unhandled panic");

            match self.next_task() {
                // Run the task
                //
                // Safety: As spawned tasks are `!Send`, `run_unchecked` must be
                // used. We are responsible for maintaining the invariant that
                // `run_unchecked` is only called on threads that spawned the
                // task initially. Because `LocalSet` itself is `!Send`, and
                // `spawn_local` spawns into the `LocalSet` on the current
                // thread, the invariant is maintained.
                Some(task) => crate::task::coop::budget(|| task.run()),
                // We have fully drained the queue of notified tasks, so the
                // local future doesn't need to be notified again — it can wait
                // until something else wakes a task in the local set.
                None => return false,
            }
        }

        true
    }

    fn next_task(&self) -> Option<task::LocalNotified<Arc<Shared>>> {
        let tick = self.tick.get();
        self.tick.set(tick.wrapping_add(1));

        let task = if tick % REMOTE_FIRST_INTERVAL == 0 {
            self.context
                .shared
                .queue
                .lock()
                .as_mut()
                .and_then(|queue| queue.pop_front())
                .or_else(|| self.pop_local())
        } else {
            self.pop_local().or_else(|| {
                self.context
                    .shared
                    .queue
                    .lock()
                    .as_mut()
                    .and_then(VecDeque::pop_front)
            })
        };

        task.map(|task| unsafe {
            // Safety: because the `LocalSet` itself is `!Send`, we know we are
            // on the same thread if we have access to the `LocalSet`, and can
            // therefore access the local run queue.
            self.context.shared.local_state.assert_owner(task)
        })
    }

    fn pop_local(&self) -> Option<task::Notified<Arc<Shared>>> {
        unsafe {
            // Safety: because the `LocalSet` itself is `!Send`, we know we are
            // on the same thread if we have access to the `LocalSet`, and can
            // therefore access the local run queue.
            self.context.shared.local_state.task_pop_front()
        }
    }

    fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        CURRENT.with(|local_data| {
            let _guard = local_data.enter(self.context.clone());
            f()
        })
    }

    /// This method is like `with`, but it just calls `f` without setting the thread-local if that
    /// fails.
    fn with_if_possible<T>(&self, f: impl FnOnce() -> T) -> T {
        let mut f = Some(f);

        let res = CURRENT.try_with(|local_data| {
            let _guard = local_data.enter(self.context.clone());
            (f.take().unwrap())()
        });

        match res {
            Ok(res) => res,
            Err(_access_error) => (f.take().unwrap())(),
        }
    }
}

cfg_unstable! {
    impl LocalSet {
        /// Configure how the `LocalSet` responds to an unhandled panic on a
        /// spawned task.
        ///
        /// By default, an unhandled panic (i.e. a panic not caught by
        /// [`std::panic::catch_unwind`]) has no impact on the `LocalSet`'s
        /// execution. The panic is error value is forwarded to the task's
        /// [`JoinHandle`] and all other spawned tasks continue running.
        ///
        /// The `unhandled_panic` option enables configuring this behavior.
        ///
        /// * `UnhandledPanic::Ignore` is the default behavior. Panics on
        ///   spawned tasks have no impact on the `LocalSet`'s execution.
        /// * `UnhandledPanic::ShutdownRuntime` will force the `LocalSet` to
        ///   shutdown immediately when a spawned task panics even if that
        ///   task's `JoinHandle` has not been dropped. All other spawned tasks
        ///   will immediately terminate and further calls to
        ///   [`LocalSet::block_on`] and [`LocalSet::run_until`] will panic.
        ///
        /// # Panics
        ///
        /// This method panics if called after the `LocalSet` has started
        /// running.
        ///
        /// # Unstable
        ///
        /// This option is currently unstable and its implementation is
        /// incomplete. The API may change or be removed in the future. See
        /// tokio-rs/tokio#4516 for more details.
        ///
        /// # Examples
        ///
        /// The following demonstrates a `LocalSet` configured to shutdown on
        /// panic. The first spawned task panics and results in the `LocalSet`
        /// shutting down. The second spawned task never has a chance to
        /// execute. The call to `run_until` will panic due to the runtime being
        /// forcibly shutdown.
        ///
        /// ```should_panic
        /// use tokio::runtime::UnhandledPanic;
        ///
        /// # #[tokio::main]
        /// # async fn main() {
        /// tokio::task::LocalSet::new()
        ///     .unhandled_panic(UnhandledPanic::ShutdownRuntime)
        ///     .run_until(async {
        ///         tokio::task::spawn_local(async { panic!("boom"); });
        ///         tokio::task::spawn_local(async {
        ///             // This task never completes
        ///         });
        ///
        ///         // Do some work, but `run_until` will panic before it completes
        /// # loop { tokio::task::yield_now().await; }
        ///     })
        ///     .await;
        /// # }
        /// ```
        ///
        /// [`JoinHandle`]: struct@crate::task::JoinHandle
        pub fn unhandled_panic(&mut self, behavior: crate::runtime::UnhandledPanic) -> &mut Self {
            // TODO: This should be set as a builder
            Rc::get_mut(&mut self.context)
                .and_then(|ctx| Arc::get_mut(&mut ctx.shared))
                .expect("Unhandled Panic behavior modified after starting LocalSet")
                .unhandled_panic = behavior;
            self
        }

        /// Returns the [`Id`] of the current `LocalSet` runtime.
        ///
        /// # Examples
        ///
        /// ```rust
        /// use tokio::task;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let local_set = task::LocalSet::new();
        ///     println!("Local set id: {}", local_set.id());
        /// }
        /// ```
        ///
        /// **Note**: This is an [unstable API][unstable]. The public API of this type
        /// may break in 1.x releases. See [the documentation on unstable
        /// features][unstable] for details.
        ///
        /// [unstable]: crate#unstable-features
        /// [`Id`]: struct@crate::runtime::Id
        pub fn id(&self) -> runtime::Id {
            self.context.shared.local_state.owned.id.into()
        }
    }
}

impl fmt::Debug for LocalSet {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("LocalSet").finish()
    }
}

impl Future for LocalSet {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let _no_blocking = crate::runtime::context::disallow_block_in_place();

        // Register the waker before starting to work
        self.context.shared.waker.register_by_ref(cx.waker());

        if self.with(|| self.tick()) {
            // If `tick` returns true, we need to notify the local future again:
            // there are still tasks remaining in the run queue.
            cx.waker().wake_by_ref();
            Poll::Pending

        // Safety: called from the thread that owns `LocalSet`. Because
        // `LocalSet` is `!Send`, this is safe.
        } else if unsafe { self.context.shared.local_state.owned_is_empty() } {
            // If the scheduler has no remaining futures, we're done!
            Poll::Ready(())
        } else {
            // There are still futures in the local set, but we've polled all the
            // futures in the run queue. Therefore, we can just return Pending
            // since the remaining futures will be woken from somewhere else.
            Poll::Pending
        }
    }
}

impl Default for LocalSet {
    fn default() -> LocalSet {
        LocalSet::new()
    }
}

impl Drop for LocalSet {
    fn drop(&mut self) {
        self.with_if_possible(|| {
            let _no_blocking = crate::runtime::context::disallow_block_in_place();

            // Shut down all tasks in the LocalOwnedTasks and close it to
            // prevent new tasks from ever being added.
            unsafe {
                // Safety: called from the thread that owns `LocalSet`
                self.context.shared.local_state.close_and_shutdown_all();
            }

            // We already called shutdown on all tasks above, so there is no
            // need to call shutdown.

            // Safety: note that this *intentionally* bypasses the unsafe
            // `Shared::local_queue()` method. This is in order to avoid the
            // debug assertion that we are on the thread that owns the
            // `LocalSet`, because on some systems (e.g. at least some macOS
            // versions), attempting to get the current thread ID can panic due
            // to the thread's local data that stores the thread ID being
            // dropped *before* the `LocalSet`.
            //
            // Despite avoiding the assertion here, it is safe for us to access
            // the local queue in `Drop`, because the `LocalSet` itself is
            // `!Send`, so we can reasonably guarantee that it will not be
            // `Drop`ped from another thread.
            let local_queue = unsafe {
                // Safety: called from the thread that owns `LocalSet`
                self.context.shared.local_state.take_local_queue()
            };
            for task in local_queue {
                drop(task);
            }

            // Take the queue from the Shared object to prevent pushing
            // notifications to it in the future.
            let queue = self.context.shared.queue.lock().take().unwrap();
            for task in queue {
                drop(task);
            }

            // Safety: called from the thread that owns `LocalSet`
            assert!(unsafe { self.context.shared.local_state.owned_is_empty() });
        });
    }
}

// === impl Context ===

impl Context {
    #[track_caller]
    fn spawn<F>(&self, future: F, meta: SpawnMeta<'_>) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let id = crate::runtime::task::Id::next();
        let future = crate::util::trace::task(future, "local", meta, id.as_u64());

        // Safety: called from the thread that owns the `LocalSet`
        let (handle, notified) = {
            self.shared.local_state.assert_called_from_owner_thread();
            self.shared.local_state.owned.bind(
                future,
                self.shared.clone(),
                id,
                SpawnLocation::capture(),
            )
        };

        if let Some(notified) = notified {
            self.shared.schedule(notified);
        }

        handle
    }
}

// === impl LocalFuture ===

impl<T: Future> Future for RunUntil<'_, T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        me.local_set.with(|| {
            me.local_set
                .context
                .shared
                .waker
                .register_by_ref(cx.waker());

            let _no_blocking = crate::runtime::context::disallow_block_in_place();
            let f = me.future;

            if let Poll::Ready(output) = f.poll(cx) {
                return Poll::Ready(output);
            }

            if me.local_set.tick() {
                // If `tick` returns `true`, we need to notify the local future again:
                // there are still tasks remaining in the run queue.
                cx.waker().wake_by_ref();
            }

            Poll::Pending
        })
    }
}

impl Shared {
    /// Schedule the provided task on the scheduler.
    fn schedule(&self, task: task::Notified<Arc<Self>>) {
        CURRENT.with(|localdata| {
            match localdata.ctx.get() {
                // If the current `LocalSet` is being polled, we don't need to wake it.
                // When we `enter` it, then the value `wake_on_schedule` is set to be true.
                // In this case it is not being polled, so we need to wake it.
                Some(cx) if cx.shared.ptr_eq(self) && !localdata.wake_on_schedule.get() => unsafe {
                    // Safety: if the current `LocalSet` context points to this
                    // `LocalSet`, then we are on the thread that owns it.
                    cx.shared.local_state.task_push_back(task);
                },

                // We are on the thread that owns the `LocalSet`, so we can
                // wake to the local queue.
                _ if context::thread_id().ok() == Some(self.local_state.owner) => {
                    unsafe {
                        // Safety: we just checked that the thread ID matches
                        // the localset's owner, so this is safe.
                        self.local_state.task_push_back(task);
                    }
                    // We still have to wake the `LocalSet`, because it isn't
                    // currently being polled.
                    self.waker.wake();
                }

                // We are *not* on the thread that owns the `LocalSet`, so we
                // have to wake to the remote queue.
                _ => {
                    // First, check whether the queue is still there (if not, the
                    // LocalSet is dropped). Then push to it if so, and if not,
                    // do nothing.
                    let mut lock = self.queue.lock();

                    if let Some(queue) = lock.as_mut() {
                        queue.push_back(task);
                        drop(lock);
                        self.waker.wake();
                    }
                }
            }
        });
    }

    fn ptr_eq(&self, other: &Shared) -> bool {
        std::ptr::eq(self, other)
    }
}

// This is safe because (and only because) we *pinky pwomise* to never touch the
// local run queue except from the thread that owns the `LocalSet`.
unsafe impl Sync for Shared {}

impl task::Schedule for Arc<Shared> {
    fn release(&self, task: &Task<Self>) -> Option<Task<Self>> {
        // Safety, this is always called from the thread that owns `LocalSet`
        unsafe { self.local_state.task_remove(task) }
    }

    fn schedule(&self, task: task::Notified<Self>) {
        Shared::schedule(self, task);
    }

    // localset does not currently support task hooks
    fn hooks(&self) -> TaskHarnessScheduleHooks {
        TaskHarnessScheduleHooks {
            task_terminate_callback: None,
        }
    }

    cfg_unstable! {
        fn unhandled_panic(&self) {
            use crate::runtime::UnhandledPanic;

            match self.unhandled_panic {
                UnhandledPanic::Ignore => {
                    // Do nothing
                }
                UnhandledPanic::ShutdownRuntime => {
                    // This hook is only called from within the runtime, so
                    // `CURRENT` should match with `&self`, i.e. there is no
                    // opportunity for a nested scheduler to be called.
                    CURRENT.with(|LocalData { ctx, .. }| match ctx.get() {
                        Some(cx) if Arc::ptr_eq(self, &cx.shared) => {
                            cx.unhandled_panic.set(true);
                            // Safety: this is always called from the thread that owns `LocalSet`
                            unsafe { cx.shared.local_state.close_and_shutdown_all(); }
                        }
                        _ => unreachable!("runtime core not set in CURRENT thread-local"),
                    })
                }
            }
        }
    }
}

impl LocalState {
    unsafe fn task_pop_front(&self) -> Option<task::Notified<Arc<Shared>>> {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.local_queue.with_mut(|ptr| (*ptr).pop_front())
    }

    unsafe fn task_push_back(&self, task: task::Notified<Arc<Shared>>) {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.local_queue.with_mut(|ptr| (*ptr).push_back(task));
    }

    unsafe fn take_local_queue(&self) -> VecDeque<task::Notified<Arc<Shared>>> {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.local_queue.with_mut(|ptr| std::mem::take(&mut (*ptr)))
    }

    unsafe fn task_remove(&self, task: &Task<Arc<Shared>>) -> Option<Task<Arc<Shared>>> {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.owned.remove(task)
    }

    /// Returns true if the `LocalSet` does not have any spawned tasks
    unsafe fn owned_is_empty(&self) -> bool {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.owned.is_empty()
    }

    unsafe fn assert_owner(
        &self,
        task: task::Notified<Arc<Shared>>,
    ) -> task::LocalNotified<Arc<Shared>> {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.owned.assert_owner(task)
    }

    unsafe fn close_and_shutdown_all(&self) {
        // The caller ensures it is called from the same thread that owns
        // the LocalSet.
        self.assert_called_from_owner_thread();

        self.owned.close_and_shutdown_all();
    }

    #[track_caller]
    fn assert_called_from_owner_thread(&self) {
        // FreeBSD has some weirdness around thread-local destruction.
        // TODO: remove this hack when thread id is cleaned up
        #[cfg(not(any(target_os = "openbsd", target_os = "freebsd")))]
        debug_assert!(
            // if we couldn't get the thread ID because we're dropping the local
            // data, skip the assertion --- the `Drop` impl is not going to be
            // called from another thread, because `LocalSet` is `!Send`
            context::thread_id()
                .map(|id| id == self.owner)
                .unwrap_or(true),
            "`LocalSet`'s local run queue must not be accessed by another thread!"
        );
    }
}

// This is `Send` because it is stored in `Shared`. It is up to the caller to
// ensure they are on the same thread that owns the `LocalSet`.
unsafe impl Send for LocalState {}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;

    // Does a `LocalSet` running on a current-thread runtime...basically work?
    //
    // This duplicates a test in `tests/task_local_set.rs`, but because this is
    // a lib test, it will run under Miri, so this is necessary to catch stacked
    // borrows violations in the `LocalSet` implementation.
    #[test]
    fn local_current_thread_scheduler() {
        let f = async {
            LocalSet::new()
                .run_until(async {
                    spawn_local(async {}).await.unwrap();
                })
                .await;
        };
        crate::runtime::Builder::new_current_thread()
            .build()
            .expect("rt")
            .block_on(f)
    }

    // Tests that when a task on a `LocalSet` is woken by an io driver on the
    // same thread, the task is woken to the localset's local queue rather than
    // its remote queue.
    //
    // This test has to be defined in the `local.rs` file as a lib test, rather
    // than in `tests/`, because it makes assertions about the local set's
    // internal state.
    #[test]
    fn wakes_to_local_queue() {
        use super::*;
        use crate::sync::Notify;
        let rt = crate::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        rt.block_on(async {
            let local = LocalSet::new();
            let notify = Arc::new(Notify::new());
            let task = local.spawn_local({
                let notify = notify.clone();
                async move {
                    notify.notified().await;
                }
            });
            let mut run_until = Box::pin(local.run_until(async move {
                task.await.unwrap();
            }));

            // poll the run until future once
            std::future::poll_fn(|cx| {
                let _ = run_until.as_mut().poll(cx);
                Poll::Ready(())
            })
            .await;

            notify.notify_one();
            let task = unsafe { local.context.shared.local_state.task_pop_front() };
            // TODO(eliza): it would be nice to be able to assert that this is
            // the local task.
            assert!(
                task.is_some(),
                "task should have been notified to the LocalSet's local queue"
            );
        })
    }
}
