use core::fmt;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::ptr::NonNull;
use core::sync::atomic::Ordering;
use core::task::Waker;

use alloc::boxed::Box;

use crate::header::Header;
use crate::raw::RawTask;
use crate::state::*;
use crate::Task;

mod sealed {
    use super::*;
    pub trait Sealed<M> {}

    impl<M, F> Sealed<M> for F where F: Fn(Runnable<M>) {}

    impl<M, F> Sealed<M> for WithInfo<F> where F: Fn(Runnable<M>, ScheduleInfo) {}
}

/// A builder that creates a new task.
#[derive(Debug)]
pub struct Builder<M> {
    /// The metadata associated with the task.
    pub(crate) metadata: M,

    /// Whether or not a panic that occurs in the task should be propagated.
    #[cfg(feature = "std")]
    pub(crate) propagate_panic: bool,
}

impl<M: Default> Default for Builder<M> {
    fn default() -> Self {
        Builder::new().metadata(M::default())
    }
}

/// Extra scheduling information that can be passed to the scheduling function.
///
/// The data source of this struct is directly from the actual implementation
/// of the crate itself, different from [`Runnable`]'s metadata, which is
/// managed by the caller.
///
/// # Examples
///
/// ```
/// use async_task::{Runnable, ScheduleInfo, WithInfo};
/// use std::sync::{Arc, Mutex};
///
/// // The future inside the task.
/// let future = async {
///     println!("Hello, world!");
/// };
///
/// // If the task gets woken up while running, it will be sent into this channel.
/// let (s, r) = flume::unbounded();
/// // Otherwise, it will be placed into this slot.
/// let lifo_slot = Arc::new(Mutex::new(None));
/// let schedule = move |runnable: Runnable, info: ScheduleInfo| {
///     if info.woken_while_running {
///         s.send(runnable).unwrap()
///     } else {
///         let last = lifo_slot.lock().unwrap().replace(runnable);
///         if let Some(last) = last {
///             s.send(last).unwrap()
///         }
///     }
/// };
///
/// // Create the actual scheduler to be spawned with some future.
/// let scheduler = WithInfo(schedule);
/// // Create a task with the future and the scheduler.
/// let (runnable, task) = async_task::spawn(future, scheduler);
/// ```
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub struct ScheduleInfo {
    /// Indicates whether the task gets woken up while running.
    ///
    /// It is set to true usually because the task has yielded itself to the
    /// scheduler.
    pub woken_while_running: bool,
}

impl ScheduleInfo {
    pub(crate) fn new(woken_while_running: bool) -> Self {
        ScheduleInfo {
            woken_while_running,
        }
    }
}

/// The trait for scheduling functions.
pub trait Schedule<M = ()>: sealed::Sealed<M> {
    /// The actual scheduling procedure.
    fn schedule(&self, runnable: Runnable<M>, info: ScheduleInfo);
}

impl<M, F> Schedule<M> for F
where
    F: Fn(Runnable<M>),
{
    fn schedule(&self, runnable: Runnable<M>, _: ScheduleInfo) {
        self(runnable)
    }
}

/// Pass a scheduling function with more scheduling information - a.k.a.
/// [`ScheduleInfo`].
///
/// Sometimes, it's useful to pass the runnable's state directly to the
/// scheduling function, such as whether it's woken up while running. The
/// scheduler can thus use the information to determine its scheduling
/// strategy.
///
/// The data source of [`ScheduleInfo`] is directly from the actual
/// implementation of the crate itself, different from [`Runnable`]'s metadata,
/// which is managed by the caller.
///
/// # Examples
///
/// ```
/// use async_task::{ScheduleInfo, WithInfo};
/// use std::sync::{Arc, Mutex};
///
/// // The future inside the task.
/// let future = async {
///     println!("Hello, world!");
/// };
///
/// // If the task gets woken up while running, it will be sent into this channel.
/// let (s, r) = flume::unbounded();
/// // Otherwise, it will be placed into this slot.
/// let lifo_slot = Arc::new(Mutex::new(None));
/// let schedule = move |runnable, info: ScheduleInfo| {
///     if info.woken_while_running {
///         s.send(runnable).unwrap()
///     } else {
///         let last = lifo_slot.lock().unwrap().replace(runnable);
///         if let Some(last) = last {
///             s.send(last).unwrap()
///         }
///     }
/// };
///
/// // Create a task with the future and the schedule function.
/// let (runnable, task) = async_task::spawn(future, WithInfo(schedule));
/// ```
#[derive(Debug)]
pub struct WithInfo<F>(pub F);

