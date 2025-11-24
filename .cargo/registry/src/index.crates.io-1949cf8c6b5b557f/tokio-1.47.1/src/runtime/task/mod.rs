//! The task module.
//!
//! The task module contains the code that manages spawned tasks and provides a
//! safe API for the rest of the runtime to use. Each task in a runtime is
//! stored in an `OwnedTasks` or `LocalOwnedTasks` object.
//!
//! # Task reference types
//!
//! A task is usually referenced by multiple handles, and there are several
//! types of handles.
//!
//!  * `OwnedTask` - tasks stored in an `OwnedTasks` or `LocalOwnedTasks` are of this
//!    reference type.
//!
//!  * `JoinHandle` - each task has a `JoinHandle` that allows access to the output
//!    of the task.
//!
//!  * `Waker` - every waker for a task has this reference type. There can be any
//!    number of waker references.
//!
//!  * `Notified` - tracks whether the task is notified.
//!
//!  * `Unowned` - this task reference type is used for tasks not stored in any
//!    runtime. Mainly used for blocking tasks, but also in tests.
//!
//! The task uses a reference count to keep track of how many active references
//! exist. The `Unowned` reference type takes up two ref-counts. All other
//! reference types take up a single ref-count.
//!
//! Besides the waker type, each task has at most one of each reference type.
//!
//! # State
//!
//! The task stores its state in an atomic `usize` with various bitfields for the
//! necessary information. The state has the following bitfields:
//!
//!  * `RUNNING` - Tracks whether the task is currently being polled or cancelled.
//!    This bit functions as a lock around the task.
//!
//!  * `COMPLETE` - Is one once the future has fully completed and has been
//!    dropped. Never unset once set. Never set together with RUNNING.
//!
//!  * `NOTIFIED` - Tracks whether a Notified object currently exists.
//!
//!  * `CANCELLED` - Is set to one for tasks that should be cancelled as soon as
//!    possible. May take any value for completed tasks.
//!
//!  * `JOIN_INTEREST` - Is set to one if there exists a `JoinHandle`.
//!
//!  * `JOIN_WAKER` - Acts as an access control bit for the join handle waker. The
//!    protocol for its usage is described below.
//!
//! The rest of the bits are used for the ref-count.
//!
//! # Fields in the task
//!
//! The task has various fields. This section describes how and when it is safe
//! to access a field.
//!
//!  * The state field is accessed with atomic instructions.
//!
//!  * The `OwnedTask` reference has exclusive access to the `owned` field.
//!
//!  * The Notified reference has exclusive access to the `queue_next` field.
//!
//!  * The `owner_id` field can be set as part of construction of the task, but
//!    is otherwise immutable and anyone can access the field immutably without
//!    synchronization.
//!
//!  * If COMPLETE is one, then the `JoinHandle` has exclusive access to the
//!    stage field. If COMPLETE is zero, then the RUNNING bitfield functions as
//!    a lock for the stage field, and it can be accessed only by the thread
//!    that set RUNNING to one.
//!
//!  * The waker field may be concurrently accessed by different threads: in one
//!    thread the runtime may complete a task and *read* the waker field to
//!    invoke the waker, and in another thread the task's `JoinHandle` may be
//!    polled, and if the task hasn't yet completed, the `JoinHandle` may *write*
//!    a waker to the waker field. The `JOIN_WAKER` bit ensures safe access by
//!    multiple threads to the waker field using the following rules:
//!
//!    1. `JOIN_WAKER` is initialized to zero.
//!
//!    2. If `JOIN_WAKER` is zero, then the `JoinHandle` has exclusive (mutable)
//!       access to the waker field.
//!
//!    3. If `JOIN_WAKER` is one, then the `JoinHandle` has shared (read-only)
//!       access to the waker field.
//!
//!    4. If `JOIN_WAKER` is one and COMPLETE is one, then the runtime has shared
//!       (read-only) access to the waker field.
//!
//!    5. If the `JoinHandle` needs to write to the waker field, then the
//!       `JoinHandle` needs to (i) successfully set `JOIN_WAKER` to zero if it is
//!       not already zero to gain exclusive access to the waker field per rule
//!       2, (ii) write a waker, and (iii) successfully set `JOIN_WAKER` to one.
//!       If the `JoinHandle` unsets `JOIN_WAKER` in the process of being dropped
//!       to clear the waker field, only steps (i) and (ii) are relevant.
//!
//!    6. The `JoinHandle` can change `JOIN_WAKER` only if COMPLETE is zero (i.e.
//!       the task hasn't yet completed). The runtime can change `JOIN_WAKER` only
//!       if COMPLETE is one.
//!
//!    7. If `JOIN_INTEREST` is zero and COMPLETE is one, then the runtime has
//!       exclusive (mutable) access to the waker field. This might happen if the
//!       `JoinHandle` gets dropped right after the task completes and the runtime
//!       sets the `COMPLETE` bit. In this case the runtime needs the mutable access
//!       to the waker field to drop it.
//!
//!    Rule 6 implies that the steps (i) or (iii) of rule 5 may fail due to a
//!    race. If step (i) fails, then the attempt to write a waker is aborted. If
//!    step (iii) fails because COMPLETE is set to one by another thread after
//!    step (i), then the waker field is cleared. Once COMPLETE is one (i.e.
//!    task has completed), the `JoinHandle` will not modify `JOIN_WAKER`. After the
//!    runtime sets COMPLETE to one, it invokes the waker if there is one so in this
//!    case when a task completes the `JOIN_WAKER` bit implicates to the runtime
//!    whether it should invoke the waker or not. After the runtime is done with
//!    using the waker during task completion, it unsets the `JOIN_WAKER` bit to give
//!    the `JoinHandle` exclusive access again so that it is able to drop the waker
//!    at a later point.
//!
//! All other fields are immutable and can be accessed immutably without
//! synchronization by anyone.
//!
//! # Safety
//!
//! This section goes through various situations and explains why the API is
//! safe in that situation.
//!
//! ## Polling or dropping the future
//!
//! Any mutable access to the future happens after obtaining a lock by modifying
//! the RUNNING field, so exclusive access is ensured.
//!
//! When the task completes, exclusive access to the output is transferred to
//! the `JoinHandle`. If the `JoinHandle` is already dropped when the transition to
//! complete happens, the thread performing that transition retains exclusive
//! access to the output and should immediately drop it.
//!
//! ## Non-Send futures
//!
//! If a future is not Send, then it is bound to a `LocalOwnedTasks`.  The future
//! will only ever be polled or dropped given a `LocalNotified` or inside a call
//! to `LocalOwnedTasks::shutdown_all`. In either case, it is guaranteed that the
//! future is on the right thread.
//!
//! If the task is never removed from the `LocalOwnedTasks`, then it is leaked, so
//! there is no risk that the task is dropped on some other thread when the last
//! ref-count drops.
//!
//! ## Non-Send output
//!
//! When a task completes, the output is placed in the stage of the task. Then,
//! a transition that sets COMPLETE to true is performed, and the value of
//! `JOIN_INTEREST` when this transition happens is read.
//!
//! If `JOIN_INTEREST` is zero when the transition to COMPLETE happens, then the
//! output is immediately dropped.
//!
//! If `JOIN_INTEREST` is one when the transition to COMPLETE happens, then the
//! `JoinHandle` is responsible for cleaning up the output. If the output is not
//! Send, then this happens:
//!
//!  1. The output is created on the thread that the future was polled on. Since
//!     only non-Send futures can have non-Send output, the future was polled on
//!     the thread that the future was spawned from.
//!  2. Since `JoinHandle<Output>` is not Send if Output is not Send, the
//!     `JoinHandle` is also on the thread that the future was spawned from.
//!  3. Thus, the `JoinHandle` will not move the output across threads when it
//!     takes or drops the output.
//!
//! ## Recursive poll/shutdown
//!
//! Calling poll from inside a shutdown call or vice-versa is not prevented by
//! the API exposed by the task module, so this has to be safe. In either case,
//! the lock in the RUNNING bitfield makes the inner call return immediately. If
//! the inner call is a `shutdown` call, then the CANCELLED bit is set, and the
//! poll call will notice it when the poll finishes, and the task is cancelled
//! at that point.

