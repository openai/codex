//! The half-lock structure
//!
//! We need a way to protect the structure with configured hooks ‒ a signal may happen in arbitrary
//! thread and needs to read them while another thread might be manipulating the structure.
//!
//! Under ordinary circumstances we would be happy to just use `Mutex<HashMap<c_int, _>>`. However,
//! as we use it in the signal handler, we are severely limited in what we can or can't use. So we
//! choose to implement kind of spin-look thing with atomics.
//!
//! In the reader it is always simply locked and then unlocked, making sure it doesn't disappear
//! while in use.
//!
//! The writer has a separate mutex (that prevents other writers; this is used outside of the
//! signal handler), makes a copy of the data and swaps an atomic pointer to the data structure.
//! But it waits until everything is unlocked (no signal handler has the old data) for dropping the
//! old instance. There's a generation trick to make sure that new signal locks another instance.
//!
//! The downside is, this is an active spin lock at the writer end. However, we assume than:
//!
//! * Signals are one time setup before we actually have threads. We just need to make *sure* we
//!   are safe even if this is not true.
//! * Signals are rare, happening at the same time as the write even rarer.
//! * Signals are short, as there is mostly nothing allowed inside them anyway.
//! * Our tool box is severely limited.
//!
//! Therefore this is hopefully reasonable trade-off.
//!
//! # Atomic orderings
//!
//! The whole code uses SeqCst conservatively. Atomics are not used because of performance here and
//! are the minor price around signals anyway. But the comments state which orderings should be
//! enough in practice in case someone wants to get inspired (but do make your own check through
//! them anyway).

use std::isize;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{self, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;

use libc;

const YIELD_EVERY: usize = 16;
const MAX_GUARDS: usize = (isize::MAX) as usize;

pub(crate) struct ReadGuard<'a, T: 'a> {
    data: &'a T,
    lock: &'a AtomicUsize,
}

impl<'a, T> Deref for ReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        // We effectively unlock; Release would be enough.
        self.lock.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) struct WriteGuard<'a, T: 'a> {
    _guard: MutexGuard<'a, ()>,
    lock: &'a HalfLock<T>,
    data: &'a T,
}

impl<'a, T> WriteGuard<'a, T> {
    pub(crate) fn store(&mut self, val: T) {
        // Move to the heap and convert to raw pointer for AtomicPtr.
        let new = Box::into_raw(Box::new(val));

        self.data = unsafe { &*new };

        // We can just put the new value in here safely, we worry only about dropping the old one.
        // Release might (?) be enough, to "upload" the data.
        let old = self.lock.data.swap(new, Ordering::SeqCst);

        // Now we make sure there's no reader having the old data.
        self.lock.write_barrier();

        drop(unsafe { Box::from_raw(old) });
    }
}

impl<'a, T> Deref for WriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // Protected by that mutex
        self.data
    }
}

pub(crate) struct HalfLock<T> {
    // We conceptually contain an instance of T
    _t: PhantomData<T>,
    // The actual data as a pointer.
    data: AtomicPtr<T>,
    // The generation of the data. Influences which slot of the lock counter we use.
    generation: AtomicUsize,
    // How many active locks are there?
    lock: [AtomicUsize; 2],
    // Mutex for the writers; only one writer.
    write_mutex: Mutex<()>,
}

impl<T> HalfLock<T> {
    pub(crate) fn new(data: T) -> Self {
        // Move to the heap so we can safely point there. Then convert to raw pointer as AtomicPtr
        // operates on raw pointers. The AtomicPtr effectively acts like Box for us semantically.
        let ptr = Box::into_raw(Box::new(data));
        Self {
            _t: PhantomData,
            data: AtomicPtr::new(ptr),
            generation: AtomicUsize::new(0),
            lock: [AtomicUsize::new(0), AtomicUsize::new(0)],
            write_mutex: Mutex::new(()),
        }
    }

