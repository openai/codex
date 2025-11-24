use std::ffi::{c_int, c_void};
use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};
use std::task::{Context, Poll};

use futures_util::stream::{FuturesUnordered, Stream};

use super::error::hyper_code;
use super::UserDataPointer;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type BoxAny = Box<dyn AsTaskType + Send + Sync>;

/// Return in a poll function to indicate it was ready.
pub const HYPER_POLL_READY: c_int = 0;
/// Return in a poll function to indicate it is still pending.
///
/// The passed in `hyper_waker` should be registered to wake up the task at
/// some later point.
pub const HYPER_POLL_PENDING: c_int = 1;
/// Return in a poll function indicate an error.
pub const HYPER_POLL_ERROR: c_int = 3;

/// A task executor for `hyper_task`s.
///
/// A task is a unit of work that may be blocked on IO, and can be polled to
/// make progress on that work.
///
/// An executor can hold many tasks, included from unrelated HTTP connections.
/// An executor is single threaded. Typically you might have one executor per
/// thread. Or, for simplicity, you may choose one executor per connection.
///
/// Progress on tasks happens only when `hyper_executor_poll` is called, and only
/// on tasks whose corresponding `hyper_waker` has been called to indicate they
/// are ready to make progress (for instance, because the OS has indicated there
/// is more data to read or more buffer space available to write).
///
/// Deadlock potential: `hyper_executor_poll` must not be called from within a task's
/// callback. Doing so will result in a deadlock.
///
/// Methods:
///
/// - hyper_executor_new:  Creates a new task executor.
/// - hyper_executor_push: Push a task onto the executor.
/// - hyper_executor_poll: Polls the executor, trying to make progress on any tasks that have notified that they are ready again.
/// - hyper_executor_free: Frees an executor and any incomplete tasks still part of it.
pub struct hyper_executor {
    /// The executor of all task futures.
    ///
    /// There should never be contention on the mutex, as it is only locked
    /// to drive the futures. However, we cannot guarantee proper usage from
    /// `hyper_executor_poll()`, which in C could potentially be called inside
    /// one of the stored futures. The mutex isn't re-entrant, so doing so
    /// would result in a deadlock, but that's better than data corruption.
    driver: Mutex<FuturesUnordered<TaskFuture>>,

    /// The queue of futures that need to be pushed into the `driver`.
    ///
    /// This is has a separate mutex since `spawn` could be called from inside
    /// a future, which would mean the driver's mutex is already locked.
    spawn_queue: Mutex<Vec<TaskFuture>>,

    /// This is used to track when a future calls `wake` while we are within
    /// `hyper_executor::poll_next`.
    is_woken: Arc<ExecWaker>,
}

#[derive(Clone)]
pub(crate) struct WeakExec(Weak<hyper_executor>);

struct ExecWaker(AtomicBool);

/// An async task.
///
/// A task represents a chunk of work that will eventually yield exactly one
/// `hyper_task_value`. Tasks are pushed onto an executor, and that executor is
/// responsible for calling the necessary private functions on the task to make
/// progress. In most cases those private functions will eventually cause read
/// or write callbacks on a `hyper_io` object to be called.
///
/// Tasks are created by various functions:
///
/// - hyper_clientconn_handshake: Creates an HTTP client handshake task.
/// - hyper_clientconn_send:      Creates a task to send a request on the client connection.
/// - hyper_body_data:            Creates a task that will poll a response body for the next buffer of data.
/// - hyper_body_foreach:         Creates a task to execute the callback with each body chunk received.
///
/// Tasks then have a userdata associated with them using `hyper_task_set_userdata`. This
/// is important, for instance, to associate a request id with a given request. When multiple
/// tasks are running on the same executor, this allows distinguishing tasks for different
/// requests.
///
/// Tasks are then pushed onto an executor, and eventually yielded from hyper_executor_poll:
///
/// - hyper_executor_push:        Push a task onto the executor.
/// - hyper_executor_poll:        Polls the executor, trying to make progress on any tasks that have notified that they are ready again.
///
/// Once a task is yielded from poll, retrieve its userdata, check its type,
/// and extract its value. This will require a case from void* to the appropriate type.
///
/// Methods on hyper_task:
///
/// - hyper_task_type:            Query the return type of this task.
/// - hyper_task_value:           Takes the output value of this task.
/// - hyper_task_set_userdata:    Set a user data pointer to be associated with this task.
/// - hyper_task_userdata:        Retrieve the userdata that has been set via hyper_task_set_userdata.
/// - hyper_task_free:            Free a task.
pub struct hyper_task {
    future: BoxFuture<BoxAny>,
    output: Option<BoxAny>,
    userdata: UserDataPointer,
}