// Some task infrastructure is here to support `JoinSet`, which is currently
// unstable. This should be removed once `JoinSet` is stabilized.
#![cfg_attr(not(tokio_unstable), allow(dead_code))]

mod core;
use self::core::Cell;
use self::core::Header;

mod error;
pub use self::error::JoinError;

mod harness;
use self::harness::Harness;

mod id;
#[cfg_attr(not(tokio_unstable), allow(unreachable_pub, unused_imports))]
pub use id::{id, try_id, Id};

#[cfg(feature = "rt")]
mod abort;
mod join;

#[cfg(feature = "rt")]
pub use self::abort::AbortHandle;

pub use self::join::JoinHandle;

mod list;
pub(crate) use self::list::{LocalOwnedTasks, OwnedTasks};

mod raw;
pub(crate) use self::raw::RawTask;

mod state;
use self::state::State;

mod waker;

pub(crate) use self::spawn_location::SpawnLocation;

cfg_taskdump! {
    pub(crate) mod trace;
}

use crate::future::Future;
use crate::util::linked_list;
use crate::util::sharded_list;

use crate::runtime::TaskCallback;
use std::marker::PhantomData;
use std::panic::Location;
use std::ptr::NonNull;
use std::{fmt, mem};

/// An owned handle to the task, tracked by ref count.
#[repr(transparent)]
pub(crate) struct Task<S: 'static> {
    raw: RawTask,
    _p: PhantomData<S>,
}

