use core::fmt;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use core::ptr::NonNull;
use core::sync::atomic::Ordering;
use core::task::{Context, Poll};

use crate::header::Header;
use crate::raw::Panic;
use crate::runnable::ScheduleInfo;
use crate::state::*;

/// A spawned task.
///
/// A [`Task`] can be awaited to retrieve the output of its future.
///
/// Dropping a [`Task`] cancels it, which means its future won't be polled again. To drop the
/// [`Task`] handle without canceling it, use [`detach()`][`Task::detach()`] instead. To cancel a
/// task gracefully and wait until it is fully destroyed, use the [`cancel()`][Task::cancel()]
/// method.
///
/// Note that canceling a task actually wakes it and reschedules one last time. Then, the executor
/// can destroy the task by simply dropping its [`Runnable`][`super::Runnable`] or by invoking
/// [`run()`][`super::Runnable::run()`].
///
/// # Examples
///
/// ```
/// use smol::{future, Executor};
/// use std::thread;
///
/// let ex = Executor::new();
///
/// // Spawn a future onto the executor.
/// let task = ex.spawn(async {
///     println!("Hello from a task!");
///     1 + 2
/// });
///
/// // Run an executor thread.
/// thread::spawn(move || future::block_on(ex.run(future::pending::<()>())));
///
/// // Wait for the task's output.
/// assert_eq!(future::block_on(task), 3);
/// ```
#[must_use = "tasks get canceled when dropped, use `.detach()` to run them in the background"]
pub struct Task<T, M = ()> {
    /// A raw task pointer.
    pub(crate) ptr: NonNull<()>,

    /// A marker capturing generic types `T` and `M`.
    pub(crate) _marker: PhantomData<(T, M)>,
}

unsafe impl<T: Send, M: Send + Sync> Send for Task<T, M> {}
unsafe impl<T, M: Send + Sync> Sync for Task<T, M> {}

impl<T, M> Unpin for Task<T, M> {}

#[cfg(feature = "std")]
impl<T, M> std::panic::UnwindSafe for Task<T, M> {}
#[cfg(feature = "std")]
impl<T, M> std::panic::RefUnwindSafe for Task<T, M> {}

impl<T, M> Task<T, M> {
    /// Detaches the task to let it keep running in the background.
    ///
    /// # Examples
    ///
    /// ```
    /// use smol::{Executor, Timer};
    /// use std::time::Duration;
    ///
    /// let ex = Executor::new();
    ///
    /// // Spawn a deamon future.
    /// ex.spawn(async {
    ///     loop {
    ///         println!("I'm a daemon task looping forever.");
    ///         Timer::after(Duration::from_secs(1)).await;
    ///     }
    /// })
    /// .detach();
    /// ```
    pub fn detach(self) {
        let mut this = self;
        let _out = this.set_detached();
        mem::forget(this);
    }

    /// Cancels the task and waits for it to stop running.
    ///
    /// Returns the task's output if it was completed just before it got canceled, or [`None`] if
    /// it didn't complete.
    ///
    /// While it's possible to simply drop the [`Task`] to cancel it, this is a cleaner way of
    /// canceling because it also waits for the task to stop running.
    ///
    /// # Examples
    ///
    /// ```
    /// # if cfg!(miri) { return; } // Miri does not support epoll
    /// use smol::{future, Executor, Timer};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let ex = Executor::new();
    ///
    /// // Spawn a deamon future.
    /// let task = ex.spawn(async {
    ///     loop {
    ///         println!("Even though I'm in an infinite loop, you can still cancel me!");
    ///         Timer::after(Duration::from_secs(1)).await;
    ///     }
    /// });
    ///
    /// // Run an executor thread.
    /// thread::spawn(move || future::block_on(ex.run(future::pending::<()>())));
    ///
    /// future::block_on(async {
    ///     Timer::after(Duration::from_secs(3)).await;
    ///     task.cancel().await;
    /// });
    /// ```
    pub async fn cancel(self) -> Option<T> {
        let mut this = self;
        this.set_canceled();
        this.fallible().await
    }

