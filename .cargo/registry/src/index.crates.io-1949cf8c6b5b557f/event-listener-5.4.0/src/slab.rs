//! Implementation of `event-listener` built exclusively on atomics.
//!
//! On `no_std`, we don't have access to `Mutex`, so we can't use intrusive linked lists like the `std`
//! implementation. Normally, we would use a concurrent atomic queue to store listeners, but benchmarks
//! show that using queues in this way is very slow, especially for the single threaded use-case.
//!
//! We've found that it's easier to assume that the `Event` won't be under high contention in most use
//! cases. Therefore, we use a spinlock that protects a linked list of listeners, and fall back to an
//! atomic queue if the lock is contended. Benchmarks show that this is about 20% slower than the std
//! implementation, but still much faster than using a queue.

#[path = "slab/node.rs"]
mod node;

use node::{Node, NothingProducer, TaskWaiting};

use crate::notify::{GenericNotify, Internal, Notification};
use crate::sync::atomic::{AtomicBool, Ordering};
use crate::sync::cell::{Cell, ConstPtr, UnsafeCell};
use crate::sync::Arc;
use crate::{RegisterResult, State, Task, TaskRef};

use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroUsize;
use core::ops;
use core::pin::Pin;

use alloc::vec::Vec;

impl<T> crate::Inner<T> {
    /// Locks the list.
    fn try_lock(&self) -> Option<ListGuard<'_, T>> {
        self.list.inner.try_lock().map(|guard| ListGuard {
            inner: self,
            guard: Some(guard),
            tasks: alloc::vec![],
        })
    }

    /// Force a queue update.
    fn queue_update(&self) {
        // Locking and unlocking the mutex will drain the queue if there is no contention.
        drop(self.try_lock());
    }

    /// Add a new listener to the list.
    ///
    /// Does nothing if the list is already registered.
    pub(crate) fn insert(&self, mut listener: Pin<&mut Option<Listener<T>>>) {
        if listener.as_ref().as_pin_ref().is_some() {
            // Already inserted.
            return;
        }

        match self.try_lock() {
            Some(mut lock) => {
                let key = lock.insert(State::Created);
                *listener = Some(Listener::HasNode(key));
            }

            None => {
                // Push it to the queue.
                let (node, task_waiting) = Node::listener();
                self.list.queue.push(node).unwrap();
                *listener = Some(Listener::Queued(task_waiting));

                // Force a queue update.
                self.queue_update();
            }
        }
    }

    /// Remove a listener from the list.
    pub(crate) fn remove(
        &self,
        mut listener: Pin<&mut Option<Listener<T>>>,
        propagate: bool,
    ) -> Option<State<T>> {
        loop {
            let state = match listener.as_mut().take() {
                Some(Listener::HasNode(key)) => {
                    match self.try_lock() {
                        Some(mut list) => {
                            // Fast path removal.
                            list.remove(key, propagate)
                        }

                        None => {
                            // Slow path removal.
                            // This is why intrusive lists don't work on no_std.
                            let node = Node::RemoveListener {
                                listener: key,
                                propagate,
                            };

                            self.list.queue.push(node).unwrap();

                            // Force a queue update.
                            self.queue_update();

                            None
                        }
                    }
                }

                Some(Listener::Queued(tw)) => {
                    // Make sure it's not added after the queue is drained.
                    if let Some(key) = tw.cancel() {
                        // If it was already added, set up our listener and try again.
                        *listener = Some(Listener::HasNode(key));
                        continue;
                    }

                    None
                }

                None => None,

                _ => unreachable!(),
            };

            return state;
        }
    }

    /// Notifies a number of entries.
    #[cold]
    pub(crate) fn notify(&self, notify: impl Notification<Tag = T>) -> usize {
        match self.try_lock() {
            Some(mut guard) => {
                // Notify the listeners.
                guard.notify(notify)
            }

            None => {
                // Push it to the queue.
                let node = Node::Notify(GenericNotify::new(
                    notify.count(Internal::new()),
                    notify.is_additional(Internal::new()),
                    NothingProducer::default(),
                ));

                self.list.queue.push(node).unwrap();

                // Force a queue update.
                self.queue_update();

                // We haven't notified anyone yet.
                0
            }
        }
    }

    /// Register a task to be notified when the event is triggered.
    ///
    /// Returns `true` if the listener was already notified, and `false` otherwise. If the listener
    /// isn't inserted, returns `None`.
    pub(crate) fn register(
        &self,
        mut listener: Pin<&mut Option<Listener<T>>>,
        task: TaskRef<'_>,
    ) -> RegisterResult<T> {
        loop {
            match listener.as_mut().take() {
                Some(Listener::HasNode(key)) => {
                    *listener = Some(Listener::HasNode(key));
                    match self.try_lock() {
                        Some(mut guard) => {
                            // Fast path registration.
                            return guard.register(listener, task);
                        }

                        None => {
                            // Wait for the lock.
                            let node = Node::Waiting(task.into_task());
                            self.list.queue.push(node).unwrap();

                            // Force a queue update.
                            self.queue_update();

                            return RegisterResult::Registered;
                        }
                    }
                }

                Some(Listener::Queued(task_waiting)) => {
                    // Force a queue update.
                    self.queue_update();

                    // Are we done yet?
                    match task_waiting.status() {
                        Some(key) => {
                            assert!(key.get() != usize::MAX);

                            // We're inserted now, adjust state.
                            *listener = Some(Listener::HasNode(key));
                        }

                        None => {
                            // We're still queued, so register the task.
                            task_waiting.register(task.into_task());
                            *listener = Some(Listener::Queued(task_waiting));

                            // Force a queue update.
                            self.queue_update();

                            return RegisterResult::Registered;
                        }
                    }
                }

                None => return RegisterResult::NeverInserted,

                _ => unreachable!(),
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct List<T> {
    /// The inner list.
    inner: Mutex<ListenerSlab<T>>,

    /// The queue of pending operations.
    queue: concurrent_queue::ConcurrentQueue<Node<T>>,
}

impl<T> List<T> {
    pub(super) fn new() -> List<T> {
        List {
            inner: Mutex::new(ListenerSlab::new()),
            queue: concurrent_queue::ConcurrentQueue::unbounded(),
        }
    }

    /// Try to get the total number of listeners without blocking.
    pub(super) fn try_total_listeners(&self) -> Option<usize> {
        self.inner.try_lock().map(|lock| lock.listeners.len())
    }
}

/// The guard returned by [`Inner::lock`].
pub(crate) struct ListGuard<'a, T> {
    /// Reference to the inner state.
    pub(crate) inner: &'a crate::Inner<T>,

    /// The locked list.
    pub(crate) guard: Option<MutexGuard<'a, ListenerSlab<T>>>,

    /// Tasks to wake up once this guard is dropped.
    tasks: Vec<Task>,
}

impl<T> ListGuard<'_, T> {
    #[cold]
    fn process_nodes_slow(&mut self, start_node: Node<T>) {
        let guard = self.guard.as_mut().unwrap();

        // Process the start node.
        self.tasks.extend(start_node.apply(guard));

        // Process all remaining nodes.
        while let Ok(node) = self.inner.list.queue.pop() {
            self.tasks.extend(node.apply(guard));
        }
    }

    #[inline]
    fn process_nodes(&mut self) {
        // Process every node left in the queue.
        if let Ok(start_node) = self.inner.list.queue.pop() {
            self.process_nodes_slow(start_node);
        }
    }
}

impl<T> ops::Deref for ListGuard<'_, T> {
    type Target = ListenerSlab<T>;

    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().unwrap()
    }
}

