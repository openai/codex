use std::alloc::{alloc_zeroed, handle_alloc_error, Layout};
use std::boxed::Box;
use std::cell::{Cell, UnsafeCell};
use std::cmp;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::atomic::{self, AtomicIsize, AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_epoch::{self as epoch, Atomic, Owned};
use crossbeam_utils::{Backoff, CachePadded};

// Minimum buffer capacity.
const MIN_CAP: usize = 64;
// Maximum number of tasks that can be stolen in `steal_batch()` and `steal_batch_and_pop()`.
const MAX_BATCH: usize = 32;
// If a buffer of at least this size is retired, thread-local garbage is flushed so that it gets
// deallocated as soon as possible.
const FLUSH_THRESHOLD_BYTES: usize = 1 << 10;

/// A buffer that holds tasks in a worker queue.
///
/// This is just a pointer to the buffer and its length - dropping an instance of this struct will
/// *not* deallocate the buffer.
struct Buffer<T> {
    /// Pointer to the allocated memory.
    ptr: *mut T,

    /// Capacity of the buffer. Always a power of two.
    cap: usize,
}

unsafe impl<T> Send for Buffer<T> {}

impl<T> Buffer<T> {
    /// Allocates a new buffer with the specified capacity.
    fn alloc(cap: usize) -> Buffer<T> {
        debug_assert_eq!(cap, cap.next_power_of_two());

        let ptr = Box::into_raw(
            (0..cap)
                .map(|_| MaybeUninit::<T>::uninit())
                .collect::<Box<[_]>>(),
        )
        .cast::<T>();

        Buffer { ptr, cap }
    }

    /// Deallocates the buffer.
    unsafe fn dealloc(self) {
        drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
            self.ptr.cast::<MaybeUninit<T>>(),
            self.cap,
        )));
    }

    /// Returns a pointer to the task at the specified `index`.
    unsafe fn at(&self, index: isize) -> *mut T {
        // `self.cap` is always a power of two.
        // We do all the loads at `MaybeUninit` because we might realize, after loading, that we
        // don't actually have the right to access this memory.
        self.ptr.offset(index & (self.cap - 1) as isize)
    }

    /// Writes `task` into the specified `index`.
    ///
    /// This method might be concurrently called with another `read` at the same index, which is
    /// technically speaking a data race and therefore UB. We should use an atomic store here, but
    /// that would be more expensive and difficult to implement generically for all types `T`.
    /// Hence, as a hack, we use a volatile write instead.
    unsafe fn write(&self, index: isize, task: MaybeUninit<T>) {
        ptr::write_volatile(self.at(index).cast::<MaybeUninit<T>>(), task)
    }

    /// Reads a task from the specified `index`.
    ///
    /// This method might be concurrently called with another `write` at the same index, which is
    /// technically speaking a data race and therefore UB. We should use an atomic load here, but
    /// that would be more expensive and difficult to implement generically for all types `T`.
    /// Hence, as a hack, we use a volatile load instead.
    unsafe fn read(&self, index: isize) -> MaybeUninit<T> {
        ptr::read_volatile(self.at(index).cast::<MaybeUninit<T>>())
    }
}

impl<T> Clone for Buffer<T> {
    fn clone(&self) -> Buffer<T> {
        *self
    }
}

impl<T> Copy for Buffer<T> {}

/// Internal queue data shared between the worker and stealers.
///
/// The implementation is based on the following work:
///
/// 1. [Chase and Lev. Dynamic circular work-stealing deque. SPAA 2005.][chase-lev]
/// 2. [Le, Pop, Cohen, and Nardelli. Correct and efficient work-stealing for weak memory models.
///    PPoPP 2013.][weak-mem]
/// 3. [Norris and Demsky. CDSchecker: checking concurrent data structures written with C/C++
///    atomics. OOPSLA 2013.][checker]
///
/// [chase-lev]: https://dl.acm.org/citation.cfm?id=1073974
/// [weak-mem]: https://dl.acm.org/citation.cfm?id=2442524
/// [checker]: https://dl.acm.org/citation.cfm?id=2509514
struct Inner<T> {
    /// The front index.
    front: AtomicIsize,

    /// The back index.
    back: AtomicIsize,

    /// The underlying buffer.
    buffer: CachePadded<Atomic<Buffer<T>>>,
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        // Load the back index, front index, and buffer.
        let b = *self.back.get_mut();
        let f = *self.front.get_mut();

        unsafe {
            let buffer = self.buffer.load(Ordering::Relaxed, epoch::unprotected());

            // Go through the buffer from front to back and drop all tasks in the queue.
            let mut i = f;
            while i != b {
                buffer.deref().at(i).drop_in_place();
                i = i.wrapping_add(1);
            }

            // Free the memory allocated by the buffer.
            buffer.into_owned().into_box().dealloc();
        }
    }
}

/// Worker queue flavor: FIFO or LIFO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Flavor {
    /// The first-in first-out flavor.
    Fifo,

    /// The last-in first-out flavor.
    Lifo,
}

/// A worker queue.
///
/// This is a FIFO or LIFO queue that is owned by a single thread, but other threads may steal
/// tasks from it. Task schedulers typically create a single worker queue per thread.
///
/// # Examples
///
/// A FIFO worker:
///
/// ```
/// use crossbeam_deque::{Steal, Worker};
///
/// let w = Worker::new_fifo();
/// let s = w.stealer();
///
/// w.push(1);
/// w.push(2);
/// w.push(3);
///
/// assert_eq!(s.steal(), Steal::Success(1));
/// assert_eq!(w.pop(), Some(2));
/// assert_eq!(w.pop(), Some(3));
/// ```
///
/// A LIFO worker:
///
/// ```
/// use crossbeam_deque::{Steal, Worker};
///
/// let w = Worker::new_lifo();
/// let s = w.stealer();
///
/// w.push(1);
/// w.push(2);
/// w.push(3);
///
/// assert_eq!(s.steal(), Steal::Success(1));
/// assert_eq!(w.pop(), Some(3));
/// assert_eq!(w.pop(), Some(2));
/// ```
pub struct Worker<T> {
    /// A reference to the inner representation of the queue.
    inner: Arc<CachePadded<Inner<T>>>,

    /// A copy of `inner.buffer` for quick access.
    buffer: Cell<Buffer<T>>,

    /// The flavor of the queue.
    flavor: Flavor,

    /// Indicates that the worker cannot be shared among threads.
    _marker: PhantomData<*mut ()>, // !Send + !Sync
}

unsafe impl<T: Send> Send for Worker<T> {}

impl<T> Worker<T> {
    /// Creates a FIFO worker queue.
    ///
    /// Tasks are pushed and popped from opposite ends.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::<i32>::new_fifo();
    /// ```
    pub fn new_fifo() -> Worker<T> {
        let buffer = Buffer::alloc(MIN_CAP);

        let inner = Arc::new(CachePadded::new(Inner {
            front: AtomicIsize::new(0),
            back: AtomicIsize::new(0),
            buffer: CachePadded::new(Atomic::new(buffer)),
        }));

        Worker {
            inner,
            buffer: Cell::new(buffer),
            flavor: Flavor::Fifo,
            _marker: PhantomData,
        }
    }

    /// Creates a LIFO worker queue.
    ///
    /// Tasks are pushed and popped from the same end.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::<i32>::new_lifo();
    /// ```
    pub fn new_lifo() -> Worker<T> {
        let buffer = Buffer::alloc(MIN_CAP);

        let inner = Arc::new(CachePadded::new(Inner {
            front: AtomicIsize::new(0),
            back: AtomicIsize::new(0),
            buffer: CachePadded::new(Atomic::new(buffer)),
        }));

        Worker {
            inner,
            buffer: Cell::new(buffer),
            flavor: Flavor::Lifo,
            _marker: PhantomData,
        }
    }

