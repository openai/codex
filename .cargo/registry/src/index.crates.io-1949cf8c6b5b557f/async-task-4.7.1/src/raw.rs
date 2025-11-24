use alloc::alloc::Layout as StdLayout;
use core::cell::UnsafeCell;
use core::future::Future;
use core::mem::{self, ManuallyDrop};
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
#[cfg(feature = "portable-atomic")]
use portable_atomic::AtomicUsize;

use crate::header::Header;
use crate::runnable::{Schedule, ScheduleInfo};
use crate::state::*;
use crate::utils::{abort, abort_on_panic, max, Layout};
use crate::Runnable;

#[cfg(feature = "std")]
pub(crate) type Panic = alloc::boxed::Box<dyn core::any::Any + Send + 'static>;

#[cfg(not(feature = "std"))]
pub(crate) type Panic = core::convert::Infallible;

/// The vtable for a task.
pub(crate) struct TaskVTable {
    /// Schedules the task.
    pub(crate) schedule: unsafe fn(*const (), ScheduleInfo),

    /// Drops the future inside the task.
    pub(crate) drop_future: unsafe fn(*const ()),

    /// Returns a pointer to the output stored after completion.
    pub(crate) get_output: unsafe fn(*const ()) -> *const (),

    /// Drops the task reference (`Runnable` or `Waker`).
    pub(crate) drop_ref: unsafe fn(ptr: *const ()),

    /// Destroys the task.
    pub(crate) destroy: unsafe fn(*const ()),

    /// Runs the task.
    pub(crate) run: unsafe fn(*const ()) -> bool,

    /// Creates a new waker associated with the task.
    pub(crate) clone_waker: unsafe fn(ptr: *const ()) -> RawWaker,

    /// The memory layout of the task. This information enables
    /// debuggers to decode raw task memory blobs. Do not remove
    /// the field, even if it appears to be unused.
    #[allow(unused)]
    pub(crate) layout_info: &'static TaskLayout,
}

/// Memory layout of a task.
///
/// This struct contains the following information:
///
/// 1. How to allocate and deallocate the task.
/// 2. How to access the fields inside the task.
#[derive(Clone, Copy)]
pub(crate) struct TaskLayout {
    /// Memory layout of the whole task.
    pub(crate) layout: StdLayout,

    /// Offset into the task at which the schedule function is stored.
    pub(crate) offset_s: usize,

    /// Offset into the task at which the future is stored.
    pub(crate) offset_f: usize,

    /// Offset into the task at which the output is stored.
    pub(crate) offset_r: usize,
}

/// Raw pointers to the fields inside a task.
pub(crate) struct RawTask<F, T, S, M> {
    /// The task header.
    pub(crate) header: *const Header<M>,

    /// The schedule function.
    pub(crate) schedule: *const S,

    /// The future.
    pub(crate) future: *mut F,

    /// The output of the future.
    pub(crate) output: *mut Result<T, Panic>,
}

impl<F, T, S, M> Copy for RawTask<F, T, S, M> {}

impl<F, T, S, M> Clone for RawTask<F, T, S, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F, T, S, M> RawTask<F, T, S, M> {
    const TASK_LAYOUT: TaskLayout = Self::eval_task_layout();

    /// Computes the memory layout for a task.
    #[inline]
    const fn eval_task_layout() -> TaskLayout {
        // Compute the layouts for `Header`, `S`, `F`, and `T`.
        let layout_header = Layout::new::<Header<M>>();
        let layout_s = Layout::new::<S>();
        let layout_f = Layout::new::<F>();
        let layout_r = Layout::new::<Result<T, Panic>>();

        // Compute the layout for `union { F, T }`.
        let size_union = max(layout_f.size(), layout_r.size());
        let align_union = max(layout_f.align(), layout_r.align());
        let layout_union = Layout::from_size_align(size_union, align_union);

        // Compute the layout for `Header` followed `S` and `union { F, T }`.
        let layout = layout_header;
        let (layout, offset_s) = leap_unwrap!(layout.extend(layout_s));
        let (layout, offset_union) = leap_unwrap!(layout.extend(layout_union));
        let offset_f = offset_union;
        let offset_r = offset_union;

        TaskLayout {
            layout: unsafe { layout.into_std() },
            offset_s,
            offset_f,
            offset_r,
        }
    }
}