impl<T> ops::DerefMut for ListGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().unwrap()
    }
}

impl<T> Drop for ListGuard<'_, T> {
    fn drop(&mut self) {
        while self.guard.is_some() {
            // Process every node left in the queue.
            self.process_nodes();

            // Update the atomic `notified` counter.
            let list = self.guard.take().unwrap();
            let notified = if list.notified < list.len {
                list.notified
            } else {
                usize::MAX
            };

            self.inner.notified.store(notified, Ordering::Release);

            // Drop the actual lock.
            drop(list);

            // Wakeup all tasks.
            for task in self.tasks.drain(..) {
                task.wake();
            }

            // There is a deadlock where a node is pushed to the end of the queue after we've finished
            // process_nodes() but before we've finished dropping the lock. This can lead to some
            // notifications not being properly delivered, or listeners not being added to the list.
            // Therefore check before we finish dropping if there is anything left in the queue, and
            // if so, lock it again and force a queue update.
            if !self.inner.list.queue.is_empty() {
                self.guard = self.inner.list.inner.try_lock();
            }
        }
    }
}

/// An entry representing a registered listener.
enum Entry<T> {
    /// Contains the listener state.
    Listener {
        /// The state of the listener.
        state: Cell<State<T>>,

        /// The previous listener in the list.
        prev: Cell<Option<NonZeroUsize>>,

        /// The next listener in the list.
        next: Cell<Option<NonZeroUsize>>,
    },