unsafe impl<S> Send for Task<S> {}
unsafe impl<S> Sync for Task<S> {}

/// A task was notified.
#[repr(transparent)]
pub(crate) struct Notified<S: 'static>(Task<S>);

impl<S> Notified<S> {
    #[cfg(all(tokio_unstable, feature = "rt-multi-thread"))]
    #[inline]
    pub(crate) fn task_meta<'task, 'meta>(&'task self) -> crate::runtime::TaskMeta<'meta> {
        self.0.task_meta()
    }
}

// safety: This type cannot be used to touch the task without first verifying
// that the value is on a thread where it is safe to poll the task.
unsafe impl<S: Schedule> Send for Notified<S> {}
unsafe impl<S: Schedule> Sync for Notified<S> {}

/// A non-Send variant of Notified with the invariant that it is on a thread
/// where it is safe to poll it.
#[repr(transparent)]
pub(crate) struct LocalNotified<S: 'static> {
    task: Task<S>,
    _not_send: PhantomData<*const ()>,
}

impl<S> LocalNotified<S> {
    #[cfg(tokio_unstable)]
    #[inline]
    pub(crate) fn task_meta<'task, 'meta>(&'task self) -> crate::runtime::TaskMeta<'meta> {
        self.task.task_meta()
    }
}

/// A task that is not owned by any `OwnedTasks`. Used for blocking tasks.
/// This type holds two ref-counts.
pub(crate) struct UnownedTask<S: 'static> {
    raw: RawTask,
    _p: PhantomData<S>,
}

// safety: This type can only be created given a Send task.
unsafe impl<S> Send for UnownedTask<S> {}
unsafe impl<S> Sync for UnownedTask<S> {}

/// Task result sent back.
pub(crate) type Result<T> = std::result::Result<T, JoinError>;

/// Hooks for scheduling tasks which are needed in the task harness.
#[derive(Clone)]
pub(crate) struct TaskHarnessScheduleHooks {
    pub(crate) task_terminate_callback: Option<TaskCallback>,
}

pub(crate) trait Schedule: Sync + Sized + 'static {
    /// The task has completed work and is ready to be released. The scheduler
    /// should release it immediately and return it. The task module will batch
    /// the ref-dec with setting other options.
    ///
    /// If the scheduler has already released the task, then None is returned.
    fn release(&self, task: &Task<Self>) -> Option<Task<Self>>;

    /// Schedule the task
    fn schedule(&self, task: Notified<Self>);

    fn hooks(&self) -> TaskHarnessScheduleHooks;

    /// Schedule the task to run in the near future, yielding the thread to
    /// other tasks.
    fn yield_now(&self, task: Notified<Self>) {
        self.schedule(task);
    }

    /// Polling the task resulted in a panic. Should the runtime shutdown?
    fn unhandled_panic(&self) {
        // By default, do nothing. This maintains the 1.0 behavior.
    }
}