    pub(crate) fn read(&self) -> ReadGuard<T> {
        // Relaxed should be enough; we only pick one or the other slot and the writer observes
        // that both were 0 at some time. So the actual value doesn't really matter for safety,
        // only the changing improves the performance.
        let gen = self.generation.load(Ordering::SeqCst);
        let lock = &self.lock[gen % 2];
        // Effectively locking something, acquire should be enough.
        let guard_cnt = lock.fetch_add(1, Ordering::SeqCst);

        // This is to prevent overflowing the counter in some degenerate cases, which could lead to
        // UB (freeing data while still in use). However, as this data structure is used only
        // internally and it's not possible to leak the guard and the guard itself takes some
        // memory, it should be really impossible to trigger this case. Still, we include it from
        // abundance of caution.
        //
        // This technically is not fully correct as enough threads being in between here and the
        // abort below could still overflow it and it could get freed for some *other* thread, but
        // that would mean having too many active threads to fit into RAM too and is even more
        // absurd corner case than the above.
        if guard_cnt > MAX_GUARDS {
            unsafe { libc::abort() };
        }

        // Acquire should be enough; we need to "download" the data, paired with the swap on the
        // same pointer.
        let data = self.data.load(Ordering::SeqCst);
        // Safe:
        // * It did point to valid data when put in.
        // * Protected by lock, so still valid.
        let data = unsafe { &*data };

        ReadGuard { data, lock }
    }

    fn update_seen(&self, seen_zero: &mut [bool; 2]) {
        for (seen, slot) in seen_zero.iter_mut().zip(&self.lock) {
            *seen = *seen || slot.load(Ordering::SeqCst) == 0;
        }
    }

    fn write_barrier(&self) {
        // Do a first check of seeing zeroes before we switch the generation. At least one of them
        // should be zero by now, due to having drained the generation before leaving the previous
        // writer.
        let mut seen_zero = [false; 2];
        self.update_seen(&mut seen_zero);
        // By switching the generation to the other slot, we make sure the currently active starts
        // draining while the other will start filling up.
        self.generation.fetch_add(1, Ordering::SeqCst); // Overflow is fine.

        let mut iter = 0usize;
        while !seen_zero.iter().all(|s| *s) {
            iter = iter.wrapping_add(1);

            // Be somewhat less aggressive while looping, switch to the other threads if possible.
            if cfg!(not(miri)) {
                if iter % YIELD_EVERY == 0 {
                    thread::yield_now();
                } else {
                    // Replaced by hint::spin_loop, but we want to support older compiler
                    #[allow(deprecated)]
                    atomic::spin_loop_hint();
                }
            }

            self.update_seen(&mut seen_zero);
        }
    }

    pub(crate) fn write(&self) -> WriteGuard<T> {
        // While it's possible the user code panics, our code in store doesn't and the data gets
        // swapped atomically. So if it panics, nothing gets changed, therefore poisons are of no
        // interest here.
        let guard = self
            .write_mutex
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        // Relaxed should be enough, as we are under the same mutex that was used to get the data
        // in.
        let data = self.data.load(Ordering::SeqCst);
        // Safe:
        // * Stored as valid data
        // * Only this method, protected by mutex, can change the pointer, so it didn't go away.
        let data = unsafe { &*data };

        WriteGuard {
            data,
            _guard: guard,
            lock: self,
        }
    }
}

impl<T> Drop for HalfLock<T> {
    fn drop(&mut self) {
        // During drop we are sure there are no other borrows of the data so we are free to just
        // drop it. Also, the drop impl won't be called in practice in our case, as it is used
        // solely as a global variable, but we provide it for completeness and tests anyway.
        //
        // unsafe: the pointer in there is always valid, we just take the last instance out.
        unsafe {
            // Acquire should be enough.
            let data = Box::from_raw(self.data.load(Ordering::SeqCst));
            drop(data);
        }
    }
}