impl<F> From<F> for WithInfo<F> {
    fn from(value: F) -> Self {
        WithInfo(value)
    }
}

impl<M, F> Schedule<M> for WithInfo<F>
where
    F: Fn(Runnable<M>, ScheduleInfo),
{
    fn schedule(&self, runnable: Runnable<M>, info: ScheduleInfo) {
        (self.0)(runnable, info)
    }
}

impl Builder<()> {
    /// Creates a new task builder.
    ///
    /// By default, this task builder has no metadata. Use the [`metadata`] method to
    /// set the metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_task::Builder;
    ///
    /// let (runnable, task) = Builder::new().spawn(|()| async {}, |_| {});
    /// ```
    pub fn new() -> Builder<()> {
        Builder {
            metadata: (),
            #[cfg(feature = "std")]
            propagate_panic: false,
        }
    }

    /// Adds metadata to the task.
    ///
    /// In certain cases, it may be useful to associate some metadata with a task. For instance,
    /// you may want to associate a name with a task, or a priority for a priority queue. This
    /// method allows the user to attach arbitrary metadata to a task that is available through
    /// the [`Runnable`] or the [`Task`].
    ///
    /// # Examples
    ///
    /// This example creates an executor that associates a "priority" number with each task, and
    /// then runs the tasks in order of priority.
    ///
    /// ```
    /// use async_task::{Builder, Runnable};
    /// use once_cell::sync::Lazy;
    /// use std::cmp;
    /// use std::collections::BinaryHeap;
    /// use std::sync::Mutex;
    ///
    /// # smol::future::block_on(async {
    /// /// A wrapper around a `Runnable<usize>` that implements `Ord` so that it can be used in a
    /// /// priority queue.
    /// struct TaskWrapper(Runnable<usize>);
    ///
    /// impl PartialEq for TaskWrapper {
    ///     fn eq(&self, other: &Self) -> bool {
    ///         self.0.metadata() == other.0.metadata()
    ///     }
    /// }
    ///
    /// impl Eq for TaskWrapper {}
    ///
    /// impl PartialOrd for TaskWrapper {
    ///    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
    ///       Some(self.cmp(other))
    ///    }
    /// }
    ///
    /// impl Ord for TaskWrapper {
    ///    fn cmp(&self, other: &Self) -> cmp::Ordering {
    ///        self.0.metadata().cmp(other.0.metadata())
    ///    }
    /// }
    ///
    /// static EXECUTOR: Lazy<Mutex<BinaryHeap<TaskWrapper>>> = Lazy::new(|| {
    ///     Mutex::new(BinaryHeap::new())
    /// });
    ///
    /// let schedule = |runnable| {
    ///     EXECUTOR.lock().unwrap().push(TaskWrapper(runnable));
    /// };
    ///
    /// // Spawn a few tasks with different priorities.
    /// let spawn_task = move |priority| {
    ///     let (runnable, task) = Builder::new().metadata(priority).spawn(
    ///         move |_| async move { priority },
    ///         schedule,
    ///     );
    ///     runnable.schedule();
    ///     task
    /// };
    ///
    /// let t1 = spawn_task(1);
    /// let t2 = spawn_task(2);
    /// let t3 = spawn_task(3);
    ///
    /// // Run the tasks in order of priority.
    /// let mut metadata_seen = vec![];
    /// while let Some(TaskWrapper(runnable)) = EXECUTOR.lock().unwrap().pop() {
    ///     metadata_seen.push(*runnable.metadata());
    ///     runnable.run();
    /// }
    ///
    /// assert_eq!(metadata_seen, vec![3, 2, 1]);
    /// assert_eq!(t1.await, 1);
    /// assert_eq!(t2.await, 2);
    /// assert_eq!(t3.await, 3);
    /// # });
    /// ```
    pub fn metadata<M>(self, metadata: M) -> Builder<M> {
        Builder {
            metadata,
            #[cfg(feature = "std")]
            propagate_panic: self.propagate_panic,
        }
    }
}

