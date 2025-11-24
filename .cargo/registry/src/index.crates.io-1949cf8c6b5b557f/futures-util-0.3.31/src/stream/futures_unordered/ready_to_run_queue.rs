use crate::task::AtomicWaker;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};

use super::abort::abort;
use super::task::Task;

pub(super) enum Dequeue<Fut> {
    Data(*const Task<Fut>),
    Empty,
    Inconsistent,
}

pub(super) struct ReadyToRunQueue<Fut> {
    // The waker of the task using `FuturesUnordered`.
    pub(super) waker: AtomicWaker,

    // Head/tail of the readiness queue
    pub(super) head: AtomicPtr<Task<Fut>>,
    pub(super) tail: UnsafeCell<*const Task<Fut>>,
    pub(super) stub: Arc<Task<Fut>>,
}

/// An MPSC queue into which the tasks containing the futures are inserted
/// whenever the future inside is scheduled for polling.
impl<Fut> ReadyToRunQueue<Fut> {
    // FIXME: this takes raw pointer without safety conditions.

    /// The enqueue function from the 1024cores intrusive MPSC queue algorithm.
    pub(super) fn enqueue(&self, task: *const Task<Fut>) {
        unsafe {
            debug_assert!((*task).queued.load(Relaxed));

            // This action does not require any coordination
            (*task).next_ready_to_run.store(ptr::null_mut(), Relaxed);

            // Note that these atomic orderings come from 1024cores
            let task = task as *mut _;
            let prev = self.head.swap(task, AcqRel);
            (*prev).next_ready_to_run.store(task, Release);
        }
    }

    /// The dequeue function from the 1024cores intrusive MPSC queue algorithm
    ///
    /// Note that this is unsafe as it required mutual exclusion (only one
    /// thread can call this) to be guaranteed elsewhere.
    pub(super) unsafe fn dequeue(&self) -> Dequeue<Fut> {
        unsafe {
            let mut tail = *self.tail.get();
            let mut next = (*tail).next_ready_to_run.load(Acquire);

            if tail == self.stub() {
                if next.is_null() {
                    return Dequeue::Empty;
                }

                *self.tail.get() = next;
                tail = next;
                next = (*next).next_ready_to_run.load(Acquire);
            }

            if !next.is_null() {
                *self.tail.get() = next;
                debug_assert!(tail != self.stub());
                return Dequeue::Data(tail);
            }

            if self.head.load(Acquire) as *const _ != tail {
                return Dequeue::Inconsistent;
            }

            self.enqueue(self.stub());

            next = (*tail).next_ready_to_run.load(Acquire);

            if !next.is_null() {
                *self.tail.get() = next;
                return Dequeue::Data(tail);
            }

            Dequeue::Inconsistent
        }
    }

    pub(super) fn stub(&self) -> *const Task<Fut> {
        Arc::as_ptr(&self.stub)
    }
}

impl<Fut> Drop for ReadyToRunQueue<Fut> {
    fn drop(&mut self) {
        // Once we're in the destructor for `Inner<Fut>` we need to clear out
        // the ready to run queue of tasks if there's anything left in there.
        //
        // Note that each task has a strong reference count associated with it
        // which is owned by the ready to run queue. All tasks should have had
        // their futures dropped already by the `FuturesUnordered` destructor
        // above, so we're just pulling out tasks and dropping their refcounts.
        unsafe {
            loop {
                match self.dequeue() {
                    Dequeue::Empty => break,
                    Dequeue::Inconsistent => abort("inconsistent in drop"),
                    Dequeue::Data(ptr) => drop(Arc::from_raw(ptr)),
                }
            }
        }
    }
}
