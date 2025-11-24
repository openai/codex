use crate::Stream;

use std::borrow::Borrow;
use std::future::poll_fn;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

/// Combine many streams into one, indexing each source stream with a unique
/// key.
///
/// `StreamMap` is similar to [`StreamExt::merge`] in that it combines source
/// streams into a single merged stream that yields values in the order that
/// they arrive from the source streams. However, `StreamMap` has a lot more
/// flexibility in usage patterns.
///
/// `StreamMap` can:
///
/// * Merge an arbitrary number of streams.
/// * Track which source stream the value was received from.
/// * Handle inserting and removing streams from the set of managed streams at
///   any point during iteration.
///
/// All source streams held by `StreamMap` are indexed using a key. This key is
/// included with the value when a source stream yields a value. The key is also
/// used to remove the stream from the `StreamMap` before the stream has
/// completed streaming.
///
/// # `Unpin`
///
/// Because the `StreamMap` API moves streams during runtime, both streams and
/// keys must be `Unpin`. In order to insert a `!Unpin` stream into a
/// `StreamMap`, use [`pin!`] to pin the stream to the stack or [`Box::pin`] to
/// pin the stream in the heap.
///
/// # Implementation
///
/// `StreamMap` is backed by a `Vec<(K, V)>`. There is no guarantee that this
/// internal implementation detail will persist in future versions, but it is
/// important to know the runtime implications. In general, `StreamMap` works
/// best with a "smallish" number of streams as all entries are scanned on
/// insert, remove, and polling. In cases where a large number of streams need
/// to be merged, it may be advisable to use tasks sending values on a shared
/// [`mpsc`] channel.
///
/// # Notes
///
/// `StreamMap` removes finished streams automatically, without alerting the user.
/// In some scenarios, the caller would want to know on closed streams.
/// To do this, use [`StreamNotifyClose`] as a wrapper to your stream.
/// It will return None when the stream is closed.
///
/// [`StreamExt::merge`]: crate::StreamExt::merge
/// [`mpsc`]: https://docs.rs/tokio/1.0/tokio/sync/mpsc/index.html
/// [`pin!`]: https://docs.rs/tokio/1.0/tokio/macro.pin.html
/// [`Box::pin`]: std::boxed::Box::pin
/// [`StreamNotifyClose`]: crate::StreamNotifyClose
///
/// # Examples
///
/// Merging two streams, then remove them after receiving the first value
///
/// ```
/// use tokio_stream::{StreamExt, StreamMap, Stream};
/// use tokio::sync::mpsc;
/// use std::pin::Pin;
///
/// #[tokio::main]
/// async fn main() {
///     let (tx1, mut rx1) = mpsc::channel::<usize>(10);
///     let (tx2, mut rx2) = mpsc::channel::<usize>(10);
///
///     // Convert the channels to a `Stream`.
///     let rx1 = Box::pin(async_stream::stream! {
///           while let Some(item) = rx1.recv().await {
///               yield item;
///           }
///     }) as Pin<Box<dyn Stream<Item = usize> + Send>>;
///
///     let rx2 = Box::pin(async_stream::stream! {
///           while let Some(item) = rx2.recv().await {
///               yield item;
///           }
///     }) as Pin<Box<dyn Stream<Item = usize> + Send>>;
///
///     tokio::spawn(async move {
///         tx1.send(1).await.unwrap();
///
///         // This value will never be received. The send may or may not return
///         // `Err` depending on if the remote end closed first or not.
///         let _ = tx1.send(2).await;
///     });
///
///     tokio::spawn(async move {
///         tx2.send(3).await.unwrap();
///         let _ = tx2.send(4).await;
///     });
///
///     let mut map = StreamMap::new();
///
///     // Insert both streams
///     map.insert("one", rx1);
///     map.insert("two", rx2);
///
///     // Read twice
///     for _ in 0..2 {
///         let (key, val) = map.next().await.unwrap();
///
///         if key == "one" {
///             assert_eq!(val, 1);
///         } else {
///             assert_eq!(val, 3);
///         }
///
///         // Remove the stream to prevent reading the next value
///         map.remove(key);
///     }
/// }
/// ```
///
/// This example models a read-only client to a chat system with channels. The
/// client sends commands to join and leave channels. `StreamMap` is used to
/// manage active channel subscriptions.
///
/// For simplicity, messages are displayed with `println!`, but they could be
/// sent to the client over a socket.
///
/// ```no_run
/// use tokio_stream::{Stream, StreamExt, StreamMap};
///
/// enum Command {
///     Join(String),
///     Leave(String),
/// }
///
/// fn commands() -> impl Stream<Item = Command> {
///     // Streams in user commands by parsing `stdin`.
/// # tokio_stream::pending()
/// }
///
/// // Join a channel, returns a stream of messages received on the channel.
/// fn join(channel: &str) -> impl Stream<Item = String> + Unpin {
///     // left as an exercise to the reader
/// # tokio_stream::pending()
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut channels = StreamMap::new();
///
///     // Input commands (join / leave channels).
///     let cmds = commands();
///     tokio::pin!(cmds);
///
///     loop {
///         tokio::select! {
///             Some(cmd) = cmds.next() => {
///                 match cmd {
///                     Command::Join(chan) => {
///                         // Join the channel and add it to the `channels`
///                         // stream map
///                         let msgs = join(&chan);
///                         channels.insert(chan, msgs);
///                     }
///                     Command::Leave(chan) => {
///                         channels.remove(&chan);
///                     }
///                 }
///             }
///             Some((chan, msg)) = channels.next() => {
///                 // Received a message, display it on stdout with the channel
///                 // it originated from.
///                 println!("{}: {}", chan, msg);
///             }
///             // Both the `commands` stream and the `channels` stream are
///             // complete. There is no more work to do, so leave the loop.
///             else => break,
///         }
///     }
/// }
/// ```
///
/// Using `StreamNotifyClose` to handle closed streams with `StreamMap`.
///
/// ```
/// use tokio_stream::{StreamExt, StreamMap, StreamNotifyClose};
///
/// #[tokio::main]
/// async fn main() {
///     let mut map = StreamMap::new();
///     let stream = StreamNotifyClose::new(tokio_stream::iter(vec![0, 1]));
///     let stream2 = StreamNotifyClose::new(tokio_stream::iter(vec![0, 1]));
///     map.insert(0, stream);
///     map.insert(1, stream2);
///     while let Some((key, val)) = map.next().await {
///         match val {
///             Some(val) => println!("got {val:?} from stream {key:?}"),
///             None => println!("stream {key:?} closed"),
///         }
///     }
/// }
/// ```

