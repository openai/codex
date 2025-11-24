//! Run-queue structures to support a work-stealing scheduler

use crate::loom::cell::UnsafeCell;
use crate::loom::sync::Arc;
use crate::runtime::scheduler::multi_thread::{Overflow, Stats};
use crate::runtime::task;

use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};

// Use wider integers when possible to increase ABA resilience.
//
// See issue #5041: <https://github.com/tokio-rs/tokio/issues/5041>.
cfg_has_atomic_u64! {
    type UnsignedShort = u32;
    type UnsignedLong = u64;
    type AtomicUnsignedShort = crate::loom::sync::atomic::AtomicU32;
    type AtomicUnsignedLong = crate::loom::sync::atomic::AtomicU64;
}
cfg_not_has_atomic_u64! {
    type UnsignedShort = u16;
    type UnsignedLong = u32;
    type AtomicUnsignedShort = crate::loom::sync::atomic::AtomicU16;
    type AtomicUnsignedLong = crate::loom::sync::atomic::AtomicU32;
}

/// Producer handle. May only be used from a single thread.
pub(crate) struct Local<T: 'static> {
    inner: Arc<Inner<T>>,
}

/// Consumer handle. May be used from many threads.
pub(crate) struct Steal<T: 'static>(Arc<Inner<T>>);

pub(crate) struct Inner<T: 'static> {
    /// Concurrently updated by many threads.
    ///
    /// Contains two `UnsignedShort` values. The `LSB` byte is the "real" head of
    /// the queue. The `UnsignedShort` in the `MSB` is set by a stealer in process
    /// of stealing values. It represents the first value being stolen in the
    /// batch. The `UnsignedShort` indices are intentionally wider than strictly
    /// required for buffer indexing in order to provide ABA mitigation and make
    /// it possible to distinguish between full and empty buffers.
    ///
    /// When both `UnsignedShort` values are the same, there is no active
    /// stealer.
    ///
    /// Tracking an in-progress stealer prevents a wrapping scenario.
    head: AtomicUnsignedLong,

    /// Only updated by producer thread but read by many threads.
    tail: AtomicUnsignedShort,

    /// Elements
    buffer: Box<[UnsafeCell<MaybeUninit<task::Notified<T>>>; LOCAL_QUEUE_CAPACITY]>,
}

unsafe impl<T> Send for Inner<T> {}
unsafe impl<T> Sync for Inner<T> {}

#[cfg(not(loom))]
const LOCAL_QUEUE_CAPACITY: usize = 256;

// Shrink the size of the local queue when using loom. This shouldn't impact
// logic, but allows loom to test more edge cases in a reasonable a mount of
// time.
#[cfg(loom)]
const LOCAL_QUEUE_CAPACITY: usize = 4;

const MASK: usize = LOCAL_QUEUE_CAPACITY - 1;

// Constructing the fixed size array directly is very awkward. The only way to
// do it is to repeat `UnsafeCell::new(MaybeUninit::uninit())` 256 times, as
// the contents are not Copy. The trick with defining a const doesn't work for
// generic types.
fn make_fixed_size<T>(buffer: Box<[T]>) -> Box<[T; LOCAL_QUEUE_CAPACITY]> {
    assert_eq!(buffer.len(), LOCAL_QUEUE_CAPACITY);

    // safety: We check that the length is correct.
    unsafe { Box::from_raw(Box::into_raw(buffer).cast()) }
}

/// Create a new local run-queue
pub(crate) fn local<T: 'static>() -> (Steal<T>, Local<T>) {
    let mut buffer = Vec::with_capacity(LOCAL_QUEUE_CAPACITY);

    for _ in 0..LOCAL_QUEUE_CAPACITY {
        buffer.push(UnsafeCell::new(MaybeUninit::uninit()));
    }

    let inner = Arc::new(Inner {
        head: AtomicUnsignedLong::new(0),
        tail: AtomicUnsignedShort::new(0),
        buffer: make_fixed_size(buffer.into_boxed_slice()),
    });

    let local = Local {
        inner: inner.clone(),
    };

    let remote = Steal(inner);

    (remote, local)
}

