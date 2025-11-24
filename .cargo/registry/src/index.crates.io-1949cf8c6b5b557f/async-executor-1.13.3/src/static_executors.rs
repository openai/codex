use crate::{debug_state, Executor, LocalExecutor, State};
use async_task::{Builder, Runnable, Task};
use slab::Slab;
use std::{
    cell::UnsafeCell,
    fmt,
    future::Future,
    marker::PhantomData,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::PoisonError,
};

impl Executor<'static> {
    /// Consumes the [`Executor`] and intentionally leaks it.
    ///
    /// Largely equivalent to calling `Box::leak(Box::new(executor))`, but the produced
    /// [`StaticExecutor`]'s functions are optimized to require fewer synchronizing operations
    /// when spawning, running, and finishing tasks.
    ///
    /// `StaticExecutor` cannot be converted back into a `Executor`, so this operation is
    /// irreversible without the use of unsafe.
    ///
    /// # Example
    ///
    /// ```
    /// use async_executor::Executor;
    /// use futures_lite::future;
    ///
    /// let ex = Executor::new().leak();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    ///
    /// future::block_on(ex.run(task));
    /// ```
    pub fn leak(self) -> &'static StaticExecutor {
        let ptr = self.state_ptr();
        // SAFETY: So long as an Executor lives, it's state pointer will always be valid
        // when accessed through state_ptr. This executor will live for the full 'static
        // lifetime so this isn't an arbitrary lifetime extension.
        let state: &'static State = unsafe { &*ptr };

        std::mem::forget(self);

        let mut active = state.active.lock().unwrap_or_else(PoisonError::into_inner);
        if !active.is_empty() {
            // Reschedule all of the active tasks.
            for waker in active.drain() {
                waker.wake();
            }
            // Overwrite to ensure that the slab is deallocated.
            *active = Slab::new();
        }

        // SAFETY: StaticExecutor has the same memory layout as State as it's repr(transparent).
        // The lifetime is not altered: 'static -> 'static.
        let static_executor: &'static StaticExecutor = unsafe { std::mem::transmute(state) };
        static_executor
    }
}

impl LocalExecutor<'static> {
    /// Consumes the [`LocalExecutor`] and intentionally leaks it.
    ///
    /// Largely equivalent to calling `Box::leak(Box::new(executor))`, but the produced
    /// [`StaticLocalExecutor`]'s functions are optimized to require fewer synchronizing operations
    /// when spawning, running, and finishing tasks.
    ///
    /// `StaticLocalExecutor` cannot be converted back into a `Executor`, so this operation is
    /// irreversible without the use of unsafe.
    ///
    /// # Example
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    /// use futures_lite::future;
    ///
    /// let ex = LocalExecutor::new().leak();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    ///
    /// future::block_on(ex.run(task));
    /// ```
    pub fn leak(self) -> &'static StaticLocalExecutor {
        let ptr = self.inner.state_ptr();
        // SAFETY: So long as a LocalExecutor lives, it's state pointer will always be valid
        // when accessed through state_ptr. This executor will live for the full 'static
        // lifetime so this isn't an arbitrary lifetime extension.
        let state: &'static State = unsafe { &*ptr };

        std::mem::forget(self);

        let mut active = state.active.lock().unwrap_or_else(PoisonError::into_inner);
        if !active.is_empty() {
            // Reschedule all of the active tasks.
            for waker in active.drain() {
                waker.wake();
            }
            // Overwrite to ensure that the slab is deallocated.
            *active = Slab::new();
        }

        // SAFETY: StaticLocalExecutor has the same memory layout as State as it's repr(transparent).
        // The lifetime is not altered: 'static -> 'static.
        let static_executor: &'static StaticLocalExecutor = unsafe { std::mem::transmute(state) };
        static_executor
    }
}

/// A static-lifetimed async [`Executor`].
///
/// This is primarily intended to be used in [`static`] variables, or types intended to be used, or can be created in non-static
/// contexts via [`Executor::leak`].
///
/// Spawning, running, and finishing tasks are optimized with the assumption that the executor will never be `Drop`'ed.
/// A static executor may require signficantly less overhead in both single-threaded and mulitthreaded use cases.
///
/// As this type does not implement `Drop`, losing the handle to the executor or failing
/// to consistently drive the executor with [`StaticExecutor::tick`] or
/// [`StaticExecutor::run`] will cause the all spawned tasks to permanently leak. Any
/// tasks at the time will not be cancelled.
///
/// [`static`]: https://doc.rust-lang.org/std/keyword.static.html
#[repr(transparent)]
pub struct StaticExecutor {
    state: State,
}