    /// An empty slot that contains the index of the next empty slot.
    Empty(NonZeroUsize),

    /// Sentinel value.
    Sentinel,
}

struct TakenState<'a, T> {
    slot: &'a Cell<State<T>>,
    state: State<T>,
}

impl<T> Drop for TakenState<'_, T> {
    fn drop(&mut self) {
        self.slot
            .set(mem::replace(&mut self.state, State::NotifiedTaken));
    }
}

impl<T> fmt::Debug for TakenState<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.state, f)
    }
}

impl<T: PartialEq> PartialEq for TakenState<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

impl<'a, T> TakenState<'a, T> {
    fn new(slot: &'a Cell<State<T>>) -> Self {
        let state = slot.replace(State::NotifiedTaken);
        Self { slot, state }
    }
}

impl<T> fmt::Debug for Entry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Listener { state, next, prev } => f
                .debug_struct("Listener")
                .field("state", &TakenState::new(state))
                .field("prev", prev)
                .field("next", next)
                .finish(),
            Entry::Empty(next) => f.debug_tuple("Empty").field(next).finish(),
            Entry::Sentinel => f.debug_tuple("Sentinel").finish(),
        }
    }
}

impl<T: PartialEq> PartialEq for Entry<T> {
    fn eq(&self, other: &Entry<T>) -> bool {
        match (self, other) {
            (
                Self::Listener {
                    state: state1,
                    prev: prev1,
                    next: next1,
                },
                Self::Listener {
                    state: state2,
                    prev: prev2,
                    next: next2,
                },
            ) => {
                if TakenState::new(state1) != TakenState::new(state2) {
                    return false;
                }

                prev1.get() == prev2.get() && next1.get() == next2.get()
            }
            (Self::Empty(next1), Self::Empty(next2)) => next1 == next2,
            (Self::Sentinel, Self::Sentinel) => true,
            _ => false,
        }
    }
}

impl<T> Entry<T> {
    fn state(&self) -> &Cell<State<T>> {
        match self {
            Entry::Listener { state, .. } => state,
            _ => unreachable!(),
        }
    }

    fn prev(&self) -> &Cell<Option<NonZeroUsize>> {
        match self {
            Entry::Listener { prev, .. } => prev,
            _ => unreachable!(),
        }
    }

    fn next(&self) -> &Cell<Option<NonZeroUsize>> {
        match self {
            Entry::Listener { next, .. } => next,
            _ => unreachable!(),
        }
    }
}

/// A linked list of entries.
pub(crate) struct ListenerSlab<T> {
    /// The raw list of entries.
    listeners: Vec<Entry<T>>,

    /// First entry in the list.
    head: Option<NonZeroUsize>,

    /// Last entry in the list.
    tail: Option<NonZeroUsize>,

    /// The first unnotified entry in the list.
    start: Option<NonZeroUsize>,

    /// The number of notified entries in the list.
    notified: usize,

    /// The total number of listeners.
    len: usize,

    /// The index of the first `Empty` entry, or the length of the list plus one if there
    /// are no empty entries.
    first_empty: NonZeroUsize,
}

impl<T> fmt::Debug for ListenerSlab<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListenerSlab")
            .field("listeners", &self.listeners)
            .field("head", &self.head)
            .field("tail", &self.tail)
            .field("start", &self.start)
            .field("notified", &self.notified)
            .field("len", &self.len)
            .field("first_empty", &self.first_empty)
            .finish()
    }
}

