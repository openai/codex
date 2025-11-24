//! The global data and participant for garbage collection.
//!
//! # Registration
//!
//! In order to track all participants in one place, we need some form of participant
//! registration. When a participant is created, it is registered to a global lock-free
//! singly-linked list of registries; and when a participant is leaving, it is unregistered from the
//! list.
//!
//! # Pinning
//!
//! Every participant contains an integer that tells whether the participant is pinned and if so,
//! what was the global epoch at the time it was pinned. Participants also hold a pin counter that
//! aids in periodic global epoch advancement.
//!
//! When a participant is pinned, a `Guard` is returned as a witness that the participant is pinned.
//! Guards are necessary for performing atomic operations, and for freeing/dropping locations.
//!
//! # Thread-local bag
//!
//! Objects that get unlinked from concurrent data structures must be stashed away until the global
//! epoch sufficiently advances so that they become safe for destruction. Pointers to such objects
//! are pushed into a thread-local bag, and when it becomes full, the bag is marked with the current
//! global epoch and pushed into the global queue of bags. We store objects in thread-local storages
//! for amortizing the synchronization cost of pushing the garbages to a global queue.
//!
//! # Global queue
//!
//! Whenever a bag is pushed into a queue, the objects in some bags in the queue are collected and
//! destroyed along the way. This design reduces contention on data structures. The global queue
//! cannot be explicitly accessed: the only way to interact with it is by calling functions
//! `defer()` that adds an object to the thread-local bag, or `collect()` that manually triggers
//! garbage collection.
//!
//! Ideally each instance of concurrent data structure may have its own queue that gets fully
//! destroyed as soon as the data structure gets dropped.

use crate::primitive::cell::UnsafeCell;
use crate::primitive::sync::atomic::{self, Ordering};
use core::cell::Cell;
use core::mem::{self, ManuallyDrop};
use core::num::Wrapping;
use core::{fmt, ptr};

use crossbeam_utils::CachePadded;

use crate::atomic::{Owned, Shared};
use crate::collector::{Collector, LocalHandle};
use crate::deferred::Deferred;
use crate::epoch::{AtomicEpoch, Epoch};
use crate::guard::{unprotected, Guard};
use crate::sync::list::{Entry, IsElement, IterError, List};
use crate::sync::queue::Queue;

/// Maximum number of objects a bag can contain.
#[cfg(not(any(crossbeam_sanitize, miri)))]
const MAX_OBJECTS: usize = 64;
// Makes it more likely to trigger any potential data races.
#[cfg(any(crossbeam_sanitize, miri))]
const MAX_OBJECTS: usize = 4;

/// A bag of deferred functions.
pub(crate) struct Bag {
    /// Stashed objects.
    deferreds: [Deferred; MAX_OBJECTS],
    len: usize,
}

/// `Bag::try_push()` requires that it is safe for another thread to execute the given functions.
unsafe impl Send for Bag {}

impl Bag {
    /// Returns a new, empty bag.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the bag is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Attempts to insert a deferred function into the bag.
    ///
    /// Returns `Ok(())` if successful, and `Err(deferred)` for the given `deferred` if the bag is
    /// full.
    ///
    /// # Safety
    ///
    /// It should be safe for another thread to execute the given function.
    pub(crate) unsafe fn try_push(&mut self, deferred: Deferred) -> Result<(), Deferred> {
        if self.len < MAX_OBJECTS {
            self.deferreds[self.len] = deferred;
            self.len += 1;
            Ok(())
        } else {
            Err(deferred)
        }
    }

    /// Seals the bag with the given epoch.
    fn seal(self, epoch: Epoch) -> SealedBag {
        SealedBag { epoch, _bag: self }
    }
}

impl Default for Bag {
    fn default() -> Self {
        Bag {
            len: 0,
            deferreds: [Deferred::NO_OP; MAX_OBJECTS],
        }
    }
}