cfg_rt! {
    /// This is the constructor for a new task. Three references to the task are
    /// created. The first task reference is usually put into an `OwnedTasks`
    /// immediately. The Notified is sent to the scheduler as an ordinary
    /// notification.
    fn new_task<T, S>(
        task: T,
        scheduler: S,
        id: Id,
        spawned_at: SpawnLocation,
    ) -> (Task<S>, Notified<S>, JoinHandle<T::Output>)
    where
        S: Schedule,
        T: Future + 'static,
        T::Output: 'static,
    {
        let raw = RawTask::new::<T, S>(
            task,
            scheduler,
            id,
            spawned_at,
        );
        let task = Task {
            raw,
            _p: PhantomData,
        };
        let notified = Notified(Task {
            raw,
            _p: PhantomData,
        });
        let join = JoinHandle::new(raw);

        (task, notified, join)
    }

    /// Creates a new task with an associated join handle. This method is used
    /// only when the task is not going to be stored in an `OwnedTasks` list.
    ///
    /// Currently only blocking tasks use this method.
    pub(crate) fn unowned<T, S>(
        task: T,
        scheduler: S,
        id: Id,
        spawned_at: SpawnLocation,
    ) -> (UnownedTask<S>, JoinHandle<T::Output>)
    where
        S: Schedule,
        T: Send + Future + 'static,
        T::Output: Send + 'static,
    {
        let (task, notified, join) = new_task(
            task,
            scheduler,
            id,
            spawned_at,
        );

        // This transfers the ref-count of task and notified into an UnownedTask.
        // This is valid because an UnownedTask holds two ref-counts.
        let unowned = UnownedTask {
            raw: task.raw,
            _p: PhantomData,
        };
        std::mem::forget(task);
        std::mem::forget(notified);

        (unowned, join)
    }
}

impl<S: 'static> Task<S> {
    unsafe fn new(raw: RawTask) -> Task<S> {
        Task {
            raw,
            _p: PhantomData,
        }
    }

    unsafe fn from_raw(ptr: NonNull<Header>) -> Task<S> {
        Task::new(RawTask::from_raw(ptr))
    }

    #[cfg(all(
        tokio_unstable,
        tokio_taskdump,
        feature = "rt",
        target_os = "linux",
        any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
    ))]
    pub(super) fn as_raw(&self) -> RawTask {
        self.raw
    }

    fn header(&self) -> &Header {
        self.raw.header()
    }

    fn header_ptr(&self) -> NonNull<Header> {
        self.raw.header_ptr()
    }

    /// Returns a [task ID] that uniquely identifies this task relative to other
    /// currently spawned tasks.
    ///
    /// [task ID]: crate::task::Id
    #[cfg(tokio_unstable)]
    pub(crate) fn id(&self) -> crate::task::Id {
        // Safety: The header pointer is valid.
        unsafe { Header::get_id(self.raw.header_ptr()) }
    }

    #[cfg(tokio_unstable)]
    pub(crate) fn spawned_at(&self) -> &'static Location<'static> {
        // Safety: The header pointer is valid.
        unsafe { Header::get_spawn_location(self.raw.header_ptr()) }
    }

    // Explicit `'task` and `'meta` lifetimes are necessary here, as otherwise,
    // the compiler infers the lifetimes to be the same, and considers the task
    // to be borrowed for the lifetime of the returned `TaskMeta`.
    #[cfg(tokio_unstable)]
    pub(crate) fn task_meta<'task, 'meta>(&'task self) -> crate::runtime::TaskMeta<'meta> {
        crate::runtime::TaskMeta {
            id: self.id(),
            spawned_at: self.spawned_at().into(),
            _phantom: PhantomData,
        }
    }

    cfg_taskdump! {
        /// Notify the task for task dumping.
        ///
        /// Returns `None` if the task has already been notified.
        pub(super) fn notify_for_tracing(&self) -> Option<Notified<S>> {
            if self.as_raw().state().transition_to_notified_for_tracing() {
                // SAFETY: `transition_to_notified_for_tracing` increments the
                // refcount.
                Some(unsafe { Notified(Task::new(self.raw)) })
            } else {
                None
            }
        }

    }
}

impl<S: 'static> Notified<S> {
    fn header(&self) -> &Header {
        self.0.header()
    }

    #[cfg(tokio_unstable)]
    #[allow(dead_code)]
    pub(crate) fn task_id(&self) -> crate::task::Id {
        self.0.id()
    }
}

impl<S: 'static> Notified<S> {
    pub(crate) unsafe fn from_raw(ptr: RawTask) -> Notified<S> {
        Notified(Task::new(ptr))
    }
}

impl<S: 'static> Notified<S> {
    pub(crate) fn into_raw(self) -> RawTask {
        let raw = self.0.raw;
        mem::forget(self);
        raw
    }
}