struct TaskFuture {
    task: Option<Box<hyper_task>>,
}

/// An async context for a task that contains the related waker.
///
/// This is provided to `hyper_io`'s read and write callbacks. Currently
/// its only purpose is to provide access to the waker. See `hyper_waker`.
///
/// Corresponding Rust type: <https://doc.rust-lang.org/std/task/struct.Context.html>
pub struct hyper_context<'a>(Context<'a>);

/// A waker that is saved and used to waken a pending task.
///
/// This is provided to `hyper_io`'s read and write callbacks via `hyper_context`
/// and `hyper_context_waker`.
///
/// When nonblocking I/O in one of those callbacks can't make progress (returns
/// `EAGAIN` or `EWOULDBLOCK`), the callback has to return to avoid blocking the
/// executor. But it also has to arrange to get called in the future when more
/// data is available. That's the role of the async context and the waker. The
/// waker can be used to tell the executor "this task is ready to make progress."
///
/// The read or write callback, upon finding it can't make progress, must get a
/// waker from the context (`hyper_context_waker`), arrange for that waker to be
/// called in the future, and then return `HYPER_POLL_PENDING`.
///
/// The arrangements for the waker to be called in the future are up to the
/// application, but usually it will involve one big `select(2)` loop that checks which
/// FDs are ready, and a correspondence between FDs and waker objects. For each
/// FD that is ready, the corresponding waker must be called. Then `hyper_executor_poll`
/// must be called. That will cause the executor to attempt to make progress on each
/// woken task.
///
/// Corresponding Rust type: <https://doc.rust-lang.org/std/task/struct.Waker.html>
pub struct hyper_waker {
    waker: std::task::Waker,
}

/// A descriptor for what type a `hyper_task` value is.
#[repr(C)]
pub enum hyper_task_return_type {
    /// The value of this task is null (does not imply an error).
    HYPER_TASK_EMPTY,
    /// The value of this task is `hyper_error *`.
    HYPER_TASK_ERROR,
    /// The value of this task is `hyper_clientconn *`.
    HYPER_TASK_CLIENTCONN,
    /// The value of this task is `hyper_response *`.
    HYPER_TASK_RESPONSE,
    /// The value of this task is `hyper_buf *`.
    HYPER_TASK_BUF,
}

pub(crate) unsafe trait AsTaskType {
    fn as_task_type(&self) -> hyper_task_return_type;
}

pub(crate) trait IntoDynTaskType {
    fn into_dyn_task_type(self) -> BoxAny;
}

// ===== impl hyper_executor =====

impl hyper_executor {
    fn new() -> Arc<hyper_executor> {
        Arc::new(hyper_executor {
            driver: Mutex::new(FuturesUnordered::new()),
            spawn_queue: Mutex::new(Vec::new()),
            is_woken: Arc::new(ExecWaker(AtomicBool::new(false))),
        })
    }

    pub(crate) fn downgrade(exec: &Arc<hyper_executor>) -> WeakExec {
        WeakExec(Arc::downgrade(exec))
    }

    fn spawn(&self, task: Box<hyper_task>) {
        self.spawn_queue
            .lock()
            .unwrap()
            .push(TaskFuture { task: Some(task) });
    }

    fn poll_next(&self) -> Option<Box<hyper_task>> {
        // Drain the queue first.
        self.drain_queue();

        let waker = futures_util::task::waker_ref(&self.is_woken);
        let mut cx = Context::from_waker(&waker);

        loop {
            {
                // Scope the lock on the driver to ensure it is dropped before
                // calling drain_queue below.
                let mut driver = self.driver.lock().unwrap();
                match Pin::new(&mut *driver).poll_next(&mut cx) {
                    Poll::Ready(val) => return val,
                    Poll::Pending => {}
                };
            }

            // poll_next returned Pending.
            // Check if any of the pending tasks tried to spawn
            // some new tasks. If so, drain into the driver and loop.
            if self.drain_queue() {
                continue;
            }

            // If the driver called `wake` while we were polling,
            // we should poll again immediately!
            if self.is_woken.0.swap(false, Ordering::SeqCst) {
                continue;
            }

            return None;
        }
    }

    /// drain_queue locks both self.spawn_queue and self.driver, so it requires
    /// that neither of them be locked already.
    fn drain_queue(&self) -> bool {
        let mut queue = self.spawn_queue.lock().unwrap();
        if queue.is_empty() {
            return false;
        }

        let driver = self.driver.lock().unwrap();

        for task in queue.drain(..) {
            driver.push(task);
        }

        true
    }
}