impl Drop for Bag {
    fn drop(&mut self) {
        // Call all deferred functions.
        for deferred in &mut self.deferreds[..self.len] {
            let no_op = Deferred::NO_OP;
            let owned_deferred = mem::replace(deferred, no_op);
            owned_deferred.call();
        }
    }
}

// can't #[derive(Debug)] because Debug is not implemented for arrays 64 items long
impl fmt::Debug for Bag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bag")
            .field("deferreds", &&self.deferreds[..self.len])
            .finish()
    }
}

/// A pair of an epoch and a bag.
#[derive(Default, Debug)]
struct SealedBag {
    epoch: Epoch,
    _bag: Bag,
}

/// It is safe to share `SealedBag` because `is_expired` only inspects the epoch.
unsafe impl Sync for SealedBag {}

impl SealedBag {
    /// Checks if it is safe to drop the bag w.r.t. the given global epoch.
    fn is_expired(&self, global_epoch: Epoch) -> bool {
        // A pinned participant can witness at most one epoch advancement. Therefore, any bag that
        // is within one epoch of the current one cannot be destroyed yet.
        global_epoch.wrapping_sub(self.epoch) >= 2
    }
}

/// The global data for a garbage collector.
pub(crate) struct Global {
    /// The intrusive linked list of `Local`s.
    locals: List<Local>,

    /// The global queue of bags of deferred functions.
    queue: Queue<SealedBag>,

    /// The global epoch.
    pub(crate) epoch: CachePadded<AtomicEpoch>,
}

impl Global {
    /// Number of bags to destroy.
    const COLLECT_STEPS: usize = 8;

    /// Creates a new global data for garbage collection.
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            locals: List::new(),
            queue: Queue::new(),
            epoch: CachePadded::new(AtomicEpoch::new(Epoch::starting())),
        }
    }

    /// Pushes the bag into the global queue and replaces the bag with a new empty bag.
    pub(crate) fn push_bag(&self, bag: &mut Bag, guard: &Guard) {
        let bag = mem::replace(bag, Bag::new());

        atomic::fence(Ordering::SeqCst);

        let epoch = self.epoch.load(Ordering::Relaxed);
        self.queue.push(bag.seal(epoch), guard);
    }

    /// Collects several bags from the global queue and executes deferred functions in them.
    ///
    /// Note: This may itself produce garbage and in turn allocate new bags.
    ///
    /// `pin()` rarely calls `collect()`, so we want the compiler to place that call on a cold
    /// path. In other words, we want the compiler to optimize branching for the case when
    /// `collect()` is not called.
    #[cold]
    pub(crate) fn collect(&self, guard: &Guard) {
        let global_epoch = self.try_advance(guard);

        let steps = if cfg!(crossbeam_sanitize) {
            usize::max_value()
        } else {
            Self::COLLECT_STEPS
        };

        for _ in 0..steps {
            match self.queue.try_pop_if(
                &|sealed_bag: &SealedBag| sealed_bag.is_expired(global_epoch),
                guard,
            ) {
                None => break,
                Some(sealed_bag) => drop(sealed_bag),
            }
        }
    }

    /// Attempts to advance the global epoch.
    ///
    /// The global epoch can advance only if all currently pinned participants have been pinned in
    /// the current epoch.
    ///
    /// Returns the current global epoch.
    ///
    /// `try_advance()` is annotated `#[cold]` because it is rarely called.
    #[cold]
    pub(crate) fn try_advance(&self, guard: &Guard) -> Epoch {
        let global_epoch = self.epoch.load(Ordering::Relaxed);
        atomic::fence(Ordering::SeqCst);

        // TODO(stjepang): `Local`s are stored in a linked list because linked lists are fairly
        // easy to implement in a lock-free manner. However, traversal can be slow due to cache
        // misses and data dependencies. We should experiment with other data structures as well.
        for local in self.locals.iter(guard) {
            match local {
                Err(IterError::Stalled) => {
                    // A concurrent thread stalled this iteration. That thread might also try to
                    // advance the epoch, in which case we leave the job to it. Otherwise, the
                    // epoch will not be advanced.
                    return global_epoch;
                }
                Ok(local) => {
                    let local_epoch = local.epoch.load(Ordering::Relaxed);

                    // If the participant was pinned in a different epoch, we cannot advance the
                    // global epoch just yet.
                    if local_epoch.is_pinned() && local_epoch.unpinned() != global_epoch {
                        return global_epoch;
                    }
                }
            }
        }
        atomic::fence(Ordering::Acquire);

        // All pinned participants were pinned in the current global epoch.
        // Now let's advance the global epoch...
        //
        // Note that if another thread already advanced it before us, this store will simply
        // overwrite the global epoch with the same value. This is true because `try_advance` was
        // called from a thread that was pinned in `global_epoch`, and the global epoch cannot be
        // advanced two steps ahead of it.
        let new_epoch = global_epoch.successor();
        self.epoch.store(new_epoch, Ordering::Release);
        new_epoch
    }
}