impl<M> Builder<M> {
    /// Propagates panics that occur in the task.
    ///
    /// When this is `true`, panics that occur in the task will be propagated to the caller of
    /// the [`Task`]. When this is false, no special action is taken when a panic occurs in the
    /// task, meaning that the caller of [`Runnable::run`] will observe a panic.
    ///
    /// This is only available when the `std` feature is enabled. By default, this is `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_task::Builder;
    /// use futures_lite::future::poll_fn;
    /// use std::future::Future;
    /// use std::panic;
    /// use std::pin::Pin;
    /// use std::task::{Context, Poll};
    ///
    /// fn did_panic<F: FnOnce()>(f: F) -> bool {
    ///     panic::catch_unwind(panic::AssertUnwindSafe(f)).is_err()
    /// }
    ///
    /// # smol::future::block_on(async {
    /// let (runnable1, mut task1) = Builder::new()
    ///    .propagate_panic(true)
    ///    .spawn(|()| async move { panic!() }, |_| {});
    ///
    /// let (runnable2, mut task2) = Builder::new()
    ///    .propagate_panic(false)
    ///    .spawn(|()| async move { panic!() }, |_| {});
    ///
    /// assert!(!did_panic(|| { runnable1.run(); }));
    /// assert!(did_panic(|| { runnable2.run(); }));
    ///
    /// let waker = poll_fn(|cx| Poll::Ready(cx.waker().clone())).await;
    /// let mut cx = Context::from_waker(&waker);
    /// assert!(did_panic(|| { let _ = Pin::new(&mut task1).poll(&mut cx); }));
    /// assert!(did_panic(|| { let _ = Pin::new(&mut task2).poll(&mut cx); }));
    /// # });
    /// ```
    #[cfg(feature = "std")]
    pub fn propagate_panic(self, propagate_panic: bool) -> Builder<M> {
        Builder {
            metadata: self.metadata,
            propagate_panic,
        }
    }

    /// Creates a new task.
    ///
    /// The returned [`Runnable`] is used to poll the `future`, and the [`Task`] is used to await its
    /// output.
    ///
    /// Method [`run()`][`Runnable::run()`] polls the task's future once. Then, the [`Runnable`]
    /// vanishes and only reappears when its [`Waker`] wakes the task, thus scheduling it to be run
    /// again.
    ///
    /// When the task is woken, its [`Runnable`] is passed to the `schedule` function.
    /// The `schedule` function should not attempt to run the [`Runnable`] nor to drop it. Instead, it
    /// should push it into a task queue so that it can be processed later.
    ///
    /// If you need to spawn a future that does not implement [`Send`] or isn't `'static`, consider
    /// using [`spawn_local()`] or [`spawn_unchecked()`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_task::Builder;
    ///
    /// // The future inside the task.
    /// let future = async {
    ///     println!("Hello, world!");
    /// };
    ///
    /// // A function that schedules the task when it gets woken up.
    /// let (s, r) = flume::unbounded();
    /// let schedule = move |runnable| s.send(runnable).unwrap();
    ///
    /// // Create a task with the future and the schedule function.
    /// let (runnable, task) = Builder::new().spawn(|()| future, schedule);
    /// ```
    pub fn spawn<F, Fut, S>(self, future: F, schedule: S) -> (Runnable<M>, Task<Fut::Output, M>)
    where
        F: FnOnce(&M) -> Fut,
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
        S: Schedule<M> + Send + Sync + 'static,
    {
        unsafe { self.spawn_unchecked(future, schedule) }
    }