impl<T> ListenerSlab<T> {
    /// Create a new, empty list.
    pub(crate) fn new() -> Self {
        Self {
            listeners: alloc::vec![Entry::Sentinel],
            head: None,
            tail: None,
            start: None,
            notified: 0,
            len: 0,
            first_empty: unsafe { NonZeroUsize::new_unchecked(1) },
        }
    }

    /// Inserts a new entry into the list.
    pub(crate) fn insert(&mut self, state: State<T>) -> NonZeroUsize {
        // Add the new entry into the list.
        let key = {
            let entry = Entry::Listener {
                state: Cell::new(state),
                prev: Cell::new(self.tail),
                next: Cell::new(None),
            };

            let key = self.first_empty;
            if self.first_empty.get() == self.listeners.len() {
                // No empty entries, so add a new entry.
                self.listeners.push(entry);

                // SAFETY: Guaranteed to not overflow, since the Vec would have panicked already.
                self.first_empty = unsafe { NonZeroUsize::new_unchecked(self.listeners.len()) };
            } else {
                // There is an empty entry, so replace it.
                let slot = &mut self.listeners[key.get()];
                let next = match mem::replace(slot, entry) {
                    Entry::Empty(next) => next,
                    _ => unreachable!(),
                };

                self.first_empty = next;
            }

            key
        };

        // Replace the tail with the new entry.
        match mem::replace(&mut self.tail, Some(key)) {
            None => self.head = Some(key),
            Some(tail) => {
                let tail = &self.listeners[tail.get()];
                tail.next().set(Some(key));
            }
        }

        // If there are no listeners that have been notified, then the new listener is the next
        // listener to be notified.
        if self.start.is_none() {
            self.start = Some(key);
        }

        // Increment the length.
        self.len += 1;

        key
    }

    /// Removes an entry from the list and returns its state.
    pub(crate) fn remove(&mut self, key: NonZeroUsize, propagate: bool) -> Option<State<T>> {
        let entry = &self.listeners[key.get()];
        let prev = entry.prev().get();
        let next = entry.next().get();

        // Unlink from the previous entry.
        match prev {
            None => self.head = next,
            Some(p) => self.listeners[p.get()].next().set(next),
        }

        // Unlink from the next entry.
        match next {
            None => self.tail = prev,
            Some(n) => self.listeners[n.get()].prev().set(prev),
        }

        // If this was the first unnotified entry, move the pointer to the next one.
        if self.start == Some(key) {
            self.start = next;
        }

        // Extract the state.
        let entry = mem::replace(
            &mut self.listeners[key.get()],
            Entry::Empty(self.first_empty),
        );
        self.first_empty = key;

        let mut state = match entry {
            Entry::Listener { state, .. } => state.into_inner(),
            _ => unreachable!(),
        };

        // Update the counters.
        if state.is_notified() {
            self.notified = self.notified.saturating_sub(1);

            if propagate {
                // Propagate the notification to the next entry.
                let state = mem::replace(&mut state, State::NotifiedTaken);
                if let State::Notified { tag, additional } = state {
                    let tags = {
                        let mut tag = Some(tag);
                        move || tag.take().expect("called more than once")
                    };

                    self.notify(GenericNotify::new(1, additional, tags));
                }
            }
        }
        self.len -= 1;

        Some(state)
    }

    /// Notifies a number of listeners.
    #[cold]
    pub(crate) fn notify(&mut self, mut notify: impl Notification<Tag = T>) -> usize {
        let mut n = notify.count(Internal::new());
        let is_additional = notify.is_additional(Internal::new());
        if !is_additional {
            // Make sure we're not notifying more than we have.
            if n <= self.notified {
                return 0;
            }
            n -= self.notified;
        }

        let original_count = n;
        while n > 0 {
            n -= 1;

            // Notify the next entry.
            match self.start {
                None => return original_count - n - 1,

                Some(e) => {
                    // Get the entry and move the pointer forwards.
                    let entry = &self.listeners[e.get()];
                    self.start = entry.next().get();

                    // Set the state to `Notified` and notify.
                    let tag = notify.next_tag(Internal::new());
                    if let State::Task(task) = entry.state().replace(State::Notified {
                        tag,
                        additional: is_additional,
                    }) {
                        task.wake();
                    }

                    // Bump the notified count.
                    self.notified += 1;
                }
            }
        }

        original_count - n
    }

