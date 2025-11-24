//! A queue of delayed elements.
//!
//! See [`DelayQueue`] for more details.
//!
//! [`DelayQueue`]: struct@DelayQueue

use crate::time::wheel::{self, Wheel};

use tokio::time::{sleep_until, Duration, Instant, Sleep};

use core::ops::{Index, IndexMut};
use slab::Slab;
use std::cmp;
use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{self, ready, Poll, Waker};

/// A queue of delayed elements.
///
/// Once an element is inserted into the `DelayQueue`, it is yielded once the
/// specified deadline has been reached.
///
/// # Usage
///
/// Elements are inserted into `DelayQueue` using the [`insert`] or
/// [`insert_at`] methods. A deadline is provided with the item and a [`Key`] is
/// returned. The key is used to remove the entry or to change the deadline at
/// which it should be yielded back.
///
/// Once delays have been configured, the `DelayQueue` is used via its
/// [`Stream`] implementation. [`poll_expired`] is called. If an entry has reached its
/// deadline, it is returned. If not, `Poll::Pending` is returned indicating that the
/// current task will be notified once the deadline has been reached.
///
/// # `Stream` implementation
///
/// Items are retrieved from the queue via [`DelayQueue::poll_expired`]. If no delays have
/// expired, no items are returned. In this case, `Poll::Pending` is returned and the
/// current task is registered to be notified once the next item's delay has
/// expired.
///
/// If no items are in the queue, i.e. `is_empty()` returns `true`, then `poll`
/// returns `Poll::Ready(None)`. This indicates that the stream has reached an end.
/// However, if a new item is inserted *after*, `poll` will once again start
/// returning items or `Poll::Pending`.
///
/// Items are returned ordered by their expirations. Items that are configured
/// to expire first will be returned first. There are no ordering guarantees
/// for items configured to expire at the same instant. Also note that delays are
/// rounded to the closest millisecond.
///
/// # Implementation
///
/// The [`DelayQueue`] is backed by a separate instance of a timer wheel similar to that used internally
/// by Tokio's standalone timer utilities such as [`sleep`]. Because of this, it offers the same
/// performance and scalability benefits.
///
/// State associated with each entry is stored in a [`slab`]. This amortizes the cost of allocation,
/// and allows reuse of the memory allocated for expired entries.
///
/// Capacity can be checked using [`capacity`] and allocated preemptively by using
/// the [`reserve`] method.
///
/// # Usage
///
/// Using `DelayQueue` to manage cache entries.
///
/// ```rust,no_run
/// use tokio_util::time::{DelayQueue, delay_queue};
///
/// use std::collections::HashMap;
/// use std::task::{ready, Context, Poll};
/// use std::time::Duration;
/// # type CacheKey = String;
/// # type Value = String;
///
/// struct Cache {
///     entries: HashMap<CacheKey, (Value, delay_queue::Key)>,
///     expirations: DelayQueue<CacheKey>,
/// }
///
/// const TTL_SECS: u64 = 30;
///
/// impl Cache {
///     fn insert(&mut self, key: CacheKey, value: Value) {
///         let delay = self.expirations
///             .insert(key.clone(), Duration::from_secs(TTL_SECS));
///
///         self.entries.insert(key, (value, delay));
///     }
///
///     fn get(&self, key: &CacheKey) -> Option<&Value> {
///         self.entries.get(key)
///             .map(|&(ref v, _)| v)
///     }
///
///     fn remove(&mut self, key: &CacheKey) {
///         if let Some((_, cache_key)) = self.entries.remove(key) {
///             self.expirations.remove(&cache_key);
///         }
///     }
///
///     fn poll_purge(&mut self, cx: &mut Context<'_>) -> Poll<()> {
///         while let Some(entry) = ready!(self.expirations.poll_expired(cx)) {
///             self.entries.remove(entry.get_ref());
///         }
///
///         Poll::Ready(())
///     }
/// }
/// ```
///
/// [`insert`]: method@Self::insert
/// [`insert_at`]: method@Self::insert_at
/// [`Key`]: struct@Key
/// [`Stream`]: https://docs.rs/futures/0.1/futures/stream/trait.Stream.html
/// [`poll_expired`]: method@Self::poll_expired
/// [`Stream::poll_expired`]: method@Self::poll_expired
/// [`DelayQueue`]: struct@DelayQueue
/// [`sleep`]: fn@tokio::time::sleep
/// [`slab`]: slab
/// [`capacity`]: method@Self::capacity
/// [`reserve`]: method@Self::reserve
#[derive(Debug)]
pub struct DelayQueue<T> {
    /// Stores data associated with entries
    slab: SlabStorage<T>,