    /// Creates a new thread-local task.
    ///
    /// This function is same as [`spawn()`], except it does not require [`Send`] on `future`. If the
    /// [`Runnable`] is used or dropped on another thread, a panic will occur.
    ///
    /// This function is only available when the `std` feature for this crate is enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_task::{Builder, Runnable};
    /// use flume::{Receiver, Sender};
    /// use std::rc::Rc;
    ///
    /// thread_local! {
    ///     // A queue that holds scheduled tasks.
    ///     static QUEUE: (Sender<Runnable>, Receiver<Runnable>) = flume::unbounded();
    /// }
    ///
    /// // Make a non-Send future.
    /// let msg: Rc<str> = "Hello, world!".into();
    /// let future = async move {
    ///     println!("{}", msg);
    /// };
    ///
    /// // A function that schedules the task when it gets woken up.
    /// let s = QUEUE.with(|(s, _)| s.clone());
    /// let schedule = move |runnable| s.send(runnable).unwrap();
    ///
    /// // Create a task with the future and the schedule function.
    /// let (runnable, task) = Builder::new().spawn_local(move |()| future, schedule);
    /// ```
    #[cfg(feature = "std")]
    pub fn spawn_local<F, Fut, S>(
        self,
        future: F,
        schedule: S,
    ) -> (Runnable<M>, Task<Fut::Output, M>)
    where
        F: FnOnce(&M) -> Fut,
        Fut: Future + 'static,
        Fut::Output: 'static,
        S: Schedule<M> + Send + Sync + 'static,
    {
        use std::mem::ManuallyDrop;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use std::thread::{self, ThreadId};

        #[inline]
        fn thread_id() -> ThreadId {
            std::thread_local! {
                static ID: ThreadId = thread::current().id();
            }
            ID.try_with(|id| *id)
                .unwrap_or_else(|_| thread::current().id())
        }

        struct Checked<F> {
            id: ThreadId,
            inner: ManuallyDrop<F>,
        }

        impl<F> Drop for Checked<F> {
            fn drop(&mut self) {
                assert!(
                    self.id == thread_id(),
                    "local task dropped by a thread that didn't spawn it"
                );
                unsafe {
                    ManuallyDrop::drop(&mut self.inner);
                }
            }
        }

        impl<F: Future> Future for Checked<F> {
            type Output = F::Output;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                assert!(
                    self.id == thread_id(),
                    "local task polled by a thread that didn't spawn it"
                );
                unsafe { self.map_unchecked_mut(|c| &mut *c.inner).poll(cx) }
            }
        }

        // Wrap the future into one that checks which thread it's on.
        let future = move |meta| {
            let future = future(meta);

            Checked {
                id: thread_id(),
                inner: ManuallyDrop::new(future),
            }
        };