    /// Creates a stealer for this queue.
    ///
    /// The returned stealer can be shared among threads and cloned.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::<i32>::new_lifo();
    /// let s = w.stealer();
    /// ```
    pub fn stealer(&self) -> Stealer<T> {
        Stealer {
            inner: self.inner.clone(),
            flavor: self.flavor,
        }
    }

    /// Resizes the internal buffer to the new capacity of `new_cap`.
    #[cold]
    unsafe fn resize(&self, new_cap: usize) {
        // Load the back index, front index, and buffer.
        let b = self.inner.back.load(Ordering::Relaxed);
        let f = self.inner.front.load(Ordering::Relaxed);
        let buffer = self.buffer.get();

        // Allocate a new buffer and copy data from the old buffer to the new one.
        let new = Buffer::alloc(new_cap);
        let mut i = f;
        while i != b {
            ptr::copy_nonoverlapping(buffer.at(i), new.at(i), 1);
            i = i.wrapping_add(1);
        }

        let guard = &epoch::pin();

        // Replace the old buffer with the new one.
        self.buffer.replace(new);
        let old =
            self.inner
                .buffer
                .swap(Owned::new(new).into_shared(guard), Ordering::Release, guard);

        // Destroy the old buffer later.
        guard.defer_unchecked(move || old.into_owned().into_box().dealloc());

        // If the buffer is very large, then flush the thread-local garbage in order to deallocate
        // it as soon as possible.
        if mem::size_of::<T>() * new_cap >= FLUSH_THRESHOLD_BYTES {
            guard.flush();
        }
    }

    /// Reserves enough capacity so that `reserve_cap` tasks can be pushed without growing the
    /// buffer.
    fn reserve(&self, reserve_cap: usize) {
        if reserve_cap > 0 {
            // Compute the current length.
            let b = self.inner.back.load(Ordering::Relaxed);
            let f = self.inner.front.load(Ordering::SeqCst);
            let len = b.wrapping_sub(f) as usize;

            // The current capacity.
            let cap = self.buffer.get().cap;

            // Is there enough capacity to push `reserve_cap` tasks?
            if cap - len < reserve_cap {
                // Keep doubling the capacity as much as is needed.
                let mut new_cap = cap * 2;
                while new_cap - len < reserve_cap {
                    new_cap *= 2;
                }

                // Resize the buffer.
                unsafe {
                    self.resize(new_cap);
                }
            }
        }
    }

    /// Returns `true` if the queue is empty.
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::new_lifo();
    ///
    /// assert!(w.is_empty());
    /// w.push(1);
    /// assert!(!w.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        let b = self.inner.back.load(Ordering::Relaxed);
        let f = self.inner.front.load(Ordering::SeqCst);
        b.wrapping_sub(f) <= 0
    }

    /// Returns the number of tasks in the deque.
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::new_lifo();
    ///
    /// assert_eq!(w.len(), 0);
    /// w.push(1);
    /// assert_eq!(w.len(), 1);
    /// w.push(1);
    /// assert_eq!(w.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        let b = self.inner.back.load(Ordering::Relaxed);
        let f = self.inner.front.load(Ordering::SeqCst);
        b.wrapping_sub(f).max(0) as usize
    }

    /// Pushes a task into the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::new_lifo();
    /// w.push(1);
    /// w.push(2);
    /// ```
    pub fn push(&self, task: T) {
        // Load the back index, front index, and buffer.
        let b = self.inner.back.load(Ordering::Relaxed);
        let f = self.inner.front.load(Ordering::Acquire);
        let mut buffer = self.buffer.get();

        // Calculate the length of the queue.
        let len = b.wrapping_sub(f);

        // Is the queue full?
        if len >= buffer.cap as isize {
            // Yes. Grow the underlying buffer.
            unsafe {
                self.resize(2 * buffer.cap);
            }
            buffer = self.buffer.get();
        }

        // Write `task` into the slot.
        unsafe {
            buffer.write(b, MaybeUninit::new(task));
        }

        atomic::fence(Ordering::Release);

        // Increment the back index.
        //
        // This ordering could be `Relaxed`, but then thread sanitizer would falsely report data
        // races because it doesn't understand fences.
        self.inner.back.store(b.wrapping_add(1), Ordering::Release);
    }

    /// Pops a task from the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::new_fifo();
    /// w.push(1);
    /// w.push(2);
    ///
    /// assert_eq!(w.pop(), Some(1));
    /// assert_eq!(w.pop(), Some(2));
    /// assert_eq!(w.pop(), None);
    /// ```
    pub fn pop(&self) -> Option<T> {
        // Load the back and front index.
        let b = self.inner.back.load(Ordering::Relaxed);
        let f = self.inner.front.load(Ordering::Relaxed);

        // Calculate the length of the queue.
        let len = b.wrapping_sub(f);

        // Is the queue empty?
        if len <= 0 {
            return None;
        }

        match self.flavor {
            // Pop from the front of the queue.
            Flavor::Fifo => {
                // Try incrementing the front index to pop the task.
                let f = self.inner.front.fetch_add(1, Ordering::SeqCst);
                let new_f = f.wrapping_add(1);

                if b.wrapping_sub(new_f) < 0 {
                    self.inner.front.store(f, Ordering::Relaxed);
                    return None;
                }

                unsafe {
                    // Read the popped task.
                    let buffer = self.buffer.get();
                    let task = buffer.read(f).assume_init();

                    // Shrink the buffer if `len - 1` is less than one fourth of the capacity.
                    if buffer.cap > MIN_CAP && len <= buffer.cap as isize / 4 {
                        self.resize(buffer.cap / 2);
                    }

                    Some(task)
                }
            }

            // Pop from the back of the queue.
            Flavor::Lifo => {
                // Decrement the back index.
                let b = b.wrapping_sub(1);
                self.inner.back.store(b, Ordering::Relaxed);

                atomic::fence(Ordering::SeqCst);

                // Load the front index.
                let f = self.inner.front.load(Ordering::Relaxed);

                // Compute the length after the back index was decremented.
                let len = b.wrapping_sub(f);

                if len < 0 {
                    // The queue is empty. Restore the back index to the original task.
                    self.inner.back.store(b.wrapping_add(1), Ordering::Relaxed);
                    None
                } else {
                    // Read the task to be popped.
                    let buffer = self.buffer.get();
                    let mut task = unsafe { Some(buffer.read(b)) };

                    // Are we popping the last task from the queue?
                    if len == 0 {
                        // Try incrementing the front index.
                        if self
                            .inner
                            .front
                            .compare_exchange(
                                f,
                                f.wrapping_add(1),
                                Ordering::SeqCst,
                                Ordering::Relaxed,
                            )
                            .is_err()
                        {
                            // Failed. We didn't pop anything. Reset to `None`.
                            task.take();
                        }

                        // Restore the back index to the original task.
                        self.inner.back.store(b.wrapping_add(1), Ordering::Relaxed);
                    } else {
                        // Shrink the buffer if `len` is less than one fourth of the capacity.
                        if buffer.cap > MIN_CAP && len < buffer.cap as isize / 4 {
                            unsafe {
                                self.resize(buffer.cap / 2);
                            }
                        }
                    }

                    task.map(|t| unsafe { t.assume_init() })
                }
            }
        }
    }
}

impl<T> fmt::Debug for Worker<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Worker { .. }")
    }
}

/// A stealer handle of a worker queue.
///
/// Stealers can be shared among threads.
///
/// Task schedulers typically have a single worker queue per worker thread.
///
/// # Examples
///
/// ```
/// use crossbeam_deque::{Steal, Worker};
///
/// let w = Worker::new_lifo();
/// w.push(1);
/// w.push(2);
///
/// let s = w.stealer();
/// assert_eq!(s.steal(), Steal::Success(1));
/// assert_eq!(s.steal(), Steal::Success(2));
/// assert_eq!(s.steal(), Steal::Empty);
/// ```
pub struct Stealer<T> {
    /// A reference to the inner representation of the queue.
    inner: Arc<CachePadded<Inner<T>>>,

    /// The flavor of the queue.
    flavor: Flavor,
}