    /// Register a task to be notified when the event is triggered.
    ///
    /// Returns `true` if the listener was already notified, and `false` otherwise. If the listener
    /// isn't inserted, returns `None`.
    pub(crate) fn register(
        &mut self,
        mut listener: Pin<&mut Option<Listener<T>>>,
        task: TaskRef<'_>,
    ) -> RegisterResult<T> {
        let key = match *listener {
            Some(Listener::HasNode(key)) => key,
            _ => return RegisterResult::NeverInserted,
        };

        let entry = &self.listeners[key.get()];

        // Take the state out and check it.
        match entry.state().replace(State::NotifiedTaken) {
            State::Notified { tag, .. } => {
                // The listener was already notified, so we don't need to do anything.
                self.remove(key, false);
                *listener = None;
                RegisterResult::Notified(tag)
            }

            State::Task(other_task) => {
                // Only replace the task if it's not the same as the one we're registering.
                if task.will_wake(other_task.as_task_ref()) {
                    entry.state().set(State::Task(other_task));
                } else {
                    entry.state().set(State::Task(task.into_task()));
                }

                RegisterResult::Registered
            }

            _ => {
                // Register the task.
                entry.state().set(State::Task(task.into_task()));
                RegisterResult::Registered
            }
        }
    }
}

pub(crate) enum Listener<T> {
    /// The listener has a node inside of the linked list.
    HasNode(NonZeroUsize),

    /// The listener has an entry in the queue that may or may not have a task waiting.
    Queued(Arc<TaskWaiting>),

    /// Eat the generic type for consistency.
    _EatGenericType(PhantomData<T>),
}

impl<T> fmt::Debug for Listener<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HasNode(key) => f.debug_tuple("HasNode").field(key).finish(),
            Self::Queued(tw) => f.debug_tuple("Queued").field(tw).finish(),
            Self::_EatGenericType(_) => unreachable!(),
        }
    }
}

impl<T> Unpin for Listener<T> {}

impl<T> PartialEq for Listener<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::HasNode(a), Self::HasNode(b)) => a == b,
            (Self::Queued(a), Self::Queued(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// A simple mutex type that optimistically assumes that the lock is uncontended.
pub(crate) struct Mutex<T> {
    /// The inner value.
    value: UnsafeCell<T>,

    /// Whether the mutex is locked.
    locked: AtomicBool,
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(lock) = self.try_lock() {
            f.debug_tuple("Mutex").field(&*lock).finish()
        } else {
            f.write_str("Mutex { <locked> }")
        }
    }
}

impl<T> Mutex<T> {
    /// Create a new mutex.
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            locked: AtomicBool::new(false),
        }
    }

    /// Lock the mutex.
    pub(crate) fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        // Try to lock the mutex.
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // We have successfully locked the mutex.
            Some(MutexGuard {
                mutex: self,
                guard: self.value.get(),
            })
        } else {
            self.try_lock_slow()
        }
    }

    #[cold]
    fn try_lock_slow(&self) -> Option<MutexGuard<'_, T>> {
        // Assume that the contention is short-term.
        // Spin for a while to see if the mutex becomes unlocked.
        let mut spins = 100u32;

        loop {
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // We have successfully locked the mutex.
                return Some(MutexGuard {
                    mutex: self,
                    guard: self.value.get(),
                });
            }

            // Use atomic loads instead of compare-exchange.
            while self.locked.load(Ordering::Relaxed) {
                // Return None once we've exhausted the number of spins.
                spins = spins.checked_sub(1)?;
            }
        }
    }
}

pub(crate) struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    guard: ConstPtr<T>,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}

impl<'a, T> ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.guard.deref() }
    }
}