impl futures_util::task::ArcWake for ExecWaker {
    fn wake_by_ref(me: &Arc<ExecWaker>) {
        me.0.store(true, Ordering::SeqCst);
    }
}

// ===== impl WeakExec =====

impl WeakExec {
    pub(crate) fn new() -> Self {
        WeakExec(Weak::new())
    }
}

impl<F> crate::rt::Executor<F> for WeakExec
where
    F: Future + Send + 'static,
    F::Output: Send + Sync + AsTaskType,
{
    fn execute(&self, fut: F) {
        if let Some(exec) = self.0.upgrade() {
            exec.spawn(hyper_task::boxed(fut));
        }
    }
}

ffi_fn! {
    /// Creates a new task executor.
    ///
    /// To avoid a memory leak, the executor must eventually be consumed by
    /// `hyper_executor_free`.
    fn hyper_executor_new() -> *const hyper_executor {
        Arc::into_raw(hyper_executor::new())
    } ?= ptr::null()
}

ffi_fn! {
    /// Frees an executor and any incomplete tasks still part of it.
    ///
    /// This should be used for any executor once it is no longer needed.
    fn hyper_executor_free(exec: *const hyper_executor) {
        drop(non_null!(Arc::from_raw(exec) ?= ()));
    }
}

ffi_fn! {
    /// Push a task onto the executor.
    ///
    /// The executor takes ownership of the task, which must not be accessed
    /// again.
    ///
    /// Ownership of the task will eventually be returned to the user from
    /// `hyper_executor_poll`.
    ///
    /// To distinguish multiple tasks running on the same executor, use
    /// hyper_task_set_userdata.
    fn hyper_executor_push(exec: *const hyper_executor, task: *mut hyper_task) -> hyper_code {
        let exec = non_null!(&*exec ?= hyper_code::HYPERE_INVALID_ARG);
        let task = non_null!(Box::from_raw(task) ?= hyper_code::HYPERE_INVALID_ARG);
        exec.spawn(task);
        hyper_code::HYPERE_OK
    }
}

ffi_fn! {
    /// Polls the executor, trying to make progress on any tasks that can do so.
    ///
    /// If any task from the executor is ready, returns one of them. The way
    /// tasks signal being finished is internal to Hyper. The order in which tasks
    /// are returned is not guaranteed. Use userdata to distinguish between tasks.
    ///
    /// To avoid a memory leak, the task must eventually be consumed by
    /// `hyper_task_free`.
    ///
    /// If there are no ready tasks, this returns `NULL`.
    fn hyper_executor_poll(exec: *const hyper_executor) -> *mut hyper_task {
        let exec = non_null!(&*exec ?= ptr::null_mut());
        match exec.poll_next() {
            Some(task) => Box::into_raw(task),
            None => ptr::null_mut(),
        }
    } ?= ptr::null_mut()
}

// ===== impl hyper_task =====

impl hyper_task {
    pub(crate) fn boxed<F>(fut: F) -> Box<hyper_task>
    where
        F: Future + Send + 'static,
        F::Output: IntoDynTaskType + Send + Sync + 'static,
    {
        Box::new(hyper_task {
            future: Box::pin(async move { fut.await.into_dyn_task_type() }),
            output: None,
            userdata: UserDataPointer(ptr::null_mut()),
        })
    }

    fn output_type(&self) -> hyper_task_return_type {
        match self.output {
            None => hyper_task_return_type::HYPER_TASK_EMPTY,
            Some(ref val) => val.as_task_type(),
        }
    }
}