    /// Lookup structure tracking all delays in the queue
    wheel: Wheel<Stack<T>>,

    /// Delays that were inserted when already expired. These cannot be stored
    /// in the wheel
    expired: Stack<T>,

    /// Delay expiring when the *first* item in the queue expires
    delay: Option<Pin<Box<Sleep>>>,

    /// Wheel polling state
    wheel_now: u64,

    /// Instant at which the timer starts
    start: Instant,

    /// Waker that is invoked when we potentially need to reset the timer.
    /// Because we lazily create the timer when the first entry is created, we
    /// need to awaken any poller that polled us before that point.
    waker: Option<Waker>,
}

#[derive(Default)]
struct SlabStorage<T> {
    inner: Slab<Data<T>>,

    // A `compact` call requires a re-mapping of the `Key`s that were changed
    // during the `compact` call of the `slab`. Since the keys that were given out
    // cannot be changed retroactively we need to keep track of these re-mappings.
    // The keys of `key_map` correspond to the old keys that were given out and
    // the values to the `Key`s that were re-mapped by the `compact` call.
    key_map: HashMap<Key, KeyInternal>,

    // Index used to create new keys to hand out.
    next_key_index: usize,

    // Whether `compact` has been called, necessary in order to decide whether
    // to include keys in `key_map`.
    compact_called: bool,
}

impl<T> SlabStorage<T> {
    pub(crate) fn with_capacity(capacity: usize) -> SlabStorage<T> {
        SlabStorage {
            inner: Slab::with_capacity(capacity),
            key_map: HashMap::new(),
            next_key_index: 0,
            compact_called: false,
        }
    }

    // Inserts data into the inner slab and re-maps keys if necessary
    pub(crate) fn insert(&mut self, val: Data<T>) -> Key {
        let mut key = KeyInternal::new(self.inner.insert(val));
        let key_contained = self.key_map.contains_key(&key.into());

        if key_contained {
            // It's possible that a `compact` call creates capacity in `self.inner` in
            // such a way that a `self.inner.insert` call creates a `key` which was
            // previously given out during an `insert` call prior to the `compact` call.
            // If `key` is contained in `self.key_map`, we have encountered this exact situation,
            // We need to create a new key `key_to_give_out` and include the relation
            // `key_to_give_out` -> `key` in `self.key_map`.
            let key_to_give_out = self.create_new_key();
            assert!(!self.key_map.contains_key(&key_to_give_out.into()));
            self.key_map.insert(key_to_give_out.into(), key);
            key = key_to_give_out;
        } else if self.compact_called {
            // Include an identity mapping in `self.key_map` in order to allow us to
            // panic if a key that was handed out is removed more than once.
            self.key_map.insert(key.into(), key);
        }

        key.into()
    }

    // Re-map the key in case compact was previously called.
    // Note: Since we include identity mappings in key_map after compact was called,
    // we have information about all keys that were handed out. In the case in which
    // compact was called and we try to remove a Key that was previously removed
    // we can detect invalid keys if no key is found in `key_map`. This is necessary
    // in order to prevent situations in which a previously removed key
    // corresponds to a re-mapped key internally and which would then be incorrectly
    // removed from the slab.
    //
    // Example to illuminate this problem:
    //
    // Let's assume our `key_map` is {1 -> 2, 2 -> 1} and we call remove(1). If we
    // were to remove 1 again, we would not find it inside `key_map` anymore.
    // If we were to imply from this that no re-mapping was necessary, we would
    // incorrectly remove 1 from `self.slab.inner`, which corresponds to the
    // handed-out key 2.
    pub(crate) fn remove(&mut self, key: &Key) -> Data<T> {
        let remapped_key = if self.compact_called {
            match self.key_map.remove(key) {
                Some(key_internal) => key_internal,
                None => panic!("invalid key"),
            }
        } else {
            (*key).into()
        };

        self.inner.remove(remapped_key.index)
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
        self.key_map.shrink_to_fit();
    }

    pub(crate) fn compact(&mut self) {
        if !self.compact_called {
            for (key, _) in self.inner.iter() {
                self.key_map.insert(Key::new(key), KeyInternal::new(key));
            }
        }

        let mut remapping = HashMap::new();
        self.inner.compact(|_, from, to| {
            remapping.insert(from, to);
            true
        });

        // At this point `key_map` contains a mapping for every element.
        for internal_key in self.key_map.values_mut() {
            if let Some(new_internal_key) = remapping.get(&internal_key.index) {
                *internal_key = KeyInternal::new(*new_internal_key);
            }
        }

        if self.key_map.capacity() > 2 * self.key_map.len() {
            self.key_map.shrink_to_fit();
        }

        self.compact_called = true;
    }