        unsafe { self.spawn_unchecked(future, schedule) }
    }

    /// Creates a new task without [`Send`], [`Sync`], and `'static` bounds.
    ///
    /// This function is same as [`spawn()`], except it does not require [`Send`], [`Sync`], and
    /// `'static` on `future` and `schedule`.
    ///
    /// # Safety
    ///
    /// - If `Fut` is not [`Send`], its [`Runnable`] must be used and dropped on the original
    ///   thread.
    /// - If `Fut` is not `'static`, borrowed non-metadata variables must outlive its [`Runnable`].
    /// - If `schedule` is not [`Send`] and [`Sync`], all instances of the [`Runnable`]'s [`Waker`]
    ///   must be used and dropped on the original thread.
    /// - If `schedule` is not `'static`, borrowed variables must outlive all instances of the
    ///   [`Runnable`]'s [`Waker`].
    ///
    /// # Examples
    ///
    /// ```
    /// use async_task::Builder;
    ///
    /// // The future inside the task.
    /// let future = async {
    ///     println!("Hello, world!");
    /// };
    ///
    /// // If the task gets woken up, it will be sent into this channel.
    /// let (s, r) = flume::unbounded();
    /// let schedule = move |runnable| s.send(runnable).unwrap();
    ///
    /// // Create a task with the future and the schedule function.
    /// let (runnable, task) = unsafe { Builder::new().spawn_unchecked(move |()| future, schedule) };
    /// ```
    pub unsafe fn spawn_unchecked<'a, F, Fut, S>(
        self,
        future: F,
        schedule: S,
    ) -> (Runnable<M>, Task<Fut::Output, M>)
    where
        F: FnOnce(&'a M) -> Fut,
        Fut: Future + 'a,
        S: Schedule<M>,
        M: 'a,
    {
        // Allocate large futures on the heap.
        let ptr = if mem::size_of::<Fut>() >= 2048 {
            let future = |meta| {
                let future = future(meta);
                Box::pin(future)
            };

            RawTask::<_, Fut::Output, S, M>::allocate(future, schedule, self)
        } else {
            RawTask::<Fut, Fut::Output, S, M>::allocate(future, schedule, self)
        };

        let runnable = Runnable::from_raw(ptr);
        let task = Task {
            ptr,
            _marker: PhantomData,
        };
        (runnable, task)
    }
}