    /// Converts this task into a [`FallibleTask`].
    ///
    /// Like [`Task`], a fallible task will poll the task's output until it is
    /// completed or cancelled due to its [`Runnable`][`super::Runnable`] being
    /// dropped without being run. Resolves to the task's output when completed,
    /// or [`None`] if it didn't complete.
    ///
    /// # Examples
    ///
    /// ```
    /// use smol::{future, Executor};
    /// use std::thread;
    ///
    /// let ex = Executor::new();
    ///
    /// // Spawn a future onto the executor.
    /// let task = ex.spawn(async {
    ///     println!("Hello from a task!");
    ///     1 + 2
    /// })
    /// .fallible();
    ///
    /// // Run an executor thread.
    /// thread::spawn(move || future::block_on(ex.run(future::pending::<()>())));
    ///
    /// // Wait for the task's output.
    /// assert_eq!(future::block_on(task), Some(3));
    /// ```
    ///
    /// ```
    /// use smol::future;
    ///
    /// // Schedule function which drops the runnable without running it.
    /// let schedule = move |runnable| drop(runnable);
    ///
    /// // Create a task with the future and the schedule function.
    /// let (runnable, task) = async_task::spawn(async {
    ///     println!("Hello from a task!");
    ///     1 + 2
    /// }, schedule);
    /// runnable.schedule();
    ///
    /// // Wait for the task's output.
    /// assert_eq!(future::block_on(task.fallible()), None);
    /// ```
    pub fn fallible(self) -> FallibleTask<T, M> {
        FallibleTask { task: self }
    }