#[derive(Debug)]
pub struct StreamMap<K, V> {
    /// Streams stored in the map
    entries: Vec<(K, V)>,
}

impl<K, V> StreamMap<K, V> {
    /// An iterator visiting all key-value pairs in arbitrary order.
    ///
    /// The iterator element type is `&'a (K, V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    ///
    /// map.insert("a", pending::<i32>());
    /// map.insert("b", pending());
    /// map.insert("c", pending());
    ///
    /// for (key, stream) in map.iter() {
    ///     println!("({}, {:?})", key, stream);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &(K, V)> {
        self.entries.iter()
    }

    /// An iterator visiting all key-value pairs mutably in arbitrary order.
    ///
    /// The iterator element type is `&'a mut (K, V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    ///
    /// map.insert("a", pending::<i32>());
    /// map.insert("b", pending());
    /// map.insert("c", pending());
    ///
    /// for (key, stream) in map.iter_mut() {
    ///     println!("({}, {:?})", key, stream);
    /// }
    /// ```
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (K, V)> {
        self.entries.iter_mut()
    }

    /// Creates an empty `StreamMap`.
    ///
    /// The stream map is initially created with a capacity of `0`, so it will
    /// not allocate until it is first inserted into.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, Pending};
    ///
    /// let map: StreamMap<&str, Pending<()>> = StreamMap::new();
    /// ```
    pub fn new() -> StreamMap<K, V> {
        StreamMap { entries: vec![] }
    }

    /// Creates an empty `StreamMap` with the specified capacity.
    ///
    /// The stream map will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the stream map will not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, Pending};
    ///
    /// let map: StreamMap<&str, Pending<()>> = StreamMap::with_capacity(10);
    /// ```
    pub fn with_capacity(capacity: usize) -> StreamMap<K, V> {
        StreamMap {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Returns an iterator visiting all keys in arbitrary order.
    ///
    /// The iterator element type is `&'a K`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    ///
    /// map.insert("a", pending::<i32>());
    /// map.insert("b", pending());
    /// map.insert("c", pending());
    ///
    /// for key in map.keys() {
    ///     println!("{}", key);
    /// }
    /// ```
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    /// An iterator visiting all values in arbitrary order.
    ///
    /// The iterator element type is `&'a V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    ///
    /// map.insert("a", pending::<i32>());
    /// map.insert("b", pending());
    /// map.insert("c", pending());
    ///
    /// for stream in map.values() {
    ///     println!("{:?}", stream);
    /// }
    /// ```
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// An iterator visiting all values mutably in arbitrary order.
    ///
    /// The iterator element type is `&'a mut V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    ///
    /// map.insert("a", pending::<i32>());
    /// map.insert("b", pending());
    /// map.insert("c", pending());
    ///
    /// for stream in map.values_mut() {
    ///     println!("{:?}", stream);
    /// }
    /// ```
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.iter_mut().map(|(_, v)| v)
    }

    /// Returns the number of streams the map can hold without reallocating.
    ///
    /// This number is a lower bound; the `StreamMap` might be able to hold
    /// more, but is guaranteed to be able to hold at least this many.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, Pending};
    ///
    /// let map: StreamMap<i32, Pending<()>> = StreamMap::with_capacity(100);
    /// assert!(map.capacity() >= 100);
    /// ```
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Returns the number of streams in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut a = StreamMap::new();
    /// assert_eq!(a.len(), 0);
    /// a.insert(1, pending::<i32>());
    /// assert_eq!(a.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the map contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut a = StreamMap::new();
    /// assert!(a.is_empty());
    /// a.insert(1, pending::<i32>());
    /// assert!(!a.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears the map, removing all key-stream pairs. Keeps the allocated
    /// memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut a = StreamMap::new();
    /// a.insert(1, pending::<i32>());
    /// a.clear();
    /// assert!(a.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Insert a key-stream pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    ///
    /// If the map did have this key present, the new `stream` replaces the old
    /// one and the old stream is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    ///
    /// assert!(map.insert(37, pending::<i32>()).is_none());
    /// assert!(!map.is_empty());
    ///
    /// map.insert(37, pending());
    /// assert!(map.insert(37, pending()).is_some());
    /// ```
    pub fn insert(&mut self, k: K, stream: V) -> Option<V>
    where
        K: Hash + Eq,
    {
        let ret = self.remove(&k);
        self.entries.push((k, stream));

        ret
    }

    /// Removes a key from the map, returning the stream at the key if the key was previously in the map.
    ///
    /// The key may be any borrowed form of the map's key type, but `Hash` and
    /// `Eq` on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    /// map.insert(1, pending::<i32>());
    /// assert!(map.remove(&1).is_some());
    /// assert!(map.remove(&1).is_none());
    /// ```
    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        for i in 0..self.entries.len() {
            if self.entries[i].0.borrow() == k {
                return Some(self.entries.swap_remove(i).1);
            }
        }

        None
    }

    /// Returns `true` if the map contains a stream for the specified key.
    ///
    /// The key may be any borrowed form of the map's key type, but `Hash` and
    /// `Eq` on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio_stream::{StreamMap, pending};
    ///
    /// let mut map = StreamMap::new();
    /// map.insert(1, pending::<i32>());
    /// assert_eq!(map.contains_key(&1), true);
    /// assert_eq!(map.contains_key(&2), false);
    /// ```
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        for i in 0..self.entries.len() {
            if self.entries[i].0.borrow() == k {
                return true;
            }
        }

        false
    }
}