/// Creates a new task.
///
/// The returned [`Runnable`] is used to poll the `future`, and the [`Task`] is used to await its
/// output.
///
/// Method [`run()`][`Runnable::run()`] polls the task's future once. Then, the [`Runnable`]
/// vanishes and only reappears when its [`Waker`] wakes the task, thus scheduling it to be run
/// again.
///
/// When the task is woken, its [`Runnable`] is passed to the `schedule` function.
/// The `schedule` function should not attempt to run the [`Runnable`] nor to drop it. Instead, it
/// should push it into a task queue so that it can be processed later.
///
/// If you need to spawn a future that does not implement [`Send`] or isn't `'static`, consider
/// using [`spawn_local()`] or [`spawn_unchecked()`] instead.
///
/// # Examples
///
/// ```
/// // The future inside the task.
/// let future = async {
///     println!("Hello, world!");
/// };
///
/// // A function that schedules the task when it gets woken up.
/// let (s, r) = flume::unbounded();
/// let schedule = move |runnable| s.send(runnable).unwrap();
///
/// // Create a task with the future and the schedule function.
/// let (runnable, task) = async_task::spawn(future, schedule);
/// ```
pub fn spawn<F, S>(future: F, schedule: S) -> (Runnable, Task<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
    S: Schedule + Send + Sync + 'static,
{
    unsafe { spawn_unchecked(future, schedule) }
}

/// Creates a new thread-local task.
///
/// This function is same as [`spawn()`], except it does not require [`Send`] on `future`. If the
/// [`Runnable`] is used or dropped on another thread, a panic will occur.
///
/// This function is only available when the `std` feature for this crate is enabled.
///
/// # Examples
///
/// ```
/// use async_task::Runnable;
/// use flume::{Receiver, Sender};
/// use std::rc::Rc;
///
/// thread_local! {
///     // A queue that holds scheduled tasks.
///     static QUEUE: (Sender<Runnable>, Receiver<Runnable>) = flume::unbounded();
/// }
///
/// // Make a non-Send future.
/// let msg: Rc<str> = "Hello, world!".into();
/// let future = async move {
///     println!("{}", msg);
/// };
///
/// // A function that schedules the task when it gets woken up.
/// let s = QUEUE.with(|(s, _)| s.clone());
/// let schedule = move |runnable| s.send(runnable).unwrap();
///
/// // Create a task with the future and the schedule function.
/// let (runnable, task) = async_task::spawn_local(future, schedule);
/// ```
#[cfg(feature = "std")]
pub fn spawn_local<F, S>(future: F, schedule: S) -> (Runnable, Task<F::Output>)
where
    F: Future + 'static,
    F::Output: 'static,
    S: Schedule + Send + Sync + 'static,
{
    Builder::new().spawn_local(move |()| future, schedule)
}

/// Creates a new task without [`Send`], [`Sync`], and `'static` bounds.
///
/// This function is same as [`spawn()`], except it does not require [`Send`], [`Sync`], and
/// `'static` on `future` and `schedule`.
///
/// # Safety
///
/// - If `future` is not [`Send`], its [`Runnable`] must be used and dropped on the original
///   thread.
/// - If `future` is not `'static`, borrowed variables must outlive its [`Runnable`].
/// - If `schedule` is not [`Send`] and [`Sync`], all instances of the [`Runnable`]'s [`Waker`]
///   must be used and dropped on the original thread.
/// - If `schedule` is not `'static`, borrowed variables must outlive all instances of the
///   [`Runnable`]'s [`Waker`].
///
/// # Examples
///
/// ```
/// // The future inside the task.
/// let future = async {
///     println!("Hello, world!");
/// };
///
/// // If the task gets woken up, it will be sent into this channel.
/// let (s, r) = flume::unbounded();
/// let schedule = move |runnable| s.send(runnable).unwrap();
///
/// // Create a task with the future and the schedule function.
/// let (runnable, task) = unsafe { async_task::spawn_unchecked(future, schedule) };
/// ```
pub unsafe fn spawn_unchecked<F, S>(future: F, schedule: S) -> (Runnable, Task<F::Output>)
where
    F: Future,
    S: Schedule,
{
    Builder::new().spawn_unchecked(move |()| future, schedule)
}

/// A handle to a runnable task.
///
/// Every spawned task has a single [`Runnable`] handle, which only exists when the task is
/// scheduled for running.
///
/// Method [`run()`][`Runnable::run()`] polls the task's future once. Then, the [`Runnable`]
/// vanishes and only reappears when its [`Waker`] wakes the task, thus scheduling it to be run
/// again.
///
/// Dropping a [`Runnable`] cancels the task, which means its future won't be polled again, and
/// awaiting the [`Task`] after that will result in a panic.
///
/// # Examples
///
/// ```
/// use async_task::Runnable;
/// use once_cell::sync::Lazy;
/// use std::{panic, thread};
///
/// // A simple executor.
/// static QUEUE: Lazy<flume::Sender<Runnable>> = Lazy::new(|| {
///     let (sender, receiver) = flume::unbounded::<Runnable>();
///     thread::spawn(|| {
///         for runnable in receiver {
///             let _ignore_panic = panic::catch_unwind(|| runnable.run());
///         }
///     });
///     sender
/// });
///
/// // Create a task with a simple future.
/// let schedule = |runnable| QUEUE.send(runnable).unwrap();
/// let (runnable, task) = async_task::spawn(async { 1 + 2 }, schedule);
///
/// // Schedule the task and await its output.
/// runnable.schedule();
/// assert_eq!(smol::future::block_on(task), 3);
/// ```
pub struct Runnable<M = ()> {
    /// A pointer to the heap-allocated task.
    pub(crate) ptr: NonNull<()>,

    /// A marker capturing generic type `M`.
    pub(crate) _marker: PhantomData<M>,
}

unsafe impl<M: Send + Sync> Send for Runnable<M> {}
unsafe impl<M: Send + Sync> Sync for Runnable<M> {}

#[cfg(feature = "std")]
impl<M> std::panic::UnwindSafe for Runnable<M> {}
#[cfg(feature = "std")]
impl<M> std::panic::RefUnwindSafe for Runnable<M> {}

impl<M> Runnable<M> {
    /// Get the metadata associated with this task.
    ///
    /// Tasks can be created with a metadata object associated with them; by default, this
    /// is a `()` value. See the [`Builder::metadata()`] method for more information.
    pub fn metadata(&self) -> &M {
        &self.header().metadata
    }

    /// Schedules the task.
    ///
    /// This is a convenience method that passes the [`Runnable`] to the schedule function.
    ///
    /// # Examples
    ///
    /// ```
    /// // A function that schedules the task when it gets woken up.
    /// let (s, r) = flume::unbounded();
    /// let schedule = move |runnable| s.send(runnable).unwrap();
    ///
    /// // Create a task with a simple future and the schedule function.
    /// let (runnable, task) = async_task::spawn(async {}, schedule);
    ///
    /// // Schedule the task.
    /// assert_eq!(r.len(), 0);
    /// runnable.schedule();
    /// assert_eq!(r.len(), 1);
    /// ```
    pub fn schedule(self) {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;
        mem::forget(self);

        unsafe {
            ((*header).vtable.schedule)(ptr, ScheduleInfo::new(false));
        }
    }

    /// Runs the task by polling its future.
    ///
    /// Returns `true` if the task was woken while running, in which case the [`Runnable`] gets
    /// rescheduled at the end of this method invocation. Otherwise, returns `false` and the
    /// [`Runnable`] vanishes until the task is woken.
    /// The return value is just a hint: `true` usually indicates that the task has yielded, i.e.
    /// it woke itself and then gave the control back to the executor.
    ///
    /// If the [`Task`] handle was dropped or if [`cancel()`][`Task::cancel()`] was called, then
    /// this method simply destroys the task.
    ///
    /// If the polled future panics, this method propagates the panic, and awaiting the [`Task`]
    /// after that will also result in a panic.
    ///
    /// # Examples
    ///
    /// ```
    /// // A function that schedules the task when it gets woken up.
    /// let (s, r) = flume::unbounded();
    /// let schedule = move |runnable| s.send(runnable).unwrap();
    ///
    /// // Create a task with a simple future and the schedule function.
    /// let (runnable, task) = async_task::spawn(async { 1 + 2 }, schedule);
    ///
    /// // Run the task and check its output.
    /// runnable.run();
    /// assert_eq!(smol::future::block_on(task), 3);
    /// ```
    pub fn run(self) -> bool {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;
        mem::forget(self);

        unsafe { ((*header).vtable.run)(ptr) }
    }

    /// Returns a waker associated with this task.
    ///
    /// # Examples
    ///
    /// ```
    /// use smol::future;
    ///
    /// // A function that schedules the task when it gets woken up.
    /// let (s, r) = flume::unbounded();
    /// let schedule = move |runnable| s.send(runnable).unwrap();
    ///
    /// // Create a task with a simple future and the schedule function.
    /// let (runnable, task) = async_task::spawn(future::pending::<()>(), schedule);
    ///
    /// // Take a waker and run the task.
    /// let waker = runnable.waker();
    /// runnable.run();
    ///
    /// // Reschedule the task by waking it.
    /// assert_eq!(r.len(), 0);
    /// waker.wake();
    /// assert_eq!(r.len(), 1);
    /// ```
    pub fn waker(&self) -> Waker {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;

        unsafe {
            let raw_waker = ((*header).vtable.clone_waker)(ptr);
            Waker::from_raw(raw_waker)
        }
    }

    fn header(&self) -> &Header<M> {
        unsafe { &*(self.ptr.as_ptr() as *const Header<M>) }
    }

    /// Converts this task into a raw pointer.
    ///
    /// To avoid a memory leak the pointer must be converted back to a Runnable using [`Runnable<M>::from_raw`][from_raw].
    ///
    /// `into_raw` does not change the state of the [`Task`], but there is no guarantee that it will be in the same state after calling [`Runnable<M>::from_raw`][from_raw],
    /// as the corresponding [`Task`] might have been dropped or cancelled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use async_task::{Runnable, spawn};

    /// let (runnable, task) = spawn(async {}, |_| {});
    /// let runnable_pointer = runnable.into_raw();
    ///
    /// unsafe {
    ///     // Convert back to an `Runnable` to prevent leak.
    ///     let runnable = Runnable::<()>::from_raw(runnable_pointer);
    ///     runnable.run();
    ///     // Further calls to `Runnable::from_raw(runnable_pointer)` would be memory-unsafe.
    /// }
    /// // The memory was freed when `x` went out of scope above, so `runnable_pointer` is now dangling!
    /// ```
    /// [from_raw]: #method.from_raw
    pub fn into_raw(self) -> NonNull<()> {
        let ptr = self.ptr;
        mem::forget(self);
        ptr
    }

    /// Converts a raw pointer into a Runnable.
    ///
    /// # Safety
    ///
    /// This method should only be used with raw pointers returned from [`Runnable<M>::into_raw`][into_raw].
    /// It is not safe to use the provided pointer once it is passed to `from_raw`.
    /// Crucially, it is unsafe to call `from_raw` multiple times with the same pointer - even if the resulting [`Runnable`] is not used -
    /// as internally `async-task` uses reference counting.
    ///
    /// It is however safe to call [`Runnable<M>::into_raw`][into_raw] on a [`Runnable`] created with `from_raw` or
    /// after the [`Task`] associated with a given Runnable has been dropped or cancelled.
    ///
    /// The state of the [`Runnable`] created with `from_raw` is not specified.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use async_task::{Runnable, spawn};

    /// let (runnable, task) = spawn(async {}, |_| {});
    /// let runnable_pointer = runnable.into_raw();
    ///
    /// drop(task);
    /// unsafe {
    ///     // Convert back to an `Runnable` to prevent leak.
    ///     let runnable = Runnable::<()>::from_raw(runnable_pointer);
    ///     let did_poll = runnable.run();
    ///     assert!(!did_poll);
    ///     // Further calls to `Runnable::from_raw(runnable_pointer)` would be memory-unsafe.
    /// }
    /// // The memory was freed when `x` went out of scope above, so `runnable_pointer` is now dangling!
    /// ```

    /// [into_raw]: #method.into_raw
    pub unsafe fn from_raw(ptr: NonNull<()>) -> Self {
        Self {
            ptr,
            _marker: Default::default(),
        }
    }
}

impl<M> Drop for Runnable<M> {
    fn drop(&mut self) {
        let ptr = self.ptr.as_ptr();
        let header = self.header();

        unsafe {
            let mut state = header.state.load(Ordering::Acquire);

            loop {
                // If the task has been completed or closed, it can't be canceled.
                if state & (COMPLETED | CLOSED) != 0 {
                    break;
                }

                // Mark the task as closed.
                match header.state.compare_exchange_weak(
                    state,
                    state | CLOSED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(s) => state = s,
                }
            }

            // Drop the future.
            (header.vtable.drop_future)(ptr);

            // Mark the task as unscheduled.
            let state = header.state.fetch_and(!SCHEDULED, Ordering::AcqRel);

            // Notify the awaiter that the future has been dropped.
            if state & AWAITER != 0 {
                (*header).notify(None);
            }

            // Drop the task reference.
            (header.vtable.drop_ref)(ptr);
        }
    }
}

impl<M: fmt::Debug> fmt::Debug for Runnable<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.ptr.as_ptr();
        let header = ptr as *const Header<M>;

        f.debug_struct("Runnable")
            .field("header", unsafe { &(*header) })
            .finish()
    }
}