impl<'a, T> ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.guard.deref_mut() }
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_family = "wasm")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn smoke_mutex() {
        let mutex = Mutex::new(0);

        {
            let mut guard = mutex.try_lock().unwrap();
            *guard += 1;
        }

        {
            let mut guard = mutex.try_lock().unwrap();
            *guard += 1;
        }

        let guard = mutex.try_lock().unwrap();
        assert_eq!(*guard, 2);
    }

    #[test]
    fn smoke_listener_slab() {
        let mut listeners = ListenerSlab::<()>::new();

        // Insert a few listeners.
        let key1 = listeners.insert(State::Created);
        let key2 = listeners.insert(State::Created);
        let key3 = listeners.insert(State::Created);

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 0);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key1));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Remove one.
        assert_eq!(listeners.remove(key2, false), Some(State::Created));

        assert_eq!(listeners.len, 2);
        assert_eq!(listeners.notified, 0);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key1));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(2).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(None),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Empty(NonZeroUsize::new(4).unwrap())
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key1)),
                next: Cell::new(None),
            }
        );
    }

    #[test]
    fn listener_slab_notify() {
        let mut listeners = ListenerSlab::new();

        // Insert a few listeners.
        let key1 = listeners.insert(State::Created);
        let key2 = listeners.insert(State::Created);
        let key3 = listeners.insert(State::Created);

        // Notify one.
        listeners.notify(GenericNotify::new(1, true, || ()));

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 1);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key2));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: true,
                    tag: ()
                }),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Remove the notified listener.
        assert_eq!(
            listeners.remove(key1, false),
            Some(State::Notified {
                additional: true,
                tag: ()
            })
        );

        assert_eq!(listeners.len, 2);
        assert_eq!(listeners.notified, 0);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key2));
        assert_eq!(listeners.start, Some(key2));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(1).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Empty(NonZeroUsize::new(4).unwrap())
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(None),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );
    }

    #[test]
    fn listener_slab_register() {
        let woken = Arc::new(AtomicBool::new(false));
        let waker = waker_fn::waker_fn({
            let woken = woken.clone();
            move || woken.store(true, Ordering::SeqCst)
        });

        let mut listeners = ListenerSlab::new();

        // Insert a few listeners.
        let key1 = listeners.insert(State::Created);
        let key2 = listeners.insert(State::Created);
        let key3 = listeners.insert(State::Created);

        // Register one.
        assert_eq!(
            listeners.register(
                Pin::new(&mut Some(Listener::HasNode(key2))),
                TaskRef::Waker(&waker)
            ),
            RegisterResult::Registered
        );

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 0);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key1));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Task(Task::Waker(waker.clone()))),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Notify the listener.
        listeners.notify(GenericNotify::new(2, false, || ()));

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 2);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key3));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: false,
                    tag: (),
                }),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: false,
                    tag: (),
                }),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        assert!(woken.load(Ordering::SeqCst));
        assert_eq!(
            listeners.register(
                Pin::new(&mut Some(Listener::HasNode(key2))),
                TaskRef::Waker(&waker)
            ),
            RegisterResult::Notified(())
        );
    }

    #[test]
    fn listener_slab_notify_prop() {
        let woken = Arc::new(AtomicBool::new(false));
        let waker = waker_fn::waker_fn({
            let woken = woken.clone();
            move || woken.store(true, Ordering::SeqCst)
        });

        let mut listeners = ListenerSlab::new();

        // Insert a few listeners.
        let key1 = listeners.insert(State::Created);
        let key2 = listeners.insert(State::Created);
        let key3 = listeners.insert(State::Created);

        // Register one.
        assert_eq!(
            listeners.register(
                Pin::new(&mut Some(Listener::HasNode(key2))),
                TaskRef::Waker(&waker)
            ),
            RegisterResult::Registered
        );

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 0);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key1));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Task(Task::Waker(waker.clone()))),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Notify the first listener.
        listeners.notify(GenericNotify::new(1, false, || ()));

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 1);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key2));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: false,
                    tag: (),
                }),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Task(Task::Waker(waker.clone()))),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Calling notify again should not change anything.
        listeners.notify(GenericNotify::new(1, false, || ()));

        assert_eq!(listeners.len, 3);
        assert_eq!(listeners.notified, 1);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key1));
        assert_eq!(listeners.start, Some(key2));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(4).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: false,
                    tag: (),
                }),
                prev: Cell::new(None),
                next: Cell::new(Some(key2)),
            }
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Task(Task::Waker(waker.clone()))),
                prev: Cell::new(Some(key1)),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Remove the first listener.
        assert_eq!(
            listeners.remove(key1, false),
            Some(State::Notified {
                additional: false,
                tag: ()
            })
        );

        assert_eq!(listeners.len, 2);
        assert_eq!(listeners.notified, 0);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key2));
        assert_eq!(listeners.start, Some(key2));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(1).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Empty(NonZeroUsize::new(4).unwrap())
        );
        assert_eq!(*listeners.listeners[2].prev(), Cell::new(None));
        assert_eq!(*listeners.listeners[2].next(), Cell::new(Some(key3)));
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Notify the second listener.
        listeners.notify(GenericNotify::new(1, false, || ()));
        assert!(woken.load(Ordering::SeqCst));

        assert_eq!(listeners.len, 2);
        assert_eq!(listeners.notified, 1);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key2));
        assert_eq!(listeners.start, Some(key3));
        assert_eq!(listeners.first_empty, NonZeroUsize::new(1).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Empty(NonZeroUsize::new(4).unwrap())
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: false,
                    tag: (),
                }),
                prev: Cell::new(None),
                next: Cell::new(Some(key3)),
            }
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Created),
                prev: Cell::new(Some(key2)),
                next: Cell::new(None),
            }
        );

        // Remove and propagate the second listener.
        assert_eq!(listeners.remove(key2, true), Some(State::NotifiedTaken));

        // The third listener should be notified.
        assert_eq!(listeners.len, 1);
        assert_eq!(listeners.notified, 1);
        assert_eq!(listeners.tail, Some(key3));
        assert_eq!(listeners.head, Some(key3));
        assert_eq!(listeners.start, None);
        assert_eq!(listeners.first_empty, NonZeroUsize::new(2).unwrap());
        assert_eq!(listeners.listeners[0], Entry::Sentinel);
        assert_eq!(
            listeners.listeners[1],
            Entry::Empty(NonZeroUsize::new(4).unwrap())
        );
        assert_eq!(
            listeners.listeners[2],
            Entry::Empty(NonZeroUsize::new(1).unwrap())
        );
        assert_eq!(
            listeners.listeners[3],
            Entry::Listener {
                state: Cell::new(State::Notified {
                    additional: false,
                    tag: (),
                }),
                prev: Cell::new(None),
                next: Cell::new(None),
            }
        );

        // Remove the third listener.
        assert_eq!(
            listeners.remove(key3, false),
            Some(State::Notified {
                additional: false,
                tag: ()
            })
        );
    }

    #[test]
    fn uncontended_inner() {
        let inner = crate::Inner::new();

        // Register two listeners.
        let (mut listener1, mut listener2, mut listener3) = (None, None, None);
        inner.insert(Pin::new(&mut listener1));
        inner.insert(Pin::new(&mut listener2));
        inner.insert(Pin::new(&mut listener3));

        assert_eq!(
            listener1,
            Some(Listener::HasNode(NonZeroUsize::new(1).unwrap()))
        );
        assert_eq!(
            listener2,
            Some(Listener::HasNode(NonZeroUsize::new(2).unwrap()))
        );

        // Register a waker in the second listener.
        let woken = Arc::new(AtomicBool::new(false));
        let waker = waker_fn::waker_fn({
            let woken = woken.clone();
            move || woken.store(true, Ordering::SeqCst)
        });
        assert_eq!(
            inner.register(Pin::new(&mut listener2), TaskRef::Waker(&waker)),
            RegisterResult::Registered
        );

        // Notify the first listener.
        inner.notify(GenericNotify::new(1, false, || ()));
        assert!(!woken.load(Ordering::SeqCst));

        // Another notify should do nothing.
        inner.notify(GenericNotify::new(1, false, || ()));
        assert!(!woken.load(Ordering::SeqCst));

        // Receive the notification.
        assert_eq!(
            inner.register(Pin::new(&mut listener1), TaskRef::Waker(&waker)),
            RegisterResult::Notified(())
        );

        // First listener is already removed.
        assert!(listener1.is_none());

        // Notify the second listener.
        inner.notify(GenericNotify::new(1, false, || ()));
        assert!(woken.load(Ordering::SeqCst));

        // Remove the second listener and propagate the notification.
        assert_eq!(
            inner.remove(Pin::new(&mut listener2), true),
            Some(State::NotifiedTaken)
        );

        // Second listener is already removed.
        assert!(listener2.is_none());

        // Third listener should be notified.
        assert_eq!(
            inner.register(Pin::new(&mut listener3), TaskRef::Waker(&waker)),
            RegisterResult::Notified(())
        );
    }
}