impl<S: Schedule> Task<S> {
    /// Preemptively cancels the task as part of the shutdown process.
    pub(crate) fn shutdown(self) {
        let raw = self.raw;
        mem::forget(self);
        raw.shutdown();
    }
}

impl<S: Schedule> LocalNotified<S> {
    /// Runs the task.
    pub(crate) fn run(self) {
        let raw = self.task.raw;
        mem::forget(self);
        raw.poll();
    }
}

impl<S: Schedule> UnownedTask<S> {
    // Used in test of the inject queue.
    #[cfg(test)]
    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    pub(super) fn into_notified(self) -> Notified<S> {
        Notified(self.into_task())
    }

    fn into_task(self) -> Task<S> {
        // Convert into a task.
        let task = Task {
            raw: self.raw,
            _p: PhantomData,
        };
        mem::forget(self);

        // Drop a ref-count since an UnownedTask holds two.
        task.header().state.ref_dec();

        task
    }

    pub(crate) fn run(self) {
        let raw = self.raw;
        mem::forget(self);

        // Transfer one ref-count to a Task object.
        let task = Task::<S> {
            raw,
            _p: PhantomData,
        };

        // Use the other ref-count to poll the task.
        raw.poll();
        // Decrement our extra ref-count
        drop(task);
    }

    pub(crate) fn shutdown(self) {
        self.into_task().shutdown();
    }
}

impl<S: 'static> Drop for Task<S> {
    fn drop(&mut self) {
        // Decrement the ref count
        if self.header().state.ref_dec() {
            // Deallocate if this is the final ref count
            self.raw.dealloc();
        }
    }
}

impl<S: 'static> Drop for UnownedTask<S> {
    fn drop(&mut self) {
        // Decrement the ref count
        if self.raw.header().state.ref_dec_twice() {
            // Deallocate if this is the final ref count
            self.raw.dealloc();
        }
    }
}

impl<S> fmt::Debug for Task<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "Task({:p})", self.header())
    }
}

impl<S> fmt::Debug for Notified<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "task::Notified({:p})", self.0.header())
    }
}

/// # Safety
///
/// Tasks are pinned.
unsafe impl<S> linked_list::Link for Task<S> {
    type Handle = Task<S>;
    type Target = Header;

    fn as_raw(handle: &Task<S>) -> NonNull<Header> {
        handle.raw.header_ptr()
    }

    unsafe fn from_raw(ptr: NonNull<Header>) -> Task<S> {
        Task::from_raw(ptr)
    }

    unsafe fn pointers(target: NonNull<Header>) -> NonNull<linked_list::Pointers<Header>> {
        self::core::Trailer::addr_of_owned(Header::get_trailer(target))
    }
}

/// # Safety
///
/// The id of a task is never changed after creation of the task, so the return value of
/// `get_shard_id` will not change. (The cast may throw away the upper 32 bits of the task id, but
/// the shard id still won't change from call to call.)
unsafe impl<S> sharded_list::ShardedListItem for Task<S> {
    unsafe fn get_shard_id(target: NonNull<Self::Target>) -> usize {
        // SAFETY: The caller guarantees that `target` points at a valid task.
        let task_id = unsafe { Header::get_id(target) };
        task_id.0.get() as usize
    }
}

/// Wrapper around [`std::panic::Location`] that's conditionally compiled out
/// when `tokio_unstable` is not enabled.
#[cfg(tokio_unstable)]
mod spawn_location {

    use std::panic::Location;

    #[derive(Copy, Clone)]
    pub(crate) struct SpawnLocation(pub &'static Location<'static>);

    impl From<&'static Location<'static>> for SpawnLocation {
        fn from(location: &'static Location<'static>) -> Self {
            Self(location)
        }
    }
}

#[cfg(not(tokio_unstable))]
mod spawn_location {
    use std::panic::Location;

    #[derive(Copy, Clone)]
    pub(crate) struct SpawnLocation();

    impl From<&'static Location<'static>> for SpawnLocation {
        fn from(_: &'static Location<'static>) -> Self {
            Self()
        }
    }

    #[cfg(test)]
    #[test]
    fn spawn_location_is_zero_sized() {
        assert_eq!(std::mem::size_of::<SpawnLocation>(), 0);
    }
}

impl SpawnLocation {
    #[track_caller]
    #[inline]
    pub(crate) fn capture() -> Self {
        Self::from(Location::caller())
    }
}