/// Participant for garbage collection.
#[repr(C)] // Note: `entry` must be the first field
pub(crate) struct Local {
    /// A node in the intrusive linked list of `Local`s.
    entry: Entry,

    /// A reference to the global data.
    ///
    /// When all guards and handles get dropped, this reference is destroyed.
    collector: UnsafeCell<ManuallyDrop<Collector>>,

    /// The local bag of deferred functions.
    pub(crate) bag: UnsafeCell<Bag>,

    /// The number of guards keeping this participant pinned.
    guard_count: Cell<usize>,

    /// The number of active handles.
    handle_count: Cell<usize>,

    /// Total number of pinnings performed.
    ///
    /// This is just an auxiliary counter that sometimes kicks off collection.
    pin_count: Cell<Wrapping<usize>>,

    /// The local epoch.
    epoch: CachePadded<AtomicEpoch>,
}

// Make sure `Local` is less than or equal to 2048 bytes.
// https://github.com/crossbeam-rs/crossbeam/issues/551
#[cfg(not(any(crossbeam_sanitize, miri)))] // `crossbeam_sanitize` and `miri` reduce the size of `Local`
#[test]
fn local_size() {
    // TODO: https://github.com/crossbeam-rs/crossbeam/issues/869
    // assert!(
    //     core::mem::size_of::<Local>() <= 2048,
    //     "An allocation of `Local` should be <= 2048 bytes."
    // );
}

impl Local {
    /// Number of pinnings after which a participant will execute some deferred functions from the
    /// global queue.
    const PINNINGS_BETWEEN_COLLECT: usize = 128;

    /// Registers a new `Local` in the provided `Global`.
    pub(crate) fn register(collector: &Collector) -> LocalHandle {
        unsafe {
            // Since we dereference no pointers in this block, it is safe to use `unprotected`.

            let local = Owned::new(Local {
                entry: Entry::default(),
                collector: UnsafeCell::new(ManuallyDrop::new(collector.clone())),
                bag: UnsafeCell::new(Bag::new()),
                guard_count: Cell::new(0),
                handle_count: Cell::new(1),
                pin_count: Cell::new(Wrapping(0)),
                epoch: CachePadded::new(AtomicEpoch::new(Epoch::starting())),
            })
            .into_shared(unprotected());
            collector.global.locals.insert(local, unprotected());
            LocalHandle {
                local: local.as_raw(),
            }
        }
    }

    /// Returns a reference to the `Global` in which this `Local` resides.
    #[inline]
    pub(crate) fn global(&self) -> &Global {
        &self.collector().global
    }

    /// Returns a reference to the `Collector` in which this `Local` resides.
    #[inline]
    pub(crate) fn collector(&self) -> &Collector {
        self.collector.with(|c| unsafe { &**c })
    }

    /// Returns `true` if the current participant is pinned.
    #[inline]
    pub(crate) fn is_pinned(&self) -> bool {
        self.guard_count.get() > 0
    }