impl<T> Local<T> {
    /// Returns the number of entries in the queue
    pub(crate) fn len(&self) -> usize {
        let (_, head) = unpack(self.inner.head.load(Acquire));
        // safety: this is the **only** thread that updates this cell.
        let tail = unsafe { self.inner.tail.unsync_load() };
        len(head, tail)
    }

    /// How many tasks can be pushed into the queue
    pub(crate) fn remaining_slots(&self) -> usize {
        let (steal, _) = unpack(self.inner.head.load(Acquire));
        // safety: this is the **only** thread that updates this cell.
        let tail = unsafe { self.inner.tail.unsync_load() };

        LOCAL_QUEUE_CAPACITY - len(steal, tail)
    }

    pub(crate) fn max_capacity(&self) -> usize {
        LOCAL_QUEUE_CAPACITY
    }

    /// Returns false if there are any entries in the queue
    ///
    /// Separate to `is_stealable` so that refactors of `is_stealable` to "protect"
    /// some tasks from stealing won't affect this
    pub(crate) fn has_tasks(&self) -> bool {
        self.len() != 0
    }

    /// Pushes a batch of tasks to the back of the queue. All tasks must fit in
    /// the local queue.
    ///
    /// # Panics
    ///
    /// The method panics if there is not enough capacity to fit in the queue.
    pub(crate) fn push_back(&mut self, tasks: impl ExactSizeIterator<Item = task::Notified<T>>) {
        let len = tasks.len();
        assert!(len <= LOCAL_QUEUE_CAPACITY);

        if len == 0 {
            // Nothing to do
            return;
        }

        let head = self.inner.head.load(Acquire);
        let (steal, _) = unpack(head);

        // safety: this is the **only** thread that updates this cell.
        let mut tail = unsafe { self.inner.tail.unsync_load() };

        if tail.wrapping_sub(steal) <= (LOCAL_QUEUE_CAPACITY - len) as UnsignedShort {
            // Yes, this if condition is structured a bit weird (first block
            // does nothing, second returns an error). It is this way to match
            // `push_back_or_overflow`.
        } else {
            panic!()
        }

        for task in tasks {
            let idx = tail as usize & MASK;

            self.inner.buffer[idx].with_mut(|ptr| {
                // Write the task to the slot
                //
                // Safety: There is only one producer and the above `if`
                // condition ensures we don't touch a cell if there is a
                // value, thus no consumer.
                unsafe {
                    ptr::write((*ptr).as_mut_ptr(), task);
                }
            });

            tail = tail.wrapping_add(1);
        }

        self.inner.tail.store(tail, Release);
    }

    /// Pushes a task to the back of the local queue, if there is not enough
    /// capacity in the queue, this triggers the overflow operation.
    ///
    /// When the queue overflows, half of the current contents of the queue is
    /// moved to the given Injection queue. This frees up capacity for more
    /// tasks to be pushed into the local queue.
    pub(crate) fn push_back_or_overflow<O: Overflow<T>>(
        &mut self,
        mut task: task::Notified<T>,
        overflow: &O,
        stats: &mut Stats,
    ) {
        let tail = loop {
            let head = self.inner.head.load(Acquire);
            let (steal, real) = unpack(head);

            // safety: this is the **only** thread that updates this cell.
            let tail = unsafe { self.inner.tail.unsync_load() };

            if tail.wrapping_sub(steal) < LOCAL_QUEUE_CAPACITY as UnsignedShort {
                // There is capacity for the task
                break tail;
            } else if steal != real {
                // Concurrently stealing, this will free up capacity, so only
                // push the task onto the inject queue
                overflow.push(task);
                return;
            } else {
                // Push the current task and half of the queue into the
                // inject queue.
                match self.push_overflow(task, real, tail, overflow, stats) {
                    Ok(_) => return,
                    // Lost the race, try again
                    Err(v) => {
                        task = v;
                    }
                }
            }
        };

        self.push_back_finish(task, tail);
    }