impl Future for TaskFuture {
    type Output = Box<hyper_task>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.task.as_mut().unwrap().future).poll(cx) {
            Poll::Ready(val) => {
                let mut task = self.task.take().unwrap();
                task.output = Some(val);
                Poll::Ready(task)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

ffi_fn! {
    /// Free a task.
    ///
    /// This should only be used if the task isn't consumed by
    /// `hyper_clientconn_handshake` or taken ownership of by
    /// `hyper_executor_push`.
    fn hyper_task_free(task: *mut hyper_task) {
        drop(non_null!(Box::from_raw(task) ?= ()));
    }
}

ffi_fn! {
    /// Takes the output value of this task.
    ///
    /// This must only be called once polling the task on an executor has finished
    /// this task.
    ///
    /// Use `hyper_task_type` to determine the type of the `void *` return value.
    ///
    /// To avoid a memory leak, a non-empty return value must eventually be
    /// consumed by a function appropriate for its type, one of
    /// `hyper_error_free`, `hyper_clientconn_free`, `hyper_response_free`, or
    /// `hyper_buf_free`.
    fn hyper_task_value(task: *mut hyper_task) -> *mut c_void {
        let task = non_null!(&mut *task ?= ptr::null_mut());

        if let Some(val) = task.output.take() {
            let p = Box::into_raw(val) as *mut c_void;
            // protect from returning fake pointers to empty types
            if p == std::ptr::NonNull::<c_void>::dangling().as_ptr() {
                ptr::null_mut()
            } else {
                p
            }
        } else {
            ptr::null_mut()
        }
    } ?= ptr::null_mut()
}

ffi_fn! {
    /// Query the return type of this task.
    fn hyper_task_type(task: *mut hyper_task) -> hyper_task_return_type {
        // instead of blowing up spectacularly, just say this null task
        // doesn't have a value to retrieve.
        non_null!(&*task ?= hyper_task_return_type::HYPER_TASK_EMPTY).output_type()
    }
}

ffi_fn! {
    /// Set a user data pointer to be associated with this task.
    ///
    /// This value will be passed to task callbacks, and can be checked later
    /// with `hyper_task_userdata`.
    ///
    /// This is useful for telling apart tasks for different requests that are
    /// running on the same executor.
    fn hyper_task_set_userdata(task: *mut hyper_task, userdata: *mut c_void) {
        if task.is_null() {
            return;
        }

        unsafe { (*task).userdata = UserDataPointer(userdata) };
    }
}

ffi_fn! {
    /// Retrieve the userdata that has been set via `hyper_task_set_userdata`.
    fn hyper_task_userdata(task: *mut hyper_task) -> *mut c_void {
        non_null!(&*task ?= ptr::null_mut()).userdata.0
    } ?= ptr::null_mut()
}

// ===== impl AsTaskType =====

unsafe impl AsTaskType for () {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_EMPTY
    }
}

unsafe impl AsTaskType for crate::Error {
    fn as_task_type(&self) -> hyper_task_return_type {
        hyper_task_return_type::HYPER_TASK_ERROR
    }
}

impl<T> IntoDynTaskType for T
where
    T: AsTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        Box::new(self)
    }
}

impl<T> IntoDynTaskType for crate::Result<T>
where
    T: IntoDynTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        match self {
            Ok(val) => val.into_dyn_task_type(),
            Err(err) => Box::new(err),
        }
    }
}

impl<T> IntoDynTaskType for Option<T>
where
    T: IntoDynTaskType + Send + Sync + 'static,
{
    fn into_dyn_task_type(self) -> BoxAny {
        match self {
            Some(val) => val.into_dyn_task_type(),
            None => ().into_dyn_task_type(),
        }
    }
}

// ===== impl hyper_context =====

impl hyper_context<'_> {
    pub(crate) fn wrap<'a, 'b>(cx: &'a mut Context<'b>) -> &'a mut hyper_context<'b> {
        // A struct with only one field has the same layout as that field.
        unsafe { std::mem::transmute::<&mut Context<'_>, &mut hyper_context<'_>>(cx) }
    }
}

ffi_fn! {
    /// Creates a waker associated with the task context.
    ///
    /// The waker can be used to inform the task's executor that the task is
    /// ready to make progress (using `hyper_waker_wake`).
    ///
    /// Typically this only needs to be called once, but it can be called
    /// multiple times, returning a new waker each time.
    ///
    /// To avoid a memory leak, the waker must eventually be consumed by
    /// `hyper_waker_free` or `hyper_waker_wake`.
    fn hyper_context_waker(cx: *mut hyper_context<'_>) -> *mut hyper_waker {
        let waker = non_null!(&mut *cx ?= ptr::null_mut()).0.waker().clone();
        Box::into_raw(Box::new(hyper_waker { waker }))
    } ?= ptr::null_mut()
}

// ===== impl hyper_waker =====

ffi_fn! {
    /// Free a waker.
    ///
    /// This should only be used if the request isn't consumed by
    /// `hyper_waker_wake`.
    fn hyper_waker_free(waker: *mut hyper_waker) {
        drop(non_null!(Box::from_raw(waker) ?= ()));
    }
}

ffi_fn! {
    /// Wake up the task associated with a waker.
    ///
    /// This does not do work towards associated task. Instead, it signals
    /// to the task's executor that the task is ready to make progress. The
    /// application is responsible for calling hyper_executor_poll, which
    /// will in turn do work on all tasks that are ready to make progress.
    ///
    /// NOTE: This consumes the waker. You should not use or free the waker afterwards.
    fn hyper_waker_wake(waker: *mut hyper_waker) {
        let waker = non_null!(Box::from_raw(waker) ?= ());
        waker.waker.wake();
    }
}