unsafe impl<T: Send> Send for Stealer<T> {}
unsafe impl<T: Send> Sync for Stealer<T> {}

impl<T> Stealer<T> {
    /// Returns `true` if the queue is empty.
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::new_lifo();
    /// let s = w.stealer();
    ///
    /// assert!(s.is_empty());
    /// w.push(1);
    /// assert!(!s.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        let f = self.inner.front.load(Ordering::Acquire);
        atomic::fence(Ordering::SeqCst);
        let b = self.inner.back.load(Ordering::Acquire);
        b.wrapping_sub(f) <= 0
    }

    /// Returns the number of tasks in the deque.
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w = Worker::new_lifo();
    /// let s = w.stealer();
    ///
    /// assert_eq!(s.len(), 0);
    /// w.push(1);
    /// assert_eq!(s.len(), 1);
    /// w.push(2);
    /// assert_eq!(s.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        let f = self.inner.front.load(Ordering::Acquire);
        atomic::fence(Ordering::SeqCst);
        let b = self.inner.back.load(Ordering::Acquire);
        b.wrapping_sub(f).max(0) as usize
    }

    /// Steals a task from the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Steal, Worker};
    ///
    /// let w = Worker::new_lifo();
    /// w.push(1);
    /// w.push(2);
    ///
    /// let s = w.stealer();
    /// assert_eq!(s.steal(), Steal::Success(1));
    /// assert_eq!(s.steal(), Steal::Success(2));
    /// ```
    pub fn steal(&self) -> Steal<T> {
        // Load the front index.
        let f = self.inner.front.load(Ordering::Acquire);

        // A SeqCst fence is needed here.
        //
        // If the current thread is already pinned (reentrantly), we must manually issue the
        // fence. Otherwise, the following pinning will issue the fence anyway, so we don't
        // have to.
        if epoch::is_pinned() {
            atomic::fence(Ordering::SeqCst);
        }

        let guard = &epoch::pin();

        // Load the back index.
        let b = self.inner.back.load(Ordering::Acquire);

        // Is the queue empty?
        if b.wrapping_sub(f) <= 0 {
            return Steal::Empty;
        }

        // Load the buffer and read the task at the front.
        let buffer = self.inner.buffer.load(Ordering::Acquire, guard);
        let task = unsafe { buffer.deref().read(f) };

        // Try incrementing the front index to steal the task.
        // If the buffer has been swapped or the increment fails, we retry.
        if self.inner.buffer.load(Ordering::Acquire, guard) != buffer
            || self
                .inner
                .front
                .compare_exchange(f, f.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
                .is_err()
        {
            // We didn't steal this task, forget it.
            return Steal::Retry;
        }

        // Return the stolen task.
        Steal::Success(unsafe { task.assume_init() })
    }

    /// Steals a batch of tasks and pushes them into another worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than some constant limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w1 = Worker::new_fifo();
    /// w1.push(1);
    /// w1.push(2);
    /// w1.push(3);
    /// w1.push(4);
    ///
    /// let s = w1.stealer();
    /// let w2 = Worker::new_fifo();
    ///
    /// let _ = s.steal_batch(&w2);
    /// assert_eq!(w2.pop(), Some(1));
    /// assert_eq!(w2.pop(), Some(2));
    /// ```
    pub fn steal_batch(&self, dest: &Worker<T>) -> Steal<()> {
        self.steal_batch_with_limit(dest, MAX_BATCH)
    }

    /// Steals no more than `limit` of tasks and pushes them into another worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than the given limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Worker;
    ///
    /// let w1 = Worker::new_fifo();
    /// w1.push(1);
    /// w1.push(2);
    /// w1.push(3);
    /// w1.push(4);
    /// w1.push(5);
    /// w1.push(6);
    ///
    /// let s = w1.stealer();
    /// let w2 = Worker::new_fifo();
    ///
    /// let _ = s.steal_batch_with_limit(&w2, 2);
    /// assert_eq!(w2.pop(), Some(1));
    /// assert_eq!(w2.pop(), Some(2));
    /// assert_eq!(w2.pop(), None);
    ///
    /// w1.push(7);
    /// w1.push(8);
    /// // Setting a large limit does not guarantee that all elements will be popped. In this case,
    /// // half of the elements are currently popped, but the number of popped elements is considered
    /// // an implementation detail that may be changed in the future.
    /// let _ = s.steal_batch_with_limit(&w2, std::usize::MAX);
    /// assert_eq!(w2.len(), 3);
    /// ```
    pub fn steal_batch_with_limit(&self, dest: &Worker<T>, limit: usize) -> Steal<()> {
        assert!(limit > 0);
        if Arc::ptr_eq(&self.inner, &dest.inner) {
            if dest.is_empty() {
                return Steal::Empty;
            } else {
                return Steal::Success(());
            }
        }

        // Load the front index.
        let mut f = self.inner.front.load(Ordering::Acquire);

        // A SeqCst fence is needed here.
        //
        // If the current thread is already pinned (reentrantly), we must manually issue the
        // fence. Otherwise, the following pinning will issue the fence anyway, so we don't
        // have to.
        if epoch::is_pinned() {
            atomic::fence(Ordering::SeqCst);
        }

        let guard = &epoch::pin();

        // Load the back index.
        let b = self.inner.back.load(Ordering::Acquire);

        // Is the queue empty?
        let len = b.wrapping_sub(f);
        if len <= 0 {
            return Steal::Empty;
        }

        // Reserve capacity for the stolen batch.
        let batch_size = cmp::min((len as usize + 1) / 2, limit);
        dest.reserve(batch_size);
        let mut batch_size = batch_size as isize;

        // Get the destination buffer and back index.
        let dest_buffer = dest.buffer.get();
        let mut dest_b = dest.inner.back.load(Ordering::Relaxed);

        // Load the buffer.
        let buffer = self.inner.buffer.load(Ordering::Acquire, guard);

        match self.flavor {
            // Steal a batch of tasks from the front at once.
            Flavor::Fifo => {
                // Copy the batch from the source to the destination buffer.
                match dest.flavor {
                    Flavor::Fifo => {
                        for i in 0..batch_size {
                            unsafe {
                                let task = buffer.deref().read(f.wrapping_add(i));
                                dest_buffer.write(dest_b.wrapping_add(i), task);
                            }
                        }
                    }
                    Flavor::Lifo => {
                        for i in 0..batch_size {
                            unsafe {
                                let task = buffer.deref().read(f.wrapping_add(i));
                                dest_buffer.write(dest_b.wrapping_add(batch_size - 1 - i), task);
                            }
                        }
                    }
                }

                // Try incrementing the front index to steal the batch.
                // If the buffer has been swapped or the increment fails, we retry.
                if self.inner.buffer.load(Ordering::Acquire, guard) != buffer
                    || self
                        .inner
                        .front
                        .compare_exchange(
                            f,
                            f.wrapping_add(batch_size),
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        )
                        .is_err()
                {
                    return Steal::Retry;
                }

                dest_b = dest_b.wrapping_add(batch_size);
            }

            // Steal a batch of tasks from the front one by one.
            Flavor::Lifo => {
                // This loop may modify the batch_size, which triggers a clippy lint warning.
                // Use a new variable to avoid the warning, and to make it clear we aren't
                // modifying the loop exit condition during iteration.
                let original_batch_size = batch_size;

                for i in 0..original_batch_size {
                    // If this is not the first steal, check whether the queue is empty.
                    if i > 0 {
                        // We've already got the current front index. Now execute the fence to
                        // synchronize with other threads.
                        atomic::fence(Ordering::SeqCst);

                        // Load the back index.
                        let b = self.inner.back.load(Ordering::Acquire);

                        // Is the queue empty?
                        if b.wrapping_sub(f) <= 0 {
                            batch_size = i;
                            break;
                        }
                    }

                    // Read the task at the front.
                    let task = unsafe { buffer.deref().read(f) };

                    // Try incrementing the front index to steal the task.
                    // If the buffer has been swapped or the increment fails, we retry.
                    if self.inner.buffer.load(Ordering::Acquire, guard) != buffer
                        || self
                            .inner
                            .front
                            .compare_exchange(
                                f,
                                f.wrapping_add(1),
                                Ordering::SeqCst,
                                Ordering::Relaxed,
                            )
                            .is_err()
                    {
                        // We didn't steal this task, forget it and break from the loop.
                        batch_size = i;
                        break;
                    }

                    // Write the stolen task into the destination buffer.
                    unsafe {
                        dest_buffer.write(dest_b, task);
                    }

                    // Move the source front index and the destination back index one step forward.
                    f = f.wrapping_add(1);
                    dest_b = dest_b.wrapping_add(1);
                }

                // If we didn't steal anything, the operation needs to be retried.
                if batch_size == 0 {
                    return Steal::Retry;
                }

                // If stealing into a FIFO queue, stolen tasks need to be reversed.
                if dest.flavor == Flavor::Fifo {
                    for i in 0..batch_size / 2 {
                        unsafe {
                            let i1 = dest_b.wrapping_sub(batch_size - i);
                            let i2 = dest_b.wrapping_sub(i + 1);
                            let t1 = dest_buffer.read(i1);
                            let t2 = dest_buffer.read(i2);
                            dest_buffer.write(i1, t2);
                            dest_buffer.write(i2, t1);
                        }
                    }
                }
            }
        }

        atomic::fence(Ordering::Release);

        // Update the back index in the destination queue.
        //
        // This ordering could be `Relaxed`, but then thread sanitizer would falsely report data
        // races because it doesn't understand fences.
        dest.inner.back.store(dest_b, Ordering::Release);

        // Return with success.
        Steal::Success(())
    }

    /// Steals a batch of tasks, pushes them into another worker, and pops a task from that worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than some constant limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Steal, Worker};
    ///
    /// let w1 = Worker::new_fifo();
    /// w1.push(1);
    /// w1.push(2);
    /// w1.push(3);
    /// w1.push(4);
    ///
    /// let s = w1.stealer();
    /// let w2 = Worker::new_fifo();
    ///
    /// assert_eq!(s.steal_batch_and_pop(&w2), Steal::Success(1));
    /// assert_eq!(w2.pop(), Some(2));
    /// ```
    pub fn steal_batch_and_pop(&self, dest: &Worker<T>) -> Steal<T> {
        self.steal_batch_with_limit_and_pop(dest, MAX_BATCH)
    }

    /// Steals no more than `limit` of tasks, pushes them into another worker, and pops a task from
    /// that worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than the given limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Steal, Worker};
    ///
    /// let w1 = Worker::new_fifo();
    /// w1.push(1);
    /// w1.push(2);
    /// w1.push(3);
    /// w1.push(4);
    /// w1.push(5);
    /// w1.push(6);
    ///
    /// let s = w1.stealer();
    /// let w2 = Worker::new_fifo();
    ///
    /// assert_eq!(s.steal_batch_with_limit_and_pop(&w2, 2), Steal::Success(1));
    /// assert_eq!(w2.pop(), Some(2));
    /// assert_eq!(w2.pop(), None);
    ///
    /// w1.push(7);
    /// w1.push(8);
    /// // Setting a large limit does not guarantee that all elements will be popped. In this case,
    /// // half of the elements are currently popped, but the number of popped elements is considered
    /// // an implementation detail that may be changed in the future.
    /// assert_eq!(s.steal_batch_with_limit_and_pop(&w2, std::usize::MAX), Steal::Success(3));
    /// assert_eq!(w2.pop(), Some(4));
    /// assert_eq!(w2.pop(), Some(5));
    /// assert_eq!(w2.pop(), None);
    /// ```
    pub fn steal_batch_with_limit_and_pop(&self, dest: &Worker<T>, limit: usize) -> Steal<T> {
        assert!(limit > 0);
        if Arc::ptr_eq(&self.inner, &dest.inner) {
            match dest.pop() {
                None => return Steal::Empty,
                Some(task) => return Steal::Success(task),
            }
        }

        // Load the front index.
        let mut f = self.inner.front.load(Ordering::Acquire);

        // A SeqCst fence is needed here.
        //
        // If the current thread is already pinned (reentrantly), we must manually issue the
        // fence. Otherwise, the following pinning will issue the fence anyway, so we don't
        // have to.
        if epoch::is_pinned() {
            atomic::fence(Ordering::SeqCst);
        }

        let guard = &epoch::pin();

        // Load the back index.
        let b = self.inner.back.load(Ordering::Acquire);

        // Is the queue empty?
        let len = b.wrapping_sub(f);
        if len <= 0 {
            return Steal::Empty;
        }

        // Reserve capacity for the stolen batch.
        let batch_size = cmp::min((len as usize - 1) / 2, limit - 1);
        dest.reserve(batch_size);
        let mut batch_size = batch_size as isize;

        // Get the destination buffer and back index.
        let dest_buffer = dest.buffer.get();
        let mut dest_b = dest.inner.back.load(Ordering::Relaxed);

        // Load the buffer
        let buffer = self.inner.buffer.load(Ordering::Acquire, guard);

        // Read the task at the front.
        let mut task = unsafe { buffer.deref().read(f) };

        match self.flavor {
            // Steal a batch of tasks from the front at once.
            Flavor::Fifo => {
                // Copy the batch from the source to the destination buffer.
                match dest.flavor {
                    Flavor::Fifo => {
                        for i in 0..batch_size {
                            unsafe {
                                let task = buffer.deref().read(f.wrapping_add(i + 1));
                                dest_buffer.write(dest_b.wrapping_add(i), task);
                            }
                        }
                    }
                    Flavor::Lifo => {
                        for i in 0..batch_size {
                            unsafe {
                                let task = buffer.deref().read(f.wrapping_add(i + 1));
                                dest_buffer.write(dest_b.wrapping_add(batch_size - 1 - i), task);
                            }
                        }
                    }
                }

                // Try incrementing the front index to steal the task.
                // If the buffer has been swapped or the increment fails, we retry.
                if self.inner.buffer.load(Ordering::Acquire, guard) != buffer
                    || self
                        .inner
                        .front
                        .compare_exchange(
                            f,
                            f.wrapping_add(batch_size + 1),
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        )
                        .is_err()
                {
                    // We didn't steal this task, forget it.
                    return Steal::Retry;
                }

                dest_b = dest_b.wrapping_add(batch_size);
            }

            // Steal a batch of tasks from the front one by one.
            Flavor::Lifo => {
                // Try incrementing the front index to steal the task.
                if self
                    .inner
                    .front
                    .compare_exchange(f, f.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
                    .is_err()
                {
                    // We didn't steal this task, forget it.
                    return Steal::Retry;
                }

                // Move the front index one step forward.
                f = f.wrapping_add(1);

                // Repeat the same procedure for the batch steals.
                //
                // This loop may modify the batch_size, which triggers a clippy lint warning.
                // Use a new variable to avoid the warning, and to make it clear we aren't
                // modifying the loop exit condition during iteration.
                let original_batch_size = batch_size;
                for i in 0..original_batch_size {
                    // We've already got the current front index. Now execute the fence to
                    // synchronize with other threads.
                    atomic::fence(Ordering::SeqCst);

                    // Load the back index.
                    let b = self.inner.back.load(Ordering::Acquire);

                    // Is the queue empty?
                    if b.wrapping_sub(f) <= 0 {
                        batch_size = i;
                        break;
                    }

                    // Read the task at the front.
                    let tmp = unsafe { buffer.deref().read(f) };

                    // Try incrementing the front index to steal the task.
                    // If the buffer has been swapped or the increment fails, we retry.
                    if self.inner.buffer.load(Ordering::Acquire, guard) != buffer
                        || self
                            .inner
                            .front
                            .compare_exchange(
                                f,
                                f.wrapping_add(1),
                                Ordering::SeqCst,
                                Ordering::Relaxed,
                            )
                            .is_err()
                    {
                        // We didn't steal this task, forget it and break from the loop.
                        batch_size = i;
                        break;
                    }

                    // Write the previously stolen task into the destination buffer.
                    unsafe {
                        dest_buffer.write(dest_b, mem::replace(&mut task, tmp));
                    }

                    // Move the source front index and the destination back index one step forward.
                    f = f.wrapping_add(1);
                    dest_b = dest_b.wrapping_add(1);
                }

                // If stealing into a FIFO queue, stolen tasks need to be reversed.
                if dest.flavor == Flavor::Fifo {
                    for i in 0..batch_size / 2 {
                        unsafe {
                            let i1 = dest_b.wrapping_sub(batch_size - i);
                            let i2 = dest_b.wrapping_sub(i + 1);
                            let t1 = dest_buffer.read(i1);
                            let t2 = dest_buffer.read(i2);
                            dest_buffer.write(i1, t2);
                            dest_buffer.write(i2, t1);
                        }
                    }
                }
            }
        }

        atomic::fence(Ordering::Release);

        // Update the back index in the destination queue.
        //
        // This ordering could be `Relaxed`, but then thread sanitizer would falsely report data
        // races because it doesn't understand fences.
        dest.inner.back.store(dest_b, Ordering::Release);

        // Return with success.
        Steal::Success(unsafe { task.assume_init() })
    }
}

impl<T> Clone for Stealer<T> {
    fn clone(&self) -> Stealer<T> {
        Stealer {
            inner: self.inner.clone(),
            flavor: self.flavor,
        }
    }
}

impl<T> fmt::Debug for Stealer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Stealer { .. }")
    }
}