    // Second half of `push_back`
    fn push_back_finish(&self, task: task::Notified<T>, tail: UnsignedShort) {
        // Map the position to a slot index.
        let idx = tail as usize & MASK;

        self.inner.buffer[idx].with_mut(|ptr| {
            // Write the task to the slot
            //
            // Safety: There is only one producer and the above `if`
            // condition ensures we don't touch a cell if there is a
            // value, thus no consumer.
            unsafe {
                ptr::write((*ptr).as_mut_ptr(), task);
            }
        });

        // Make the task available. Synchronizes with a load in
        // `steal_into2`.
        self.inner.tail.store(tail.wrapping_add(1), Release);
    }

    /// Moves a batch of tasks into the inject queue.
    ///
    /// This will temporarily make some of the tasks unavailable to stealers.
    /// Once `push_overflow` is done, a notification is sent out, so if other
    /// workers "missed" some of the tasks during a steal, they will get
    /// another opportunity.
    #[inline(never)]
    fn push_overflow<O: Overflow<T>>(
        &mut self,
        task: task::Notified<T>,
        head: UnsignedShort,
        tail: UnsignedShort,
        overflow: &O,
        stats: &mut Stats,
    ) -> Result<(), task::Notified<T>> {
        /// How many elements are we taking from the local queue.
        ///
        /// This is one less than the number of tasks pushed to the inject
        /// queue as we are also inserting the `task` argument.
        const NUM_TASKS_TAKEN: UnsignedShort = (LOCAL_QUEUE_CAPACITY / 2) as UnsignedShort;

        assert_eq!(
            tail.wrapping_sub(head) as usize,
            LOCAL_QUEUE_CAPACITY,
            "queue is not full; tail = {tail}; head = {head}"
        );

        let prev = pack(head, head);

        // Claim a bunch of tasks
        //
        // We are claiming the tasks **before** reading them out of the buffer.
        // This is safe because only the **current** thread is able to push new
        // tasks.
        //
        // There isn't really any need for memory ordering... Relaxed would
        // work. This is because all tasks are pushed into the queue from the
        // current thread (or memory has been acquired if the local queue handle
        // moved).
        if self
            .inner
            .head
            .compare_exchange(
                prev,
                pack(
                    head.wrapping_add(NUM_TASKS_TAKEN),
                    head.wrapping_add(NUM_TASKS_TAKEN),
                ),
                Release,
                Relaxed,
            )
            .is_err()
        {
            // We failed to claim the tasks, losing the race. Return out of
            // this function and try the full `push` routine again. The queue
            // may not be full anymore.
            return Err(task);
        }

        /// An iterator that takes elements out of the run queue.
        struct BatchTaskIter<'a, T: 'static> {
            buffer: &'a [UnsafeCell<MaybeUninit<task::Notified<T>>>; LOCAL_QUEUE_CAPACITY],
            head: UnsignedLong,
            i: UnsignedLong,
        }
        impl<'a, T: 'static> Iterator for BatchTaskIter<'a, T> {
            type Item = task::Notified<T>;

            #[inline]
            fn next(&mut self) -> Option<task::Notified<T>> {
                if self.i == UnsignedLong::from(NUM_TASKS_TAKEN) {
                    None
                } else {
                    let i_idx = self.i.wrapping_add(self.head) as usize & MASK;
                    let slot = &self.buffer[i_idx];

                    // safety: Our CAS from before has assumed exclusive ownership
                    // of the task pointers in this range.
                    let task = slot.with(|ptr| unsafe { ptr::read((*ptr).as_ptr()) });

                    self.i += 1;
                    Some(task)
                }
            }
        }