    /// Puts the task in canceled state.
    fn set_canceled(&mut self) {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;

        unsafe {
            let mut state = (*header).state.load(Ordering::Acquire);

            loop {
                // If the task has been completed or closed, it can't be canceled.
                if state & (COMPLETED | CLOSED) != 0 {
                    break;
                }

                // If the task is not scheduled nor running, we'll need to schedule it.
                let new = if state & (SCHEDULED | RUNNING) == 0 {
                    (state | SCHEDULED | CLOSED) + REFERENCE
                } else {
                    state | CLOSED
                };

                // Mark the task as closed.
                match (*header).state.compare_exchange_weak(
                    state,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // If the task is not scheduled nor running, schedule it one more time so
                        // that its future gets dropped by the executor.
                        if state & (SCHEDULED | RUNNING) == 0 {
                            ((*header).vtable.schedule)(ptr, ScheduleInfo::new(false));
                        }

                        // Notify the awaiter that the task has been closed.
                        if state & AWAITER != 0 {
                            (*header).notify(None);
                        }

                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    /// Puts the task in detached state.
    fn set_detached(&mut self) -> Option<Result<T, Panic>> {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;

        unsafe {
            // A place where the output will be stored in case it needs to be dropped.
            let mut output = None;

            // Optimistically assume the `Task` is being detached just after creating the task.
            // This is a common case so if the `Task` is datached, the overhead of it is only one
            // compare-exchange operation.
            if let Err(mut state) = (*header).state.compare_exchange_weak(
                SCHEDULED | TASK | REFERENCE,
                SCHEDULED | REFERENCE,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                loop {
                    // If the task has been completed but not yet closed, that means its output
                    // must be dropped.
                    if state & COMPLETED != 0 && state & CLOSED == 0 {
                        // Mark the task as closed in order to grab its output.
                        match (*header).state.compare_exchange_weak(
                            state,
                            state | CLOSED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => {
                                // Read the output.
                                output = Some(
                                    (((*header).vtable.get_output)(ptr) as *mut Result<T, Panic>)
                                        .read(),
                                );

                                // Update the state variable because we're continuing the loop.
                                state |= CLOSED;
                            }
                            Err(s) => state = s,
                        }
                    } else {
                        // If this is the last reference to the task and it's not closed, then
                        // close it and schedule one more time so that its future gets dropped by
                        // the executor.
                        let new = if state & (!(REFERENCE - 1) | CLOSED) == 0 {
                            SCHEDULED | CLOSED | REFERENCE
                        } else {
                            state & !TASK
                        };

                        // Unset the `TASK` flag.
                        match (*header).state.compare_exchange_weak(
                            state,
                            new,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(_) => {
                                // If this is the last reference to the task, we need to either
                                // schedule dropping its future or destroy it.
                                if state & !(REFERENCE - 1) == 0 {
                                    if state & CLOSED == 0 {
                                        ((*header).vtable.schedule)(ptr, ScheduleInfo::new(false));
                                    } else {
                                        ((*header).vtable.destroy)(ptr);
                                    }
                                }

                                break;
                            }
                            Err(s) => state = s,
                        }
                    }
                }
            }

            output
        }
    }

    /// Polls the task to retrieve its output.
    ///
    /// Returns `Some` if the task has completed or `None` if it was closed.
    ///
    /// A task becomes closed in the following cases:
    ///
    /// 1. It gets canceled by `Runnable::drop()`, `Task::drop()`, or `Task::cancel()`.
    /// 2. Its output gets awaited by the `Task`.
    /// 3. It panics while polling the future.
    /// 4. It is completed and the `Task` gets dropped.
    fn poll_task(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;

        unsafe {
            let mut state = (*header).state.load(Ordering::Acquire);

            loop {
                // If the task has been closed, notify the awaiter and return `None`.
                if state & CLOSED != 0 {
                    // If the task is scheduled or running, we need to wait until its future is
                    // dropped.
                    if state & (SCHEDULED | RUNNING) != 0 {
                        // Replace the waker with one associated with the current task.
                        (*header).register(cx.waker());

                        // Reload the state after registering. It is possible changes occurred just
                        // before registration so we need to check for that.
                        state = (*header).state.load(Ordering::Acquire);

                        // If the task is still scheduled or running, we need to wait because its
                        // future is not dropped yet.
                        if state & (SCHEDULED | RUNNING) != 0 {
                            return Poll::Pending;
                        }
                    }

                    // Even though the awaiter is most likely the current task, it could also be
                    // another task.
                    (*header).notify(Some(cx.waker()));
                    return Poll::Ready(None);
                }

                // If the task is not completed, register the current task.
                if state & COMPLETED == 0 {
                    // Replace the waker with one associated with the current task.
                    (*header).register(cx.waker());

                    // Reload the state after registering. It is possible that the task became
                    // completed or closed just before registration so we need to check for that.
                    state = (*header).state.load(Ordering::Acquire);

                    // If the task has been closed, restart.
                    if state & CLOSED != 0 {
                        continue;
                    }

                    // If the task is still not completed, we're blocked on it.
                    if state & COMPLETED == 0 {
                        return Poll::Pending;
                    }
                }

                // Since the task is now completed, mark it as closed in order to grab its output.
                match (*header).state.compare_exchange(
                    state,
                    state | CLOSED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Notify the awaiter. Even though the awaiter is most likely the current
                        // task, it could also be another task.
                        if state & AWAITER != 0 {
                            (*header).notify(Some(cx.waker()));
                        }

                        // Take the output from the task.
                        let output = ((*header).vtable.get_output)(ptr) as *mut Result<T, Panic>;
                        let output = output.read();

                        // Propagate the panic if the task panicked.
                        let output = match output {
                            Ok(output) => output,
                            Err(panic) => {
                                #[cfg(feature = "std")]
                                std::panic::resume_unwind(panic);

                                #[cfg(not(feature = "std"))]
                                match panic {}
                            }
                        };

                        return Poll::Ready(Some(output));
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    fn header(&self) -> &Header<M> {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;
        unsafe { &*header }
    }

    /// Returns `true` if the current task is finished.
    ///
    /// Note that in a multithreaded environment, this task can change finish immediately after calling this function.
    pub fn is_finished(&self) -> bool {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;

        unsafe {
            let state = (*header).state.load(Ordering::Acquire);
            state & (CLOSED | COMPLETED) != 0
        }
    }

    /// Get the metadata associated with this task.
    ///
    /// Tasks can be created with a metadata object associated with them; by default, this
    /// is a `()` value. See the [`Builder::metadata()`] method for more information.
    pub fn metadata(&self) -> &M {
        &self.header().metadata
    }
}

impl<T, M> Drop for Task<T, M> {
    fn drop(&mut self) {
        self.set_canceled();
        self.set_detached();
    }
}

impl<T, M> Future for Task<T, M> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.poll_task(cx) {
            Poll::Ready(t) => Poll::Ready(t.expect("Task polled after completion")),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T, M: fmt::Debug> fmt::Debug for Task<T, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("header", self.header())
            .finish()
    }
}

/// A spawned task with a fallible response.
///
/// This type behaves like [`Task`], however it produces an `Option<T>` when
/// polled and will return `None` if the executor dropped its
/// [`Runnable`][`super::Runnable`] without being run.
///
/// This can be useful to avoid the panic produced when polling the `Task`
/// future if the executor dropped its `Runnable`.
#[must_use = "tasks get canceled when dropped, use `.detach()` to run them in the background"]
pub struct FallibleTask<T, M = ()> {
    task: Task<T, M>,
}

impl<T, M> FallibleTask<T, M> {
    /// Detaches the task to let it keep running in the background.
    ///
    /// # Examples
    ///
    /// ```
    /// use smol::{Executor, Timer};
    /// use std::time::Duration;
    ///
    /// let ex = Executor::new();
    ///
    /// // Spawn a deamon future.
    /// ex.spawn(async {
    ///     loop {
    ///         println!("I'm a daemon task looping forever.");
    ///         Timer::after(Duration::from_secs(1)).await;
    ///     }
    /// })
    /// .fallible()
    /// .detach();
    /// ```
    pub fn detach(self) {
        self.task.detach()
    }

    /// Cancels the task and waits for it to stop running.
    ///
    /// Returns the task's output if it was completed just before it got canceled, or [`None`] if
    /// it didn't complete.
    ///
    /// While it's possible to simply drop the [`Task`] to cancel it, this is a cleaner way of
    /// canceling because it also waits for the task to stop running.
    ///
    /// # Examples
    ///
    /// ```
    /// # if cfg!(miri) { return; } // Miri does not support epoll
    /// use smol::{future, Executor, Timer};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let ex = Executor::new();
    ///
    /// // Spawn a deamon future.
    /// let task = ex.spawn(async {
    ///     loop {
    ///         println!("Even though I'm in an infinite loop, you can still cancel me!");
    ///         Timer::after(Duration::from_secs(1)).await;
    ///     }
    /// })
    /// .fallible();
    ///
    /// // Run an executor thread.
    /// thread::spawn(move || future::block_on(ex.run(future::pending::<()>())));
    ///
    /// future::block_on(async {
    ///     Timer::after(Duration::from_secs(3)).await;
    ///     task.cancel().await;
    /// });
    /// ```
    pub async fn cancel(self) -> Option<T> {
        self.task.cancel().await
    }

    /// Returns `true` if the current task is finished.
    ///
    /// Note that in a multithreaded environment, this task can change finish immediately after calling this function.
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
}

impl<T, M> Future for FallibleTask<T, M> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.task.poll_task(cx)
    }
}

impl<T, M: fmt::Debug> fmt::Debug for FallibleTask<T, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FallibleTask")
            .field("header", self.task.header())
            .finish()
    }
}