// Bits indicating the state of a slot:
// * If a task has been written into the slot, `WRITE` is set.
// * If a task has been read from the slot, `READ` is set.
// * If the block is being destroyed, `DESTROY` is set.
const WRITE: usize = 1;
const READ: usize = 2;
const DESTROY: usize = 4;

// Each block covers one "lap" of indices.
const LAP: usize = 64;
// The maximum number of values a block can hold.
const BLOCK_CAP: usize = LAP - 1;
// How many lower bits are reserved for metadata.
const SHIFT: usize = 1;
// Indicates that the block is not the last one.
const HAS_NEXT: usize = 1;

/// A slot in a block.
struct Slot<T> {
    /// The task.
    task: UnsafeCell<MaybeUninit<T>>,

    /// The state of the slot.
    state: AtomicUsize,
}

impl<T> Slot<T> {
    /// Waits until a task is written into the slot.
    fn wait_write(&self) {
        let backoff = Backoff::new();
        while self.state.load(Ordering::Acquire) & WRITE == 0 {
            backoff.snooze();
        }
    }
}

/// A block in a linked list.
///
/// Each block in the list can hold up to `BLOCK_CAP` values.
struct Block<T> {
    /// The next block in the linked list.
    next: AtomicPtr<Block<T>>,

    /// Slots for values.
    slots: [Slot<T>; BLOCK_CAP],
}