        // safety: The CAS above ensures that no consumer will look at these
        // values again, and we are the only producer.
        let batch_iter = BatchTaskIter {
            buffer: &self.inner.buffer,
            head: head as UnsignedLong,
            i: 0,
        };
        overflow.push_batch(batch_iter.chain(std::iter::once(task)));

        // Add 1 to factor in the task currently being scheduled.
        stats.incr_overflow_count();

        Ok(())
    }

    /// Pops a task from the local queue.
    pub(crate) fn pop(&mut self) -> Option<task::Notified<T>> {
        let mut head = self.inner.head.load(Acquire);

        let idx = loop {
            let (steal, real) = unpack(head);

            // safety: this is the **only** thread that updates this cell.
            let tail = unsafe { self.inner.tail.unsync_load() };

            if real == tail {
                // queue is empty
                return None;
            }

            let next_real = real.wrapping_add(1);

            // If `steal == real` there are no concurrent stealers. Both `steal`
            // and `real` are updated.
            let next = if steal == real {
                pack(next_real, next_real)
            } else {
                assert_ne!(steal, next_real);
                pack(steal, next_real)
            };

            // Attempt to claim a task.
            let res = self
                .inner
                .head
                .compare_exchange(head, next, AcqRel, Acquire);

            match res {
                Ok(_) => break real as usize & MASK,
                Err(actual) => head = actual,
            }
        };

        Some(self.inner.buffer[idx].with(|ptr| unsafe { ptr::read(ptr).assume_init() }))
    }
}

impl<T> Steal<T> {
    /// Returns the number of entries in the queue
    pub(crate) fn len(&self) -> usize {
        let (_, head) = unpack(self.0.head.load(Acquire));
        let tail = self.0.tail.load(Acquire);
        len(head, tail)
    }

    /// Return true if the queue is empty,
    /// false if there are any entries in the queue
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Steals half the tasks from self and place them into `dst`.
    pub(crate) fn steal_into(
        &self,
        dst: &mut Local<T>,
        dst_stats: &mut Stats,
    ) -> Option<task::Notified<T>> {
        // Safety: the caller is the only thread that mutates `dst.tail` and
        // holds a mutable reference.
        let dst_tail = unsafe { dst.inner.tail.unsync_load() };

        // To the caller, `dst` may **look** empty but still have values
        // contained in the buffer. If another thread is concurrently stealing
        // from `dst` there may not be enough capacity to steal.
        let (steal, _) = unpack(dst.inner.head.load(Acquire));

        if dst_tail.wrapping_sub(steal) > LOCAL_QUEUE_CAPACITY as UnsignedShort / 2 {
            // we *could* try to steal less here, but for simplicity, we're just
            // going to abort.
            return None;
        }

        // Steal the tasks into `dst`'s buffer. This does not yet expose the
        // tasks in `dst`.
        let mut n = self.steal_into2(dst, dst_tail);

        if n == 0 {
            // No tasks were stolen
            return None;
        }

        dst_stats.incr_steal_count(n as u16);
        dst_stats.incr_steal_operations();

        // We are returning a task here
        n -= 1;

        let ret_pos = dst_tail.wrapping_add(n);
        let ret_idx = ret_pos as usize & MASK;

        // safety: the value was written as part of `steal_into2` and not
        // exposed to stealers, so no other thread can access it.
        let ret = dst.inner.buffer[ret_idx].with(|ptr| unsafe { ptr::read((*ptr).as_ptr()) });

        if n == 0 {
            // The `dst` queue is empty, but a single task was stolen
            return Some(ret);
        }

        // Make the stolen items available to consumers
        dst.inner.tail.store(dst_tail.wrapping_add(n), Release);

        Some(ret)
    }

