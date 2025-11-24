//! Coordinates idling workers

use crate::loom::sync::atomic::AtomicUsize;
use crate::runtime::scheduler::multi_thread::Shared;

use std::fmt;
use std::sync::atomic::Ordering::{self, SeqCst};

pub(super) struct Idle {
    /// Tracks both the number of searching workers and the number of unparked
    /// workers.
    ///
    /// Used as a fast-path to avoid acquiring the lock when needed.
    state: AtomicUsize,

    /// Total number of workers.
    num_workers: usize,
}

/// Data synchronized by the scheduler mutex
pub(super) struct Synced {
    /// Sleeping workers
    sleepers: Vec<usize>,
}

const UNPARK_SHIFT: usize = 16;
const UNPARK_MASK: usize = !SEARCH_MASK;
const SEARCH_MASK: usize = (1 << UNPARK_SHIFT) - 1;

#[derive(Copy, Clone)]
struct State(usize);

impl Idle {
    pub(super) fn new(num_workers: usize) -> (Idle, Synced) {
        let init = State::new(num_workers);

        let idle = Idle {
            state: AtomicUsize::new(init.into()),
            num_workers,
        };

        let synced = Synced {
            sleepers: Vec::with_capacity(num_workers),
        };

        (idle, synced)
    }

    /// If there are no workers actively searching, returns the index of a
    /// worker currently sleeping.
    pub(super) fn worker_to_notify(&self, shared: &Shared) -> Option<usize> {
        // If at least one worker is spinning, work being notified will
        // eventually be found. A searching thread will find **some** work and
        // notify another worker, eventually leading to our work being found.
        //
        // For this to happen, this load must happen before the thread
        // transitioning `num_searching` to zero. Acquire / Release does not
        // provide sufficient guarantees, so this load is done with `SeqCst` and
        // will pair with the `fetch_sub(1)` when transitioning out of
        // searching.
        if !self.notify_should_wakeup() {
            return None;
        }

        // Acquire the lock
        let mut lock = shared.synced.lock();

        // Check again, now that the lock is acquired
        if !self.notify_should_wakeup() {
            return None;
        }

        // A worker should be woken up, atomically increment the number of
        // searching workers as well as the number of unparked workers.
        State::unpark_one(&self.state, 1);

        // Get the worker to unpark
        let ret = lock.idle.sleepers.pop();
        debug_assert!(ret.is_some());

        ret
    }

    /// Returns `true` if the worker needs to do a final check for submitted
    /// work.
    pub(super) fn transition_worker_to_parked(
        &self,
        shared: &Shared,
        worker: usize,
        is_searching: bool,
    ) -> bool {
        // Acquire the lock
        let mut lock = shared.synced.lock();

        // Decrement the number of unparked threads
        let ret = State::dec_num_unparked(&self.state, is_searching);

        // Track the sleeping worker
        lock.idle.sleepers.push(worker);

        ret
    }

    pub(super) fn transition_worker_to_searching(&self) -> bool {
        let state = State::load(&self.state, SeqCst);
        if 2 * state.num_searching() >= self.num_workers {
            return false;
        }

        // It is possible for this routine to allow more than 50% of the workers
        // to search. That is OK. Limiting searchers is only an optimization to
        // prevent too much contention.
        State::inc_num_searching(&self.state, SeqCst);
        true
    }

    /// A lightweight transition from searching -> running.
    ///
    /// Returns `true` if this is the final searching worker. The caller
    /// **must** notify a new worker.
    pub(super) fn transition_worker_from_searching(&self) -> bool {
        State::dec_num_searching(&self.state)
    }

    /// Unpark a specific worker. This happens if tasks are submitted from
    /// within the worker's park routine.
    ///
    /// Returns `true` if the worker was parked before calling the method.
    pub(super) fn unpark_worker_by_id(&self, shared: &Shared, worker_id: usize) -> bool {
        let mut lock = shared.synced.lock();
        let sleepers = &mut lock.idle.sleepers;

        for index in 0..sleepers.len() {
            if sleepers[index] == worker_id {
                sleepers.swap_remove(index);

                // Update the state accordingly while the lock is held.
                State::unpark_one(&self.state, 0);

                return true;
            }
        }

        false
    }

    /// Returns `true` if `worker_id` is contained in the sleep set.
    pub(super) fn is_parked(&self, shared: &Shared, worker_id: usize) -> bool {
        let lock = shared.synced.lock();
        lock.idle.sleepers.contains(&worker_id)
    }

    fn notify_should_wakeup(&self) -> bool {
        let state = State(self.state.fetch_add(0, SeqCst));
        state.num_searching() == 0 && state.num_unparked() < self.num_workers
    }
}

impl State {
    fn new(num_workers: usize) -> State {
        // All workers start in the unparked state
        let ret = State(num_workers << UNPARK_SHIFT);
        debug_assert_eq!(num_workers, ret.num_unparked());
        debug_assert_eq!(0, ret.num_searching());
        ret
    }

    fn load(cell: &AtomicUsize, ordering: Ordering) -> State {
        State(cell.load(ordering))
    }

    fn unpark_one(cell: &AtomicUsize, num_searching: usize) {
        cell.fetch_add(num_searching | (1 << UNPARK_SHIFT), SeqCst);
    }

    fn inc_num_searching(cell: &AtomicUsize, ordering: Ordering) {
        cell.fetch_add(1, ordering);
    }

    /// Returns `true` if this is the final searching worker
    fn dec_num_searching(cell: &AtomicUsize) -> bool {
        let state = State(cell.fetch_sub(1, SeqCst));
        state.num_searching() == 1
    }

    /// Track a sleeping worker
    ///
    /// Returns `true` if this is the final searching worker.
    fn dec_num_unparked(cell: &AtomicUsize, is_searching: bool) -> bool {
        let mut dec = 1 << UNPARK_SHIFT;

        if is_searching {
            dec += 1;
        }

        let prev = State(cell.fetch_sub(dec, SeqCst));
        is_searching && prev.num_searching() == 1
    }

    /// Number of workers currently searching
    fn num_searching(self) -> usize {
        self.0 & SEARCH_MASK
    }

    /// Number of workers currently unparked
    fn num_unparked(self) -> usize {
        (self.0 & UNPARK_MASK) >> UNPARK_SHIFT
    }
}

impl From<usize> for State {
    fn from(src: usize) -> State {
        State(src)
    }
}

impl From<State> for usize {
    fn from(src: State) -> usize {
        src.0
    }
}

impl fmt::Debug for State {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("worker::State")
            .field("num_unparked", &self.num_unparked())
            .field("num_searching", &self.num_searching())
            .finish()
    }
}

#[test]
fn test_state() {
    assert_eq!(0, UNPARK_MASK & SEARCH_MASK);
    assert_eq!(0, !(UNPARK_MASK | SEARCH_MASK));

    let state = State::new(10);
    assert_eq!(10, state.num_unparked());
    assert_eq!(0, state.num_searching());
}