impl<K, V> StreamMap<K, V>
where
    K: Unpin,
    V: Stream + Unpin,
{
    /// Polls the next value, includes the vec entry index
    fn poll_next_entry(&mut self, cx: &mut Context<'_>) -> Poll<Option<(usize, V::Item)>> {
        let start = self::rand::thread_rng_n(self.entries.len() as u32) as usize;
        let mut idx = start;

        for _ in 0..self.entries.len() {
            let (_, stream) = &mut self.entries[idx];

            match Pin::new(stream).poll_next(cx) {
                Poll::Ready(Some(val)) => return Poll::Ready(Some((idx, val))),
                Poll::Ready(None) => {
                    // Remove the entry
                    self.entries.swap_remove(idx);

                    // Check if this was the last entry, if so the cursor needs
                    // to wrap
                    if idx == self.entries.len() {
                        idx = 0;
                    } else if idx < start && start <= self.entries.len() {
                        // The stream being swapped into the current index has
                        // already been polled, so skip it.
                        idx = idx.wrapping_add(1) % self.entries.len();
                    }
                }
                Poll::Pending => {
                    idx = idx.wrapping_add(1) % self.entries.len();
                }
            }
        }

        // If the map is empty, then the stream is complete.
        if self.entries.is_empty() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

impl<K, V> Default for StreamMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> StreamMap<K, V>
where
    K: Clone + Unpin,
    V: Stream + Unpin,
{
    /// Receives multiple items on this [`StreamMap`], extending the provided `buffer`.
    ///
    /// This method returns the number of items that is appended to the `buffer`.
    ///
    /// Note that this method does not guarantee that exactly `limit` items
    /// are received. Rather, if at least one item is available, it returns
    /// as many items as it can up to the given limit. This method returns
    /// zero only if the `StreamMap` is empty (or if `limit` is zero).
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If `next_many` is used as the event in a
    /// [`tokio::select!`](tokio::select) statement and some other branch
    /// completes first, it is guaranteed that no items were received on any of
    /// the underlying streams.
    pub async fn next_many(&mut self, buffer: &mut Vec<(K, V::Item)>, limit: usize) -> usize {
        poll_fn(|cx| self.poll_next_many(cx, buffer, limit)).await
    }

    /// Polls to receive multiple items on this `StreamMap`, extending the provided `buffer`.
    ///
    /// This method returns:
    /// * `Poll::Pending` if no items are available but the `StreamMap` is not empty.
    /// * `Poll::Ready(count)` where `count` is the number of items successfully received and
    ///   stored in `buffer`. This can be less than, or equal to, `limit`.
    /// * `Poll::Ready(0)` if `limit` is set to zero or when the `StreamMap` is empty.
    ///
    /// Note that this method does not guarantee that exactly `limit` items
    /// are received. Rather, if at least one item is available, it returns
    /// as many items as it can up to the given limit. This method returns
    /// zero only if the `StreamMap` is empty (or if `limit` is zero).
    pub fn poll_next_many(
        &mut self,
        cx: &mut Context<'_>,
        buffer: &mut Vec<(K, V::Item)>,
        limit: usize,
    ) -> Poll<usize> {
        if limit == 0 || self.entries.is_empty() {
            return Poll::Ready(0);
        }

        let mut added = 0;

        let start = self::rand::thread_rng_n(self.entries.len() as u32) as usize;
        let mut idx = start;

        while added < limit {
            // Indicates whether at least one stream returned a value when polled or not
            let mut should_loop = false;

            for _ in 0..self.entries.len() {
                let (_, stream) = &mut self.entries[idx];

                match Pin::new(stream).poll_next(cx) {
                    Poll::Ready(Some(val)) => {
                        added += 1;

                        let key = self.entries[idx].0.clone();
                        buffer.push((key, val));

                        should_loop = true;

                        idx = idx.wrapping_add(1) % self.entries.len();
                    }
                    Poll::Ready(None) => {
                        // Remove the entry
                        self.entries.swap_remove(idx);

                        // Check if this was the last entry, if so the cursor needs
                        // to wrap
                        if idx == self.entries.len() {
                            idx = 0;
                        } else if idx < start && start <= self.entries.len() {
                            // The stream being swapped into the current index has
                            // already been polled, so skip it.
                            idx = idx.wrapping_add(1) % self.entries.len();
                        }
                    }
                    Poll::Pending => {
                        idx = idx.wrapping_add(1) % self.entries.len();
                    }
                }
            }

            if !should_loop {
                break;
            }
        }

        if added > 0 {
            Poll::Ready(added)
        } else if self.entries.is_empty() {
            Poll::Ready(0)
        } else {
            Poll::Pending
        }
    }
}

impl<K, V> Stream for StreamMap<K, V>
where
    K: Clone + Unpin,
    V: Stream + Unpin,
{
    type Item = (K, V::Item);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some((idx, val)) = ready!(self.poll_next_entry(cx)) {
            let key = self.entries[idx].0.clone();
            Poll::Ready(Some((key, val)))
        } else {
            Poll::Ready(None)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let mut ret = (0, Some(0));

        for (_, stream) in &self.entries {
            let hint = stream.size_hint();

            ret.0 += hint.0;

            match (ret.1, hint.1) {
                (Some(a), Some(b)) => ret.1 = Some(a + b),
                (Some(_), None) => ret.1 = None,
                _ => {}
            }
        }

        ret
    }
}

impl<K, V> FromIterator<(K, V)> for StreamMap<K, V>
where
    K: Hash + Eq,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let iterator = iter.into_iter();
        let (lower_bound, _) = iterator.size_hint();
        let mut stream_map = Self::with_capacity(lower_bound);

        for (key, value) in iterator {
            stream_map.insert(key, value);
        }

        stream_map
    }
}