impl<T> Block<T> {
    const LAYOUT: Layout = {
        let layout = Layout::new::<Self>();
        assert!(
            layout.size() != 0,
            "Block should never be zero-sized, as it has an AtomicPtr field"
        );
        layout
    };

    /// Creates an empty block.
    fn new() -> Box<Self> {
        // SAFETY: layout is not zero-sized
        let ptr = unsafe { alloc_zeroed(Self::LAYOUT) };
        // Handle allocation failure
        if ptr.is_null() {
            handle_alloc_error(Self::LAYOUT)
        }
        // SAFETY: This is safe because:
        //  [1] `Block::next` (AtomicPtr) may be safely zero initialized.
        //  [2] `Block::slots` (Array) may be safely zero initialized because of [3, 4].
        //  [3] `Slot::task` (UnsafeCell) may be safely zero initialized because it
        //       holds a MaybeUninit.
        //  [4] `Slot::state` (AtomicUsize) may be safely zero initialized.
        // TODO: unsafe { Box::new_zeroed().assume_init() }
        unsafe { Box::from_raw(ptr.cast()) }
    }

    /// Waits until the next pointer is set.
    fn wait_next(&self) -> *mut Block<T> {
        let backoff = Backoff::new();
        loop {
            let next = self.next.load(Ordering::Acquire);
            if !next.is_null() {
                return next;
            }
            backoff.snooze();
        }
    }

    /// Sets the `DESTROY` bit in slots starting from `start` and destroys the block.
    unsafe fn destroy(this: *mut Block<T>, count: usize) {
        // It is not necessary to set the `DESTROY` bit in the last slot because that slot has
        // begun destruction of the block.
        for i in (0..count).rev() {
            let slot = (*this).slots.get_unchecked(i);

            // Mark the `DESTROY` bit if a thread is still using the slot.
            if slot.state.load(Ordering::Acquire) & READ == 0
                && slot.state.fetch_or(DESTROY, Ordering::AcqRel) & READ == 0
            {
                // If a thread is still using the slot, it will continue destruction of the block.
                return;
            }
        }

        // No thread is using the block, now it is safe to destroy it.
        drop(Box::from_raw(this));
    }
}

/// A position in a queue.
struct Position<T> {
    /// The index in the queue.
    index: AtomicUsize,

    /// The block in the linked list.
    block: AtomicPtr<Block<T>>,
}

/// An injector queue.
///
/// This is a FIFO queue that can be shared among multiple threads. Task schedulers typically have
/// a single injector queue, which is the entry point for new tasks.
///
/// # Examples
///
/// ```
/// use crossbeam_deque::{Injector, Steal};
///
/// let q = Injector::new();
/// q.push(1);
/// q.push(2);
///
/// assert_eq!(q.steal(), Steal::Success(1));
/// assert_eq!(q.steal(), Steal::Success(2));
/// assert_eq!(q.steal(), Steal::Empty);
/// ```
pub struct Injector<T> {
    /// The head of the queue.
    head: CachePadded<Position<T>>,

    /// The tail of the queue.
    tail: CachePadded<Position<T>>,

    /// Indicates that dropping a `Injector<T>` may drop values of type `T`.
    _marker: PhantomData<T>,
}

unsafe impl<T: Send> Send for Injector<T> {}
unsafe impl<T: Send> Sync for Injector<T> {}

impl<T> Default for Injector<T> {
    fn default() -> Self {
        let block = Box::into_raw(Block::<T>::new());
        Self {
            head: CachePadded::new(Position {
                block: AtomicPtr::new(block),
                index: AtomicUsize::new(0),
            }),
            tail: CachePadded::new(Position {
                block: AtomicPtr::new(block),
                index: AtomicUsize::new(0),
            }),
            _marker: PhantomData,
        }
    }
}

impl<T> Injector<T> {
    /// Creates a new injector queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Injector;
    ///
    /// let q = Injector::<i32>::new();
    /// ```
    pub fn new() -> Injector<T> {
        Self::default()
    }