    // Steal tasks from `self`, placing them into `dst`. Returns the number of
    // tasks that were stolen.
    fn steal_into2(&self, dst: &mut Local<T>, dst_tail: UnsignedShort) -> UnsignedShort {
        let mut prev_packed = self.0.head.load(Acquire);
        let mut next_packed;

        let n = loop {
            let (src_head_steal, src_head_real) = unpack(prev_packed);
            let src_tail = self.0.tail.load(Acquire);

            // If these two do not match, another thread is concurrently
            // stealing from the queue.
            if src_head_steal != src_head_real {
                return 0;
            }

            // Number of available tasks to steal
            let n = src_tail.wrapping_sub(src_head_real);
            let n = n - n / 2;

            if n == 0 {
                // No tasks available to steal
                return 0;
            }

            // Update the real head index to acquire the tasks.
            let steal_to = src_head_real.wrapping_add(n);
            assert_ne!(src_head_steal, steal_to);
            next_packed = pack(src_head_steal, steal_to);

            // Claim all those tasks. This is done by incrementing the "real"
            // head but not the steal. By doing this, no other thread is able to
            // steal from this queue until the current thread completes.
            let res = self
                .0
                .head
                .compare_exchange(prev_packed, next_packed, AcqRel, Acquire);

            match res {
                Ok(_) => break n,
                Err(actual) => prev_packed = actual,
            }
        };

        assert!(
            n <= LOCAL_QUEUE_CAPACITY as UnsignedShort / 2,
            "actual = {n}"
        );

        let (first, _) = unpack(next_packed);

        // Take all the tasks
        for i in 0..n {
            // Compute the positions
            let src_pos = first.wrapping_add(i);
            let dst_pos = dst_tail.wrapping_add(i);

            // Map to slots
            let src_idx = src_pos as usize & MASK;
            let dst_idx = dst_pos as usize & MASK;

            // Read the task
            //
            // safety: We acquired the task with the atomic exchange above.
            let task = self.0.buffer[src_idx].with(|ptr| unsafe { ptr::read((*ptr).as_ptr()) });

            // Write the task to the new slot
            //
            // safety: `dst` queue is empty and we are the only producer to
            // this queue.
            dst.inner.buffer[dst_idx]
                .with_mut(|ptr| unsafe { ptr::write((*ptr).as_mut_ptr(), task) });
        }

        let mut prev_packed = next_packed;

        // Update `src_head_steal` to match `src_head_real` signalling that the
        // stealing routine is complete.
        loop {
            let head = unpack(prev_packed).1;
            next_packed = pack(head, head);

            let res = self
                .0
                .head
                .compare_exchange(prev_packed, next_packed, AcqRel, Acquire);

            match res {
                Ok(_) => return n,
                Err(actual) => {
                    let (actual_steal, actual_real) = unpack(actual);

                    assert_ne!(actual_steal, actual_real);

                    prev_packed = actual;
                }
            }
        }
    }
}

impl<T> Clone for Steal<T> {
    fn clone(&self) -> Steal<T> {
        Steal(self.0.clone())
    }
}

impl<T> Drop for Local<T> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(self.pop().is_none(), "queue not empty");
        }
    }
}

/// Calculate the length of the queue using the head and tail.
/// The `head` can be the `steal` or `real` head.
fn len(head: UnsignedShort, tail: UnsignedShort) -> usize {
    tail.wrapping_sub(head) as usize
}

/// Split the head value into the real head and the index a stealer is working
/// on.
fn unpack(n: UnsignedLong) -> (UnsignedShort, UnsignedShort) {
    let real = n & UnsignedShort::MAX as UnsignedLong;
    let steal = n >> (mem::size_of::<UnsignedShort>() * 8);

    (steal as UnsignedShort, real as UnsignedShort)
}

/// Join the two head values
fn pack(steal: UnsignedShort, real: UnsignedShort) -> UnsignedLong {
    (real as UnsignedLong) | ((steal as UnsignedLong) << (mem::size_of::<UnsignedShort>() * 8))
}

#[test]
fn test_local_queue_capacity() {
    assert!(LOCAL_QUEUE_CAPACITY - 1 <= u8::MAX as usize);
}