impl<K, V> Extend<(K, V)> for StreamMap<K, V> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (K, V)>,
    {
        self.entries.extend(iter);
    }
}

mod rand {
    use std::cell::Cell;

    mod loom {
        #[cfg(not(loom))]
        pub(crate) mod rand {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hash, Hasher};
            use std::sync::atomic::AtomicU32;
            use std::sync::atomic::Ordering::Relaxed;

            static COUNTER: AtomicU32 = AtomicU32::new(1);

            pub(crate) fn seed() -> u64 {
                let rand_state = RandomState::new();

                let mut hasher = rand_state.build_hasher();

                // Hash some unique-ish data to generate some new state
                COUNTER.fetch_add(1, Relaxed).hash(&mut hasher);

                // Get the seed
                hasher.finish()
            }
        }

        #[cfg(loom)]
        pub(crate) mod rand {
            pub(crate) fn seed() -> u64 {
                1
            }
        }
    }

    /// Fast random number generate
    ///
    /// Implement `xorshift64+`: 2 32-bit `xorshift` sequences added together.
    /// Shift triplet `[17,7,16]` was calculated as indicated in Marsaglia's
    /// `Xorshift` paper: <https://www.jstatsoft.org/article/view/v008i14/xorshift.pdf>
    /// This generator passes the SmallCrush suite, part of TestU01 framework:
    /// <http://simul.iro.umontreal.ca/testu01/tu01.html>
    #[derive(Debug)]
    pub(crate) struct FastRand {
        one: Cell<u32>,
        two: Cell<u32>,
    }

    impl FastRand {
        /// Initialize a new, thread-local, fast random number generator.
        pub(crate) fn new(seed: u64) -> FastRand {
            let one = (seed >> 32) as u32;
            let mut two = seed as u32;

            if two == 0 {
                // This value cannot be zero
                two = 1;
            }

            FastRand {
                one: Cell::new(one),
                two: Cell::new(two),
            }
        }

        pub(crate) fn fastrand_n(&self, n: u32) -> u32 {
            // This is similar to fastrand() % n, but faster.
            // See https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
            let mul = (self.fastrand() as u64).wrapping_mul(n as u64);
            (mul >> 32) as u32
        }

        fn fastrand(&self) -> u32 {
            let mut s1 = self.one.get();
            let s0 = self.two.get();

            s1 ^= s1 << 17;
            s1 = s1 ^ s0 ^ s1 >> 7 ^ s0 >> 16;

            self.one.set(s0);
            self.two.set(s1);

            s0.wrapping_add(s1)
        }
    }

    // Used by `StreamMap`
    pub(crate) fn thread_rng_n(n: u32) -> u32 {
        thread_local! {
            static THREAD_RNG: FastRand = FastRand::new(loom::rand::seed());
        }

        THREAD_RNG.with(|rng| rng.fastrand_n(n))
    }
}