    /// Pushes a task into the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Injector;
    ///
    /// let w = Injector::new();
    /// w.push(1);
    /// w.push(2);
    /// ```
    pub fn push(&self, task: T) {
        let backoff = Backoff::new();
        let mut tail = self.tail.index.load(Ordering::Acquire);
        let mut block = self.tail.block.load(Ordering::Acquire);
        let mut next_block = None;

        loop {
            // Calculate the offset of the index into the block.
            let offset = (tail >> SHIFT) % LAP;

            // If we reached the end of the block, wait until the next one is installed.
            if offset == BLOCK_CAP {
                backoff.snooze();
                tail = self.tail.index.load(Ordering::Acquire);
                block = self.tail.block.load(Ordering::Acquire);
                continue;
            }

            // If we're going to have to install the next block, allocate it in advance in order to
            // make the wait for other threads as short as possible.
            if offset + 1 == BLOCK_CAP && next_block.is_none() {
                next_block = Some(Block::<T>::new());
            }

            let new_tail = tail + (1 << SHIFT);

            // Try advancing the tail forward.
            match self.tail.index.compare_exchange_weak(
                tail,
                new_tail,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => unsafe {
                    // If we've reached the end of the block, install the next one.
                    if offset + 1 == BLOCK_CAP {
                        let next_block = Box::into_raw(next_block.unwrap());
                        let next_index = new_tail.wrapping_add(1 << SHIFT);

                        self.tail.block.store(next_block, Ordering::Release);
                        self.tail.index.store(next_index, Ordering::Release);
                        (*block).next.store(next_block, Ordering::Release);
                    }

                    // Write the task into the slot.
                    let slot = (*block).slots.get_unchecked(offset);
                    slot.task.get().write(MaybeUninit::new(task));
                    slot.state.fetch_or(WRITE, Ordering::Release);

                    return;
                },
                Err(t) => {
                    tail = t;
                    block = self.tail.block.load(Ordering::Acquire);
                    backoff.spin();
                }
            }
        }
    }

    /// Steals a task from the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Injector, Steal};
    ///
    /// let q = Injector::new();
    /// q.push(1);
    /// q.push(2);
    ///
    /// assert_eq!(q.steal(), Steal::Success(1));
    /// assert_eq!(q.steal(), Steal::Success(2));
    /// assert_eq!(q.steal(), Steal::Empty);
    /// ```
    pub fn steal(&self) -> Steal<T> {
        let mut head;
        let mut block;
        let mut offset;

        let backoff = Backoff::new();
        loop {
            head = self.head.index.load(Ordering::Acquire);
            block = self.head.block.load(Ordering::Acquire);

            // Calculate the offset of the index into the block.
            offset = (head >> SHIFT) % LAP;

            // If we reached the end of the block, wait until the next one is installed.
            if offset == BLOCK_CAP {
                backoff.snooze();
            } else {
                break;
            }
        }

        let mut new_head = head + (1 << SHIFT);

        if new_head & HAS_NEXT == 0 {
            atomic::fence(Ordering::SeqCst);
            let tail = self.tail.index.load(Ordering::Relaxed);

            // If the tail equals the head, that means the queue is empty.
            if head >> SHIFT == tail >> SHIFT {
                return Steal::Empty;
            }

            // If head and tail are not in the same block, set `HAS_NEXT` in head.
            if (head >> SHIFT) / LAP != (tail >> SHIFT) / LAP {
                new_head |= HAS_NEXT;
            }
        }

        // Try moving the head index forward.
        if self
            .head
            .index
            .compare_exchange_weak(head, new_head, Ordering::SeqCst, Ordering::Acquire)
            .is_err()
        {
            return Steal::Retry;
        }

        unsafe {
            // If we've reached the end of the block, move to the next one.
            if offset + 1 == BLOCK_CAP {
                let next = (*block).wait_next();
                let mut next_index = (new_head & !HAS_NEXT).wrapping_add(1 << SHIFT);
                if !(*next).next.load(Ordering::Relaxed).is_null() {
                    next_index |= HAS_NEXT;
                }

                self.head.block.store(next, Ordering::Release);
                self.head.index.store(next_index, Ordering::Release);
            }

            // Read the task.
            let slot = (*block).slots.get_unchecked(offset);
            slot.wait_write();
            let task = slot.task.get().read().assume_init();

            // Destroy the block if we've reached the end, or if another thread wanted to destroy
            // but couldn't because we were busy reading from the slot.
            if (offset + 1 == BLOCK_CAP)
                || (slot.state.fetch_or(READ, Ordering::AcqRel) & DESTROY != 0)
            {
                Block::destroy(block, offset);
            }

            Steal::Success(task)
        }
    }

    /// Steals a batch of tasks and pushes them into a worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than some constant limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Injector, Worker};
    ///
    /// let q = Injector::new();
    /// q.push(1);
    /// q.push(2);
    /// q.push(3);
    /// q.push(4);
    ///
    /// let w = Worker::new_fifo();
    /// let _ = q.steal_batch(&w);
    /// assert_eq!(w.pop(), Some(1));
    /// assert_eq!(w.pop(), Some(2));
    /// ```
    pub fn steal_batch(&self, dest: &Worker<T>) -> Steal<()> {
        self.steal_batch_with_limit(dest, MAX_BATCH)
    }

    /// Steals no more than of tasks and pushes them into a worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than some constant limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Injector, Worker};
    ///
    /// let q = Injector::new();
    /// q.push(1);
    /// q.push(2);
    /// q.push(3);
    /// q.push(4);
    /// q.push(5);
    /// q.push(6);
    ///
    /// let w = Worker::new_fifo();
    /// let _ = q.steal_batch_with_limit(&w, 2);
    /// assert_eq!(w.pop(), Some(1));
    /// assert_eq!(w.pop(), Some(2));
    /// assert_eq!(w.pop(), None);
    ///
    /// q.push(7);
    /// q.push(8);
    /// // Setting a large limit does not guarantee that all elements will be popped. In this case,
    /// // half of the elements are currently popped, but the number of popped elements is considered
    /// // an implementation detail that may be changed in the future.
    /// let _ = q.steal_batch_with_limit(&w, std::usize::MAX);
    /// assert_eq!(w.len(), 3);
    /// ```
    pub fn steal_batch_with_limit(&self, dest: &Worker<T>, limit: usize) -> Steal<()> {
        assert!(limit > 0);
        let mut head;
        let mut block;
        let mut offset;

        let backoff = Backoff::new();
        loop {
            head = self.head.index.load(Ordering::Acquire);
            block = self.head.block.load(Ordering::Acquire);

            // Calculate the offset of the index into the block.
            offset = (head >> SHIFT) % LAP;

            // If we reached the end of the block, wait until the next one is installed.
            if offset == BLOCK_CAP {
                backoff.snooze();
            } else {
                break;
            }
        }

        let mut new_head = head;
        let advance;

        if new_head & HAS_NEXT == 0 {
            atomic::fence(Ordering::SeqCst);
            let tail = self.tail.index.load(Ordering::Relaxed);

            // If the tail equals the head, that means the queue is empty.
            if head >> SHIFT == tail >> SHIFT {
                return Steal::Empty;
            }

            // If head and tail are not in the same block, set `HAS_NEXT` in head. Also, calculate
            // the right batch size to steal.
            if (head >> SHIFT) / LAP != (tail >> SHIFT) / LAP {
                new_head |= HAS_NEXT;
                // We can steal all tasks till the end of the block.
                advance = (BLOCK_CAP - offset).min(limit);
            } else {
                let len = (tail - head) >> SHIFT;
                // Steal half of the available tasks.
                advance = ((len + 1) / 2).min(limit);
            }
        } else {
            // We can steal all tasks till the end of the block.
            advance = (BLOCK_CAP - offset).min(limit);
        }

        new_head += advance << SHIFT;
        let new_offset = offset + advance;

        // Try moving the head index forward.
        if self
            .head
            .index
            .compare_exchange_weak(head, new_head, Ordering::SeqCst, Ordering::Acquire)
            .is_err()
        {
            return Steal::Retry;
        }

        // Reserve capacity for the stolen batch.
        let batch_size = new_offset - offset;
        dest.reserve(batch_size);

        // Get the destination buffer and back index.
        let dest_buffer = dest.buffer.get();
        let dest_b = dest.inner.back.load(Ordering::Relaxed);

        unsafe {
            // If we've reached the end of the block, move to the next one.
            if new_offset == BLOCK_CAP {
                let next = (*block).wait_next();
                let mut next_index = (new_head & !HAS_NEXT).wrapping_add(1 << SHIFT);
                if !(*next).next.load(Ordering::Relaxed).is_null() {
                    next_index |= HAS_NEXT;
                }

                self.head.block.store(next, Ordering::Release);
                self.head.index.store(next_index, Ordering::Release);
            }

            // Copy values from the injector into the destination queue.
            match dest.flavor {
                Flavor::Fifo => {
                    for i in 0..batch_size {
                        // Read the task.
                        let slot = (*block).slots.get_unchecked(offset + i);
                        slot.wait_write();
                        let task = slot.task.get().read();

                        // Write it into the destination queue.
                        dest_buffer.write(dest_b.wrapping_add(i as isize), task);
                    }
                }

                Flavor::Lifo => {
                    for i in 0..batch_size {
                        // Read the task.
                        let slot = (*block).slots.get_unchecked(offset + i);
                        slot.wait_write();
                        let task = slot.task.get().read();

                        // Write it into the destination queue.
                        dest_buffer.write(dest_b.wrapping_add((batch_size - 1 - i) as isize), task);
                    }
                }
            }

            atomic::fence(Ordering::Release);

            // Update the back index in the destination queue.
            //
            // This ordering could be `Relaxed`, but then thread sanitizer would falsely report
            // data races because it doesn't understand fences.
            dest.inner
                .back
                .store(dest_b.wrapping_add(batch_size as isize), Ordering::Release);

            // Destroy the block if we've reached the end, or if another thread wanted to destroy
            // but couldn't because we were busy reading from the slot.
            if new_offset == BLOCK_CAP {
                Block::destroy(block, offset);
            } else {
                for i in offset..new_offset {
                    let slot = (*block).slots.get_unchecked(i);

                    if slot.state.fetch_or(READ, Ordering::AcqRel) & DESTROY != 0 {
                        Block::destroy(block, offset);
                        break;
                    }
                }
            }

            Steal::Success(())
        }
    }