    /// Adds `deferred` to the thread-local bag.
    ///
    /// # Safety
    ///
    /// It should be safe for another thread to execute the given function.
    pub(crate) unsafe fn defer(&self, mut deferred: Deferred, guard: &Guard) {
        let bag = self.bag.with_mut(|b| &mut *b);

        while let Err(d) = bag.try_push(deferred) {
            self.global().push_bag(bag, guard);
            deferred = d;
        }
    }

    pub(crate) fn flush(&self, guard: &Guard) {
        let bag = self.bag.with_mut(|b| unsafe { &mut *b });

        if !bag.is_empty() {
            self.global().push_bag(bag, guard);
        }

        self.global().collect(guard);
    }

    /// Pins the `Local`.
    #[inline]
    pub(crate) fn pin(&self) -> Guard {
        let guard = Guard { local: self };

        let guard_count = self.guard_count.get();
        self.guard_count.set(guard_count.checked_add(1).unwrap());

        if guard_count == 0 {
            let global_epoch = self.global().epoch.load(Ordering::Relaxed);
            let new_epoch = global_epoch.pinned();

            // Now we must store `new_epoch` into `self.epoch` and execute a `SeqCst` fence.
            // The fence makes sure that any future loads from `Atomic`s will not happen before
            // this store.
            if cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                not(miri)
            )) {
                // HACK(stjepang): On x86 architectures there are two different ways of executing
                // a `SeqCst` fence.
                //
                // 1. `atomic::fence(SeqCst)`, which compiles into a `mfence` instruction.
                // 2. `_.compare_exchange(_, _, SeqCst, SeqCst)`, which compiles into a `lock cmpxchg`
                //    instruction.
                //
                // Both instructions have the effect of a full barrier, but benchmarks have shown
                // that the second one makes pinning faster in this particular case.  It is not
                // clear that this is permitted by the C++ memory model (SC fences work very
                // differently from SC accesses), but experimental evidence suggests that this
                // works fine.  Using inline assembly would be a viable (and correct) alternative,
                // but alas, that is not possible on stable Rust.
                let current = Epoch::starting();
                let res = self.epoch.compare_exchange(
                    current,
                    new_epoch,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
                debug_assert!(res.is_ok(), "participant was expected to be unpinned");
                // We add a compiler fence to make it less likely for LLVM to do something wrong
                // here.  Formally, this is not enough to get rid of data races; practically,
                // it should go a long way.
                atomic::compiler_fence(Ordering::SeqCst);
            } else {
                self.epoch.store(new_epoch, Ordering::Relaxed);
                atomic::fence(Ordering::SeqCst);
            }

            // Increment the pin counter.
            let count = self.pin_count.get();
            self.pin_count.set(count + Wrapping(1));

            // After every `PINNINGS_BETWEEN_COLLECT` try advancing the epoch and collecting
            // some garbage.
            if count.0 % Self::PINNINGS_BETWEEN_COLLECT == 0 {
                self.global().collect(&guard);
            }
        }