impl<F, T, S, M> RawTask<F, T, S, M>
where
    F: Future<Output = T>,
    S: Schedule<M>,
{
    const RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_waker,
        Self::wake,
        Self::wake_by_ref,
        Self::drop_waker,
    );

    /// Allocates a task with the given `future` and `schedule` function.
    ///
    /// It is assumed that initially only the `Runnable` and the `Task` exist.
    pub(crate) fn allocate<'a, Gen: FnOnce(&'a M) -> F>(
        future: Gen,
        schedule: S,
        builder: crate::Builder<M>,
    ) -> NonNull<()>
    where
        F: 'a,
        M: 'a,
    {
        // Compute the layout of the task for allocation. Abort if the computation fails.
        //
        // n.b. notgull: task_layout now automatically aborts instead of panicking
        let task_layout = Self::task_layout();

        unsafe {
            // Allocate enough space for the entire task.
            let ptr = match NonNull::new(alloc::alloc::alloc(task_layout.layout) as *mut ()) {
                None => abort(),
                Some(p) => p,
            };

            let raw = Self::from_ptr(ptr.as_ptr());

            let crate::Builder {
                metadata,
                #[cfg(feature = "std")]
                propagate_panic,
            } = builder;

            // Write the header as the first field of the task.
            (raw.header as *mut Header<M>).write(Header {
                state: AtomicUsize::new(SCHEDULED | TASK | REFERENCE),
                awaiter: UnsafeCell::new(None),
                vtable: &TaskVTable {
                    schedule: Self::schedule,
                    drop_future: Self::drop_future,
                    get_output: Self::get_output,
                    drop_ref: Self::drop_ref,
                    destroy: Self::destroy,
                    run: Self::run,
                    clone_waker: Self::clone_waker,
                    layout_info: &Self::TASK_LAYOUT,
                },
                metadata,
                #[cfg(feature = "std")]
                propagate_panic,
            });

            // Write the schedule function as the third field of the task.
            (raw.schedule as *mut S).write(schedule);

            // Generate the future, now that the metadata has been pinned in place.
            let future = abort_on_panic(|| future(&(*raw.header).metadata));

            // Write the future as the fourth field of the task.
            raw.future.write(future);

            ptr
        }
    }

    /// Creates a `RawTask` from a raw task pointer.
    #[inline]
    pub(crate) fn from_ptr(ptr: *const ()) -> Self {
        let task_layout = Self::task_layout();
        let p = ptr as *const u8;

        unsafe {
            Self {
                header: p as *const Header<M>,
                schedule: p.add(task_layout.offset_s) as *const S,
                future: p.add(task_layout.offset_f) as *mut F,
                output: p.add(task_layout.offset_r) as *mut Result<T, Panic>,
            }
        }
    }

    /// Returns the layout of the task.
    #[inline]
    fn task_layout() -> TaskLayout {
        Self::TASK_LAYOUT
    }
    /// Wakes a waker.
    unsafe fn wake(ptr: *const ()) {
        // This is just an optimization. If the schedule function has captured variables, then
        // we'll do less reference counting if we wake the waker by reference and then drop it.
        if mem::size_of::<S>() > 0 {
            Self::wake_by_ref(ptr);
            Self::drop_waker(ptr);
            return;
        }

        let raw = Self::from_ptr(ptr);

        let mut state = (*raw.header).state.load(Ordering::Acquire);

        loop {
            // If the task is completed or closed, it can't be woken up.
            if state & (COMPLETED | CLOSED) != 0 {
                // Drop the waker.
                Self::drop_waker(ptr);
                break;
            }

            // If the task is already scheduled, we just need to synchronize with the thread that
            // will run the task by "publishing" our current view of the memory.
            if state & SCHEDULED != 0 {
                // Update the state without actually modifying it.
                match (*raw.header).state.compare_exchange_weak(
                    state,
                    state,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Drop the waker.
                        Self::drop_waker(ptr);
                        break;
                    }
                    Err(s) => state = s,
                }
            } else {
                // Mark the task as scheduled.
                match (*raw.header).state.compare_exchange_weak(
                    state,
                    state | SCHEDULED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // If the task is not yet scheduled and isn't currently running, now is the
                        // time to schedule it.
                        if state & RUNNING == 0 {
                            // Schedule the task.
                            Self::schedule(ptr, ScheduleInfo::new(false));
                        } else {
                            // Drop the waker.
                            Self::drop_waker(ptr);
                        }

                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    /// Wakes a waker by reference.
    unsafe fn wake_by_ref(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        let mut state = (*raw.header).state.load(Ordering::Acquire);

        loop {
            // If the task is completed or closed, it can't be woken up.
            if state & (COMPLETED | CLOSED) != 0 {
                break;
            }

            // If the task is already scheduled, we just need to synchronize with the thread that
            // will run the task by "publishing" our current view of the memory.
            if state & SCHEDULED != 0 {
                // Update the state without actually modifying it.
                match (*raw.header).state.compare_exchange_weak(
                    state,
                    state,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(s) => state = s,
                }
            } else {
                // If the task is not running, we can schedule right away.
                let new = if state & RUNNING == 0 {
                    (state | SCHEDULED) + REFERENCE
                } else {
                    state | SCHEDULED
                };

                // Mark the task as scheduled.
                match (*raw.header).state.compare_exchange_weak(
                    state,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // If the task is not running, now is the time to schedule.
                        if state & RUNNING == 0 {
                            // If the reference count overflowed, abort.
                            if state > isize::MAX as usize {
                                abort();
                            }

                            // Schedule the task. There is no need to call `Self::schedule(ptr)`
                            // because the schedule function cannot be destroyed while the waker is
                            // still alive.
                            let task = Runnable::from_raw(NonNull::new_unchecked(ptr as *mut ()));
                            (*raw.schedule).schedule(task, ScheduleInfo::new(false));
                        }

                        break;
                    }
                    Err(s) => state = s,
                }
            }
        }
    }

    /// Clones a waker.
    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        let raw = Self::from_ptr(ptr);

        // Increment the reference count. With any kind of reference-counted data structure,
        // relaxed ordering is appropriate when incrementing the counter.
        let state = (*raw.header).state.fetch_add(REFERENCE, Ordering::Relaxed);

        // If the reference count overflowed, abort.
        if state > isize::MAX as usize {
            abort();
        }

        RawWaker::new(ptr, &Self::RAW_WAKER_VTABLE)
    }

    /// Drops a waker.
    ///
    /// This function will decrement the reference count. If it drops down to zero, the associated
    /// `Task` has been dropped too, and the task has not been completed, then it will get
    /// scheduled one more time so that its future gets dropped by the executor.
    #[inline]
    unsafe fn drop_waker(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        // Decrement the reference count.
        let new = (*raw.header).state.fetch_sub(REFERENCE, Ordering::AcqRel) - REFERENCE;

        // If this was the last reference to the task and the `Task` has been dropped too,
        // then we need to decide how to destroy the task.
        if new & !(REFERENCE - 1) == 0 && new & TASK == 0 {
            if new & (COMPLETED | CLOSED) == 0 {
                // If the task was not completed nor closed, close it and schedule one more time so
                // that its future gets dropped by the executor.
                (*raw.header)
                    .state
                    .store(SCHEDULED | CLOSED | REFERENCE, Ordering::Release);
                Self::schedule(ptr, ScheduleInfo::new(false));
            } else {
                // Otherwise, destroy the task right away.
                Self::destroy(ptr);
            }
        }
    }

    /// Drops a task reference (`Runnable` or `Waker`).
    ///
    /// This function will decrement the reference count. If it drops down to zero and the
    /// associated `Task` handle has been dropped too, then the task gets destroyed.
    #[inline]
    unsafe fn drop_ref(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        // Decrement the reference count.
        let new = (*raw.header).state.fetch_sub(REFERENCE, Ordering::AcqRel) - REFERENCE;

        // If this was the last reference to the task and the `Task` has been dropped too,
        // then destroy the task.
        if new & !(REFERENCE - 1) == 0 && new & TASK == 0 {
            Self::destroy(ptr);
        }
    }

    /// Schedules a task for running.
    ///
    /// This function doesn't modify the state of the task. It only passes the task reference to
    /// its schedule function.
    unsafe fn schedule(ptr: *const (), info: ScheduleInfo) {
        let raw = Self::from_ptr(ptr);

        // If the schedule function has captured variables, create a temporary waker that prevents
        // the task from getting deallocated while the function is being invoked.
        let _waker;
        if mem::size_of::<S>() > 0 {
            _waker = Waker::from_raw(Self::clone_waker(ptr));
        }

        let task = Runnable::from_raw(NonNull::new_unchecked(ptr as *mut ()));
        (*raw.schedule).schedule(task, info);
    }

    /// Drops the future inside a task.
    #[inline]
    unsafe fn drop_future(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);

        // We need a safeguard against panics because the destructor can panic.
        abort_on_panic(|| {
            raw.future.drop_in_place();
        })
    }

    /// Returns a pointer to the output inside a task.
    unsafe fn get_output(ptr: *const ()) -> *const () {
        let raw = Self::from_ptr(ptr);
        raw.output as *const ()
    }

    /// Cleans up task's resources and deallocates it.
    ///
    /// The schedule function will be dropped, and the task will then get deallocated.
    /// The task must be closed before this function is called.
    #[inline]
    unsafe fn destroy(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let task_layout = Self::task_layout();

        // We need a safeguard against panics because destructors can panic.
        abort_on_panic(|| {
            // Drop the header along with the metadata.
            (raw.header as *mut Header<M>).drop_in_place();

            // Drop the schedule function.
            (raw.schedule as *mut S).drop_in_place();
        });

        // Finally, deallocate the memory reserved by the task.
        alloc::alloc::dealloc(ptr as *mut u8, task_layout.layout);
    }

    /// Runs a task.
    ///
    /// If polling its future panics, the task will be closed and the panic will be propagated into
    /// the caller.
    unsafe fn run(ptr: *const ()) -> bool {
        let raw = Self::from_ptr(ptr);

        // Create a context from the raw task pointer and the vtable inside the its header.
        let waker = ManuallyDrop::new(Waker::from_raw(RawWaker::new(ptr, &Self::RAW_WAKER_VTABLE)));
        let cx = &mut Context::from_waker(&waker);

        let mut state = (*raw.header).state.load(Ordering::Acquire);

        // Update the task's state before polling its future.
        loop {
            // If the task has already been closed, drop the task reference and return.
            if state & CLOSED != 0 {
                // Drop the future.
                Self::drop_future(ptr);

                // Mark the task as unscheduled.
                let state = (*raw.header).state.fetch_and(!SCHEDULED, Ordering::AcqRel);

                // Take the awaiter out.
                let mut awaiter = None;
                if state & AWAITER != 0 {
                    awaiter = (*raw.header).take(None);
                }

                // Drop the task reference.
                Self::drop_ref(ptr);

                // Notify the awaiter that the future has been dropped.
                if let Some(w) = awaiter {
                    abort_on_panic(|| w.wake());
                }
                return false;
            }

            // Mark the task as unscheduled and running.
            match (*raw.header).state.compare_exchange_weak(
                state,
                (state & !SCHEDULED) | RUNNING,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Update the state because we're continuing with polling the future.
                    state = (state & !SCHEDULED) | RUNNING;
                    break;
                }
                Err(s) => state = s,
            }
        }

        // Poll the inner future, but surround it with a guard that closes the task in case polling
        // panics.
        // If available, we should also try to catch the panic so that it is propagated correctly.
        let guard = Guard(raw);

        // Panic propagation is not available for no_std.
        #[cfg(not(feature = "std"))]
        let poll = <F as Future>::poll(Pin::new_unchecked(&mut *raw.future), cx).map(Ok);

        #[cfg(feature = "std")]
        let poll = {
            // Check if we should propagate panics.
            if (*raw.header).propagate_panic {
                // Use catch_unwind to catch the panic.
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    <F as Future>::poll(Pin::new_unchecked(&mut *raw.future), cx)
                })) {
                    Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
                    Ok(Poll::Pending) => Poll::Pending,
                    Err(e) => Poll::Ready(Err(e)),
                }
            } else {
                <F as Future>::poll(Pin::new_unchecked(&mut *raw.future), cx).map(Ok)
            }
        };

        mem::forget(guard);

        match poll {
            Poll::Ready(out) => {
                // Replace the future with its output.
                Self::drop_future(ptr);
                raw.output.write(out);

                // The task is now completed.
                loop {
                    // If the `Task` is dropped, we'll need to close it and drop the output.
                    let new = if state & TASK == 0 {
                        (state & !RUNNING & !SCHEDULED) | COMPLETED | CLOSED
                    } else {
                        (state & !RUNNING & !SCHEDULED) | COMPLETED
                    };

                    // Mark the task as not running and completed.
                    match (*raw.header).state.compare_exchange_weak(
                        state,
                        new,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            // If the `Task` is dropped or if the task was closed while running,
                            // now it's time to drop the output.
                            if state & TASK == 0 || state & CLOSED != 0 {
                                // Drop the output.
                                abort_on_panic(|| raw.output.drop_in_place());
                            }

                            // Take the awaiter out.
                            let mut awaiter = None;
                            if state & AWAITER != 0 {
                                awaiter = (*raw.header).take(None);
                            }

                            // Drop the task reference.
                            Self::drop_ref(ptr);

                            // Notify the awaiter that the future has been dropped.
                            if let Some(w) = awaiter {
                                abort_on_panic(|| w.wake());
                            }
                            break;
                        }
                        Err(s) => state = s,
                    }
                }
            }
            Poll::Pending => {
                let mut future_dropped = false;

                // The task is still not completed.
                loop {
                    // If the task was closed while running, we'll need to unschedule in case it
                    // was woken up and then destroy it.
                    let new = if state & CLOSED != 0 {
                        state & !RUNNING & !SCHEDULED
                    } else {
                        state & !RUNNING
                    };

                    if state & CLOSED != 0 && !future_dropped {
                        // The thread that closed the task didn't drop the future because it was
                        // running so now it's our responsibility to do so.
                        Self::drop_future(ptr);
                        future_dropped = true;
                    }

                    // Mark the task as not running.
                    match (*raw.header).state.compare_exchange_weak(
                        state,
                        new,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(state) => {
                            // If the task was closed while running, we need to notify the awaiter.
                            // If the task was woken up while running, we need to schedule it.
                            // Otherwise, we just drop the task reference.
                            if state & CLOSED != 0 {
                                // Take the awaiter out.
                                let mut awaiter = None;
                                if state & AWAITER != 0 {
                                    awaiter = (*raw.header).take(None);
                                }

                                // Drop the task reference.
                                Self::drop_ref(ptr);

                                // Notify the awaiter that the future has been dropped.
                                if let Some(w) = awaiter {
                                    abort_on_panic(|| w.wake());
                                }
                            } else if state & SCHEDULED != 0 {
                                // The thread that woke the task up didn't reschedule it because
                                // it was running so now it's our responsibility to do so.
                                Self::schedule(ptr, ScheduleInfo::new(true));
                                return true;
                            } else {
                                // Drop the task reference.
                                Self::drop_ref(ptr);
                            }
                            break;
                        }
                        Err(s) => state = s,
                    }
                }
            }
        }

        return false;

        /// A guard that closes the task if polling its future panics.
        struct Guard<F, T, S, M>(RawTask<F, T, S, M>)
        where
            F: Future<Output = T>,
            S: Schedule<M>;

        impl<F, T, S, M> Drop for Guard<F, T, S, M>
        where
            F: Future<Output = T>,
            S: Schedule<M>,
        {
            fn drop(&mut self) {
                let raw = self.0;
                let ptr = raw.header as *const ();

                unsafe {
                    let mut state = (*raw.header).state.load(Ordering::Acquire);

                    loop {
                        // If the task was closed while running, then unschedule it, drop its
                        // future, and drop the task reference.
                        if state & CLOSED != 0 {
                            // The thread that closed the task didn't drop the future because it
                            // was running so now it's our responsibility to do so.
                            RawTask::<F, T, S, M>::drop_future(ptr);

                            // Mark the task as not running and not scheduled.
                            (*raw.header)
                                .state
                                .fetch_and(!RUNNING & !SCHEDULED, Ordering::AcqRel);

                            // Take the awaiter out.
                            let mut awaiter = None;
                            if state & AWAITER != 0 {
                                awaiter = (*raw.header).take(None);
                            }

                            // Drop the task reference.
                            RawTask::<F, T, S, M>::drop_ref(ptr);

                            // Notify the awaiter that the future has been dropped.
                            if let Some(w) = awaiter {
                                abort_on_panic(|| w.wake());
                            }
                            break;
                        }

                        // Mark the task as not running, not scheduled, and closed.
                        match (*raw.header).state.compare_exchange_weak(
                            state,
                            (state & !RUNNING & !SCHEDULED) | CLOSED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        ) {
                            Ok(state) => {
                                // Drop the future because the task is now closed.
                                RawTask::<F, T, S, M>::drop_future(ptr);

                                // Take the awaiter out.
                                let mut awaiter = None;
                                if state & AWAITER != 0 {
                                    awaiter = (*raw.header).take(None);
                                }

                                // Drop the task reference.
                                RawTask::<F, T, S, M>::drop_ref(ptr);

                                // Notify the awaiter that the future has been dropped.
                                if let Some(w) = awaiter {
                                    abort_on_panic(|| w.wake());
                                }
                                break;
                            }
                            Err(s) => state = s,
                        }
                    }
                }
            }
        }
    }
}