// SAFETY: Executor stores no thread local state that can be accessed via other thread.
unsafe impl Send for StaticExecutor {}
// SAFETY: Executor internally synchronizes all of it's operations internally.
unsafe impl Sync for StaticExecutor {}

impl UnwindSafe for StaticExecutor {}
impl RefUnwindSafe for StaticExecutor {}

impl fmt::Debug for StaticExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_state(&self.state, "StaticExecutor", f)
    }
}

impl StaticExecutor {
    /// Creates a new StaticExecutor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::StaticExecutor;
    ///
    /// static EXECUTOR: StaticExecutor = StaticExecutor::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            state: State::new(),
        }
    }

    /// Spawns a task onto the executor.
    ///
    /// Note: unlike [`Executor::spawn`], this function requires being called with a `'static`
    /// borrow on the executor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::StaticExecutor;
    ///
    /// static EXECUTOR: StaticExecutor = StaticExecutor::new();
    ///
    /// let task = EXECUTOR.spawn(async {
    ///     println!("Hello world");
    /// });
    /// ```
    pub fn spawn<T: Send + 'static>(
        &'static self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        let (runnable, task) = Builder::new()
            .propagate_panic(true)
            .spawn(|()| future, self.schedule());
        runnable.schedule();
        task
    }

    /// Spawns a non-`'static` task onto the executor.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the returned task terminates
    /// or is cancelled before the end of 'a.
    pub unsafe fn spawn_scoped<'a, T: Send + 'a>(
        &'static self,
        future: impl Future<Output = T> + Send + 'a,
    ) -> Task<T> {
        // SAFETY:
        //
        // - `future` is `Send`
        // - `future` is not `'static`, but the caller guarantees that the
        //    task, and thus its `Runnable` must not live longer than `'a`.
        // - `self.schedule()` is `Send`, `Sync` and `'static`, as checked below.
        //    Therefore we do not need to worry about what is done with the
        //    `Waker`.
        let (runnable, task) = unsafe {
            Builder::new()
                .propagate_panic(true)
                .spawn_unchecked(|()| future, self.schedule())
        };
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
    /// use async_executor::StaticExecutor;
    ///
    /// static EXECUTOR: StaticExecutor = StaticExecutor::new();
    ///
    /// assert!(!EXECUTOR.try_tick()); // no tasks to run
    ///
    /// let task = EXECUTOR.spawn(async {
    ///     println!("Hello world");
    /// });
    ///
    /// assert!(EXECUTOR.try_tick()); // a task was found
    /// ```
    pub fn try_tick(&self) -> bool {
        self.state.try_tick()
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
    /// use async_executor::StaticExecutor;
    /// use futures_lite::future;
    ///
    /// static EXECUTOR: StaticExecutor = StaticExecutor::new();
    ///
    /// let task = EXECUTOR.spawn(async {
    ///     println!("Hello world");
    /// });
    ///
    /// future::block_on(EXECUTOR.tick()); // runs the task
    /// ```
    pub async fn tick(&self) {
        self.state.tick().await;
    }

    /// Runs the executor until the given future completes.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::StaticExecutor;
    /// use futures_lite::future;
    ///
    /// static EXECUTOR: StaticExecutor = StaticExecutor::new();
    ///
    /// let task = EXECUTOR.spawn(async { 1 + 2 });
    /// let res = future::block_on(EXECUTOR.run(async { task.await * 2 }));
    ///
    /// assert_eq!(res, 6);
    /// ```
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        self.state.run(future).await
    }

    /// Returns a function that schedules a runnable task when it gets woken up.
    fn schedule(&'static self) -> impl Fn(Runnable) + Send + Sync + 'static {
        let state: &'static State = &self.state;
        // TODO: If possible, push into the current local queue and notify the ticker.
        move |runnable| {
            let result = state.queue.push(runnable);
            debug_assert!(result.is_ok()); // Since we use unbounded queue, push will never fail.
            state.notify();
        }
    }
}

impl Default for StaticExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// A static async [`LocalExecutor`] created from [`LocalExecutor::leak`].
///
/// This is primarily intended to be used in [`thread_local`] variables, or can be created in non-static
/// contexts via [`LocalExecutor::leak`].
///
/// Spawning, running, and finishing tasks are optimized with the assumption that the executor will never be `Drop`'ed.
/// A static executor may require signficantly less overhead in both single-threaded and mulitthreaded use cases.
///
/// As this type does not implement `Drop`, losing the handle to the executor or failing
/// to consistently drive the executor with [`StaticLocalExecutor::tick`] or
/// [`StaticLocalExecutor::run`] will cause the all spawned tasks to permanently leak. Any
/// tasks at the time will not be cancelled.
///
/// [`thread_local]: https://doc.rust-lang.org/std/macro.thread_local.html
#[repr(transparent)]
pub struct StaticLocalExecutor {
    state: State,
    marker_: PhantomData<UnsafeCell<()>>,
}

impl UnwindSafe for StaticLocalExecutor {}
impl RefUnwindSafe for StaticLocalExecutor {}

impl fmt::Debug for StaticLocalExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_state(&self.state, "StaticLocalExecutor", f)
    }
}