    // Tries to re-map a `Key` that was given out to the user to its
    // corresponding internal key.
    fn remap_key(&self, key: &Key) -> Option<KeyInternal> {
        let key_map = &self.key_map;
        if self.compact_called {
            key_map.get(key).copied()
        } else {
            Some((*key).into())
        }
    }

    fn create_new_key(&mut self) -> KeyInternal {
        while self.key_map.contains_key(&Key::new(self.next_key_index)) {
            self.next_key_index = self.next_key_index.wrapping_add(1);
        }

        KeyInternal::new(self.next_key_index)
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub(crate) fn clear(&mut self) {
        self.inner.clear();
        self.key_map.clear();
        self.compact_called = false;
    }

    pub(crate) fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);

        if self.compact_called {
            self.key_map.reserve(additional);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn contains(&self, key: &Key) -> bool {
        let remapped_key = self.remap_key(key);

        match remapped_key {
            Some(internal_key) => self.inner.contains(internal_key.index),
            None => false,
        }
    }
}

impl<T> fmt::Debug for SlabStorage<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if fmt.alternate() {
            fmt.debug_map().entries(self.inner.iter()).finish()
        } else {
            fmt.debug_struct("Slab")
                .field("len", &self.len())
                .field("cap", &self.capacity())
                .finish()
        }
    }
}

impl<T> Index<Key> for SlabStorage<T> {
    type Output = Data<T>;

    fn index(&self, key: Key) -> &Self::Output {
        let remapped_key = self.remap_key(&key);

        match remapped_key {
            Some(internal_key) => &self.inner[internal_key.index],
            None => panic!("Invalid index {}", key.index),
        }
    }
}

impl<T> IndexMut<Key> for SlabStorage<T> {
    fn index_mut(&mut self, key: Key) -> &mut Data<T> {
        let remapped_key = self.remap_key(&key);

        match remapped_key {
            Some(internal_key) => &mut self.inner[internal_key.index],
            None => panic!("Invalid index {}", key.index),
        }
    }
}

/// An entry in `DelayQueue` that has expired and been removed.
///
/// Values are returned by [`DelayQueue::poll_expired`].
///
/// [`DelayQueue::poll_expired`]: method@DelayQueue::poll_expired
#[derive(Debug)]
pub struct Expired<T> {
    /// The data stored in the queue
    data: T,

    /// The expiration time
    deadline: Instant,

    /// The key associated with the entry
    key: Key,
}

/// Token to a value stored in a `DelayQueue`.
///
/// Instances of `Key` are returned by [`DelayQueue::insert`]. See [`DelayQueue`]
/// documentation for more details.
///
/// [`DelayQueue`]: struct@DelayQueue
/// [`DelayQueue::insert`]: method@DelayQueue::insert
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    index: usize,
}

// Whereas `Key` is given out to users that use `DelayQueue`, internally we use
// `KeyInternal` as the key type in order to make the logic of mapping between keys
// as a result of `compact` calls clearer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct KeyInternal {
    index: usize,
}

#[derive(Debug)]
struct Stack<T> {
    /// Head of the stack
    head: Option<Key>,
    _p: PhantomData<fn() -> T>,
}

#[derive(Debug)]
struct Data<T> {
    /// The data being stored in the queue and will be returned at the requested
    /// instant.
    inner: T,

    /// The instant at which the item is returned.
    when: u64,

    /// Set to true when stored in the `expired` queue
    expired: bool,

    /// Next entry in the stack
    next: Option<Key>,

    /// Previous entry in the stack
    prev: Option<Key>,
}

/// Maximum number of entries the queue can handle
const MAX_ENTRIES: usize = (1 << 30) - 1;

impl<T> DelayQueue<T> {
    /// Creates a new, empty, `DelayQueue`.
    ///
    /// The queue will not allocate storage until items are inserted into it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tokio_util::time::DelayQueue;
    /// let delay_queue: DelayQueue<u32> = DelayQueue::new();
    /// ```
    pub fn new() -> DelayQueue<T> {
        DelayQueue::with_capacity(0)
    }