    /// Steals a batch of tasks, pushes them into a worker, and pops a task from that worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than some constant limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Injector, Steal, Worker};
    ///
    /// let q = Injector::new();
    /// q.push(1);
    /// q.push(2);
    /// q.push(3);
    /// q.push(4);
    ///
    /// let w = Worker::new_fifo();
    /// assert_eq!(q.steal_batch_and_pop(&w), Steal::Success(1));
    /// assert_eq!(w.pop(), Some(2));
    /// ```
    pub fn steal_batch_and_pop(&self, dest: &Worker<T>) -> Steal<T> {
        // TODO: we use `MAX_BATCH + 1` as the hard limit for Injecter as the performance is slightly
        // better, but we may change it in the future to be compatible with the same method in Stealer.
        self.steal_batch_with_limit_and_pop(dest, MAX_BATCH + 1)
    }

    /// Steals no more than `limit` of tasks, pushes them into a worker, and pops a task from that worker.
    ///
    /// How many tasks exactly will be stolen is not specified. That said, this method will try to
    /// steal around half of the tasks in the queue, but also not more than the given limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::{Injector, Steal, Worker};
    ///
    /// let q = Injector::new();
    /// q.push(1);
    /// q.push(2);
    /// q.push(3);
    /// q.push(4);
    /// q.push(5);
    /// q.push(6);
    ///
    /// let w = Worker::new_fifo();
    /// assert_eq!(q.steal_batch_with_limit_and_pop(&w, 2), Steal::Success(1));
    /// assert_eq!(w.pop(), Some(2));
    /// assert_eq!(w.pop(), None);
    ///
    /// q.push(7);
    /// // Setting a large limit does not guarantee that all elements will be popped. In this case,
    /// // half of the elements are currently popped, but the number of popped elements is considered
    /// // an implementation detail that may be changed in the future.
    /// assert_eq!(q.steal_batch_with_limit_and_pop(&w, std::usize::MAX), Steal::Success(3));
    /// assert_eq!(w.pop(), Some(4));
    /// assert_eq!(w.pop(), Some(5));
    /// assert_eq!(w.pop(), None);
    /// ```
    pub fn steal_batch_with_limit_and_pop(&self, dest: &Worker<T>, limit: usize) -> Steal<T> {
        assert!(limit > 0);
        let mut head;
        let mut block;
        let mut offset;

        let backoff = Backoff::new();
        loop {
            head = self.head.index.load(Ordering::Acquire);
            block = self.head.block.load(Ordering::Acquire);

            // Calculate the offset of the index into the block.
            offset = (head >> SHIFT) % LAP;

            // If we reached the end of the block, wait until the next one is installed.
            if offset == BLOCK_CAP {
                backoff.snooze();
            } else {
                break;
            }
        }

        let mut new_head = head;
        let advance;

        if new_head & HAS_NEXT == 0 {
            atomic::fence(Ordering::SeqCst);
            let tail = self.tail.index.load(Ordering::Relaxed);

            // If the tail equals the head, that means the queue is empty.
            if head >> SHIFT == tail >> SHIFT {
                return Steal::Empty;
            }

            // If head and tail are not in the same block, set `HAS_NEXT` in head.
            if (head >> SHIFT) / LAP != (tail >> SHIFT) / LAP {
                new_head |= HAS_NEXT;
                // We can steal all tasks till the end of the block.
                advance = (BLOCK_CAP - offset).min(limit);
            } else {
                let len = (tail - head) >> SHIFT;
                // Steal half of the available tasks.
                advance = ((len + 1) / 2).min(limit);
            }
        } else {
            // We can steal all tasks till the end of the block.
            advance = (BLOCK_CAP - offset).min(limit);
        }

        new_head += advance << SHIFT;
        let new_offset = offset + advance;

        // Try moving the head index forward.
        if self
            .head
            .index
            .compare_exchange_weak(head, new_head, Ordering::SeqCst, Ordering::Acquire)
            .is_err()
        {
            return Steal::Retry;
        }

        // Reserve capacity for the stolen batch.
        let batch_size = new_offset - offset - 1;
        dest.reserve(batch_size);

        // Get the destination buffer and back index.
        let dest_buffer = dest.buffer.get();
        let dest_b = dest.inner.back.load(Ordering::Relaxed);

        unsafe {
            // If we've reached the end of the block, move to the next one.
            if new_offset == BLOCK_CAP {
                let next = (*block).wait_next();
                let mut next_index = (new_head & !HAS_NEXT).wrapping_add(1 << SHIFT);
                if !(*next).next.load(Ordering::Relaxed).is_null() {
                    next_index |= HAS_NEXT;
                }

                self.head.block.store(next, Ordering::Release);
                self.head.index.store(next_index, Ordering::Release);
            }

            // Read the task.
            let slot = (*block).slots.get_unchecked(offset);
            slot.wait_write();
            let task = slot.task.get().read();

            match dest.flavor {
                Flavor::Fifo => {
                    // Copy values from the injector into the destination queue.
                    for i in 0..batch_size {
                        // Read the task.
                        let slot = (*block).slots.get_unchecked(offset + i + 1);
                        slot.wait_write();
                        let task = slot.task.get().read();

                        // Write it into the destination queue.
                        dest_buffer.write(dest_b.wrapping_add(i as isize), task);
                    }
                }

                Flavor::Lifo => {
                    // Copy values from the injector into the destination queue.
                    for i in 0..batch_size {
                        // Read the task.
                        let slot = (*block).slots.get_unchecked(offset + i + 1);
                        slot.wait_write();
                        let task = slot.task.get().read();

                        // Write it into the destination queue.
                        dest_buffer.write(dest_b.wrapping_add((batch_size - 1 - i) as isize), task);
                    }
                }
            }

            atomic::fence(Ordering::Release);

            // Update the back index in the destination queue.
            //
            // This ordering could be `Relaxed`, but then thread sanitizer would falsely report
            // data races because it doesn't understand fences.
            dest.inner
                .back
                .store(dest_b.wrapping_add(batch_size as isize), Ordering::Release);

            // Destroy the block if we've reached the end, or if another thread wanted to destroy
            // but couldn't because we were busy reading from the slot.
            if new_offset == BLOCK_CAP {
                Block::destroy(block, offset);
            } else {
                for i in offset..new_offset {
                    let slot = (*block).slots.get_unchecked(i);

                    if slot.state.fetch_or(READ, Ordering::AcqRel) & DESTROY != 0 {
                        Block::destroy(block, offset);
                        break;
                    }
                }
            }

            Steal::Success(task.assume_init())
        }
    }