impl StaticLocalExecutor {
    /// Creates a new StaticLocalExecutor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::StaticLocalExecutor;
    ///
    /// thread_local! {
    ///     static EXECUTOR: StaticLocalExecutor = StaticLocalExecutor::new();
    /// }
    /// ```
    pub const fn new() -> Self {
        Self {
            state: State::new(),
            marker_: PhantomData,
        }
    }

    /// Spawns a task onto the executor.
    ///
    /// Note: unlike [`LocalExecutor::spawn`], this function requires being called with a `'static`
    /// borrow on the executor.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    ///
    /// let ex = LocalExecutor::new().leak();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// ```
    pub fn spawn<T: 'static>(&'static self, future: impl Future<Output = T> + 'static) -> Task<T> {
        let (runnable, task) = Builder::new()
            .propagate_panic(true)
            .spawn_local(|()| future, self.schedule());
        runnable.schedule();
        task
    }

    /// Spawns a non-`'static` task onto the executor.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the returned task terminates
    /// or is cancelled before the end of 'a.
    pub unsafe fn spawn_scoped<'a, T: 'a>(
        &'static self,
        future: impl Future<Output = T> + 'a,
    ) -> Task<T> {
        // SAFETY:
        //
        // - `future` is not `Send` but `StaticLocalExecutor` is `!Sync`,
        //   `try_tick`, `tick` and `run` can only be called from the origin
        //    thread of the `StaticLocalExecutor`. Similarly, `spawn_scoped` can only
        //    be called from the origin thread, ensuring that `future` and the executor
        //    share the same origin thread. The `Runnable` can be scheduled from other
        //    threads, but because of the above `Runnable` can only be called or
        //    dropped on the origin thread.
        // - `future` is not `'static`, but the caller guarantees that the
        //    task, and thus its `Runnable` must not live longer than `'a`.
        // - `self.schedule()` is `Send`, `Sync` and `'static`, as checked below.
        //    Therefore we do not need to worry about what is done with the
        //    `Waker`.
        let (runnable, task) = unsafe {
            Builder::new()
                .propagate_panic(true)
                .spawn_unchecked(|()| future, self.schedule())
        };
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
    /// use async_executor::LocalExecutor;
    ///
    /// let ex = LocalExecutor::new().leak();
    /// assert!(!ex.try_tick()); // no tasks to run
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// assert!(ex.try_tick()); // a task was found
    /// ```
    pub fn try_tick(&self) -> bool {
        self.state.try_tick()
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
    /// let ex = LocalExecutor::new().leak();
    ///
    /// let task = ex.spawn(async {
    ///     println!("Hello world");
    /// });
    /// future::block_on(ex.tick()); // runs the task
    /// ```
    pub async fn tick(&self) {
        self.state.tick().await;
    }

    /// Runs the executor until the given future completes.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_executor::LocalExecutor;
    /// use futures_lite::future;
    ///
    /// let ex = LocalExecutor::new().leak();
    ///
    /// let task = ex.spawn(async { 1 + 2 });
    /// let res = future::block_on(ex.run(async { task.await * 2 }));
    ///
    /// assert_eq!(res, 6);
    /// ```
    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        self.state.run(future).await
    }

    /// Returns a function that schedules a runnable task when it gets woken up.
    fn schedule(&'static self) -> impl Fn(Runnable) + Send + Sync + 'static {
        let state: &'static State = &self.state;
        // TODO: If possible, push into the current local queue and notify the ticker.
        move |runnable| {
            let result = state.queue.push(runnable);
            debug_assert!(result.is_ok()); // Since we use unbounded queue, push will never fail.
            state.notify();
        }
    }
}

impl Default for StaticLocalExecutor {
    fn default() -> Self {
        Self::new()
    }
}