        guard
    }

    /// Unpins the `Local`.
    #[inline]
    pub(crate) fn unpin(&self) {
        let guard_count = self.guard_count.get();
        self.guard_count.set(guard_count - 1);

        if guard_count == 1 {
            self.epoch.store(Epoch::starting(), Ordering::Release);

            if self.handle_count.get() == 0 {
                self.finalize();
            }
        }
    }

    /// Unpins and then pins the `Local`.
    #[inline]
    pub(crate) fn repin(&self) {
        let guard_count = self.guard_count.get();

        // Update the local epoch only if there's only one guard.
        if guard_count == 1 {
            let epoch = self.epoch.load(Ordering::Relaxed);
            let global_epoch = self.global().epoch.load(Ordering::Relaxed).pinned();

            // Update the local epoch only if the global epoch is greater than the local epoch.
            if epoch != global_epoch {
                // We store the new epoch with `Release` because we need to ensure any memory
                // accesses from the previous epoch do not leak into the new one.
                self.epoch.store(global_epoch, Ordering::Release);

                // However, we don't need a following `SeqCst` fence, because it is safe for memory
                // accesses from the new epoch to be executed before updating the local epoch. At
                // worse, other threads will see the new epoch late and delay GC slightly.
            }
        }
    }

    /// Increments the handle count.
    #[inline]
    pub(crate) fn acquire_handle(&self) {
        let handle_count = self.handle_count.get();
        debug_assert!(handle_count >= 1);
        self.handle_count.set(handle_count + 1);
    }

    /// Decrements the handle count.
    #[inline]
    pub(crate) fn release_handle(&self) {
        let guard_count = self.guard_count.get();
        let handle_count = self.handle_count.get();
        debug_assert!(handle_count >= 1);
        self.handle_count.set(handle_count - 1);

        if guard_count == 0 && handle_count == 1 {
            self.finalize();
        }
    }

    /// Removes the `Local` from the global linked list.
    #[cold]
    fn finalize(&self) {
        debug_assert_eq!(self.guard_count.get(), 0);
        debug_assert_eq!(self.handle_count.get(), 0);

        // Temporarily increment handle count. This is required so that the following call to `pin`
        // doesn't call `finalize` again.
        self.handle_count.set(1);
        unsafe {
            // Pin and move the local bag into the global queue. It's important that `push_bag`
            // doesn't defer destruction on any new garbage.
            let guard = &self.pin();
            self.global()
                .push_bag(self.bag.with_mut(|b| &mut *b), guard);
        }
        // Revert the handle count back to zero.
        self.handle_count.set(0);

        unsafe {
            // Take the reference to the `Global` out of this `Local`. Since we're not protected
            // by a guard at this time, it's crucial that the reference is read before marking the
            // `Local` as deleted.
            let collector: Collector = ptr::read(self.collector.with(|c| &*(*c)));

            // Mark this node in the linked list as deleted.
            self.entry.delete(unprotected());

            // Finally, drop the reference to the global. Note that this might be the last reference
            // to the `Global`. If so, the global data will be destroyed and all deferred functions
            // in its queue will be executed.
            drop(collector);
        }
    }
}

impl IsElement<Self> for Local {
    fn entry_of(local: &Self) -> &Entry {
        // SAFETY: `Local` is `repr(C)` and `entry` is the first field of it.
        unsafe {
            let entry_ptr = (local as *const Self).cast::<Entry>();
            &*entry_ptr
        }
    }

    unsafe fn element_of(entry: &Entry) -> &Self {
        // SAFETY: `Local` is `repr(C)` and `entry` is the first field of it.
        let local_ptr = (entry as *const Entry).cast::<Self>();
        &*local_ptr
    }

    unsafe fn finalize(entry: &Entry, guard: &Guard) {
        guard.defer_destroy(Shared::from(Self::element_of(entry) as *const _));
    }
}

#[cfg(all(test, not(crossbeam_loom)))]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn check_defer() {
        static FLAG: AtomicUsize = AtomicUsize::new(0);
        fn set() {
            FLAG.store(42, Ordering::Relaxed);
        }

        let d = Deferred::new(set);
        assert_eq!(FLAG.load(Ordering::Relaxed), 0);
        d.call();
        assert_eq!(FLAG.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn check_bag() {
        static FLAG: AtomicUsize = AtomicUsize::new(0);
        fn incr() {
            FLAG.fetch_add(1, Ordering::Relaxed);
        }

        let mut bag = Bag::new();
        assert!(bag.is_empty());

        for _ in 0..MAX_OBJECTS {
            assert!(unsafe { bag.try_push(Deferred::new(incr)).is_ok() });
            assert!(!bag.is_empty());
            assert_eq!(FLAG.load(Ordering::Relaxed), 0);
        }

        let result = unsafe { bag.try_push(Deferred::new(incr)) };
        assert!(result.is_err());
        assert!(!bag.is_empty());
        assert_eq!(FLAG.load(Ordering::Relaxed), 0);

        drop(bag);
        assert_eq!(FLAG.load(Ordering::Relaxed), MAX_OBJECTS);
    }
}