    /// Creates a new, empty, `DelayQueue` with the specified capacity.
    ///
    /// The queue will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the queue will not allocate for
    /// storage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tokio_util::time::DelayQueue;
    /// # use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::with_capacity(10);
    ///
    /// // These insertions are done without further allocation
    /// for i in 0..10 {
    ///     delay_queue.insert(i, Duration::from_secs(i));
    /// }
    ///
    /// // This will make the queue allocate additional storage
    /// delay_queue.insert(11, Duration::from_secs(11));
    /// # }
    /// ```
    pub fn with_capacity(capacity: usize) -> DelayQueue<T> {
        DelayQueue {
            wheel: Wheel::new(),
            slab: SlabStorage::with_capacity(capacity),
            expired: Stack::default(),
            delay: None,
            wheel_now: 0,
            start: Instant::now(),
            waker: None,
        }
    }

    /// Inserts `value` into the queue set to expire at a specific instant in
    /// time.
    ///
    /// This function is identical to `insert`, but takes an `Instant` instead
    /// of a `Duration`.
    ///
    /// `value` is stored in the queue until `when` is reached. At which point,
    /// `value` will be returned from [`poll_expired`]. If `when` has already been
    /// reached, then `value` is immediately made available to poll.
    ///
    /// The return value represents the insertion and is used as an argument to
    /// [`remove`] and [`reset`]. Note that [`Key`] is a token and is reused once
    /// `value` is removed from the queue either by calling [`poll_expired`] after
    /// `when` is reached or by calling [`remove`]. At this point, the caller
    /// must take care to not use the returned [`Key`] again as it may reference
    /// a different item in the queue.
    ///
    /// See [type] level documentation for more details.
    ///
    /// # Panics
    ///
    /// This function panics if `when` is too far in the future.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio::time::{Duration, Instant};
    /// use tokio_util::time::DelayQueue;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// let key = delay_queue.insert_at(
    ///     "foo", Instant::now() + Duration::from_secs(5));
    ///
    /// // Remove the entry
    /// let item = delay_queue.remove(&key);
    /// assert_eq!(*item.get_ref(), "foo");
    /// # }
    /// ```
    ///
    /// [`poll_expired`]: method@Self::poll_expired
    /// [`remove`]: method@Self::remove
    /// [`reset`]: method@Self::reset
    /// [`Key`]: struct@Key
    /// [type]: #
    #[track_caller]
    pub fn insert_at(&mut self, value: T, when: Instant) -> Key {
        assert!(self.slab.len() < MAX_ENTRIES, "max entries exceeded");

        // Normalize the deadline. Values cannot be set to expire in the past.
        let when = self.normalize_deadline(when);

        // Insert the value in the store
        let key = self.slab.insert(Data {
            inner: value,
            when,
            expired: false,
            next: None,
            prev: None,
        });

        self.insert_idx(when, key);

        // Set a new delay if the current's deadline is later than the one of the new item
        let should_set_delay = if let Some(ref delay) = self.delay {
            let current_exp = self.normalize_deadline(delay.deadline());
            current_exp > when
        } else {
            true
        };

        if should_set_delay {
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }

            let delay_time = self.start + Duration::from_millis(when);
            if let Some(ref mut delay) = &mut self.delay {
                delay.as_mut().reset(delay_time);
            } else {
                self.delay = Some(Box::pin(sleep_until(delay_time)));
            }
        }