    /// Returns `true` if the queue is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Injector;
    ///
    /// let q = Injector::new();
    ///
    /// assert!(q.is_empty());
    /// q.push(1);
    /// assert!(!q.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        let head = self.head.index.load(Ordering::SeqCst);
        let tail = self.tail.index.load(Ordering::SeqCst);
        head >> SHIFT == tail >> SHIFT
    }

    /// Returns the number of tasks in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Injector;
    ///
    /// let q = Injector::new();
    ///
    /// assert_eq!(q.len(), 0);
    /// q.push(1);
    /// assert_eq!(q.len(), 1);
    /// q.push(1);
    /// assert_eq!(q.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        loop {
            // Load the tail index, then load the head index.
            let mut tail = self.tail.index.load(Ordering::SeqCst);
            let mut head = self.head.index.load(Ordering::SeqCst);

            // If the tail index didn't change, we've got consistent indices to work with.
            if self.tail.index.load(Ordering::SeqCst) == tail {
                // Erase the lower bits.
                tail &= !((1 << SHIFT) - 1);
                head &= !((1 << SHIFT) - 1);

                // Fix up indices if they fall onto block ends.
                if (tail >> SHIFT) & (LAP - 1) == LAP - 1 {
                    tail = tail.wrapping_add(1 << SHIFT);
                }
                if (head >> SHIFT) & (LAP - 1) == LAP - 1 {
                    head = head.wrapping_add(1 << SHIFT);
                }

                // Rotate indices so that head falls into the first block.
                let lap = (head >> SHIFT) / LAP;
                tail = tail.wrapping_sub((lap * LAP) << SHIFT);
                head = head.wrapping_sub((lap * LAP) << SHIFT);

                // Remove the lower bits.
                tail >>= SHIFT;
                head >>= SHIFT;

                // Return the difference minus the number of blocks between tail and head.
                return tail - head - tail / LAP;
            }
        }
    }
}

impl<T> Drop for Injector<T> {
    fn drop(&mut self) {
        let mut head = *self.head.index.get_mut();
        let mut tail = *self.tail.index.get_mut();
        let mut block = *self.head.block.get_mut();

        // Erase the lower bits.
        head &= !((1 << SHIFT) - 1);
        tail &= !((1 << SHIFT) - 1);

        unsafe {
            // Drop all values between `head` and `tail` and deallocate the heap-allocated blocks.
            while head != tail {
                let offset = (head >> SHIFT) % LAP;

                if offset < BLOCK_CAP {
                    // Drop the task in the slot.
                    let slot = (*block).slots.get_unchecked(offset);
                    (*slot.task.get()).assume_init_drop();
                } else {
                    // Deallocate the block and move to the next one.
                    let next = *(*block).next.get_mut();
                    drop(Box::from_raw(block));
                    block = next;
                }

                head = head.wrapping_add(1 << SHIFT);
            }

            // Deallocate the last remaining block.
            drop(Box::from_raw(block));
        }
    }
}

impl<T> fmt::Debug for Injector<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Worker { .. }")
    }
}

/// Possible outcomes of a steal operation.
///
/// # Examples
///
/// There are lots of ways to chain results of steal operations together:
///
/// ```
/// use crossbeam_deque::Steal::{self, Empty, Retry, Success};
///
/// let collect = |v: Vec<Steal<i32>>| v.into_iter().collect::<Steal<i32>>();
///
/// assert_eq!(collect(vec![Empty, Empty, Empty]), Empty);
/// assert_eq!(collect(vec![Empty, Retry, Empty]), Retry);
/// assert_eq!(collect(vec![Retry, Success(1), Empty]), Success(1));
///
/// assert_eq!(collect(vec![Empty, Empty]).or_else(|| Retry), Retry);
/// assert_eq!(collect(vec![Retry, Empty]).or_else(|| Success(1)), Success(1));
/// ```
#[must_use]
#[derive(PartialEq, Eq, Copy, Clone)]
pub enum Steal<T> {
    /// The queue was empty at the time of stealing.
    Empty,

    /// At least one task was successfully stolen.
    Success(T),

    /// The steal operation needs to be retried.
    Retry,
}

impl<T> Steal<T> {
    /// Returns `true` if the queue was empty at the time of stealing.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Steal::{Empty, Retry, Success};
    ///
    /// assert!(!Success(7).is_empty());
    /// assert!(!Retry::<i32>.is_empty());
    ///
    /// assert!(Empty::<i32>.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        match self {
            Steal::Empty => true,
            _ => false,
        }
    }

    /// Returns `true` if at least one task was stolen.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Steal::{Empty, Retry, Success};
    ///
    /// assert!(!Empty::<i32>.is_success());
    /// assert!(!Retry::<i32>.is_success());
    ///
    /// assert!(Success(7).is_success());
    /// ```
    pub fn is_success(&self) -> bool {
        match self {
            Steal::Success(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if the steal operation needs to be retried.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Steal::{Empty, Retry, Success};
    ///
    /// assert!(!Empty::<i32>.is_retry());
    /// assert!(!Success(7).is_retry());
    ///
    /// assert!(Retry::<i32>.is_retry());
    /// ```
    pub fn is_retry(&self) -> bool {
        match self {
            Steal::Retry => true,
            _ => false,
        }
    }

    /// Returns the result of the operation, if successful.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Steal::{Empty, Retry, Success};
    ///
    /// assert_eq!(Empty::<i32>.success(), None);
    /// assert_eq!(Retry::<i32>.success(), None);
    ///
    /// assert_eq!(Success(7).success(), Some(7));
    /// ```
    pub fn success(self) -> Option<T> {
        match self {
            Steal::Success(res) => Some(res),
            _ => None,
        }
    }

    /// If no task was stolen, attempts another steal operation.
    ///
    /// Returns this steal result if it is `Success`. Otherwise, closure `f` is invoked and then:
    ///
    /// * If the second steal resulted in `Success`, it is returned.
    /// * If both steals were unsuccessful but any resulted in `Retry`, then `Retry` is returned.
    /// * If both resulted in `None`, then `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_deque::Steal::{Empty, Retry, Success};
    ///
    /// assert_eq!(Success(1).or_else(|| Success(2)), Success(1));
    /// assert_eq!(Retry.or_else(|| Success(2)), Success(2));
    ///
    /// assert_eq!(Retry.or_else(|| Empty), Retry::<i32>);
    /// assert_eq!(Empty.or_else(|| Retry), Retry::<i32>);
    ///
    /// assert_eq!(Empty.or_else(|| Empty), Empty::<i32>);
    /// ```
    pub fn or_else<F>(self, f: F) -> Steal<T>
    where
        F: FnOnce() -> Steal<T>,
    {
        match self {
            Steal::Empty => f(),
            Steal::Success(_) => self,
            Steal::Retry => {
                if let Steal::Success(res) = f() {
                    Steal::Success(res)
                } else {
                    Steal::Retry
                }
            }
        }
    }
}

impl<T> fmt::Debug for Steal<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Steal::Empty => f.pad("Empty"),
            Steal::Success(_) => f.pad("Success(..)"),
            Steal::Retry => f.pad("Retry"),
        }
    }
}

impl<T> FromIterator<Steal<T>> for Steal<T> {
    /// Consumes items until a `Success` is found and returns it.
    ///
    /// If no `Success` was found, but there was at least one `Retry`, then returns `Retry`.
    /// Otherwise, `Empty` is returned.
    fn from_iter<I>(iter: I) -> Steal<T>
    where
        I: IntoIterator<Item = Steal<T>>,
    {
        let mut retry = false;
        for s in iter {
            match &s {
                Steal::Empty => {}
                Steal::Success(_) => return s,
                Steal::Retry => retry = true,
            }
        }

        if retry {
            Steal::Retry
        } else {
            Steal::Empty
        }
    }
}
