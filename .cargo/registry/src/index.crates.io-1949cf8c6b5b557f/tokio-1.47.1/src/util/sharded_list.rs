use std::ptr::NonNull;
use std::sync::atomic::Ordering;

use crate::loom::sync::{Mutex, MutexGuard};
use crate::util::metric_atomics::{MetricAtomicU64, MetricAtomicUsize};

use super::linked_list::{Link, LinkedList};

/// An intrusive linked list supporting highly concurrent updates.
///
/// It currently relies on `LinkedList`, so it is the caller's
/// responsibility to ensure the list is empty before dropping it.
///
/// Note: Due to its inner sharded design, the order of nodes cannot be guaranteed.
pub(crate) struct ShardedList<L, T> {
    lists: Box<[Mutex<LinkedList<L, T>>]>,
    added: MetricAtomicU64,
    count: MetricAtomicUsize,
    shard_mask: usize,
}

/// Determines which linked list an item should be stored in.
///
/// # Safety
///
/// Implementations must guarantee that the id of an item does not change from
/// call to call.
pub(crate) unsafe trait ShardedListItem: Link {
    /// # Safety
    /// The provided pointer must point at a valid list item.
    unsafe fn get_shard_id(target: NonNull<Self::Target>) -> usize;
}

impl<L, T> ShardedList<L, T> {
    /// Creates a new and empty sharded linked list with the specified size.
    pub(crate) fn new(sharded_size: usize) -> Self {
        assert!(sharded_size.is_power_of_two());

        let shard_mask = sharded_size - 1;
        let mut lists = Vec::with_capacity(sharded_size);
        for _ in 0..sharded_size {
            lists.push(Mutex::new(LinkedList::<L, T>::new()))
        }
        Self {
            lists: lists.into_boxed_slice(),
            added: MetricAtomicU64::new(0),
            count: MetricAtomicUsize::new(0),
            shard_mask,
        }
    }
}

/// Used to get the lock of shard.
pub(crate) struct ShardGuard<'a, L, T> {
    lock: MutexGuard<'a, LinkedList<L, T>>,
    added: &'a MetricAtomicU64,
    count: &'a MetricAtomicUsize,
    id: usize,
}

impl<L: ShardedListItem> ShardedList<L, L::Target> {
    /// Removes the last element from a list specified by `shard_id` and returns it, or None if it is
    /// empty.
    pub(crate) fn pop_back(&self, shard_id: usize) -> Option<L::Handle> {
        let mut lock = self.shard_inner(shard_id);
        let node = lock.pop_back();
        if node.is_some() {
            self.count.decrement();
        }
        node
    }

    /// Removes the specified node from the list.
    ///
    /// # Safety
    ///
    /// The caller **must** ensure that exactly one of the following is true:
    /// - `node` is currently contained by `self`,
    /// - `node` is not contained by any list,
    /// - `node` is currently contained by some other `GuardedLinkedList`.
    pub(crate) unsafe fn remove(&self, node: NonNull<L::Target>) -> Option<L::Handle> {
        let id = L::get_shard_id(node);
        let mut lock = self.shard_inner(id);
        // SAFETY: Since the shard id cannot change, it's not possible for this node
        // to be in any other list of the same sharded list.
        let node = unsafe { lock.remove(node) };
        if node.is_some() {
            self.count.decrement();
        }
        node
    }

    /// Gets the lock of `ShardedList`, makes us have the write permission.
    pub(crate) fn lock_shard(&self, val: &L::Handle) -> ShardGuard<'_, L, L::Target> {
        let id = unsafe { L::get_shard_id(L::as_raw(val)) };
        ShardGuard {
            lock: self.shard_inner(id),
            added: &self.added,
            count: &self.count,
            id,
        }
    }

    /// Gets the count of elements in this list.
    pub(crate) fn len(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    cfg_64bit_metrics! {
        /// Gets the total number of elements added to this list.
        pub(crate) fn added(&self) -> u64 {
            self.added.load(Ordering::Relaxed)
        }
    }

    /// Returns whether the linked list does not contain any node.
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Gets the shard size of this `SharedList`.
    ///
    /// Used to help us to decide the parameter `shard_id` of the `pop_back` method.
    pub(crate) fn shard_size(&self) -> usize {
        self.shard_mask + 1
    }

    #[inline]
    fn shard_inner(&self, id: usize) -> MutexGuard<'_, LinkedList<L, <L as Link>::Target>> {
        // Safety: This modulo operation ensures that the index is not out of bounds.
        unsafe { self.lists.get_unchecked(id & self.shard_mask).lock() }
    }
}

impl<'a, L: ShardedListItem> ShardGuard<'a, L, L::Target> {
    /// Push a value to this shard.
    pub(crate) fn push(mut self, val: L::Handle) {
        let id = unsafe { L::get_shard_id(L::as_raw(&val)) };
        assert_eq!(id, self.id);
        self.lock.push_front(val);
        self.added.add(1, Ordering::Relaxed);
        self.count.increment();
    }
}

cfg_taskdump! {
    impl<L: ShardedListItem> ShardedList<L, L::Target> {
        pub(crate) fn for_each<F>(&self, mut f: F)
        where
            F: FnMut(&L::Handle),
        {
            let mut guards = Vec::with_capacity(self.lists.len());
            for list in self.lists.iter() {
                guards.push(list.lock());
            }
            for g in &mut guards {
                g.for_each(&mut f);
            }
        }
    }
}