        key
    }

    /// Attempts to pull out the next value of the delay queue, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the queue is exhausted.
    pub fn poll_expired(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<Expired<T>>> {
        if !self
            .waker
            .as_ref()
            .map(|w| w.will_wake(cx.waker()))
            .unwrap_or(false)
        {
            self.waker = Some(cx.waker().clone());
        }

        let item = ready!(self.poll_idx(cx));
        Poll::Ready(item.map(|key| {
            let data = self.slab.remove(&key);
            debug_assert!(data.next.is_none());
            debug_assert!(data.prev.is_none());

            Expired {
                key,
                data: data.inner,
                deadline: self.start + Duration::from_millis(data.when),
            }
        }))
    }

    /// Inserts `value` into the queue set to expire after the requested duration
    /// elapses.
    ///
    /// This function is identical to `insert_at`, but takes a `Duration`
    /// instead of an `Instant`.
    ///
    /// `value` is stored in the queue until `timeout` duration has
    /// elapsed after `insert` was called. At that point, `value` will
    /// be returned from [`poll_expired`]. If `timeout` is a `Duration` of
    /// zero, then `value` is immediately made available to poll.
    ///
    /// The return value represents the insertion and is used as an
    /// argument to [`remove`] and [`reset`]. Note that [`Key`] is a
    /// token and is reused once `value` is removed from the queue
    /// either by calling [`poll_expired`] after `timeout` has elapsed
    /// or by calling [`remove`]. At this point, the caller must not
    /// use the returned [`Key`] again as it may reference a different
    /// item in the queue.
    ///
    /// See [type] level documentation for more details.
    ///
    /// # Panics
    ///
    /// This function panics if `timeout` is greater than the maximum
    /// duration supported by the timer in the current `Runtime`.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// let key = delay_queue.insert("foo", Duration::from_secs(5));
    ///
    /// // Remove the entry
    /// let item = delay_queue.remove(&key);
    /// assert_eq!(*item.get_ref(), "foo");
    /// # }
    /// ```
    ///
    /// [`poll_expired`]: method@Self::poll_expired
    /// [`remove`]: method@Self::remove
    /// [`reset`]: method@Self::reset
    /// [`Key`]: struct@Key
    /// [type]: #
    #[track_caller]
    pub fn insert(&mut self, value: T, timeout: Duration) -> Key {
        self.insert_at(value, Instant::now() + timeout)
    }

    #[track_caller]
    fn insert_idx(&mut self, when: u64, key: Key) {
        use self::wheel::{InsertError, Stack};

        // Register the deadline with the timer wheel
        match self.wheel.insert(when, key, &mut self.slab) {
            Ok(_) => {}
            Err((_, InsertError::Elapsed)) => {
                self.slab[key].expired = true;
                // The delay is already expired, store it in the expired queue
                self.expired.push(key, &mut self.slab);
            }
            Err((_, err)) => panic!("invalid deadline; err={err:?}"),
        }
    }

    /// Returns the deadline of the item associated with `key`.
    ///
    /// Since the queue operates at millisecond granularity, the returned
    /// deadline may not exactly match the value that was given when initially
    /// inserting the item into the queue.
    ///
    /// # Panics
    ///
    /// This function panics if `key` is not contained by the queue.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    ///
    /// let key1 = delay_queue.insert("foo", Duration::from_secs(5));
    /// let key2 = delay_queue.insert("bar", Duration::from_secs(10));
    ///
    /// assert!(delay_queue.deadline(&key1) < delay_queue.deadline(&key2));
    /// # }
    /// ```
    #[track_caller]
    pub fn deadline(&self, key: &Key) -> Instant {
        self.start + Duration::from_millis(self.slab[*key].when)
    }

    /// Removes the key from the expired queue or the timer wheel
    /// depending on its expiration status.
    ///
    /// # Panics
    ///
    /// Panics if the key is not contained in the expired queue or the wheel.
    #[track_caller]
    fn remove_key(&mut self, key: &Key) {
        use crate::time::wheel::Stack;

        // Special case the `expired` queue
        if self.slab[*key].expired {
            self.expired.remove(key, &mut self.slab);
        } else {
            self.wheel.remove(key, &mut self.slab);
        }
    }

    /// Removes the item associated with `key` from the queue.
    ///
    /// There must be an item associated with `key`. The function returns the
    /// removed item as well as the `Instant` at which it will the delay will
    /// have expired.
    ///
    /// # Panics
    ///
    /// The function panics if `key` is not contained by the queue.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// let key = delay_queue.insert("foo", Duration::from_secs(5));
    ///
    /// // Remove the entry
    /// let item = delay_queue.remove(&key);
    /// assert_eq!(*item.get_ref(), "foo");
    /// # }
    /// ```
    #[track_caller]
    pub fn remove(&mut self, key: &Key) -> Expired<T> {
        let prev_deadline = self.next_deadline();

        self.remove_key(key);
        let data = self.slab.remove(key);

        let next_deadline = self.next_deadline();
        if prev_deadline != next_deadline {
            match (next_deadline, &mut self.delay) {
                (None, _) => self.delay = None,
                (Some(deadline), Some(delay)) => delay.as_mut().reset(deadline),
                (Some(deadline), None) => self.delay = Some(Box::pin(sleep_until(deadline))),
            }
        }

        if self.slab.is_empty() {
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }

        Expired {
            key: Key::new(key.index),
            data: data.inner,
            deadline: self.start + Duration::from_millis(data.when),
        }
    }

    /// Attempts to remove the item associated with `key` from the queue.
    ///
    /// Removes the item associated with `key`, and returns it along with the
    /// `Instant` at which it would have expired, if it exists.
    ///
    /// Returns `None` if `key` is not in the queue.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// let key = delay_queue.insert("foo", Duration::from_secs(5));
    ///
    /// // The item is in the queue, `try_remove` returns `Some(Expired("foo"))`.
    /// let item = delay_queue.try_remove(&key);
    /// assert_eq!(item.unwrap().into_inner(), "foo");
    ///
    /// // The item is not in the queue anymore, `try_remove` returns `None`.
    /// let item = delay_queue.try_remove(&key);
    /// assert!(item.is_none());
    /// # }
    /// ```
    pub fn try_remove(&mut self, key: &Key) -> Option<Expired<T>> {
        if self.slab.contains(key) {
            Some(self.remove(key))
        } else {
            None
        }
    }

    /// Sets the delay of the item associated with `key` to expire at `when`.
    ///
    /// This function is identical to `reset` but takes an `Instant` instead of
    /// a `Duration`.
    ///
    /// The item remains in the queue but the delay is set to expire at `when`.
    /// If `when` is in the past, then the item is immediately made available to
    /// the caller.
    ///
    /// # Panics
    ///
    /// This function panics if `when` is too far in the future or if `key` is
    /// not contained by the queue.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio::time::{Duration, Instant};
    /// use tokio_util::time::DelayQueue;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// let key = delay_queue.insert("foo", Duration::from_secs(5));
    ///
    /// // "foo" is scheduled to be returned in 5 seconds
    ///
    /// delay_queue.reset_at(&key, Instant::now() + Duration::from_secs(10));
    ///
    /// // "foo" is now scheduled to be returned in 10 seconds
    /// # }
    /// ```
    #[track_caller]
    pub fn reset_at(&mut self, key: &Key, when: Instant) {
        self.remove_key(key);

        // Normalize the deadline. Values cannot be set to expire in the past.
        let when = self.normalize_deadline(when);

        self.slab[*key].when = when;
        self.slab[*key].expired = false;

        self.insert_idx(when, *key);

        let next_deadline = self.next_deadline();
        if let (Some(ref mut delay), Some(deadline)) = (&mut self.delay, next_deadline) {
            // This should awaken us if necessary (ie, if already expired)
            delay.as_mut().reset(deadline);
        }
    }

    /// Shrink the capacity of the slab, which `DelayQueue` uses internally for storage allocation.
    /// This function is not guaranteed to, and in most cases, won't decrease the capacity of the slab
    /// to the number of elements still contained in it, because elements cannot be moved to a different
    /// index. To decrease the capacity to the size of the slab use [`compact`].
    ///
    /// This function can take O(n) time even when the capacity cannot be reduced or the allocation is
    /// shrunk in place. Repeated calls run in O(1) though.
    ///
    /// [`compact`]: method@Self::compact
    pub fn shrink_to_fit(&mut self) {
        self.slab.shrink_to_fit();
    }

    /// Shrink the capacity of the slab, which `DelayQueue` uses internally for storage allocation,
    /// to the number of elements that are contained in it.
    ///
    /// This methods runs in O(n).
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::with_capacity(10);
    ///
    /// let key1 = delay_queue.insert(5, Duration::from_secs(5));
    /// let key2 = delay_queue.insert(10, Duration::from_secs(10));
    /// let key3 = delay_queue.insert(15, Duration::from_secs(15));
    ///
    /// delay_queue.remove(&key2);
    ///
    /// delay_queue.compact();
    /// assert_eq!(delay_queue.capacity(), 2);
    /// # }
    /// ```
    pub fn compact(&mut self) {
        self.slab.compact();
    }

    /// Gets the [`Key`] that [`poll_expired`] will pull out of the queue next, without
    /// pulling it out or waiting for the deadline to expire.
    ///
    /// Entries that have already expired may be returned in any order, but it is
    /// guaranteed that this method returns them in the same order as when items
    /// are popped from the `DelayQueue`.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    ///
    /// let key1 = delay_queue.insert("foo", Duration::from_secs(10));
    /// let key2 = delay_queue.insert("bar", Duration::from_secs(5));
    /// let key3 = delay_queue.insert("baz", Duration::from_secs(15));
    ///
    /// assert_eq!(delay_queue.peek().unwrap(), key2);
    /// # }
    /// ```
    ///
    /// [`Key`]: struct@Key
    /// [`poll_expired`]: method@Self::poll_expired
    pub fn peek(&self) -> Option<Key> {
        use self::wheel::Stack;

        self.expired.peek().or_else(|| self.wheel.peek())
    }

    /// Returns the next time to poll as determined by the wheel.
    ///
    /// Note that this does not include deadlines in the `expired` queue.
    fn next_deadline(&self) -> Option<Instant> {
        self.wheel
            .poll_at()
            .map(|poll_at| self.start + Duration::from_millis(poll_at))
    }

    /// Sets the delay of the item associated with `key` to expire after
    /// `timeout`.
    ///
    /// This function is identical to `reset_at` but takes a `Duration` instead
    /// of an `Instant`.
    ///
    /// The item remains in the queue but the delay is set to expire after
    /// `timeout`. If `timeout` is zero, then the item is immediately made
    /// available to the caller.
    ///
    /// # Panics
    ///
    /// This function panics if `timeout` is greater than the maximum supported
    /// duration or if `key` is not contained by the queue.
    ///
    /// # Examples
    ///
    /// Basic usage
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// let key = delay_queue.insert("foo", Duration::from_secs(5));
    ///
    /// // "foo" is scheduled to be returned in 5 seconds
    ///
    /// delay_queue.reset(&key, Duration::from_secs(10));
    ///
    /// // "foo"is now scheduled to be returned in 10 seconds
    /// # }
    /// ```
    #[track_caller]
    pub fn reset(&mut self, key: &Key, timeout: Duration) {
        self.reset_at(key, Instant::now() + timeout);
    }

    /// Clears the queue, removing all items.
    ///
    /// After calling `clear`, [`poll_expired`] will return `Ok(Ready(None))`.
    ///
    /// Note that this method has no effect on the allocated capacity.
    ///
    /// [`poll_expired`]: method@Self::poll_expired
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    ///
    /// delay_queue.insert("foo", Duration::from_secs(5));
    ///
    /// assert!(!delay_queue.is_empty());
    ///
    /// delay_queue.clear();
    ///
    /// assert!(delay_queue.is_empty());
    /// # }
    /// ```
    pub fn clear(&mut self) {
        self.slab.clear();
        self.expired = Stack::default();
        self.wheel = Wheel::new();
        self.delay = None;
    }

    /// Returns the number of elements the queue can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    ///
    /// let delay_queue: DelayQueue<i32> = DelayQueue::with_capacity(10);
    /// assert_eq!(delay_queue.capacity(), 10);
    /// ```
    pub fn capacity(&self) -> usize {
        self.slab.capacity()
    }

    /// Returns the number of elements currently in the queue.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue: DelayQueue<i32> = DelayQueue::with_capacity(10);
    /// assert_eq!(delay_queue.len(), 0);
    /// delay_queue.insert(3, Duration::from_secs(5));
    /// assert_eq!(delay_queue.len(), 1);
    /// # }
    /// ```
    pub fn len(&self) -> usize {
        self.slab.len()
    }

    /// Reserves capacity for at least `additional` more items to be queued
    /// without allocating.
    ///
    /// `reserve` does nothing if the queue already has sufficient capacity for
    /// `additional` more values. If more capacity is required, a new segment of
    /// memory will be allocated and all existing values will be copied into it.
    /// As such, if the queue is already very large, a call to `reserve` can end
    /// up being expensive.
    ///
    /// The queue may reserve more than `additional` extra space in order to
    /// avoid frequent reallocations.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds the maximum number of entries the
    /// queue can contain.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    ///
    /// delay_queue.insert("hello", Duration::from_secs(10));
    /// delay_queue.reserve(10);
    ///
    /// assert!(delay_queue.capacity() >= 11);
    /// # }
    /// ```
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        assert!(
            self.slab.capacity() + additional <= MAX_ENTRIES,
            "max queue capacity exceeded"
        );
        self.slab.reserve(additional);
    }

    /// Returns `true` if there are no items in the queue.
    ///
    /// Note that this function returns `false` even if all items have not yet
    /// expired and a call to `poll` will return `Poll::Pending`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_util::time::DelayQueue;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let mut delay_queue = DelayQueue::new();
    /// assert!(delay_queue.is_empty());
    ///
    /// delay_queue.insert("hello", Duration::from_secs(5));
    /// assert!(!delay_queue.is_empty());
    /// # }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.slab.is_empty()
    }

    /// Polls the queue, returning the index of the next slot in the slab that
    /// should be returned.
    ///
    /// A slot should be returned when the associated deadline has been reached.
    fn poll_idx(&mut self, cx: &mut task::Context<'_>) -> Poll<Option<Key>> {
        use self::wheel::Stack;

        let expired = self.expired.pop(&mut self.slab);

        if expired.is_some() {
            return Poll::Ready(expired);
        }

        loop {
            if let Some(ref mut delay) = self.delay {
                if !delay.is_elapsed() {
                    ready!(Pin::new(&mut *delay).poll(cx));
                }

                let now = crate::time::ms(delay.deadline() - self.start, crate::time::Round::Down);

                self.wheel_now = now;
            }

            // We poll the wheel to get the next value out before finding the next deadline.
            let wheel_idx = self.wheel.poll(self.wheel_now, &mut self.slab);

            self.delay = self.next_deadline().map(|when| Box::pin(sleep_until(when)));

            if let Some(idx) = wheel_idx {
                return Poll::Ready(Some(idx));
            }

            if self.delay.is_none() {
                return Poll::Ready(None);
            }
        }
    }

    fn normalize_deadline(&self, when: Instant) -> u64 {
        let when = if when < self.start {
            0
        } else {
            crate::time::ms(when - self.start, crate::time::Round::Up)
        };

        cmp::max(when, self.wheel.elapsed())
    }
}

// We never put `T` in a `Pin`...
impl<T> Unpin for DelayQueue<T> {}

impl<T> Default for DelayQueue<T> {
    fn default() -> DelayQueue<T> {
        DelayQueue::new()
    }
}

impl<T> futures_core::Stream for DelayQueue<T> {
    // DelayQueue seems much more specific, where a user may care that it
    // has reached capacity, so return those errors instead of panicking.
    type Item = Expired<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        DelayQueue::poll_expired(self.get_mut(), cx)
    }
}

impl<T> wheel::Stack for Stack<T> {
    type Owned = Key;
    type Borrowed = Key;
    type Store = SlabStorage<T>;

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn push(&mut self, item: Self::Owned, store: &mut Self::Store) {
        // Ensure the entry is not already in a stack.
        debug_assert!(store[item].next.is_none());
        debug_assert!(store[item].prev.is_none());

        // Remove the old head entry
        let old = self.head.take();

        if let Some(idx) = old {
            store[idx].prev = Some(item);
        }

        store[item].next = old;
        self.head = Some(item);
    }

    fn pop(&mut self, store: &mut Self::Store) -> Option<Self::Owned> {
        if let Some(key) = self.head {
            self.head = store[key].next;

            if let Some(idx) = self.head {
                store[idx].prev = None;
            }

            store[key].next = None;
            debug_assert!(store[key].prev.is_none());

            Some(key)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<Self::Owned> {
        self.head
    }

    #[track_caller]
    fn remove(&mut self, item: &Self::Borrowed, store: &mut Self::Store) {
        let key = *item;
        assert!(store.contains(item));

        // Ensure that the entry is in fact contained by the stack
        debug_assert!({
            // This walks the full linked list even if an entry is found.
            let mut next = self.head;
            let mut contains = false;

            while let Some(idx) = next {
                let data = &store[idx];

                if idx == *item {
                    debug_assert!(!contains);
                    contains = true;
                }

                next = data.next;
            }

            contains
        });

        if let Some(next) = store[key].next {
            store[next].prev = store[key].prev;
        }

        if let Some(prev) = store[key].prev {
            store[prev].next = store[key].next;
        } else {
            self.head = store[key].next;
        }

        store[key].next = None;
        store[key].prev = None;
    }

    fn when(item: &Self::Borrowed, store: &Self::Store) -> u64 {
        store[*item].when
    }
}

impl<T> Default for Stack<T> {
    fn default() -> Stack<T> {
        Stack {
            head: None,
            _p: PhantomData,
        }
    }
}

impl Key {
    pub(crate) fn new(index: usize) -> Key {
        Key { index }
    }
}

impl KeyInternal {
    pub(crate) fn new(index: usize) -> KeyInternal {
        KeyInternal { index }
    }
}

impl From<Key> for KeyInternal {
    fn from(item: Key) -> Self {
        KeyInternal::new(item.index)
    }
}

impl From<KeyInternal> for Key {
    fn from(item: KeyInternal) -> Self {
        Key::new(item.index)
    }
}

impl<T> Expired<T> {
    /// Returns a reference to the inner value.
    pub fn get_ref(&self) -> &T {
        &self.data
    }

    /// Returns a mutable reference to the inner value.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Consumes `self` and returns the inner value.
    pub fn into_inner(self) -> T {
        self.data
    }

    /// Returns the deadline that the expiration was set to.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Returns the key that the expiration is indexed by.
    pub fn key(&self) -> Key {
        self.key
    }
}
