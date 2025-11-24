use crate::raw::{
    Allocator, Bucket, Global, RawDrain, RawExtractIf, RawIntoIter, RawIter, RawTable,
};
use crate::{Equivalent, TryReserveError};
use core::borrow::Borrow;
use core::fmt::{self, Debug};
use core::hash::{BuildHasher, Hash};
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::mem;
use core::ops::Index;

/// Default hasher for `HashMap`.
#[cfg(feature = "ahash")]
pub type DefaultHashBuilder = core::hash::BuildHasherDefault<ahash::AHasher>;

/// Dummy default hasher for `HashMap`.
#[cfg(not(feature = "ahash"))]
pub enum DefaultHashBuilder {}

/// A hash map implemented with quadratic probing and SIMD lookup.
///
/// The default hashing algorithm is currently [`AHash`], though this is
/// subject to change at any point in the future. This hash function is very
/// fast for all types of keys, but this algorithm will typically *not* protect
/// against attacks such as HashDoS.
///
/// The hashing algorithm can be replaced on a per-`HashMap` basis using the
/// [`default`], [`with_hasher`], and [`with_capacity_and_hasher`] methods. Many
/// alternative algorithms are available on crates.io, such as the [`fnv`] crate.
///
/// It is required that the keys implement the [`Eq`] and [`Hash`] traits, although
/// this can frequently be achieved by using `#[derive(PartialEq, Eq, Hash)]`.
/// If you implement these yourself, it is important that the following
/// property holds:
///
/// ```text
/// k1 == k2 -> hash(k1) == hash(k2)
/// ```
///
/// In other words, if two keys are equal, their hashes must be equal.
///
/// It is a logic error for a key to be modified in such a way that the key's
/// hash, as determined by the [`Hash`] trait, or its equality, as determined by
/// the [`Eq`] trait, changes while it is in the map. This is normally only
/// possible through [`Cell`], [`RefCell`], global state, I/O, or unsafe code.
///
/// It is also a logic error for the [`Hash`] implementation of a key to panic.
/// This is generally only possible if the trait is implemented manually. If a
/// panic does occur then the contents of the `HashMap` may become corrupted and
/// some items may be dropped from the table.
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// // Type inference lets us omit an explicit type signature (which
/// // would be `HashMap<String, String>` in this example).
/// let mut book_reviews = HashMap::new();
///
/// // Review some books.
/// book_reviews.insert(
///     "Adventures of Huckleberry Finn".to_string(),
///     "My favorite book.".to_string(),
/// );
/// book_reviews.insert(
///     "Grimms' Fairy Tales".to_string(),
///     "Masterpiece.".to_string(),
/// );
/// book_reviews.insert(
///     "Pride and Prejudice".to_string(),
///     "Very enjoyable.".to_string(),
/// );
/// book_reviews.insert(
///     "The Adventures of Sherlock Holmes".to_string(),
///     "Eye lyked it alot.".to_string(),
/// );
///
/// // Check for a specific one.
/// // When collections store owned values (String), they can still be
/// // queried using references (&str).
/// if !book_reviews.contains_key("Les Misérables") {
///     println!("We've got {} reviews, but Les Misérables ain't one.",
///              book_reviews.len());
/// }
///
/// // oops, this review has a lot of spelling mistakes, let's delete it.
/// book_reviews.remove("The Adventures of Sherlock Holmes");
///
/// // Look up the values associated with some keys.
/// let to_find = ["Pride and Prejudice", "Alice's Adventure in Wonderland"];
/// for &book in &to_find {
///     match book_reviews.get(book) {
///         Some(review) => println!("{}: {}", book, review),
///         None => println!("{} is unreviewed.", book)
///     }
/// }
///
/// // Look up the value for a key (will panic if the key is not found).
/// println!("Review for Jane: {}", book_reviews["Pride and Prejudice"]);
///
/// // Iterate over everything.
/// for (book, review) in &book_reviews {
///     println!("{}: \"{}\"", book, review);
/// }
/// ```
///
/// `HashMap` also implements an [`Entry API`](#method.entry), which allows
/// for more complex methods of getting, setting, updating and removing keys and
/// their values:
///
/// ```
/// use hashbrown::HashMap;
///
/// // type inference lets us omit an explicit type signature (which
/// // would be `HashMap<&str, u8>` in this example).
/// let mut player_stats = HashMap::new();
///
/// fn random_stat_buff() -> u8 {
///     // could actually return some random value here - let's just return
///     // some fixed value for now
///     42
/// }
///
/// // insert a key only if it doesn't already exist
/// player_stats.entry("health").or_insert(100);
///
/// // insert a key using a function that provides a new value only if it
/// // doesn't already exist
/// player_stats.entry("defence").or_insert_with(random_stat_buff);
///
/// // update a key, guarding against the key possibly not being set
/// let stat = player_stats.entry("attack").or_insert(100);
/// *stat += random_stat_buff();
/// ```
///
/// The easiest way to use `HashMap` with a custom key type is to derive [`Eq`] and [`Hash`].
/// We must also derive [`PartialEq`].
///
/// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
/// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
/// [`PartialEq`]: https://doc.rust-lang.org/std/cmp/trait.PartialEq.html
/// [`RefCell`]: https://doc.rust-lang.org/std/cell/struct.RefCell.html
/// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
/// [`default`]: #method.default
/// [`with_hasher`]: #method.with_hasher
/// [`with_capacity_and_hasher`]: #method.with_capacity_and_hasher
/// [`fnv`]: https://crates.io/crates/fnv
/// [`AHash`]: https://crates.io/crates/ahash
///
/// ```
/// use hashbrown::HashMap;
///
/// #[derive(Hash, Eq, PartialEq, Debug)]
/// struct Viking {
///     name: String,
///     country: String,
/// }
///
/// impl Viking {
///     /// Creates a new Viking.
///     fn new(name: &str, country: &str) -> Viking {
///         Viking { name: name.to_string(), country: country.to_string() }
///     }
/// }
///
/// // Use a HashMap to store the vikings' health points.
/// let mut vikings = HashMap::new();
///
/// vikings.insert(Viking::new("Einar", "Norway"), 25);
/// vikings.insert(Viking::new("Olaf", "Denmark"), 24);
/// vikings.insert(Viking::new("Harald", "Iceland"), 12);
///
/// // Use derived implementation to print the status of the vikings.
/// for (viking, health) in &vikings {
///     println!("{:?} has {} hp", viking, health);
/// }
/// ```
///
/// A `HashMap` with fixed list of elements can be initialized from an array:
///
/// ```
/// use hashbrown::HashMap;
///
/// let timber_resources: HashMap<&str, i32> = [("Norway", 100), ("Denmark", 50), ("Iceland", 10)]
///     .into_iter().collect();
/// // use the values stored in map
/// ```
pub struct HashMap<K, V, S = DefaultHashBuilder, A: Allocator = Global> {
    pub(crate) hash_builder: S,
    pub(crate) table: RawTable<(K, V), A>,
}

impl<K: Clone, V: Clone, S: Clone, A: Allocator + Clone> Clone for HashMap<K, V, S, A> {
    fn clone(&self) -> Self {
        HashMap {
            hash_builder: self.hash_builder.clone(),
            table: self.table.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.table.clone_from(&source.table);

        // Update hash_builder only if we successfully cloned all elements.
        self.hash_builder.clone_from(&source.hash_builder);
    }
}

/// Ensures that a single closure type across uses of this which, in turn prevents multiple
/// instances of any functions like RawTable::reserve from being generated
#[cfg_attr(feature = "inline-more", inline)]
pub(crate) fn make_hasher<Q, V, S>(hash_builder: &S) -> impl Fn(&(Q, V)) -> u64 + '_
where
    Q: Hash,
    S: BuildHasher,
{
    move |val| make_hash::<Q, S>(hash_builder, &val.0)
}

/// Ensures that a single closure type across uses of this which, in turn prevents multiple
/// instances of any functions like RawTable::reserve from being generated
#[cfg_attr(feature = "inline-more", inline)]
fn equivalent_key<Q, K, V>(k: &Q) -> impl Fn(&(K, V)) -> bool + '_
where
    Q: ?Sized + Equivalent<K>,
{
    move |x| k.equivalent(&x.0)
}

/// Ensures that a single closure type across uses of this which, in turn prevents multiple
/// instances of any functions like RawTable::reserve from being generated
#[cfg_attr(feature = "inline-more", inline)]
fn equivalent<Q, K>(k: &Q) -> impl Fn(&K) -> bool + '_
where
    Q: ?Sized + Equivalent<K>,
{
    move |x| k.equivalent(x)
}

#[cfg(not(feature = "nightly"))]
#[cfg_attr(feature = "inline-more", inline)]
pub(crate) fn make_hash<Q, S>(hash_builder: &S, val: &Q) -> u64
where
    Q: Hash + ?Sized,
    S: BuildHasher,
{
    use core::hash::Hasher;
    let mut state = hash_builder.build_hasher();
    val.hash(&mut state);
    state.finish()
}

#[cfg(feature = "nightly")]
#[cfg_attr(feature = "inline-more", inline)]
pub(crate) fn make_hash<Q, S>(hash_builder: &S, val: &Q) -> u64
where
    Q: Hash + ?Sized,
    S: BuildHasher,
{
    hash_builder.hash_one(val)
}

#[cfg(feature = "ahash")]
impl<K, V> HashMap<K, V, DefaultHashBuilder> {
    /// Creates an empty `HashMap`.
    ///
    /// The hash map is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`], for example with
    /// [`with_hasher`](HashMap::with_hasher) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// assert_eq!(map.len(), 0);
    /// assert_eq!(map.capacity(), 0);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty `HashMap` with the specified capacity.
    ///
    /// The hash map will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash map will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`], for example with
    /// [`with_capacity_and_hasher`](HashMap::with_capacity_and_hasher) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::with_capacity(10);
    /// assert_eq!(map.len(), 0);
    /// assert!(map.capacity() >= 10);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, DefaultHashBuilder::default())
    }
}

#[cfg(feature = "ahash")]
impl<K, V, A: Allocator> HashMap<K, V, DefaultHashBuilder, A> {
    /// Creates an empty `HashMap` using the given allocator.
    ///
    /// The hash map is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`], for example with
    /// [`with_hasher_in`](HashMap::with_hasher_in) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use bumpalo::Bump;
    ///
    /// let bump = Bump::new();
    /// let mut map = HashMap::new_in(&bump);
    ///
    /// // The created HashMap holds none elements
    /// assert_eq!(map.len(), 0);
    ///
    /// // The created HashMap also doesn't allocate memory
    /// assert_eq!(map.capacity(), 0);
    ///
    /// // Now we insert element inside created HashMap
    /// map.insert("One", 1);
    /// // We can see that the HashMap holds 1 element
    /// assert_eq!(map.len(), 1);
    /// // And it also allocates some capacity
    /// assert!(map.capacity() > 1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn new_in(alloc: A) -> Self {
        Self::with_hasher_in(DefaultHashBuilder::default(), alloc)
    }

    /// Creates an empty `HashMap` with the specified capacity using the given allocator.
    ///
    /// The hash map will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash map will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`], for example with
    /// [`with_capacity_and_hasher_in`](HashMap::with_capacity_and_hasher_in) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use bumpalo::Bump;
    ///
    /// let bump = Bump::new();
    /// let mut map = HashMap::with_capacity_in(5, &bump);
    ///
    /// // The created HashMap holds none elements
    /// assert_eq!(map.len(), 0);
    /// // But it can hold at least 5 elements without reallocating
    /// let empty_map_capacity = map.capacity();
    /// assert!(empty_map_capacity >= 5);
    ///
    /// // Now we insert some 5 elements inside created HashMap
    /// map.insert("One",   1);
    /// map.insert("Two",   2);
    /// map.insert("Three", 3);
    /// map.insert("Four",  4);
    /// map.insert("Five",  5);
    ///
    /// // We can see that the HashMap holds 5 elements
    /// assert_eq!(map.len(), 5);
    /// // But its capacity isn't changed
    /// assert_eq!(map.capacity(), empty_map_capacity)
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self::with_capacity_and_hasher_in(capacity, DefaultHashBuilder::default(), alloc)
    }
}

impl<K, V, S> HashMap<K, V, S> {
    /// Creates an empty `HashMap` which will use the given hash builder to hash
    /// keys.
    ///
    /// The hash map is initially created with a capacity of 0, so it will not
    /// allocate until it is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`].
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashMap to be useful, see its documentation for details.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut map = HashMap::with_hasher(s);
    /// assert_eq!(map.len(), 0);
    /// assert_eq!(map.capacity(), 0);
    ///
    /// map.insert(1, 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub const fn with_hasher(hash_builder: S) -> Self {
        Self {
            hash_builder,
            table: RawTable::new(),
        }
    }

    /// Creates an empty `HashMap` with the specified capacity, using `hash_builder`
    /// to hash the keys.
    ///
    /// The hash map will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash map will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`].
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashMap to be useful, see its documentation for details.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut map = HashMap::with_capacity_and_hasher(10, s);
    /// assert_eq!(map.len(), 0);
    /// assert!(map.capacity() >= 10);
    ///
    /// map.insert(1, 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        Self {
            hash_builder,
            table: RawTable::with_capacity(capacity),
        }
    }
}

impl<K, V, S, A: Allocator> HashMap<K, V, S, A> {
    /// Returns a reference to the underlying allocator.
    #[inline]
    pub fn allocator(&self) -> &A {
        self.table.allocator()
    }

    /// Creates an empty `HashMap` which will use the given hash builder to hash
    /// keys. It will be allocated with the given allocator.
    ///
    /// The hash map is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`].
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut map = HashMap::with_hasher(s);
    /// map.insert(1, 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub const fn with_hasher_in(hash_builder: S, alloc: A) -> Self {
        Self {
            hash_builder,
            table: RawTable::new_in(alloc),
        }
    }

    /// Creates an empty `HashMap` with the specified capacity, using `hash_builder`
    /// to hash the keys. It will be allocated with the given allocator.
    ///
    /// The hash map will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash map will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashMap` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashMap`].
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut map = HashMap::with_capacity_and_hasher(10, s);
    /// map.insert(1, 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity_and_hasher_in(capacity: usize, hash_builder: S, alloc: A) -> Self {
        Self {
            hash_builder,
            table: RawTable::with_capacity_in(capacity, alloc),
        }
    }

    /// Returns a reference to the map's [`BuildHasher`].
    ///
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let hasher = DefaultHashBuilder::default();
    /// let map: HashMap<i32, i32> = HashMap::with_hasher(hasher);
    /// let hasher: &DefaultHashBuilder = map.hasher();
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn hasher(&self) -> &S {
        &self.hash_builder
    }

    /// Returns the number of elements the map can hold without reallocating.
    ///
    /// This number is a lower bound; the `HashMap<K, V>` might be able to hold
    /// more, but is guaranteed to be able to hold at least this many.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// let map: HashMap<i32, i32> = HashMap::with_capacity(100);
    /// assert_eq!(map.len(), 0);
    /// assert!(map.capacity() >= 100);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }

    /// An iterator visiting all keys in arbitrary order.
    /// The iterator element type is `&'a K`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    /// assert_eq!(map.len(), 3);
    /// let mut vec: Vec<&str> = Vec::new();
    ///
    /// for key in map.keys() {
    ///     println!("{}", key);
    ///     vec.push(*key);
    /// }
    ///
    /// // The `Keys` iterator produces keys in arbitrary order, so the
    /// // keys must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, ["a", "b", "c"]);
    ///
    /// assert_eq!(map.len(), 3);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys { inner: self.iter() }
    }

    /// An iterator visiting all values in arbitrary order.
    /// The iterator element type is `&'a V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    /// assert_eq!(map.len(), 3);
    /// let mut vec: Vec<i32> = Vec::new();
    ///
    /// for val in map.values() {
    ///     println!("{}", val);
    ///     vec.push(*val);
    /// }
    ///
    /// // The `Values` iterator produces values in arbitrary order, so the
    /// // values must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [1, 2, 3]);
    ///
    /// assert_eq!(map.len(), 3);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn values(&self) -> Values<'_, K, V> {
        Values { inner: self.iter() }
    }

    /// An iterator visiting all values mutably in arbitrary order.
    /// The iterator element type is `&'a mut V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    ///
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    ///
    /// for val in map.values_mut() {
    ///     *val = *val + 10;
    /// }
    ///
    /// assert_eq!(map.len(), 3);
    /// let mut vec: Vec<i32> = Vec::new();
    ///
    /// for val in map.values() {
    ///     println!("{}", val);
    ///     vec.push(*val);
    /// }
    ///
    /// // The `Values` iterator produces values in arbitrary order, so the
    /// // values must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [11, 12, 13]);
    ///
    /// assert_eq!(map.len(), 3);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut {
            inner: self.iter_mut(),
        }
    }

    /// An iterator visiting all key-value pairs in arbitrary order.
    /// The iterator element type is `(&'a K, &'a V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    /// assert_eq!(map.len(), 3);
    /// let mut vec: Vec<(&str, i32)> = Vec::new();
    ///
    /// for (key, val) in map.iter() {
    ///     println!("key: {} val: {}", key, val);
    ///     vec.push((*key, *val));
    /// }
    ///
    /// // The `Iter` iterator produces items in arbitrary order, so the
    /// // items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [("a", 1), ("b", 2), ("c", 3)]);
    ///
    /// assert_eq!(map.len(), 3);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn iter(&self) -> Iter<'_, K, V> {
        // Here we tie the lifetime of self to the iter.
        unsafe {
            Iter {
                inner: self.table.iter(),
                marker: PhantomData,
            }
        }
    }

    /// An iterator visiting all key-value pairs in arbitrary order,
    /// with mutable references to the values.
    /// The iterator element type is `(&'a K, &'a mut V)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    ///
    /// // Update all values
    /// for (_, val) in map.iter_mut() {
    ///     *val *= 2;
    /// }
    ///
    /// assert_eq!(map.len(), 3);
    /// let mut vec: Vec<(&str, i32)> = Vec::new();
    ///
    /// for (key, val) in &map {
    ///     println!("key: {} val: {}", key, val);
    ///     vec.push((*key, *val));
    /// }
    ///
    /// // The `Iter` iterator produces items in arbitrary order, so the
    /// // items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [("a", 2), ("b", 4), ("c", 6)]);
    ///
    /// assert_eq!(map.len(), 3);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        // Here we tie the lifetime of self to the iter.
        unsafe {
            IterMut {
                inner: self.table.iter(),
                marker: PhantomData,
            }
        }
    }

    #[cfg(test)]
    #[cfg_attr(feature = "inline-more", inline)]
    fn raw_capacity(&self) -> usize {
        self.table.buckets()
    }

    /// Returns the number of elements in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// assert_eq!(a.len(), 0);
    /// a.insert(1, "a");
    /// assert_eq!(a.len(), 1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Returns `true` if the map contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// assert!(a.is_empty());
    /// a.insert(1, "a");
    /// assert!(!a.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears the map, returning all key-value pairs as an iterator. Keeps the
    /// allocated memory for reuse.
    ///
    /// If the returned iterator is dropped before being fully consumed, it
    /// drops the remaining key-value pairs. The returned iterator keeps a
    /// mutable borrow on the vector to optimize its implementation.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// a.insert(1, "a");
    /// a.insert(2, "b");
    /// let capacity_before_drain = a.capacity();
    ///
    /// for (k, v) in a.drain().take(1) {
    ///     assert!(k == 1 || k == 2);
    ///     assert!(v == "a" || v == "b");
    /// }
    ///
    /// // As we can see, the map is empty and contains no element.
    /// assert!(a.is_empty() && a.len() == 0);
    /// // But map capacity is equal to old one.
    /// assert_eq!(a.capacity(), capacity_before_drain);
    ///
    /// let mut a = HashMap::new();
    /// a.insert(1, "a");
    /// a.insert(2, "b");
    ///
    /// {   // Iterator is dropped without being consumed.
    ///     let d = a.drain();
    /// }
    ///
    /// // But the map is empty even if we do not use Drain iterator.
    /// assert!(a.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn drain(&mut self) -> Drain<'_, K, V, A> {
        Drain {
            inner: self.table.drain(),
        }
    }

    /// Retains only the elements specified by the predicate. Keeps the
    /// allocated memory for reuse.
    ///
    /// In other words, remove all pairs `(k, v)` such that `f(&k, &mut v)` returns `false`.
    /// The elements are visited in unsorted (and unspecified) order.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = (0..8).map(|x|(x, x*10)).collect();
    /// assert_eq!(map.len(), 8);
    ///
    /// map.retain(|&k, _| k % 2 == 0);
    ///
    /// // We can see, that the number of elements inside map is changed.
    /// assert_eq!(map.len(), 4);
    ///
    /// let mut vec: Vec<(i32, i32)> = map.iter().map(|(&k, &v)| (k, v)).collect();
    /// vec.sort_unstable();
    /// assert_eq!(vec, [(0, 0), (2, 20), (4, 40), (6, 60)]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        // Here we only use `iter` as a temporary, preventing use-after-free
        unsafe {
            for item in self.table.iter() {
                let &mut (ref key, ref mut value) = item.as_mut();
                if !f(key, value) {
                    self.table.erase(item);
                }
            }
        }
    }

    /// Drains elements which are true under the given predicate,
    /// and returns an iterator over the removed items.
    ///
    /// In other words, move all pairs `(k, v)` such that `f(&k, &mut v)` returns `true` out
    /// into another iterator.
    ///
    /// Note that `extract_if` lets you mutate every value in the filter closure, regardless of
    /// whether you choose to keep or remove it.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`retain()`] with a negated predicate if you do not need the returned iterator.
    ///
    /// Keeps the allocated memory for reuse.
    ///
    /// [`retain()`]: HashMap::retain
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = (0..8).map(|x| (x, x)).collect();
    ///
    /// let drained: HashMap<i32, i32> = map.extract_if(|k, _v| k % 2 == 0).collect();
    ///
    /// let mut evens = drained.keys().cloned().collect::<Vec<_>>();
    /// let mut odds = map.keys().cloned().collect::<Vec<_>>();
    /// evens.sort();
    /// odds.sort();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    ///
    /// let mut map: HashMap<i32, i32> = (0..8).map(|x| (x, x)).collect();
    ///
    /// {   // Iterator is dropped without being consumed.
    ///     let d = map.extract_if(|k, _v| k % 2 != 0);
    /// }
    ///
    /// // ExtractIf was not exhausted, therefore no elements were drained.
    /// assert_eq!(map.len(), 8);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn extract_if<F>(&mut self, f: F) -> ExtractIf<'_, K, V, F, A>
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        ExtractIf {
            f,
            inner: RawExtractIf {
                iter: unsafe { self.table.iter() },
                table: &mut self.table,
            },
        }
    }

    /// Clears the map, removing all key-value pairs. Keeps the allocated memory
    /// for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// a.insert(1, "a");
    /// let capacity_before_clear = a.capacity();
    ///
    /// a.clear();
    ///
    /// // Map is empty.
    /// assert!(a.is_empty());
    /// // But map capacity is equal to old one.
    /// assert_eq!(a.capacity(), capacity_before_clear);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn clear(&mut self) {
        self.table.clear();
    }

    /// Creates a consuming iterator visiting all the keys in arbitrary order.
    /// The map cannot be used after calling this.
    /// The iterator element type is `K`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    ///
    /// let mut vec: Vec<&str> = map.into_keys().collect();
    ///
    /// // The `IntoKeys` iterator produces keys in arbitrary order, so the
    /// // keys must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, ["a", "b", "c"]);
    /// ```
    #[inline]
    pub fn into_keys(self) -> IntoKeys<K, V, A> {
        IntoKeys {
            inner: self.into_iter(),
        }
    }

    /// Creates a consuming iterator visiting all the values in arbitrary order.
    /// The map cannot be used after calling this.
    /// The iterator element type is `V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("a", 1);
    /// map.insert("b", 2);
    /// map.insert("c", 3);
    ///
    /// let mut vec: Vec<i32> = map.into_values().collect();
    ///
    /// // The `IntoValues` iterator produces values in arbitrary order, so
    /// // the values must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    #[inline]
    pub fn into_values(self) -> IntoValues<K, V, A> {
        IntoValues {
            inner: self.into_iter(),
        }
    }
}

impl<K, V, S, A> HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    /// Reserves capacity for at least `additional` more elements to be inserted
    /// in the `HashMap`. The collection may reserve more space to avoid
    /// frequent reallocations.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds [`isize::MAX`] bytes and [`abort`] the program
    /// in case of allocation error. Use [`try_reserve`](HashMap::try_reserve) instead
    /// if you want to handle memory allocation failure.
    ///
    /// [`isize::MAX`]: https://doc.rust-lang.org/std/primitive.isize.html
    /// [`abort`]: https://doc.rust-lang.org/alloc/alloc/fn.handle_alloc_error.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// // Map is empty and doesn't allocate memory
    /// assert_eq!(map.capacity(), 0);
    ///
    /// map.reserve(10);
    ///
    /// // And now map can hold at least 10 elements
    /// assert!(map.capacity() >= 10);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn reserve(&mut self, additional: usize) {
        self.table
            .reserve(additional, make_hasher::<_, V, S>(&self.hash_builder));
    }

    /// Tries to reserve capacity for at least `additional` more elements to be inserted
    /// in the given `HashMap<K,V>`. The collection may reserve more space to avoid
    /// frequent reallocations.
    ///
    /// # Errors
    ///
    /// If the capacity overflows, or the allocator reports a failure, then an error
    /// is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, isize> = HashMap::new();
    /// // Map is empty and doesn't allocate memory
    /// assert_eq!(map.capacity(), 0);
    ///
    /// map.try_reserve(10).expect("why is the test harness OOMing on 10 bytes?");
    ///
    /// // And now map can hold at least 10 elements
    /// assert!(map.capacity() >= 10);
    /// ```
    /// If the capacity overflows, or the allocator reports a failure, then an error
    /// is returned:
    /// ```
    /// # fn test() {
    /// use hashbrown::HashMap;
    /// use hashbrown::TryReserveError;
    /// let mut map: HashMap<i32, i32> = HashMap::new();
    ///
    /// match map.try_reserve(usize::MAX) {
    ///     Err(error) => match error {
    ///         TryReserveError::CapacityOverflow => {}
    ///         _ => panic!("TryReserveError::AllocError ?"),
    ///     },
    ///     _ => panic!(),
    /// }
    /// # }
    /// # fn main() {
    /// #     #[cfg(not(miri))]
    /// #     test()
    /// # }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.table
            .try_reserve(additional, make_hasher::<_, V, S>(&self.hash_builder))
    }

    /// Shrinks the capacity of the map as much as possible. It will drop
    /// down as much as possible while maintaining the internal rules
    /// and possibly leaving some space in accordance with the resize policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = HashMap::with_capacity(100);
    /// map.insert(1, 2);
    /// map.insert(3, 4);
    /// assert!(map.capacity() >= 100);
    /// map.shrink_to_fit();
    /// assert!(map.capacity() >= 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn shrink_to_fit(&mut self) {
        self.table
            .shrink_to(0, make_hasher::<_, V, S>(&self.hash_builder));
    }

    /// Shrinks the capacity of the map with a lower limit. It will drop
    /// down no lower than the supplied limit while maintaining the internal rules
    /// and possibly leaving some space in accordance with the resize policy.
    ///
    /// This function does nothing if the current capacity is smaller than the
    /// supplied minimum capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = HashMap::with_capacity(100);
    /// map.insert(1, 2);
    /// map.insert(3, 4);
    /// assert!(map.capacity() >= 100);
    /// map.shrink_to(10);
    /// assert!(map.capacity() >= 10);
    /// map.shrink_to(0);
    /// assert!(map.capacity() >= 2);
    /// map.shrink_to(10);
    /// assert!(map.capacity() >= 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.table
            .shrink_to(min_capacity, make_hasher::<_, V, S>(&self.hash_builder));
    }

    /// Gets the given key's corresponding entry in the map for in-place manipulation.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut letters = HashMap::new();
    ///
    /// for ch in "a short treatise on fungi".chars() {
    ///     let counter = letters.entry(ch).or_insert(0);
    ///     *counter += 1;
    /// }
    ///
    /// assert_eq!(letters[&'s'], 2);
    /// assert_eq!(letters[&'t'], 3);
    /// assert_eq!(letters[&'u'], 1);
    /// assert_eq!(letters.get(&'y'), None);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, S, A> {
        let hash = make_hash::<K, S>(&self.hash_builder, &key);
        if let Some(elem) = self.table.find(hash, equivalent_key(&key)) {
            Entry::Occupied(OccupiedEntry {
                hash,
                key: Some(key),
                elem,
                table: self,
            })
        } else {
            Entry::Vacant(VacantEntry {
                hash,
                key,
                table: self,
            })
        }
    }

    /// Gets the given key's corresponding entry by reference in the map for in-place manipulation.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut words: HashMap<String, usize> = HashMap::new();
    /// let source = ["poneyland", "horseyland", "poneyland", "poneyland"];
    /// for (i, &s) in source.iter().enumerate() {
    ///     let counter = words.entry_ref(s).or_insert(0);
    ///     *counter += 1;
    /// }
    ///
    /// assert_eq!(words["poneyland"], 3);
    /// assert_eq!(words["horseyland"], 1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn entry_ref<'a, 'b, Q: ?Sized>(&'a mut self, key: &'b Q) -> EntryRef<'a, 'b, K, Q, V, S, A>
    where
        Q: Hash + Equivalent<K>,
    {
        let hash = make_hash::<Q, S>(&self.hash_builder, key);
        if let Some(elem) = self.table.find(hash, equivalent_key(key)) {
            EntryRef::Occupied(OccupiedEntryRef {
                hash,
                key: Some(KeyOrRef::Borrowed(key)),
                elem,
                table: self,
            })
        } else {
            EntryRef::Vacant(VacantEntryRef {
                hash,
                key: KeyOrRef::Borrowed(key),
                table: self,
            })
        }
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.get(&1), Some(&"a"));
    /// assert_eq!(map.get(&2), None);
    /// ```
    #[inline]
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        Q: Hash + Equivalent<K>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.get_inner(k) {
            Some((_, v)) => Some(v),
            None => None,
        }
    }

    /// Returns the key-value pair corresponding to the supplied key.
    ///
    /// The supplied key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.get_key_value(&1), Some((&1, &"a")));
    /// assert_eq!(map.get_key_value(&2), None);
    /// ```
    #[inline]
    pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&K, &V)>
    where
        Q: Hash + Equivalent<K>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.get_inner(k) {
            Some((key, value)) => Some((key, value)),
            None => None,
        }
    }

    #[inline]
    fn get_inner<Q: ?Sized>(&self, k: &Q) -> Option<&(K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if self.table.is_empty() {
            None
        } else {
            let hash = make_hash::<Q, S>(&self.hash_builder, k);
            self.table.get(hash, equivalent_key(k))
        }
    }

    /// Returns the key-value pair corresponding to the supplied key, with a mutable reference to value.
    ///
    /// The supplied key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// let (k, v) = map.get_key_value_mut(&1).unwrap();
    /// assert_eq!(k, &1);
    /// assert_eq!(v, &mut "a");
    /// *v = "b";
    /// assert_eq!(map.get_key_value_mut(&1), Some((&1, &mut "b")));
    /// assert_eq!(map.get_key_value_mut(&2), None);
    /// ```
    #[inline]
    pub fn get_key_value_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<(&K, &mut V)>
    where
        Q: Hash + Equivalent<K>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.get_inner_mut(k) {
            Some(&mut (ref key, ref mut value)) => Some((key, value)),
            None => None,
        }
    }

    /// Returns `true` if the map contains a value for the specified key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.contains_key(&1), true);
    /// assert_eq!(map.contains_key(&2), false);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_inner(k).is_some()
    }

    /// Returns a mutable reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// if let Some(x) = map.get_mut(&1) {
    ///     *x = "b";
    /// }
    /// assert_eq!(map[&1], "b");
    ///
    /// assert_eq!(map.get_mut(&2), None);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        Q: Hash + Equivalent<K>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.get_inner_mut(k) {
            Some(&mut (_, ref mut v)) => Some(v),
            None => None,
        }
    }

    #[inline]
    fn get_inner_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut (K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if self.table.is_empty() {
            None
        } else {
            let hash = make_hash::<Q, S>(&self.hash_builder, k);
            self.table.get_mut(hash, equivalent_key(k))
        }
    }

    /// Attempts to get mutable references to `N` values in the map at once.
    ///
    /// Returns an array of length `N` with the results of each query. For soundness, at most one
    /// mutable reference will be returned to any value. `None` will be returned if any of the
    /// keys are duplicates or missing.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Bodleian Library".to_string(), 1602);
    /// libraries.insert("Athenæum".to_string(), 1807);
    /// libraries.insert("Herzogin-Anna-Amalia-Bibliothek".to_string(), 1691);
    /// libraries.insert("Library of Congress".to_string(), 1800);
    ///
    /// let got = libraries.get_many_mut([
    ///     "Athenæum",
    ///     "Library of Congress",
    /// ]);
    /// assert_eq!(
    ///     got,
    ///     Some([
    ///         &mut 1807,
    ///         &mut 1800,
    ///     ]),
    /// );
    ///
    /// // Missing keys result in None
    /// let got = libraries.get_many_mut([
    ///     "Athenæum",
    ///     "New York Public Library",
    /// ]);
    /// assert_eq!(got, None);
    ///
    /// // Duplicate keys result in None
    /// let got = libraries.get_many_mut([
    ///     "Athenæum",
    ///     "Athenæum",
    /// ]);
    /// assert_eq!(got, None);
    /// ```
    pub fn get_many_mut<Q: ?Sized, const N: usize>(&mut self, ks: [&Q; N]) -> Option<[&'_ mut V; N]>
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_many_mut_inner(ks).map(|res| res.map(|(_, v)| v))
    }

    /// Attempts to get mutable references to `N` values in the map at once, without validating that
    /// the values are unique.
    ///
    /// Returns an array of length `N` with the results of each query. `None` will be returned if
    /// any of the keys are missing.
    ///
    /// For a safe alternative see [`get_many_mut`](`HashMap::get_many_mut`).
    ///
    /// # Safety
    ///
    /// Calling this method with overlapping keys is *[undefined behavior]* even if the resulting
    /// references are not used.
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Bodleian Library".to_string(), 1602);
    /// libraries.insert("Athenæum".to_string(), 1807);
    /// libraries.insert("Herzogin-Anna-Amalia-Bibliothek".to_string(), 1691);
    /// libraries.insert("Library of Congress".to_string(), 1800);
    ///
    /// let got = libraries.get_many_mut([
    ///     "Athenæum",
    ///     "Library of Congress",
    /// ]);
    /// assert_eq!(
    ///     got,
    ///     Some([
    ///         &mut 1807,
    ///         &mut 1800,
    ///     ]),
    /// );
    ///
    /// // Missing keys result in None
    /// let got = libraries.get_many_mut([
    ///     "Athenæum",
    ///     "New York Public Library",
    /// ]);
    /// assert_eq!(got, None);
    /// ```
    pub unsafe fn get_many_unchecked_mut<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> Option<[&'_ mut V; N]>
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_many_unchecked_mut_inner(ks)
            .map(|res| res.map(|(_, v)| v))
    }

    /// Attempts to get mutable references to `N` values in the map at once, with immutable
    /// references to the corresponding keys.
    ///
    /// Returns an array of length `N` with the results of each query. For soundness, at most one
    /// mutable reference will be returned to any value. `None` will be returned if any of the keys
    /// are duplicates or missing.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Bodleian Library".to_string(), 1602);
    /// libraries.insert("Athenæum".to_string(), 1807);
    /// libraries.insert("Herzogin-Anna-Amalia-Bibliothek".to_string(), 1691);
    /// libraries.insert("Library of Congress".to_string(), 1800);
    ///
    /// let got = libraries.get_many_key_value_mut([
    ///     "Bodleian Library",
    ///     "Herzogin-Anna-Amalia-Bibliothek",
    /// ]);
    /// assert_eq!(
    ///     got,
    ///     Some([
    ///         (&"Bodleian Library".to_string(), &mut 1602),
    ///         (&"Herzogin-Anna-Amalia-Bibliothek".to_string(), &mut 1691),
    ///     ]),
    /// );
    /// // Missing keys result in None
    /// let got = libraries.get_many_key_value_mut([
    ///     "Bodleian Library",
    ///     "Gewandhaus",
    /// ]);
    /// assert_eq!(got, None);
    ///
    /// // Duplicate keys result in None
    /// let got = libraries.get_many_key_value_mut([
    ///     "Bodleian Library",
    ///     "Herzogin-Anna-Amalia-Bibliothek",
    ///     "Herzogin-Anna-Amalia-Bibliothek",
    /// ]);
    /// assert_eq!(got, None);
    /// ```
    pub fn get_many_key_value_mut<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> Option<[(&'_ K, &'_ mut V); N]>
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_many_mut_inner(ks)
            .map(|res| res.map(|(k, v)| (&*k, v)))
    }

    /// Attempts to get mutable references to `N` values in the map at once, with immutable
    /// references to the corresponding keys, without validating that the values are unique.
    ///
    /// Returns an array of length `N` with the results of each query. `None` will be returned if
    /// any of the keys are missing.
    ///
    /// For a safe alternative see [`get_many_key_value_mut`](`HashMap::get_many_key_value_mut`).
    ///
    /// # Safety
    ///
    /// Calling this method with overlapping keys is *[undefined behavior]* even if the resulting
    /// references are not used.
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Bodleian Library".to_string(), 1602);
    /// libraries.insert("Athenæum".to_string(), 1807);
    /// libraries.insert("Herzogin-Anna-Amalia-Bibliothek".to_string(), 1691);
    /// libraries.insert("Library of Congress".to_string(), 1800);
    ///
    /// let got = libraries.get_many_key_value_mut([
    ///     "Bodleian Library",
    ///     "Herzogin-Anna-Amalia-Bibliothek",
    /// ]);
    /// assert_eq!(
    ///     got,
    ///     Some([
    ///         (&"Bodleian Library".to_string(), &mut 1602),
    ///         (&"Herzogin-Anna-Amalia-Bibliothek".to_string(), &mut 1691),
    ///     ]),
    /// );
    /// // Missing keys result in None
    /// let got = libraries.get_many_key_value_mut([
    ///     "Bodleian Library",
    ///     "Gewandhaus",
    /// ]);
    /// assert_eq!(got, None);
    /// ```
    pub unsafe fn get_many_key_value_unchecked_mut<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> Option<[(&'_ K, &'_ mut V); N]>
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_many_unchecked_mut_inner(ks)
            .map(|res| res.map(|(k, v)| (&*k, v)))
    }

    fn get_many_mut_inner<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> Option<[&'_ mut (K, V); N]>
    where
        Q: Hash + Equivalent<K>,
    {
        let hashes = self.build_hashes_inner(ks);
        self.table
            .get_many_mut(hashes, |i, (k, _)| ks[i].equivalent(k))
    }

    unsafe fn get_many_unchecked_mut_inner<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> Option<[&'_ mut (K, V); N]>
    where
        Q: Hash + Equivalent<K>,
    {
        let hashes = self.build_hashes_inner(ks);
        self.table
            .get_many_unchecked_mut(hashes, |i, (k, _)| ks[i].equivalent(k))
    }

    fn build_hashes_inner<Q: ?Sized, const N: usize>(&self, ks: [&Q; N]) -> [u64; N]
    where
        Q: Hash + Equivalent<K>,
    {
        let mut hashes = [0_u64; N];
        for i in 0..N {
            hashes[i] = make_hash::<Q, S>(&self.hash_builder, ks[i]);
        }
        hashes
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, [`None`] is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical. See the [`std::collections`]
    /// [module-level documentation] for more.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    /// [`std::collections`]: https://doc.rust-lang.org/std/collections/index.html
    /// [module-level documentation]: https://doc.rust-lang.org/std/collections/index.html#insert-and-complex-keys
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// assert_eq!(map.insert(37, "a"), None);
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.insert(37, "b");
    /// assert_eq!(map.insert(37, "c"), Some("b"));
    /// assert_eq!(map[&37], "c");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        let hash = make_hash::<K, S>(&self.hash_builder, &k);
        let hasher = make_hasher::<_, V, S>(&self.hash_builder);
        match self
            .table
            .find_or_find_insert_slot(hash, equivalent_key(&k), hasher)
        {
            Ok(bucket) => Some(mem::replace(unsafe { &mut bucket.as_mut().1 }, v)),
            Err(slot) => {
                unsafe {
                    self.table.insert_in_slot(hash, slot, (k, v));
                }
                None
            }
        }
    }

    /// Insert a key-value pair into the map without checking
    /// if the key already exists in the map.
    ///
    /// Returns a reference to the key and value just inserted.
    ///
    /// This operation is safe if a key does not exist in the map.
    ///
    /// However, if a key exists in the map already, the behavior is unspecified:
    /// this operation may panic, loop forever, or any following operation with the map
    /// may panic, loop forever or return arbitrary result.
    ///
    /// That said, this operation (and following operations) are guaranteed to
    /// not violate memory safety.
    ///
    /// This operation is faster than regular insert, because it does not perform
    /// lookup before insertion.
    ///
    /// This operation is useful during initial population of the map.
    /// For example, when constructing a map from another map, we know
    /// that keys are unique.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map1 = HashMap::new();
    /// assert_eq!(map1.insert(1, "a"), None);
    /// assert_eq!(map1.insert(2, "b"), None);
    /// assert_eq!(map1.insert(3, "c"), None);
    /// assert_eq!(map1.len(), 3);
    ///
    /// let mut map2 = HashMap::new();
    ///
    /// for (key, value) in map1.into_iter() {
    ///     map2.insert_unique_unchecked(key, value);
    /// }
    ///
    /// let (key, value) = map2.insert_unique_unchecked(4, "d");
    /// assert_eq!(key, &4);
    /// assert_eq!(value, &mut "d");
    /// *value = "e";
    ///
    /// assert_eq!(map2[&1], "a");
    /// assert_eq!(map2[&2], "b");
    /// assert_eq!(map2[&3], "c");
    /// assert_eq!(map2[&4], "e");
    /// assert_eq!(map2.len(), 4);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_unique_unchecked(&mut self, k: K, v: V) -> (&K, &mut V) {
        let hash = make_hash::<K, S>(&self.hash_builder, &k);
        let bucket = self
            .table
            .insert(hash, (k, v), make_hasher::<_, V, S>(&self.hash_builder));
        let (k_ref, v_ref) = unsafe { bucket.as_mut() };
        (k_ref, v_ref)
    }

    /// Tries to insert a key-value pair into the map, and returns
    /// a mutable reference to the value in the entry.
    ///
    /// # Errors
    ///
    /// If the map already had this key present, nothing is updated, and
    /// an error containing the occupied entry and the value is returned.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::OccupiedError;
    ///
    /// let mut map = HashMap::new();
    /// assert_eq!(map.try_insert(37, "a").unwrap(), &"a");
    ///
    /// match map.try_insert(37, "b") {
    ///     Err(OccupiedError { entry, value }) => {
    ///         assert_eq!(entry.key(), &37);
    ///         assert_eq!(entry.get(), &"a");
    ///         assert_eq!(value, "b");
    ///     }
    ///     _ => panic!()
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn try_insert(
        &mut self,
        key: K,
        value: V,
    ) -> Result<&mut V, OccupiedError<'_, K, V, S, A>> {
        match self.entry(key) {
            Entry::Occupied(entry) => Err(OccupiedError { entry, value }),
            Entry::Vacant(entry) => Ok(entry.insert(value)),
        }
    }

    /// Removes a key from the map, returning the value at the key if the key
    /// was previously in the map. Keeps the allocated memory for reuse.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// // The map is empty
    /// assert!(map.is_empty() && map.capacity() == 0);
    ///
    /// map.insert(1, "a");
    ///
    /// assert_eq!(map.remove(&1), Some("a"));
    /// assert_eq!(map.remove(&1), None);
    ///
    /// // Now map holds none elements
    /// assert!(map.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        Q: Hash + Equivalent<K>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.remove_entry(k) {
            Some((_, v)) => Some(v),
            None => None,
        }
    }

    /// Removes a key from the map, returning the stored key and value if the
    /// key was previously in the map. Keeps the allocated memory for reuse.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// // The map is empty
    /// assert!(map.is_empty() && map.capacity() == 0);
    ///
    /// map.insert(1, "a");
    ///
    /// assert_eq!(map.remove_entry(&1), Some((1, "a")));
    /// assert_eq!(map.remove(&1), None);
    ///
    /// // Now map hold none elements
    /// assert!(map.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        let hash = make_hash::<Q, S>(&self.hash_builder, k);
        self.table.remove_entry(hash, equivalent_key(k))
    }
}

impl<K, V, S, A: Allocator> HashMap<K, V, S, A> {
    /// Creates a raw entry builder for the HashMap.
    ///
    /// Raw entries provide the lowest level of control for searching and
    /// manipulating a map. They must be manually initialized with a hash and
    /// then manually searched. After this, insertions into a vacant entry
    /// still require an owned key to be provided.
    ///
    /// Raw entries are useful for such exotic situations as:
    ///
    /// * Hash memoization
    /// * Deferring the creation of an owned key until it is known to be required
    /// * Using a search key that doesn't work with the Borrow trait
    /// * Using custom comparison logic without newtype wrappers
    ///
    /// Because raw entries provide much more low-level control, it's much easier
    /// to put the HashMap into an inconsistent state which, while memory-safe,
    /// will cause the map to produce seemingly random results. Higher-level and
    /// more foolproof APIs like `entry` should be preferred when possible.
    ///
    /// In particular, the hash used to initialized the raw entry must still be
    /// consistent with the hash of the key that is ultimately stored in the entry.
    /// This is because implementations of HashMap may need to recompute hashes
    /// when resizing, at which point only the keys are available.
    ///
    /// Raw entries give mutable access to the keys. This must not be used
    /// to modify how the key would compare or hash, as the map will not re-evaluate
    /// where the key should go, meaning the keys may become "lost" if their
    /// location does not reflect their state. For instance, if you change a key
    /// so that the map now contains keys which compare equal, search may start
    /// acting erratically, with two keys randomly masking each other. Implementations
    /// are free to assume this doesn't happen (within the limits of memory-safety).
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map = HashMap::new();
    /// map.extend([("a", 100), ("b", 200), ("c", 300)]);
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// // Existing key (insert and update)
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(mut view) => {
    ///         assert_eq!(view.get(), &100);
    ///         let v = view.get_mut();
    ///         let new_v = (*v) * 10;
    ///         *v = new_v;
    ///         assert_eq!(view.insert(1111), 1000);
    ///     }
    /// }
    ///
    /// assert_eq!(map[&"a"], 1111);
    /// assert_eq!(map.len(), 3);
    ///
    /// // Existing key (take)
    /// let hash = compute_hash(map.hasher(), &"c");
    /// match map.raw_entry_mut().from_key_hashed_nocheck(hash, &"c") {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(view) => {
    ///         assert_eq!(view.remove_entry(), ("c", 300));
    ///     }
    /// }
    /// assert_eq!(map.raw_entry().from_key(&"c"), None);
    /// assert_eq!(map.len(), 2);
    ///
    /// // Nonexistent key (insert and update)
    /// let key = "d";
    /// let hash = compute_hash(map.hasher(), &key);
    /// match map.raw_entry_mut().from_hash(hash, |q| *q == key) {
    ///     RawEntryMut::Occupied(_) => unreachable!(),
    ///     RawEntryMut::Vacant(view) => {
    ///         let (k, value) = view.insert("d", 4000);
    ///         assert_eq!((*k, *value), ("d", 4000));
    ///         *value = 40000;
    ///     }
    /// }
    /// assert_eq!(map[&"d"], 40000);
    /// assert_eq!(map.len(), 3);
    ///
    /// match map.raw_entry_mut().from_hash(hash, |q| *q == key) {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(view) => {
    ///         assert_eq!(view.remove_entry(), ("d", 40000));
    ///     }
    /// }
    /// assert_eq!(map.get(&"d"), None);
    /// assert_eq!(map.len(), 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_entry_mut(&mut self) -> RawEntryBuilderMut<'_, K, V, S, A> {
        RawEntryBuilderMut { map: self }
    }

    /// Creates a raw immutable entry builder for the HashMap.
    ///
    /// Raw entries provide the lowest level of control for searching and
    /// manipulating a map. They must be manually initialized with a hash and
    /// then manually searched.
    ///
    /// This is useful for
    /// * Hash memoization
    /// * Using a search key that doesn't work with the Borrow trait
    /// * Using custom comparison logic without newtype wrappers
    ///
    /// Unless you are in such a situation, higher-level and more foolproof APIs like
    /// `get` should be preferred.
    ///
    /// Immutable raw entries have very limited use; you might instead want `raw_entry_mut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.extend([("a", 100), ("b", 200), ("c", 300)]);
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// for k in ["a", "b", "c", "d", "e", "f"] {
    ///     let hash = compute_hash(map.hasher(), k);
    ///     let v = map.get(&k).cloned();
    ///     let kv = v.as_ref().map(|v| (&k, v));
    ///
    ///     println!("Key: {} and value: {:?}", k, v);
    ///
    ///     assert_eq!(map.raw_entry().from_key(&k), kv);
    ///     assert_eq!(map.raw_entry().from_hash(hash, |q| *q == k), kv);
    ///     assert_eq!(map.raw_entry().from_key_hashed_nocheck(hash, &k), kv);
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_entry(&self) -> RawEntryBuilder<'_, K, V, S, A> {
        RawEntryBuilder { map: self }
    }

    /// Returns a reference to the [`RawTable`] used underneath [`HashMap`].
    /// This function is only available if the `raw` feature of the crate is enabled.
    ///
    /// See [`raw_table_mut`] for more.
    ///
    /// [`raw_table_mut`]: Self::raw_table_mut
    #[cfg(feature = "raw")]
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_table(&self) -> &RawTable<(K, V), A> {
        &self.table
    }

    /// Returns a mutable reference to the [`RawTable`] used underneath [`HashMap`].
    /// This function is only available if the `raw` feature of the crate is enabled.
    ///
    /// # Note
    ///
    /// Calling this function is safe, but using the raw hash table API may require
    /// unsafe functions or blocks.
    ///
    /// `RawTable` API gives the lowest level of control under the map that can be useful
    /// for extending the HashMap's API, but may lead to *[undefined behavior]*.
    ///
    /// [`HashMap`]: struct.HashMap.html
    /// [`RawTable`]: crate::raw::RawTable
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.extend([("a", 10), ("b", 20), ("c", 30)]);
    /// assert_eq!(map.len(), 3);
    ///
    /// // Let's imagine that we have a value and a hash of the key, but not the key itself.
    /// // However, if you want to remove the value from the map by hash and value, and you
    /// // know exactly that the value is unique, then you can create a function like this:
    /// fn remove_by_hash<K, V, S, F>(
    ///     map: &mut HashMap<K, V, S>,
    ///     hash: u64,
    ///     is_match: F,
    /// ) -> Option<(K, V)>
    /// where
    ///     F: Fn(&(K, V)) -> bool,
    /// {
    ///     let raw_table = map.raw_table_mut();
    ///     match raw_table.find(hash, is_match) {
    ///         Some(bucket) => Some(unsafe { raw_table.remove(bucket).0 }),
    ///         None => None,
    ///     }
    /// }
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// let hash = compute_hash(map.hasher(), "a");
    /// assert_eq!(remove_by_hash(&mut map, hash, |(_, v)| *v == 10), Some(("a", 10)));
    /// assert_eq!(map.get(&"a"), None);
    /// assert_eq!(map.len(), 2);
    /// ```
    #[cfg(feature = "raw")]
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_table_mut(&mut self) -> &mut RawTable<(K, V), A> {
        &mut self.table
    }
}

impl<K, V, S, A> PartialEq for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    V: PartialEq,
    S: BuildHasher,
    A: Allocator,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

impl<K, V, S, A> Eq for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
    A: Allocator,
{
}

impl<K, V, S, A> Debug for HashMap<K, V, S, A>
where
    K: Debug,
    V: Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V, S, A> Default for HashMap<K, V, S, A>
where
    S: Default,
    A: Default + Allocator,
{
    /// Creates an empty `HashMap<K, V, S, A>`, with the `Default` value for the hasher and allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// // You can specify all types of HashMap, including hasher and allocator.
    /// // Created map is empty and don't allocate memory
    /// let map: HashMap<u32, String> = Default::default();
    /// assert_eq!(map.capacity(), 0);
    /// let map: HashMap<u32, String, RandomState> = HashMap::default();
    /// assert_eq!(map.capacity(), 0);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn default() -> Self {
        Self::with_hasher_in(Default::default(), Default::default())
    }
}

impl<K, Q: ?Sized, V, S, A> Index<&Q> for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    Q: Hash + Equivalent<K>,
    S: BuildHasher,
    A: Allocator,
{
    type Output = V;

    /// Returns a reference to the value corresponding to the supplied key.
    ///
    /// # Panics
    ///
    /// Panics if the key is not present in the `HashMap`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let map: HashMap<_, _> = [("a", "One"), ("b", "Two")].into();
    ///
    /// assert_eq!(map[&"a"], "One");
    /// assert_eq!(map[&"b"], "Two");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

// The default hasher is used to match the std implementation signature
#[cfg(feature = "ahash")]
impl<K, V, A, const N: usize> From<[(K, V); N]> for HashMap<K, V, DefaultHashBuilder, A>
where
    K: Eq + Hash,
    A: Default + Allocator,
{
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let map1 = HashMap::from([(1, 2), (3, 4)]);
    /// let map2: HashMap<_, _> = [(1, 2), (3, 4)].into();
    /// assert_eq!(map1, map2);
    /// ```
    fn from(arr: [(K, V); N]) -> Self {
        arr.into_iter().collect()
    }
}

/// An iterator over the entries of a `HashMap` in arbitrary order.
/// The iterator element type is `(&'a K, &'a V)`.
///
/// This `struct` is created by the [`iter`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`iter`]: struct.HashMap.html#method.iter
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut iter = map.iter();
/// let mut vec = vec![iter.next(), iter.next(), iter.next()];
///
/// // The `Iter` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some((&1, &"a")), Some((&2, &"b")), Some((&3, &"c"))]);
///
/// // It is fused iterator
/// assert_eq!(iter.next(), None);
/// assert_eq!(iter.next(), None);
/// ```
pub struct Iter<'a, K, V> {
    inner: RawIter<(K, V)>,
    marker: PhantomData<(&'a K, &'a V)>,
}

// FIXME(#26925) Remove in favor of `#[derive(Clone)]`
impl<K, V> Clone for Iter<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Iter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: Debug, V: Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A mutable iterator over the entries of a `HashMap` in arbitrary order.
/// The iterator element type is `(&'a K, &'a mut V)`.
///
/// This `struct` is created by the [`iter_mut`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`iter_mut`]: struct.HashMap.html#method.iter_mut
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let mut map: HashMap<_, _> = [(1, "One".to_owned()), (2, "Two".into())].into();
///
/// let mut iter = map.iter_mut();
/// iter.next().map(|(_, v)| v.push_str(" Mississippi"));
/// iter.next().map(|(_, v)| v.push_str(" Mississippi"));
///
/// // It is fused iterator
/// assert_eq!(iter.next(), None);
/// assert_eq!(iter.next(), None);
///
/// assert_eq!(map.get(&1).unwrap(), &"One Mississippi".to_owned());
/// assert_eq!(map.get(&2).unwrap(), &"Two Mississippi".to_owned());
/// ```
pub struct IterMut<'a, K, V> {
    inner: RawIter<(K, V)>,
    // To ensure invariance with respect to V
    marker: PhantomData<(&'a K, &'a mut V)>,
}

// We override the default Send impl which has K: Sync instead of K: Send. Both
// are correct, but this one is more general since it allows keys which
// implement Send but not Sync.
unsafe impl<K: Send, V: Send> Send for IterMut<'_, K, V> {}

impl<K, V> IterMut<'_, K, V> {
    /// Returns a iterator of references over the remaining items.
    #[cfg_attr(feature = "inline-more", inline)]
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

/// An owning iterator over the entries of a `HashMap` in arbitrary order.
/// The iterator element type is `(K, V)`.
///
/// This `struct` is created by the [`into_iter`] method on [`HashMap`]
/// (provided by the [`IntoIterator`] trait). See its documentation for more.
/// The map cannot be used after calling that method.
///
/// [`into_iter`]: struct.HashMap.html#method.into_iter
/// [`HashMap`]: struct.HashMap.html
/// [`IntoIterator`]: https://doc.rust-lang.org/core/iter/trait.IntoIterator.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut iter = map.into_iter();
/// let mut vec = vec![iter.next(), iter.next(), iter.next()];
///
/// // The `IntoIter` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some((1, "a")), Some((2, "b")), Some((3, "c"))]);
///
/// // It is fused iterator
/// assert_eq!(iter.next(), None);
/// assert_eq!(iter.next(), None);
/// ```
pub struct IntoIter<K, V, A: Allocator = Global> {
    inner: RawIntoIter<(K, V), A>,
}

impl<K, V, A: Allocator> IntoIter<K, V, A> {
    /// Returns a iterator of references over the remaining items.
    #[cfg_attr(feature = "inline-more", inline)]
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.inner.iter(),
            marker: PhantomData,
        }
    }
}

/// An owning iterator over the keys of a `HashMap` in arbitrary order.
/// The iterator element type is `K`.
///
/// This `struct` is created by the [`into_keys`] method on [`HashMap`].
/// See its documentation for more.
/// The map cannot be used after calling that method.
///
/// [`into_keys`]: struct.HashMap.html#method.into_keys
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut keys = map.into_keys();
/// let mut vec = vec![keys.next(), keys.next(), keys.next()];
///
/// // The `IntoKeys` iterator produces keys in arbitrary order, so the
/// // keys must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some(1), Some(2), Some(3)]);
///
/// // It is fused iterator
/// assert_eq!(keys.next(), None);
/// assert_eq!(keys.next(), None);
/// ```
pub struct IntoKeys<K, V, A: Allocator = Global> {
    inner: IntoIter<K, V, A>,
}

impl<K, V, A: Allocator> Iterator for IntoKeys<K, V, A> {
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<K> {
        self.inner.next().map(|(k, _)| k)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (k, _)| f(acc, k))
    }
}

impl<K, V, A: Allocator> ExactSizeIterator for IntoKeys<K, V, A> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<K, V, A: Allocator> FusedIterator for IntoKeys<K, V, A> {}

impl<K: Debug, V: Debug, A: Allocator> fmt::Debug for IntoKeys<K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.inner.iter().map(|(k, _)| k))
            .finish()
    }
}

/// An owning iterator over the values of a `HashMap` in arbitrary order.
/// The iterator element type is `V`.
///
/// This `struct` is created by the [`into_values`] method on [`HashMap`].
/// See its documentation for more. The map cannot be used after calling that method.
///
/// [`into_values`]: struct.HashMap.html#method.into_values
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut values = map.into_values();
/// let mut vec = vec![values.next(), values.next(), values.next()];
///
/// // The `IntoValues` iterator produces values in arbitrary order, so
/// // the values must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some("a"), Some("b"), Some("c")]);
///
/// // It is fused iterator
/// assert_eq!(values.next(), None);
/// assert_eq!(values.next(), None);
/// ```
pub struct IntoValues<K, V, A: Allocator = Global> {
    inner: IntoIter<K, V, A>,
}

impl<K, V, A: Allocator> Iterator for IntoValues<K, V, A> {
    type Item = V;

    #[inline]
    fn next(&mut self) -> Option<V> {
        self.inner.next().map(|(_, v)| v)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (_, v)| f(acc, v))
    }
}

impl<K, V, A: Allocator> ExactSizeIterator for IntoValues<K, V, A> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<K, V, A: Allocator> FusedIterator for IntoValues<K, V, A> {}

impl<K, V: Debug, A: Allocator> fmt::Debug for IntoValues<K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.inner.iter().map(|(_, v)| v))
            .finish()
    }
}

/// An iterator over the keys of a `HashMap` in arbitrary order.
/// The iterator element type is `&'a K`.
///
/// This `struct` is created by the [`keys`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`keys`]: struct.HashMap.html#method.keys
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut keys = map.keys();
/// let mut vec = vec![keys.next(), keys.next(), keys.next()];
///
/// // The `Keys` iterator produces keys in arbitrary order, so the
/// // keys must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some(&1), Some(&2), Some(&3)]);
///
/// // It is fused iterator
/// assert_eq!(keys.next(), None);
/// assert_eq!(keys.next(), None);
/// ```
pub struct Keys<'a, K, V> {
    inner: Iter<'a, K, V>,
}

// FIXME(#26925) Remove in favor of `#[derive(Clone)]`
impl<K, V> Clone for Keys<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Keys {
            inner: self.inner.clone(),
        }
    }
}

impl<K: Debug, V> fmt::Debug for Keys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// An iterator over the values of a `HashMap` in arbitrary order.
/// The iterator element type is `&'a V`.
///
/// This `struct` is created by the [`values`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`values`]: struct.HashMap.html#method.values
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut values = map.values();
/// let mut vec = vec![values.next(), values.next(), values.next()];
///
/// // The `Values` iterator produces values in arbitrary order, so the
/// // values must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some(&"a"), Some(&"b"), Some(&"c")]);
///
/// // It is fused iterator
/// assert_eq!(values.next(), None);
/// assert_eq!(values.next(), None);
/// ```
pub struct Values<'a, K, V> {
    inner: Iter<'a, K, V>,
}

// FIXME(#26925) Remove in favor of `#[derive(Clone)]`
impl<K, V> Clone for Values<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Values {
            inner: self.inner.clone(),
        }
    }
}

impl<K, V: Debug> fmt::Debug for Values<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A draining iterator over the entries of a `HashMap` in arbitrary
/// order. The iterator element type is `(K, V)`.
///
/// This `struct` is created by the [`drain`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`drain`]: struct.HashMap.html#method.drain
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let mut map: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut drain_iter = map.drain();
/// let mut vec = vec![drain_iter.next(), drain_iter.next(), drain_iter.next()];
///
/// // The `Drain` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some((1, "a")), Some((2, "b")), Some((3, "c"))]);
///
/// // It is fused iterator
/// assert_eq!(drain_iter.next(), None);
/// assert_eq!(drain_iter.next(), None);
/// ```
pub struct Drain<'a, K, V, A: Allocator = Global> {
    inner: RawDrain<'a, (K, V), A>,
}

impl<K, V, A: Allocator> Drain<'_, K, V, A> {
    /// Returns a iterator of references over the remaining items.
    #[cfg_attr(feature = "inline-more", inline)]
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.inner.iter(),
            marker: PhantomData,
        }
    }
}

/// A draining iterator over entries of a `HashMap` which don't satisfy the predicate
/// `f(&k, &mut v)` in arbitrary order. The iterator element type is `(K, V)`.
///
/// This `struct` is created by the [`extract_if`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`extract_if`]: struct.HashMap.html#method.extract_if
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let mut map: HashMap<i32, &str> = [(1, "a"), (2, "b"), (3, "c")].into();
///
/// let mut extract_if = map.extract_if(|k, _v| k % 2 != 0);
/// let mut vec = vec![extract_if.next(), extract_if.next()];
///
/// // The `ExtractIf` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [Some((1, "a")),Some((3, "c"))]);
///
/// // It is fused iterator
/// assert_eq!(extract_if.next(), None);
/// assert_eq!(extract_if.next(), None);
/// drop(extract_if);
///
/// assert_eq!(map.len(), 1);
/// ```
#[must_use = "Iterators are lazy unless consumed"]
pub struct ExtractIf<'a, K, V, F, A: Allocator = Global>
where
    F: FnMut(&K, &mut V) -> bool,
{
    f: F,
    inner: RawExtractIf<'a, (K, V), A>,
}

impl<K, V, F, A> Iterator for ExtractIf<'_, K, V, F, A>
where
    F: FnMut(&K, &mut V) -> bool,
    A: Allocator,
{
    type Item = (K, V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next(|&mut (ref k, ref mut v)| (self.f)(k, v))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.inner.iter.size_hint().1)
    }
}

impl<K, V, F> FusedIterator for ExtractIf<'_, K, V, F> where F: FnMut(&K, &mut V) -> bool {}

/// A mutable iterator over the values of a `HashMap` in arbitrary order.
/// The iterator element type is `&'a mut V`.
///
/// This `struct` is created by the [`values_mut`] method on [`HashMap`]. See its
/// documentation for more.
///
/// [`values_mut`]: struct.HashMap.html#method.values_mut
/// [`HashMap`]: struct.HashMap.html
///
/// # Examples
///
/// ```
/// use hashbrown::HashMap;
///
/// let mut map: HashMap<_, _> = [(1, "One".to_owned()), (2, "Two".into())].into();
///
/// let mut values = map.values_mut();
/// values.next().map(|v| v.push_str(" Mississippi"));
/// values.next().map(|v| v.push_str(" Mississippi"));
///
/// // It is fused iterator
/// assert_eq!(values.next(), None);
/// assert_eq!(values.next(), None);
///
/// assert_eq!(map.get(&1).unwrap(), &"One Mississippi".to_owned());
/// assert_eq!(map.get(&2).unwrap(), &"Two Mississippi".to_owned());
/// ```
pub struct ValuesMut<'a, K, V> {
    inner: IterMut<'a, K, V>,
}

/// A builder for computing where in a [`HashMap`] a key-value pair would be stored.
///
/// See the [`HashMap::raw_entry_mut`] docs for usage examples.
///
/// [`HashMap::raw_entry_mut`]: struct.HashMap.html#method.raw_entry_mut
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{RawEntryBuilderMut, RawEntryMut::Vacant, RawEntryMut::Occupied};
/// use hashbrown::HashMap;
/// use core::hash::{BuildHasher, Hash};
///
/// let mut map = HashMap::new();
/// map.extend([(1, 11), (2, 12), (3, 13), (4, 14), (5, 15), (6, 16)]);
/// assert_eq!(map.len(), 6);
///
/// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
///     use core::hash::Hasher;
///     let mut state = hash_builder.build_hasher();
///     key.hash(&mut state);
///     state.finish()
/// }
///
/// let builder: RawEntryBuilderMut<_, _, _> = map.raw_entry_mut();
///
/// // Existing key
/// match builder.from_key(&6) {
///     Vacant(_) => unreachable!(),
///     Occupied(view) => assert_eq!(view.get(), &16),
/// }
///
/// for key in 0..12 {
///     let hash = compute_hash(map.hasher(), &key);
///     let value = map.get(&key).cloned();
///     let key_value = value.as_ref().map(|v| (&key, v));
///
///     println!("Key: {} and value: {:?}", key, value);
///
///     match map.raw_entry_mut().from_key(&key) {
///         Occupied(mut o) => assert_eq!(Some(o.get_key_value()), key_value),
///         Vacant(_) => assert_eq!(value, None),
///     }
///     match map.raw_entry_mut().from_key_hashed_nocheck(hash, &key) {
///         Occupied(mut o) => assert_eq!(Some(o.get_key_value()), key_value),
///         Vacant(_) => assert_eq!(value, None),
///     }
///     match map.raw_entry_mut().from_hash(hash, |q| *q == key) {
///         Occupied(mut o) => assert_eq!(Some(o.get_key_value()), key_value),
///         Vacant(_) => assert_eq!(value, None),
///     }
/// }
///
/// assert_eq!(map.len(), 6);
/// ```
pub struct RawEntryBuilderMut<'a, K, V, S, A: Allocator = Global> {
    map: &'a mut HashMap<K, V, S, A>,
}

/// A view into a single entry in a map, which may either be vacant or occupied.
///
/// This is a lower-level version of [`Entry`].
///
/// This `enum` is constructed through the [`raw_entry_mut`] method on [`HashMap`],
/// then calling one of the methods of that [`RawEntryBuilderMut`].
///
/// [`HashMap`]: struct.HashMap.html
/// [`Entry`]: enum.Entry.html
/// [`raw_entry_mut`]: struct.HashMap.html#method.raw_entry_mut
/// [`RawEntryBuilderMut`]: struct.RawEntryBuilderMut.html
///
/// # Examples
///
/// ```
/// use core::hash::{BuildHasher, Hash};
/// use hashbrown::hash_map::{HashMap, RawEntryMut, RawOccupiedEntryMut};
///
/// let mut map = HashMap::new();
/// map.extend([('a', 1), ('b', 2), ('c', 3)]);
/// assert_eq!(map.len(), 3);
///
/// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
///     use core::hash::Hasher;
///     let mut state = hash_builder.build_hasher();
///     key.hash(&mut state);
///     state.finish()
/// }
///
/// // Existing key (insert)
/// let raw: RawEntryMut<_, _, _> = map.raw_entry_mut().from_key(&'a');
/// let _raw_o: RawOccupiedEntryMut<_, _, _> = raw.insert('a', 10);
/// assert_eq!(map.len(), 3);
///
/// // Nonexistent key (insert)
/// map.raw_entry_mut().from_key(&'d').insert('d', 40);
/// assert_eq!(map.len(), 4);
///
/// // Existing key (or_insert)
/// let hash = compute_hash(map.hasher(), &'b');
/// let kv = map
///     .raw_entry_mut()
///     .from_key_hashed_nocheck(hash, &'b')
///     .or_insert('b', 20);
/// assert_eq!(kv, (&mut 'b', &mut 2));
/// *kv.1 = 20;
/// assert_eq!(map.len(), 4);
///
/// // Nonexistent key (or_insert)
/// let hash = compute_hash(map.hasher(), &'e');
/// let kv = map
///     .raw_entry_mut()
///     .from_key_hashed_nocheck(hash, &'e')
///     .or_insert('e', 50);
/// assert_eq!(kv, (&mut 'e', &mut 50));
/// assert_eq!(map.len(), 5);
///
/// // Existing key (or_insert_with)
/// let hash = compute_hash(map.hasher(), &'c');
/// let kv = map
///     .raw_entry_mut()
///     .from_hash(hash, |q| q == &'c')
///     .or_insert_with(|| ('c', 30));
/// assert_eq!(kv, (&mut 'c', &mut 3));
/// *kv.1 = 30;
/// assert_eq!(map.len(), 5);
///
/// // Nonexistent key (or_insert_with)
/// let hash = compute_hash(map.hasher(), &'f');
/// let kv = map
///     .raw_entry_mut()
///     .from_hash(hash, |q| q == &'f')
///     .or_insert_with(|| ('f', 60));
/// assert_eq!(kv, (&mut 'f', &mut 60));
/// assert_eq!(map.len(), 6);
///
/// println!("Our HashMap: {:?}", map);
///
/// let mut vec: Vec<_> = map.iter().map(|(&k, &v)| (k, v)).collect();
/// // The `Iter` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [('a', 10), ('b', 20), ('c', 30), ('d', 40), ('e', 50), ('f', 60)]);
/// ```
pub enum RawEntryMut<'a, K, V, S, A: Allocator = Global> {
    /// An occupied entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::{hash_map::RawEntryMut, HashMap};
    /// let mut map: HashMap<_, _> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(_) => { }
    /// }
    /// ```
    Occupied(RawOccupiedEntryMut<'a, K, V, S, A>),
    /// A vacant entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::{hash_map::RawEntryMut, HashMap};
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    ///
    /// match map.raw_entry_mut().from_key("a") {
    ///     RawEntryMut::Occupied(_) => unreachable!(),
    ///     RawEntryMut::Vacant(_) => { }
    /// }
    /// ```
    Vacant(RawVacantEntryMut<'a, K, V, S, A>),
}

/// A view into an occupied entry in a `HashMap`.
/// It is part of the [`RawEntryMut`] enum.
///
/// [`RawEntryMut`]: enum.RawEntryMut.html
///
/// # Examples
///
/// ```
/// use core::hash::{BuildHasher, Hash};
/// use hashbrown::hash_map::{HashMap, RawEntryMut, RawOccupiedEntryMut};
///
/// let mut map = HashMap::new();
/// map.extend([("a", 10), ("b", 20), ("c", 30)]);
///
/// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
///     use core::hash::Hasher;
///     let mut state = hash_builder.build_hasher();
///     key.hash(&mut state);
///     state.finish()
/// }
///
/// let _raw_o: RawOccupiedEntryMut<_, _, _> = map.raw_entry_mut().from_key(&"a").insert("a", 100);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (insert and update)
/// match map.raw_entry_mut().from_key(&"a") {
///     RawEntryMut::Vacant(_) => unreachable!(),
///     RawEntryMut::Occupied(mut view) => {
///         assert_eq!(view.get(), &100);
///         let v = view.get_mut();
///         let new_v = (*v) * 10;
///         *v = new_v;
///         assert_eq!(view.insert(1111), 1000);
///     }
/// }
///
/// assert_eq!(map[&"a"], 1111);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (take)
/// let hash = compute_hash(map.hasher(), &"c");
/// match map.raw_entry_mut().from_key_hashed_nocheck(hash, &"c") {
///     RawEntryMut::Vacant(_) => unreachable!(),
///     RawEntryMut::Occupied(view) => {
///         assert_eq!(view.remove_entry(), ("c", 30));
///     }
/// }
/// assert_eq!(map.raw_entry().from_key(&"c"), None);
/// assert_eq!(map.len(), 2);
///
/// let hash = compute_hash(map.hasher(), &"b");
/// match map.raw_entry_mut().from_hash(hash, |q| *q == "b") {
///     RawEntryMut::Vacant(_) => unreachable!(),
///     RawEntryMut::Occupied(view) => {
///         assert_eq!(view.remove_entry(), ("b", 20));
///     }
/// }
/// assert_eq!(map.get(&"b"), None);
/// assert_eq!(map.len(), 1);
/// ```
pub struct RawOccupiedEntryMut<'a, K, V, S, A: Allocator = Global> {
    elem: Bucket<(K, V)>,
    table: &'a mut RawTable<(K, V), A>,
    hash_builder: &'a S,
}

unsafe impl<K, V, S, A> Send for RawOccupiedEntryMut<'_, K, V, S, A>
where
    K: Send,
    V: Send,
    S: Send,
    A: Send + Allocator,
{
}
unsafe impl<K, V, S, A> Sync for RawOccupiedEntryMut<'_, K, V, S, A>
where
    K: Sync,
    V: Sync,
    S: Sync,
    A: Sync + Allocator,
{
}

/// A view into a vacant entry in a `HashMap`.
/// It is part of the [`RawEntryMut`] enum.
///
/// [`RawEntryMut`]: enum.RawEntryMut.html
///
/// # Examples
///
/// ```
/// use core::hash::{BuildHasher, Hash};
/// use hashbrown::hash_map::{HashMap, RawEntryMut, RawVacantEntryMut};
///
/// let mut map = HashMap::<&str, i32>::new();
///
/// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
///     use core::hash::Hasher;
///     let mut state = hash_builder.build_hasher();
///     key.hash(&mut state);
///     state.finish()
/// }
///
/// let raw_v: RawVacantEntryMut<_, _, _> = match map.raw_entry_mut().from_key(&"a") {
///     RawEntryMut::Vacant(view) => view,
///     RawEntryMut::Occupied(_) => unreachable!(),
/// };
/// raw_v.insert("a", 10);
/// assert!(map[&"a"] == 10 && map.len() == 1);
///
/// // Nonexistent key (insert and update)
/// let hash = compute_hash(map.hasher(), &"b");
/// match map.raw_entry_mut().from_key_hashed_nocheck(hash, &"b") {
///     RawEntryMut::Occupied(_) => unreachable!(),
///     RawEntryMut::Vacant(view) => {
///         let (k, value) = view.insert("b", 2);
///         assert_eq!((*k, *value), ("b", 2));
///         *value = 20;
///     }
/// }
/// assert!(map[&"b"] == 20 && map.len() == 2);
///
/// let hash = compute_hash(map.hasher(), &"c");
/// match map.raw_entry_mut().from_hash(hash, |q| *q == "c") {
///     RawEntryMut::Occupied(_) => unreachable!(),
///     RawEntryMut::Vacant(view) => {
///         assert_eq!(view.insert("c", 30), (&mut "c", &mut 30));
///     }
/// }
/// assert!(map[&"c"] == 30 && map.len() == 3);
/// ```
pub struct RawVacantEntryMut<'a, K, V, S, A: Allocator = Global> {
    table: &'a mut RawTable<(K, V), A>,
    hash_builder: &'a S,
}

/// A builder for computing where in a [`HashMap`] a key-value pair would be stored.
///
/// See the [`HashMap::raw_entry`] docs for usage examples.
///
/// [`HashMap::raw_entry`]: struct.HashMap.html#method.raw_entry
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{HashMap, RawEntryBuilder};
/// use core::hash::{BuildHasher, Hash};
///
/// let mut map = HashMap::new();
/// map.extend([(1, 10), (2, 20), (3, 30)]);
///
/// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
///     use core::hash::Hasher;
///     let mut state = hash_builder.build_hasher();
///     key.hash(&mut state);
///     state.finish()
/// }
///
/// for k in 0..6 {
///     let hash = compute_hash(map.hasher(), &k);
///     let v = map.get(&k).cloned();
///     let kv = v.as_ref().map(|v| (&k, v));
///
///     println!("Key: {} and value: {:?}", k, v);
///     let builder: RawEntryBuilder<_, _, _> = map.raw_entry();
///     assert_eq!(builder.from_key(&k), kv);
///     assert_eq!(map.raw_entry().from_hash(hash, |q| *q == k), kv);
///     assert_eq!(map.raw_entry().from_key_hashed_nocheck(hash, &k), kv);
/// }
/// ```
pub struct RawEntryBuilder<'a, K, V, S, A: Allocator = Global> {
    map: &'a HashMap<K, V, S, A>,
}

impl<'a, K, V, S, A: Allocator> RawEntryBuilderMut<'a, K, V, S, A> {
    /// Creates a `RawEntryMut` from the given key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// let key = "a";
    /// let entry: RawEntryMut<&str, u32, _> = map.raw_entry_mut().from_key(&key);
    /// entry.insert(key, 100);
    /// assert_eq!(map[&"a"], 100);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_key<Q: ?Sized>(self, k: &Q) -> RawEntryMut<'a, K, V, S, A>
    where
        S: BuildHasher,
        Q: Hash + Equivalent<K>,
    {
        let hash = make_hash::<Q, S>(&self.map.hash_builder, k);
        self.from_key_hashed_nocheck(hash, k)
    }

    /// Creates a `RawEntryMut` from the given key and its hash.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// let key = "a";
    /// let hash = compute_hash(map.hasher(), &key);
    /// let entry: RawEntryMut<&str, u32, _> = map.raw_entry_mut().from_key_hashed_nocheck(hash, &key);
    /// entry.insert(key, 100);
    /// assert_eq!(map[&"a"], 100);
    /// ```
    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_key_hashed_nocheck<Q: ?Sized>(self, hash: u64, k: &Q) -> RawEntryMut<'a, K, V, S, A>
    where
        Q: Equivalent<K>,
    {
        self.from_hash(hash, equivalent(k))
    }
}

impl<'a, K, V, S, A: Allocator> RawEntryBuilderMut<'a, K, V, S, A> {
    /// Creates a `RawEntryMut` from the given hash and matching function.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// let key = "a";
    /// let hash = compute_hash(map.hasher(), &key);
    /// let entry: RawEntryMut<&str, u32, _> = map.raw_entry_mut().from_hash(hash, |k| k == &key);
    /// entry.insert(key, 100);
    /// assert_eq!(map[&"a"], 100);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_hash<F>(self, hash: u64, is_match: F) -> RawEntryMut<'a, K, V, S, A>
    where
        for<'b> F: FnMut(&'b K) -> bool,
    {
        self.search(hash, is_match)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn search<F>(self, hash: u64, mut is_match: F) -> RawEntryMut<'a, K, V, S, A>
    where
        for<'b> F: FnMut(&'b K) -> bool,
    {
        match self.map.table.find(hash, |(k, _)| is_match(k)) {
            Some(elem) => RawEntryMut::Occupied(RawOccupiedEntryMut {
                elem,
                table: &mut self.map.table,
                hash_builder: &self.map.hash_builder,
            }),
            None => RawEntryMut::Vacant(RawVacantEntryMut {
                table: &mut self.map.table,
                hash_builder: &self.map.hash_builder,
            }),
        }
    }
}

impl<'a, K, V, S, A: Allocator> RawEntryBuilder<'a, K, V, S, A> {
    /// Access an immutable entry by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    /// let key = "a";
    /// assert_eq!(map.raw_entry().from_key(&key), Some((&"a", &100)));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_key<Q: ?Sized>(self, k: &Q) -> Option<(&'a K, &'a V)>
    where
        S: BuildHasher,
        Q: Hash + Equivalent<K>,
    {
        let hash = make_hash::<Q, S>(&self.map.hash_builder, k);
        self.from_key_hashed_nocheck(hash, k)
    }

    /// Access an immutable entry by a key and its hash.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::HashMap;
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// let map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    /// let key = "a";
    /// let hash = compute_hash(map.hasher(), &key);
    /// assert_eq!(map.raw_entry().from_key_hashed_nocheck(hash, &key), Some((&"a", &100)));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_key_hashed_nocheck<Q: ?Sized>(self, hash: u64, k: &Q) -> Option<(&'a K, &'a V)>
    where
        Q: Equivalent<K>,
    {
        self.from_hash(hash, equivalent(k))
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn search<F>(self, hash: u64, mut is_match: F) -> Option<(&'a K, &'a V)>
    where
        F: FnMut(&K) -> bool,
    {
        match self.map.table.get(hash, |(k, _)| is_match(k)) {
            Some((key, value)) => Some((key, value)),
            None => None,
        }
    }

    /// Access an immutable entry by hash and matching function.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::HashMap;
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// let map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    /// let key = "a";
    /// let hash = compute_hash(map.hasher(), &key);
    /// assert_eq!(map.raw_entry().from_hash(hash, |k| k == &key), Some((&"a", &100)));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_hash<F>(self, hash: u64, is_match: F) -> Option<(&'a K, &'a V)>
    where
        F: FnMut(&K) -> bool,
    {
        self.search(hash, is_match)
    }
}

impl<'a, K, V, S, A: Allocator> RawEntryMut<'a, K, V, S, A> {
    /// Sets the value of the entry, and returns a RawOccupiedEntryMut.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// let entry = map.raw_entry_mut().from_key("horseyland").insert("horseyland", 37);
    ///
    /// assert_eq!(entry.remove_entry(), ("horseyland", 37));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, key: K, value: V) -> RawOccupiedEntryMut<'a, K, V, S, A>
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            RawEntryMut::Vacant(entry) => entry.insert_entry(key, value),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// mutable references to the key and value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// map.raw_entry_mut().from_key("poneyland").or_insert("poneyland", 3);
    /// assert_eq!(map["poneyland"], 3);
    ///
    /// *map.raw_entry_mut().from_key("poneyland").or_insert("poneyland", 10).1 *= 2;
    /// assert_eq!(map["poneyland"], 6);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert(self, default_key: K, default_val: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(entry) => entry.into_key_value(),
            RawEntryMut::Vacant(entry) => entry.insert(default_key, default_val),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns mutable references to the key and value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, String> = HashMap::new();
    ///
    /// map.raw_entry_mut().from_key("poneyland").or_insert_with(|| {
    ///     ("poneyland", "hoho".to_string())
    /// });
    ///
    /// assert_eq!(map["poneyland"], "hoho".to_string());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert_with<F>(self, default: F) -> (&'a mut K, &'a mut V)
    where
        F: FnOnce() -> (K, V),
        K: Hash,
        S: BuildHasher,
    {
        match self {
            RawEntryMut::Occupied(entry) => entry.into_key_value(),
            RawEntryMut::Vacant(entry) => {
                let (k, v) = default();
                entry.insert(k, v)
            }
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// map.raw_entry_mut()
    ///    .from_key("poneyland")
    ///    .and_modify(|_k, v| { *v += 1 })
    ///    .or_insert("poneyland", 42);
    /// assert_eq!(map["poneyland"], 42);
    ///
    /// map.raw_entry_mut()
    ///    .from_key("poneyland")
    ///    .and_modify(|_k, v| { *v += 1 })
    ///    .or_insert("poneyland", 0);
    /// assert_eq!(map["poneyland"], 43);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut K, &mut V),
    {
        match self {
            RawEntryMut::Occupied(mut entry) => {
                {
                    let (k, v) = entry.get_key_value_mut();
                    f(k, v);
                }
                RawEntryMut::Occupied(entry)
            }
            RawEntryMut::Vacant(entry) => RawEntryMut::Vacant(entry),
        }
    }

    /// Provides shared access to the key and owned access to the value of
    /// an occupied entry and allows to replace or remove it based on the
    /// value of the returned option.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::RawEntryMut;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// let entry = map
    ///     .raw_entry_mut()
    ///     .from_key("poneyland")
    ///     .and_replace_entry_with(|_k, _v| panic!());
    ///
    /// match entry {
    ///     RawEntryMut::Vacant(_) => {},
    ///     RawEntryMut::Occupied(_) => panic!(),
    /// }
    ///
    /// map.insert("poneyland", 42);
    ///
    /// let entry = map
    ///     .raw_entry_mut()
    ///     .from_key("poneyland")
    ///     .and_replace_entry_with(|k, v| {
    ///         assert_eq!(k, &"poneyland");
    ///         assert_eq!(v, 42);
    ///         Some(v + 1)
    ///     });
    ///
    /// match entry {
    ///     RawEntryMut::Occupied(e) => {
    ///         assert_eq!(e.key(), &"poneyland");
    ///         assert_eq!(e.get(), &43);
    ///     },
    ///     RawEntryMut::Vacant(_) => panic!(),
    /// }
    ///
    /// assert_eq!(map["poneyland"], 43);
    ///
    /// let entry = map
    ///     .raw_entry_mut()
    ///     .from_key("poneyland")
    ///     .and_replace_entry_with(|_k, _v| None);
    ///
    /// match entry {
    ///     RawEntryMut::Vacant(_) => {},
    ///     RawEntryMut::Occupied(_) => panic!(),
    /// }
    ///
    /// assert!(!map.contains_key("poneyland"));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_replace_entry_with<F>(self, f: F) -> Self
    where
        F: FnOnce(&K, V) -> Option<V>,
    {
        match self {
            RawEntryMut::Occupied(entry) => entry.replace_entry_with(f),
            RawEntryMut::Vacant(_) => self,
        }
    }
}

impl<'a, K, V, S, A: Allocator> RawOccupiedEntryMut<'a, K, V, S, A> {
    /// Gets a reference to the key in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => assert_eq!(o.key(), &"a")
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &K {
        unsafe { &self.elem.as_ref().0 }
    }

    /// Gets a mutable reference to the key in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    /// use std::rc::Rc;
    ///
    /// let key_one = Rc::new("a");
    /// let key_two = Rc::new("a");
    ///
    /// let mut map: HashMap<Rc<&str>, u32> = HashMap::new();
    /// map.insert(key_one.clone(), 10);
    ///
    /// assert_eq!(map[&key_one], 10);
    /// assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    /// match map.raw_entry_mut().from_key(&key_one) {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(mut o) => {
    ///         *o.key_mut() = key_two.clone();
    ///     }
    /// }
    /// assert_eq!(map[&key_two], 10);
    /// assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key_mut(&mut self) -> &mut K {
        unsafe { &mut self.elem.as_mut().0 }
    }

    /// Converts the entry into a mutable reference to the key in the entry
    /// with a lifetime bound to the map itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    /// use std::rc::Rc;
    ///
    /// let key_one = Rc::new("a");
    /// let key_two = Rc::new("a");
    ///
    /// let mut map: HashMap<Rc<&str>, u32> = HashMap::new();
    /// map.insert(key_one.clone(), 10);
    ///
    /// assert_eq!(map[&key_one], 10);
    /// assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    /// let inside_key: &mut Rc<&str>;
    ///
    /// match map.raw_entry_mut().from_key(&key_one) {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => inside_key = o.into_key(),
    /// }
    /// *inside_key = key_two.clone();
    ///
    /// assert_eq!(map[&key_two], 10);
    /// assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_key(self) -> &'a mut K {
        unsafe { &mut self.elem.as_mut().0 }
    }

    /// Gets a reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => assert_eq!(o.get(), &100),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &V {
        unsafe { &self.elem.as_ref().1 }
    }

    /// Converts the OccupiedEntry into a mutable reference to the value in the entry
    /// with a lifetime bound to the map itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// let value: &mut u32;
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => value = o.into_mut(),
    /// }
    /// *value += 900;
    ///
    /// assert_eq!(map[&"a"], 1000);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut self.elem.as_mut().1 }
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(mut o) => *o.get_mut() += 900,
    /// }
    ///
    /// assert_eq!(map[&"a"], 1000);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.elem.as_mut().1 }
    }

    /// Gets a reference to the key and value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => assert_eq!(o.get_key_value(), (&"a", &100)),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_key_value(&self) -> (&K, &V) {
        unsafe {
            let (key, value) = self.elem.as_ref();
            (key, value)
        }
    }

    /// Gets a mutable reference to the key and value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    /// use std::rc::Rc;
    ///
    /// let key_one = Rc::new("a");
    /// let key_two = Rc::new("a");
    ///
    /// let mut map: HashMap<Rc<&str>, u32> = HashMap::new();
    /// map.insert(key_one.clone(), 10);
    ///
    /// assert_eq!(map[&key_one], 10);
    /// assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    /// match map.raw_entry_mut().from_key(&key_one) {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(mut o) => {
    ///         let (inside_key, inside_value) = o.get_key_value_mut();
    ///         *inside_key = key_two.clone();
    ///         *inside_value = 100;
    ///     }
    /// }
    /// assert_eq!(map[&key_two], 100);
    /// assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_key_value_mut(&mut self) -> (&mut K, &mut V) {
        unsafe {
            let &mut (ref mut key, ref mut value) = self.elem.as_mut();
            (key, value)
        }
    }

    /// Converts the OccupiedEntry into a mutable reference to the key and value in the entry
    /// with a lifetime bound to the map itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    /// use std::rc::Rc;
    ///
    /// let key_one = Rc::new("a");
    /// let key_two = Rc::new("a");
    ///
    /// let mut map: HashMap<Rc<&str>, u32> = HashMap::new();
    /// map.insert(key_one.clone(), 10);
    ///
    /// assert_eq!(map[&key_one], 10);
    /// assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    /// let inside_key: &mut Rc<&str>;
    /// let inside_value: &mut u32;
    /// match map.raw_entry_mut().from_key(&key_one) {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => {
    ///         let tuple = o.into_key_value();
    ///         inside_key = tuple.0;
    ///         inside_value = tuple.1;
    ///     }
    /// }
    /// *inside_key = key_two.clone();
    /// *inside_value = 100;
    /// assert_eq!(map[&key_two], 100);
    /// assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_key_value(self) -> (&'a mut K, &'a mut V) {
        unsafe {
            let &mut (ref mut key, ref mut value) = self.elem.as_mut();
            (key, value)
        }
    }

    /// Sets the value of the entry, and returns the entry's old value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(mut o) => assert_eq!(o.insert(1000), 100),
    /// }
    ///
    /// assert_eq!(map[&"a"], 1000);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Sets the value of the entry, and returns the entry's old value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    /// use std::rc::Rc;
    ///
    /// let key_one = Rc::new("a");
    /// let key_two = Rc::new("a");
    ///
    /// let mut map: HashMap<Rc<&str>, u32> = HashMap::new();
    /// map.insert(key_one.clone(), 10);
    ///
    /// assert_eq!(map[&key_one], 10);
    /// assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    /// match map.raw_entry_mut().from_key(&key_one) {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(mut o) => {
    ///         let old_key = o.insert_key(key_two.clone());
    ///         assert!(Rc::ptr_eq(&old_key, &key_one));
    ///     }
    /// }
    /// assert_eq!(map[&key_two], 10);
    /// assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_key(&mut self, key: K) -> K {
        mem::replace(self.key_mut(), key)
    }

    /// Takes the value out of the entry, and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => assert_eq!(o.remove(), 100),
    /// }
    /// assert_eq!(map.get(&"a"), None);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Take the ownership of the key and value from the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => assert_eq!(o.remove_entry(), ("a", 100)),
    /// }
    /// assert_eq!(map.get(&"a"), None);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove_entry(self) -> (K, V) {
        unsafe { self.table.remove(self.elem).0 }
    }

    /// Provides shared access to the key and owned access to the value of
    /// the entry and allows to replace or remove it based on the
    /// value of the returned option.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// let raw_entry = match map.raw_entry_mut().from_key(&"a") {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => o.replace_entry_with(|k, v| {
    ///         assert_eq!(k, &"a");
    ///         assert_eq!(v, 100);
    ///         Some(v + 900)
    ///     }),
    /// };
    /// let raw_entry = match raw_entry {
    ///     RawEntryMut::Vacant(_) => panic!(),
    ///     RawEntryMut::Occupied(o) => o.replace_entry_with(|k, v| {
    ///         assert_eq!(k, &"a");
    ///         assert_eq!(v, 1000);
    ///         None
    ///     }),
    /// };
    /// match raw_entry {
    ///     RawEntryMut::Vacant(_) => { },
    ///     RawEntryMut::Occupied(_) => panic!(),
    /// };
    /// assert_eq!(map.get(&"a"), None);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_entry_with<F>(self, f: F) -> RawEntryMut<'a, K, V, S, A>
    where
        F: FnOnce(&K, V) -> Option<V>,
    {
        unsafe {
            let still_occupied = self
                .table
                .replace_bucket_with(self.elem.clone(), |(key, value)| {
                    f(&key, value).map(|new_value| (key, new_value))
                });

            if still_occupied {
                RawEntryMut::Occupied(self)
            } else {
                RawEntryMut::Vacant(RawVacantEntryMut {
                    table: self.table,
                    hash_builder: self.hash_builder,
                })
            }
        }
    }
}

impl<'a, K, V, S, A: Allocator> RawVacantEntryMut<'a, K, V, S, A> {
    /// Sets the value of the entry with the VacantEntry's key,
    /// and returns a mutable reference to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.raw_entry_mut().from_key(&"c") {
    ///     RawEntryMut::Occupied(_) => panic!(),
    ///     RawEntryMut::Vacant(v) => assert_eq!(v.insert("c", 300), (&mut "c", &mut 300)),
    /// }
    ///
    /// assert_eq!(map[&"c"], 300);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash = make_hash::<K, S>(self.hash_builder, &key);
        self.insert_hashed_nocheck(hash, key, value)
    }

    /// Sets the value of the entry with the VacantEntry's key,
    /// and returns a mutable reference to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// fn compute_hash<K: Hash + ?Sized, S: BuildHasher>(hash_builder: &S, key: &K) -> u64 {
    ///     use core::hash::Hasher;
    ///     let mut state = hash_builder.build_hasher();
    ///     key.hash(&mut state);
    ///     state.finish()
    /// }
    ///
    /// let mut map: HashMap<&str, u32> = [("a", 100), ("b", 200)].into();
    /// let key = "c";
    /// let hash = compute_hash(map.hasher(), &key);
    ///
    /// match map.raw_entry_mut().from_key_hashed_nocheck(hash, &key) {
    ///     RawEntryMut::Occupied(_) => panic!(),
    ///     RawEntryMut::Vacant(v) => assert_eq!(
    ///         v.insert_hashed_nocheck(hash, key, 300),
    ///         (&mut "c", &mut 300)
    ///     ),
    /// }
    ///
    /// assert_eq!(map[&"c"], 300);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    #[allow(clippy::shadow_unrelated)]
    pub fn insert_hashed_nocheck(self, hash: u64, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let &mut (ref mut k, ref mut v) = self.table.insert_entry(
            hash,
            (key, value),
            make_hasher::<_, V, S>(self.hash_builder),
        );
        (k, v)
    }

    /// Set the value of an entry with a custom hasher function.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::{BuildHasher, Hash};
    /// use hashbrown::hash_map::{HashMap, RawEntryMut};
    ///
    /// fn make_hasher<K, S>(hash_builder: &S) -> impl Fn(&K) -> u64 + '_
    /// where
    ///     K: Hash + ?Sized,
    ///     S: BuildHasher,
    /// {
    ///     move |key: &K| {
    ///         use core::hash::Hasher;
    ///         let mut state = hash_builder.build_hasher();
    ///         key.hash(&mut state);
    ///         state.finish()
    ///     }
    /// }
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// let key = "a";
    /// let hash_builder = map.hasher().clone();
    /// let hash = make_hasher(&hash_builder)(&key);
    ///
    /// match map.raw_entry_mut().from_hash(hash, |q| q == &key) {
    ///     RawEntryMut::Occupied(_) => panic!(),
    ///     RawEntryMut::Vacant(v) => assert_eq!(
    ///         v.insert_with_hasher(hash, key, 100, make_hasher(&hash_builder)),
    ///         (&mut "a", &mut 100)
    ///     ),
    /// }
    /// map.extend([("b", 200), ("c", 300), ("d", 400), ("e", 500), ("f", 600)]);
    /// assert_eq!(map[&"a"], 100);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_with_hasher<H>(
        self,
        hash: u64,
        key: K,
        value: V,
        hasher: H,
    ) -> (&'a mut K, &'a mut V)
    where
        H: Fn(&K) -> u64,
    {
        let &mut (ref mut k, ref mut v) = self
            .table
            .insert_entry(hash, (key, value), |x| hasher(&x.0));
        (k, v)
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn insert_entry(self, key: K, value: V) -> RawOccupiedEntryMut<'a, K, V, S, A>
    where
        K: Hash,
        S: BuildHasher,
    {
        let hash = make_hash::<K, S>(self.hash_builder, &key);
        let elem = self.table.insert(
            hash,
            (key, value),
            make_hasher::<_, V, S>(self.hash_builder),
        );
        RawOccupiedEntryMut {
            elem,
            table: self.table,
            hash_builder: self.hash_builder,
        }
    }
}

impl<K, V, S, A: Allocator> Debug for RawEntryBuilderMut<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilder").finish()
    }
}

impl<K: Debug, V: Debug, S, A: Allocator> Debug for RawEntryMut<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RawEntryMut::Vacant(ref v) => f.debug_tuple("RawEntry").field(v).finish(),
            RawEntryMut::Occupied(ref o) => f.debug_tuple("RawEntry").field(o).finish(),
        }
    }
}

impl<K: Debug, V: Debug, S, A: Allocator> Debug for RawOccupiedEntryMut<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawOccupiedEntryMut")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}

impl<K, V, S, A: Allocator> Debug for RawVacantEntryMut<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawVacantEntryMut").finish()
    }
}

impl<K, V, S, A: Allocator> Debug for RawEntryBuilder<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilder").finish()
    }
}

/// A view into a single entry in a map, which may either be vacant or occupied.
///
/// This `enum` is constructed from the [`entry`] method on [`HashMap`].
///
/// [`HashMap`]: struct.HashMap.html
/// [`entry`]: struct.HashMap.html#method.entry
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{Entry, HashMap, OccupiedEntry};
///
/// let mut map = HashMap::new();
/// map.extend([("a", 10), ("b", 20), ("c", 30)]);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (insert)
/// let entry: Entry<_, _, _> = map.entry("a");
/// let _raw_o: OccupiedEntry<_, _, _> = entry.insert(1);
/// assert_eq!(map.len(), 3);
/// // Nonexistent key (insert)
/// map.entry("d").insert(4);
///
/// // Existing key (or_insert)
/// let v = map.entry("b").or_insert(2);
/// assert_eq!(std::mem::replace(v, 2), 20);
/// // Nonexistent key (or_insert)
/// map.entry("e").or_insert(5);
///
/// // Existing key (or_insert_with)
/// let v = map.entry("c").or_insert_with(|| 3);
/// assert_eq!(std::mem::replace(v, 3), 30);
/// // Nonexistent key (or_insert_with)
/// map.entry("f").or_insert_with(|| 6);
///
/// println!("Our HashMap: {:?}", map);
///
/// let mut vec: Vec<_> = map.iter().map(|(&k, &v)| (k, v)).collect();
/// // The `Iter` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, [("a", 1), ("b", 2), ("c", 3), ("d", 4), ("e", 5), ("f", 6)]);
/// ```
pub enum Entry<'a, K, V, S, A = Global>
where
    A: Allocator,
{
    /// An occupied entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{Entry, HashMap};
    /// let mut map: HashMap<_, _> = [("a", 100), ("b", 200)].into();
    ///
    /// match map.entry("a") {
    ///     Entry::Vacant(_) => unreachable!(),
    ///     Entry::Occupied(_) => { }
    /// }
    /// ```
    Occupied(OccupiedEntry<'a, K, V, S, A>),

    /// A vacant entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{Entry, HashMap};
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    ///
    /// match map.entry("a") {
    ///     Entry::Occupied(_) => unreachable!(),
    ///     Entry::Vacant(_) => { }
    /// }
    /// ```
    Vacant(VacantEntry<'a, K, V, S, A>),
}

impl<K: Debug, V: Debug, S, A: Allocator> Debug for Entry<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Entry::Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Entry::Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}

/// A view into an occupied entry in a `HashMap`.
/// It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{Entry, HashMap, OccupiedEntry};
///
/// let mut map = HashMap::new();
/// map.extend([("a", 10), ("b", 20), ("c", 30)]);
///
/// let _entry_o: OccupiedEntry<_, _, _> = map.entry("a").insert(100);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (insert and update)
/// match map.entry("a") {
///     Entry::Vacant(_) => unreachable!(),
///     Entry::Occupied(mut view) => {
///         assert_eq!(view.get(), &100);
///         let v = view.get_mut();
///         *v *= 10;
///         assert_eq!(view.insert(1111), 1000);
///     }
/// }
///
/// assert_eq!(map[&"a"], 1111);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (take)
/// match map.entry("c") {
///     Entry::Vacant(_) => unreachable!(),
///     Entry::Occupied(view) => {
///         assert_eq!(view.remove_entry(), ("c", 30));
///     }
/// }
/// assert_eq!(map.get(&"c"), None);
/// assert_eq!(map.len(), 2);
/// ```
pub struct OccupiedEntry<'a, K, V, S = DefaultHashBuilder, A: Allocator = Global> {
    hash: u64,
    key: Option<K>,
    elem: Bucket<(K, V)>,
    table: &'a mut HashMap<K, V, S, A>,
}

unsafe impl<K, V, S, A> Send for OccupiedEntry<'_, K, V, S, A>
where
    K: Send,
    V: Send,
    S: Send,
    A: Send + Allocator,
{
}
unsafe impl<K, V, S, A> Sync for OccupiedEntry<'_, K, V, S, A>
where
    K: Sync,
    V: Sync,
    S: Sync,
    A: Sync + Allocator,
{
}

impl<K: Debug, V: Debug, S, A: Allocator> Debug for OccupiedEntry<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}

/// A view into a vacant entry in a `HashMap`.
/// It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{Entry, HashMap, VacantEntry};
///
/// let mut map = HashMap::<&str, i32>::new();
///
/// let entry_v: VacantEntry<_, _, _> = match map.entry("a") {
///     Entry::Vacant(view) => view,
///     Entry::Occupied(_) => unreachable!(),
/// };
/// entry_v.insert(10);
/// assert!(map[&"a"] == 10 && map.len() == 1);
///
/// // Nonexistent key (insert and update)
/// match map.entry("b") {
///     Entry::Occupied(_) => unreachable!(),
///     Entry::Vacant(view) => {
///         let value = view.insert(2);
///         assert_eq!(*value, 2);
///         *value = 20;
///     }
/// }
/// assert!(map[&"b"] == 20 && map.len() == 2);
/// ```
pub struct VacantEntry<'a, K, V, S = DefaultHashBuilder, A: Allocator = Global> {
    hash: u64,
    key: K,
    table: &'a mut HashMap<K, V, S, A>,
}

impl<K: Debug, V, S, A: Allocator> Debug for VacantEntry<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}

/// A view into a single entry in a map, which may either be vacant or occupied,
/// with any borrowed form of the map's key type.
///
///
/// This `enum` is constructed from the [`entry_ref`] method on [`HashMap`].
///
/// [`Hash`] and [`Eq`] on the borrowed form of the map's key type *must* match those
/// for the key type. It also require that key may be constructed from the borrowed
/// form through the [`From`] trait.
///
/// [`HashMap`]: struct.HashMap.html
/// [`entry_ref`]: struct.HashMap.html#method.entry_ref
/// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
/// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
/// [`From`]: https://doc.rust-lang.org/std/convert/trait.From.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{EntryRef, HashMap, OccupiedEntryRef};
///
/// let mut map = HashMap::new();
/// map.extend([("a".to_owned(), 10), ("b".into(), 20), ("c".into(), 30)]);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (insert)
/// let key = String::from("a");
/// let entry: EntryRef<_, _, _, _> = map.entry_ref(&key);
/// let _raw_o: OccupiedEntryRef<_, _, _, _> = entry.insert(1);
/// assert_eq!(map.len(), 3);
/// // Nonexistent key (insert)
/// map.entry_ref("d").insert(4);
///
/// // Existing key (or_insert)
/// let v = map.entry_ref("b").or_insert(2);
/// assert_eq!(std::mem::replace(v, 2), 20);
/// // Nonexistent key (or_insert)
/// map.entry_ref("e").or_insert(5);
///
/// // Existing key (or_insert_with)
/// let v = map.entry_ref("c").or_insert_with(|| 3);
/// assert_eq!(std::mem::replace(v, 3), 30);
/// // Nonexistent key (or_insert_with)
/// map.entry_ref("f").or_insert_with(|| 6);
///
/// println!("Our HashMap: {:?}", map);
///
/// for (key, value) in ["a", "b", "c", "d", "e", "f"].into_iter().zip(1..=6) {
///     assert_eq!(map[key], value)
/// }
/// assert_eq!(map.len(), 6);
/// ```
pub enum EntryRef<'a, 'b, K, Q: ?Sized, V, S, A = Global>
where
    A: Allocator,
{
    /// An occupied entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    /// let mut map: HashMap<_, _> = [("a".to_owned(), 100), ("b".into(), 200)].into();
    ///
    /// match map.entry_ref("a") {
    ///     EntryRef::Vacant(_) => unreachable!(),
    ///     EntryRef::Occupied(_) => { }
    /// }
    /// ```
    Occupied(OccupiedEntryRef<'a, 'b, K, Q, V, S, A>),

    /// A vacant entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    /// let mut map: HashMap<String, i32> = HashMap::new();
    ///
    /// match map.entry_ref("a") {
    ///     EntryRef::Occupied(_) => unreachable!(),
    ///     EntryRef::Vacant(_) => { }
    /// }
    /// ```
    Vacant(VacantEntryRef<'a, 'b, K, Q, V, S, A>),
}

impl<K: Borrow<Q>, Q: ?Sized + Debug, V: Debug, S, A: Allocator> Debug
    for EntryRef<'_, '_, K, Q, V, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            EntryRef::Vacant(ref v) => f.debug_tuple("EntryRef").field(v).finish(),
            EntryRef::Occupied(ref o) => f.debug_tuple("EntryRef").field(o).finish(),
        }
    }
}

enum KeyOrRef<'a, K, Q: ?Sized> {
    Borrowed(&'a Q),
    Owned(K),
}

impl<'a, K, Q: ?Sized> KeyOrRef<'a, K, Q> {
    fn into_owned(self) -> K
    where
        K: From<&'a Q>,
    {
        match self {
            Self::Borrowed(borrowed) => borrowed.into(),
            Self::Owned(owned) => owned,
        }
    }
}

impl<'a, K: Borrow<Q>, Q: ?Sized> AsRef<Q> for KeyOrRef<'a, K, Q> {
    fn as_ref(&self) -> &Q {
        match self {
            Self::Borrowed(borrowed) => borrowed,
            Self::Owned(owned) => owned.borrow(),
        }
    }
}

/// A view into an occupied entry in a `HashMap`.
/// It is part of the [`EntryRef`] enum.
///
/// [`EntryRef`]: enum.EntryRef.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{EntryRef, HashMap, OccupiedEntryRef};
///
/// let mut map = HashMap::new();
/// map.extend([("a".to_owned(), 10), ("b".into(), 20), ("c".into(), 30)]);
///
/// let key = String::from("a");
/// let _entry_o: OccupiedEntryRef<_, _, _, _> = map.entry_ref(&key).insert(100);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (insert and update)
/// match map.entry_ref("a") {
///     EntryRef::Vacant(_) => unreachable!(),
///     EntryRef::Occupied(mut view) => {
///         assert_eq!(view.get(), &100);
///         let v = view.get_mut();
///         *v *= 10;
///         assert_eq!(view.insert(1111), 1000);
///     }
/// }
///
/// assert_eq!(map["a"], 1111);
/// assert_eq!(map.len(), 3);
///
/// // Existing key (take)
/// match map.entry_ref("c") {
///     EntryRef::Vacant(_) => unreachable!(),
///     EntryRef::Occupied(view) => {
///         assert_eq!(view.remove_entry(), ("c".to_owned(), 30));
///     }
/// }
/// assert_eq!(map.get("c"), None);
/// assert_eq!(map.len(), 2);
/// ```
pub struct OccupiedEntryRef<'a, 'b, K, Q: ?Sized, V, S, A: Allocator = Global> {
    hash: u64,
    key: Option<KeyOrRef<'b, K, Q>>,
    elem: Bucket<(K, V)>,
    table: &'a mut HashMap<K, V, S, A>,
}

unsafe impl<'a, 'b, K, Q, V, S, A> Send for OccupiedEntryRef<'a, 'b, K, Q, V, S, A>
where
    K: Send,
    Q: Sync + ?Sized,
    V: Send,
    S: Send,
    A: Send + Allocator,
{
}
unsafe impl<'a, 'b, K, Q, V, S, A> Sync for OccupiedEntryRef<'a, 'b, K, Q, V, S, A>
where
    K: Sync,
    Q: Sync + ?Sized,
    V: Sync,
    S: Sync,
    A: Sync + Allocator,
{
}

impl<K: Borrow<Q>, Q: ?Sized + Debug, V: Debug, S, A: Allocator> Debug
    for OccupiedEntryRef<'_, '_, K, Q, V, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntryRef")
            .field("key", &self.key().borrow())
            .field("value", &self.get())
            .finish()
    }
}

/// A view into a vacant entry in a `HashMap`.
/// It is part of the [`EntryRef`] enum.
///
/// [`EntryRef`]: enum.EntryRef.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{EntryRef, HashMap, VacantEntryRef};
///
/// let mut map = HashMap::<String, i32>::new();
///
/// let entry_v: VacantEntryRef<_, _, _, _> = match map.entry_ref("a") {
///     EntryRef::Vacant(view) => view,
///     EntryRef::Occupied(_) => unreachable!(),
/// };
/// entry_v.insert(10);
/// assert!(map["a"] == 10 && map.len() == 1);
///
/// // Nonexistent key (insert and update)
/// match map.entry_ref("b") {
///     EntryRef::Occupied(_) => unreachable!(),
///     EntryRef::Vacant(view) => {
///         let value = view.insert(2);
///         assert_eq!(*value, 2);
///         *value = 20;
///     }
/// }
/// assert!(map["b"] == 20 && map.len() == 2);
/// ```
pub struct VacantEntryRef<'a, 'b, K, Q: ?Sized, V, S, A: Allocator = Global> {
    hash: u64,
    key: KeyOrRef<'b, K, Q>,
    table: &'a mut HashMap<K, V, S, A>,
}

impl<K: Borrow<Q>, Q: ?Sized + Debug, V, S, A: Allocator> Debug
    for VacantEntryRef<'_, '_, K, Q, V, S, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntryRef").field(&self.key()).finish()
    }
}

/// The error returned by [`try_insert`](HashMap::try_insert) when the key already exists.
///
/// Contains the occupied entry, and the value that was not inserted.
///
/// # Examples
///
/// ```
/// use hashbrown::hash_map::{HashMap, OccupiedError};
///
/// let mut map: HashMap<_, _> = [("a", 10), ("b", 20)].into();
///
/// // try_insert method returns mutable reference to the value if keys are vacant,
/// // but if the map did have key present, nothing is updated, and the provided
/// // value is returned inside `Err(_)` variant
/// match map.try_insert("a", 100) {
///     Err(OccupiedError { mut entry, value }) => {
///         assert_eq!(entry.key(), &"a");
///         assert_eq!(value, 100);
///         assert_eq!(entry.insert(100), 10)
///     }
///     _ => unreachable!(),
/// }
/// assert_eq!(map[&"a"], 100);
/// ```
pub struct OccupiedError<'a, K, V, S, A: Allocator = Global> {
    /// The entry in the map that was already occupied.
    pub entry: OccupiedEntry<'a, K, V, S, A>,
    /// The value which was not inserted, because the entry was already occupied.
    pub value: V,
}

impl<K: Debug, V: Debug, S, A: Allocator> Debug for OccupiedError<'_, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedError")
            .field("key", self.entry.key())
            .field("old_value", self.entry.get())
            .field("new_value", &self.value)
            .finish()
    }
}

impl<'a, K: Debug, V: Debug, S, A: Allocator> fmt::Display for OccupiedError<'a, K, V, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to insert {:?}, key {:?} already exists with value {:?}",
            self.value,
            self.entry.key(),
            self.entry.get(),
        )
    }
}

impl<'a, K, V, S, A: Allocator> IntoIterator for &'a HashMap<K, V, S, A> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    /// Creates an iterator over the entries of a `HashMap` in arbitrary order.
    /// The iterator element type is `(&'a K, &'a V)`.
    ///
    /// Return the same `Iter` struct as by the [`iter`] method on [`HashMap`].
    ///
    /// [`iter`]: struct.HashMap.html#method.iter
    /// [`HashMap`]: struct.HashMap.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// let map_one: HashMap<_, _> = [(1, "a"), (2, "b"), (3, "c")].into();
    /// let mut map_two = HashMap::new();
    ///
    /// for (key, value) in &map_one {
    ///     println!("Key: {}, Value: {}", key, value);
    ///     map_two.insert_unique_unchecked(*key, *value);
    /// }
    ///
    /// assert_eq!(map_one, map_two);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}

impl<'a, K, V, S, A: Allocator> IntoIterator for &'a mut HashMap<K, V, S, A> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    /// Creates an iterator over the entries of a `HashMap` in arbitrary order
    /// with mutable references to the values. The iterator element type is
    /// `(&'a K, &'a mut V)`.
    ///
    /// Return the same `IterMut` struct as by the [`iter_mut`] method on
    /// [`HashMap`].
    ///
    /// [`iter_mut`]: struct.HashMap.html#method.iter_mut
    /// [`HashMap`]: struct.HashMap.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// let mut map: HashMap<_, _> = [("a", 1), ("b", 2), ("c", 3)].into();
    ///
    /// for (key, value) in &mut map {
    ///     println!("Key: {}, Value: {}", key, value);
    ///     *value *= 2;
    /// }
    ///
    /// let mut vec = map.iter().collect::<Vec<_>>();
    /// // The `Iter` iterator produces items in arbitrary order, so the
    /// // items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [(&"a", &2), (&"b", &4), (&"c", &6)]);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn into_iter(self) -> IterMut<'a, K, V> {
        self.iter_mut()
    }
}

impl<K, V, S, A: Allocator> IntoIterator for HashMap<K, V, S, A> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V, A>;

    /// Creates a consuming iterator, that is, one that moves each key-value
    /// pair out of the map in arbitrary order. The map cannot be used after
    /// calling this.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let map: HashMap<_, _> = [("a", 1), ("b", 2), ("c", 3)].into();
    ///
    /// // Not possible with .iter()
    /// let mut vec: Vec<(&str, i32)> = map.into_iter().collect();
    /// // The `IntoIter` iterator produces items in arbitrary order, so
    /// // the items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [("a", 1), ("b", 2), ("c", 3)]);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn into_iter(self) -> IntoIter<K, V, A> {
        IntoIter {
            inner: self.table.into_iter(),
        }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.inner.next() {
            Some(x) => unsafe {
                let r = x.as_ref();
                Some((&r.0, &r.1))
            },
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, x| unsafe {
            let (k, v) = x.as_ref();
            f(acc, (k, v))
        })
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<K, V> FusedIterator for Iter<'_, K, V> {}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.inner.next() {
            Some(x) => unsafe {
                let r = x.as_mut();
                Some((&r.0, &mut r.1))
            },
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, x| unsafe {
            let (k, v) = x.as_mut();
            f(acc, (k, v))
        })
    }
}
impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<K, V> FusedIterator for IterMut<'_, K, V> {}

impl<K, V> fmt::Debug for IterMut<'_, K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<K, V, A: Allocator> Iterator for IntoIter<K, V, A> {
    type Item = (K, V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<(K, V)> {
        self.inner.next()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, f)
    }
}
impl<K, V, A: Allocator> ExactSizeIterator for IntoIter<K, V, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<K, V, A: Allocator> FusedIterator for IntoIter<K, V, A> {}

impl<K: Debug, V: Debug, A: Allocator> fmt::Debug for IntoIter<K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a K> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.inner.next() {
            Some((k, _)) => Some(k),
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (k, _)| f(acc, k))
    }
}
impl<K, V> ExactSizeIterator for Keys<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<K, V> FusedIterator for Keys<'_, K, V> {}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a V> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.inner.next() {
            Some((_, v)) => Some(v),
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (_, v)| f(acc, v))
    }
}
impl<K, V> ExactSizeIterator for Values<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<K, V> FusedIterator for Values<'_, K, V> {}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a mut V> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.inner.next() {
            Some((_, v)) => Some(v),
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (_, v)| f(acc, v))
    }
}
impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}

impl<K, V: Debug> fmt::Debug for ValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.inner.iter().map(|(_, val)| val))
            .finish()
    }
}

impl<'a, K, V, A: Allocator> Iterator for Drain<'a, K, V, A> {
    type Item = (K, V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<(K, V)> {
        self.inner.next()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, f)
    }
}
impl<K, V, A: Allocator> ExactSizeIterator for Drain<'_, K, V, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
impl<K, V, A: Allocator> FusedIterator for Drain<'_, K, V, A> {}

impl<K, V, A> fmt::Debug for Drain<'_, K, V, A>
where
    K: fmt::Debug,
    V: fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, K, V, S, A: Allocator> Entry<'a, K, V, S, A> {
    /// Sets the value of the entry, and returns an OccupiedEntry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// let entry = map.entry("horseyland").insert(37);
    ///
    /// assert_eq!(entry.key(), &"horseyland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, value: V) -> OccupiedEntry<'a, K, V, S, A>
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            Entry::Vacant(entry) => entry.insert_entry(value),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry("poneyland").or_insert(3);
    /// assert_eq!(map["poneyland"], 3);
    ///
    /// // existing key
    /// *map.entry("poneyland").or_insert(10) *= 2;
    /// assert_eq!(map["poneyland"], 6);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry("poneyland").or_insert_with(|| 3);
    /// assert_eq!(map["poneyland"], 3);
    ///
    /// // existing key
    /// *map.entry("poneyland").or_insert_with(|| 10) *= 2;
    /// assert_eq!(map["poneyland"], 6);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Ensures a value is in the entry by inserting, if empty, the result of the default function.
    /// This method allows for generating key-derived values for insertion by providing the default
    /// function a reference to the key that was moved during the `.entry(key)` method call.
    ///
    /// The reference to the moved key is provided so that cloning or copying the key is
    /// unnecessary, unlike with `.or_insert_with(|| ... )`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, usize> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry("poneyland").or_insert_with_key(|key| key.chars().count());
    /// assert_eq!(map["poneyland"], 9);
    ///
    /// // existing key
    /// *map.entry("poneyland").or_insert_with_key(|key| key.chars().count() * 10) *= 2;
    /// assert_eq!(map["poneyland"], 18);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    /// Returns a reference to this entry's key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(3);
    /// // existing key
    /// assert_eq!(map.entry("poneyland").key(), &"poneyland");
    /// // nonexistent key
    /// assert_eq!(map.entry("horseland").key(), &"horseland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &K {
        match *self {
            Entry::Occupied(ref entry) => entry.key(),
            Entry::Vacant(ref entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// map.entry("poneyland")
    ///    .and_modify(|e| { *e += 1 })
    ///    .or_insert(42);
    /// assert_eq!(map["poneyland"], 42);
    ///
    /// map.entry("poneyland")
    ///    .and_modify(|e| { *e += 1 })
    ///    .or_insert(42);
    /// assert_eq!(map["poneyland"], 43);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }

    /// Provides shared access to the key and owned access to the value of
    /// an occupied entry and allows to replace or remove it based on the
    /// value of the returned option.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// let entry = map
    ///     .entry("poneyland")
    ///     .and_replace_entry_with(|_k, _v| panic!());
    ///
    /// match entry {
    ///     Entry::Vacant(e) => {
    ///         assert_eq!(e.key(), &"poneyland");
    ///     }
    ///     Entry::Occupied(_) => panic!(),
    /// }
    ///
    /// map.insert("poneyland", 42);
    ///
    /// let entry = map
    ///     .entry("poneyland")
    ///     .and_replace_entry_with(|k, v| {
    ///         assert_eq!(k, &"poneyland");
    ///         assert_eq!(v, 42);
    ///         Some(v + 1)
    ///     });
    ///
    /// match entry {
    ///     Entry::Occupied(e) => {
    ///         assert_eq!(e.key(), &"poneyland");
    ///         assert_eq!(e.get(), &43);
    ///     }
    ///     Entry::Vacant(_) => panic!(),
    /// }
    ///
    /// assert_eq!(map["poneyland"], 43);
    ///
    /// let entry = map
    ///     .entry("poneyland")
    ///     .and_replace_entry_with(|_k, _v| None);
    ///
    /// match entry {
    ///     Entry::Vacant(e) => assert_eq!(e.key(), &"poneyland"),
    ///     Entry::Occupied(_) => panic!(),
    /// }
    ///
    /// assert!(!map.contains_key("poneyland"));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_replace_entry_with<F>(self, f: F) -> Self
    where
        F: FnOnce(&K, V) -> Option<V>,
    {
        match self {
            Entry::Occupied(entry) => entry.replace_entry_with(f),
            Entry::Vacant(_) => self,
        }
    }
}

impl<'a, K, V: Default, S, A: Allocator> Entry<'a, K, V, S, A> {
    /// Ensures a value is in the entry by inserting the default value if empty,
    /// and returns a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, Option<u32>> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry("poneyland").or_default();
    /// assert_eq!(map["poneyland"], None);
    ///
    /// map.insert("horseland", Some(3));
    ///
    /// // existing key
    /// assert_eq!(map.entry("horseland").or_default(), &mut Some(3));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_default(self) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Default::default()),
        }
    }
}

impl<'a, K, V, S, A: Allocator> OccupiedEntry<'a, K, V, S, A> {
    /// Gets a reference to the key in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{Entry, HashMap};
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// match map.entry("poneyland") {
    ///     Entry::Vacant(_) => panic!(),
    ///     Entry::Occupied(entry) => assert_eq!(entry.key(), &"poneyland"),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &K {
        unsafe { &self.elem.as_ref().0 }
    }

    /// Take the ownership of the key and value from the map.
    /// Keeps the allocated memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// // The map is empty
    /// assert!(map.is_empty() && map.capacity() == 0);
    ///
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(o) = map.entry("poneyland") {
    ///     // We delete the entry from the map.
    ///     assert_eq!(o.remove_entry(), ("poneyland", 12));
    /// }
    ///
    /// assert_eq!(map.contains_key("poneyland"), false);
    /// // Now map hold none elements
    /// assert!(map.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove_entry(self) -> (K, V) {
        unsafe { self.table.table.remove(self.elem).0 }
    }

    /// Gets a reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// match map.entry("poneyland") {
    ///     Entry::Vacant(_) => panic!(),
    ///     Entry::Occupied(entry) => assert_eq!(entry.get(), &12),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &V {
        unsafe { &self.elem.as_ref().1 }
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// If you need a reference to the `OccupiedEntry` which may outlive the
    /// destruction of the `Entry` value, see [`into_mut`].
    ///
    /// [`into_mut`]: #method.into_mut
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// assert_eq!(map["poneyland"], 12);
    /// if let Entry::Occupied(mut o) = map.entry("poneyland") {
    ///     *o.get_mut() += 10;
    ///     assert_eq!(*o.get(), 22);
    ///
    ///     // We can use the same Entry multiple times.
    ///     *o.get_mut() += 2;
    /// }
    ///
    /// assert_eq!(map["poneyland"], 24);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.elem.as_mut().1 }
    }

    /// Converts the OccupiedEntry into a mutable reference to the value in the entry
    /// with a lifetime bound to the map itself.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see [`get_mut`].
    ///
    /// [`get_mut`]: #method.get_mut
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{Entry, HashMap};
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// assert_eq!(map["poneyland"], 12);
    ///
    /// let value: &mut u32;
    /// match map.entry("poneyland") {
    ///     Entry::Occupied(entry) => value = entry.into_mut(),
    ///     Entry::Vacant(_) => panic!(),
    /// }
    /// *value += 10;
    ///
    /// assert_eq!(map["poneyland"], 22);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut self.elem.as_mut().1 }
    }

    /// Sets the value of the entry, and returns the entry's old value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(mut o) = map.entry("poneyland") {
    ///     assert_eq!(o.insert(15), 12);
    /// }
    ///
    /// assert_eq!(map["poneyland"], 15);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Takes the value out of the entry, and returns it.
    /// Keeps the allocated memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// // The map is empty
    /// assert!(map.is_empty() && map.capacity() == 0);
    ///
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(o) = map.entry("poneyland") {
    ///     assert_eq!(o.remove(), 12);
    /// }
    ///
    /// assert_eq!(map.contains_key("poneyland"), false);
    /// // Now map hold none elements
    /// assert!(map.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Replaces the entry, returning the old key and value. The new key in the hash map will be
    /// the key used to create this entry.
    ///
    /// # Panics
    ///
    /// Will panic if this OccupiedEntry was created through [`Entry::insert`].
    ///
    /// # Examples
    ///
    /// ```
    ///  use hashbrown::hash_map::{Entry, HashMap};
    ///  use std::rc::Rc;
    ///
    ///  let mut map: HashMap<Rc<String>, u32> = HashMap::new();
    ///  let key_one = Rc::new("Stringthing".to_string());
    ///  let key_two = Rc::new("Stringthing".to_string());
    ///
    ///  map.insert(key_one.clone(), 15);
    ///  assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    ///  match map.entry(key_two.clone()) {
    ///      Entry::Occupied(entry) => {
    ///          let (old_key, old_value): (Rc<String>, u32) = entry.replace_entry(16);
    ///          assert!(Rc::ptr_eq(&key_one, &old_key) && old_value == 15);
    ///      }
    ///      Entry::Vacant(_) => panic!(),
    ///  }
    ///
    ///  assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    ///  assert_eq!(map[&"Stringthing".to_owned()], 16);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_entry(self, value: V) -> (K, V) {
        let entry = unsafe { self.elem.as_mut() };

        let old_key = mem::replace(&mut entry.0, self.key.unwrap());
        let old_value = mem::replace(&mut entry.1, value);

        (old_key, old_value)
    }

    /// Replaces the key in the hash map with the key used to create this entry.
    ///
    /// # Panics
    ///
    /// Will panic if this OccupiedEntry was created through [`Entry::insert`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{Entry, HashMap};
    /// use std::rc::Rc;
    ///
    /// let mut map: HashMap<Rc<String>, usize> = HashMap::with_capacity(6);
    /// let mut keys_one: Vec<Rc<String>> = Vec::with_capacity(6);
    /// let mut keys_two: Vec<Rc<String>> = Vec::with_capacity(6);
    ///
    /// for (value, key) in ["a", "b", "c", "d", "e", "f"].into_iter().enumerate() {
    ///     let rc_key = Rc::new(key.to_owned());
    ///     keys_one.push(rc_key.clone());
    ///     map.insert(rc_key.clone(), value);
    ///     keys_two.push(Rc::new(key.to_owned()));
    /// }
    ///
    /// assert!(
    ///     keys_one.iter().all(|key| Rc::strong_count(key) == 2)
    ///         && keys_two.iter().all(|key| Rc::strong_count(key) == 1)
    /// );
    ///
    /// reclaim_memory(&mut map, &keys_two);
    ///
    /// assert!(
    ///     keys_one.iter().all(|key| Rc::strong_count(key) == 1)
    ///         && keys_two.iter().all(|key| Rc::strong_count(key) == 2)
    /// );
    ///
    /// fn reclaim_memory(map: &mut HashMap<Rc<String>, usize>, keys: &[Rc<String>]) {
    ///     for key in keys {
    ///         if let Entry::Occupied(entry) = map.entry(key.clone()) {
    ///         // Replaces the entry's key with our version of it in `keys`.
    ///             entry.replace_key();
    ///         }
    ///     }
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_key(self) -> K {
        let entry = unsafe { self.elem.as_mut() };
        mem::replace(&mut entry.0, self.key.unwrap())
    }

    /// Provides shared access to the key and owned access to the value of
    /// the entry and allows to replace or remove it based on the
    /// value of the returned option.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.insert("poneyland", 42);
    ///
    /// let entry = match map.entry("poneyland") {
    ///     Entry::Occupied(e) => {
    ///         e.replace_entry_with(|k, v| {
    ///             assert_eq!(k, &"poneyland");
    ///             assert_eq!(v, 42);
    ///             Some(v + 1)
    ///         })
    ///     }
    ///     Entry::Vacant(_) => panic!(),
    /// };
    ///
    /// match entry {
    ///     Entry::Occupied(e) => {
    ///         assert_eq!(e.key(), &"poneyland");
    ///         assert_eq!(e.get(), &43);
    ///     }
    ///     Entry::Vacant(_) => panic!(),
    /// }
    ///
    /// assert_eq!(map["poneyland"], 43);
    ///
    /// let entry = match map.entry("poneyland") {
    ///     Entry::Occupied(e) => e.replace_entry_with(|_k, _v| None),
    ///     Entry::Vacant(_) => panic!(),
    /// };
    ///
    /// match entry {
    ///     Entry::Vacant(e) => {
    ///         assert_eq!(e.key(), &"poneyland");
    ///     }
    ///     Entry::Occupied(_) => panic!(),
    /// }
    ///
    /// assert!(!map.contains_key("poneyland"));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_entry_with<F>(self, f: F) -> Entry<'a, K, V, S, A>
    where
        F: FnOnce(&K, V) -> Option<V>,
    {
        unsafe {
            let mut spare_key = None;

            self.table
                .table
                .replace_bucket_with(self.elem.clone(), |(key, value)| {
                    if let Some(new_value) = f(&key, value) {
                        Some((key, new_value))
                    } else {
                        spare_key = Some(key);
                        None
                    }
                });

            if let Some(key) = spare_key {
                Entry::Vacant(VacantEntry {
                    hash: self.hash,
                    key,
                    table: self.table,
                })
            } else {
                Entry::Occupied(self)
            }
        }
    }
}

impl<'a, K, V, S, A: Allocator> VacantEntry<'a, K, V, S, A> {
    /// Gets a reference to the key that would be used when inserting a value
    /// through the `VacantEntry`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// assert_eq!(map.entry("poneyland").key(), &"poneyland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Take ownership of the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{Entry, HashMap};
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// match map.entry("poneyland") {
    ///     Entry::Occupied(_) => panic!(),
    ///     Entry::Vacant(v) => assert_eq!(v.into_key(), "poneyland"),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_key(self) -> K {
        self.key
    }

    /// Sets the value of the entry with the VacantEntry's key,
    /// and returns a mutable reference to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// if let Entry::Vacant(o) = map.entry("poneyland") {
    ///     o.insert(37);
    /// }
    /// assert_eq!(map["poneyland"], 37);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Hash,
        S: BuildHasher,
    {
        let table = &mut self.table.table;
        let entry = table.insert_entry(
            self.hash,
            (self.key, value),
            make_hasher::<_, V, S>(&self.table.hash_builder),
        );
        &mut entry.1
    }

    #[cfg_attr(feature = "inline-more", inline)]
    pub(crate) fn insert_entry(self, value: V) -> OccupiedEntry<'a, K, V, S, A>
    where
        K: Hash,
        S: BuildHasher,
    {
        let elem = self.table.table.insert(
            self.hash,
            (self.key, value),
            make_hasher::<_, V, S>(&self.table.hash_builder),
        );
        OccupiedEntry {
            hash: self.hash,
            key: None,
            elem,
            table: self.table,
        }
    }
}

impl<'a, 'b, K, Q: ?Sized, V, S, A: Allocator> EntryRef<'a, 'b, K, Q, V, S, A> {
    /// Sets the value of the entry, and returns an OccupiedEntryRef.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// let entry = map.entry_ref("horseyland").insert(37);
    ///
    /// assert_eq!(entry.key(), "horseyland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, value: V) -> OccupiedEntryRef<'a, 'b, K, Q, V, S, A>
    where
        K: Hash + From<&'b Q>,
        S: BuildHasher,
    {
        match self {
            EntryRef::Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            EntryRef::Vacant(entry) => entry.insert_entry(value),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry_ref("poneyland").or_insert(3);
    /// assert_eq!(map["poneyland"], 3);
    ///
    /// // existing key
    /// *map.entry_ref("poneyland").or_insert(10) *= 2;
    /// assert_eq!(map["poneyland"], 6);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Hash + From<&'b Q>,
        S: BuildHasher,
    {
        match self {
            EntryRef::Occupied(entry) => entry.into_mut(),
            EntryRef::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry_ref("poneyland").or_insert_with(|| 3);
    /// assert_eq!(map["poneyland"], 3);
    ///
    /// // existing key
    /// *map.entry_ref("poneyland").or_insert_with(|| 10) *= 2;
    /// assert_eq!(map["poneyland"], 6);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V
    where
        K: Hash + From<&'b Q>,
        S: BuildHasher,
    {
        match self {
            EntryRef::Occupied(entry) => entry.into_mut(),
            EntryRef::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Ensures a value is in the entry by inserting, if empty, the result of the default function.
    /// This method allows for generating key-derived values for insertion by providing the default
    /// function an access to the borrower form of the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, usize> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry_ref("poneyland").or_insert_with_key(|key| key.chars().count());
    /// assert_eq!(map["poneyland"], 9);
    ///
    /// // existing key
    /// *map.entry_ref("poneyland").or_insert_with_key(|key| key.chars().count() * 10) *= 2;
    /// assert_eq!(map["poneyland"], 18);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert_with_key<F: FnOnce(&Q) -> V>(self, default: F) -> &'a mut V
    where
        K: Hash + Borrow<Q> + From<&'b Q>,
        S: BuildHasher,
    {
        match self {
            EntryRef::Occupied(entry) => entry.into_mut(),
            EntryRef::Vacant(entry) => {
                let value = default(entry.key.as_ref());
                entry.insert(value)
            }
        }
    }

    /// Returns a reference to this entry's key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.entry_ref("poneyland").or_insert(3);
    /// // existing key
    /// assert_eq!(map.entry_ref("poneyland").key(), "poneyland");
    /// // nonexistent key
    /// assert_eq!(map.entry_ref("horseland").key(), "horseland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &Q
    where
        K: Borrow<Q>,
    {
        match *self {
            EntryRef::Occupied(ref entry) => entry.key().borrow(),
            EntryRef::Vacant(ref entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    ///
    /// map.entry_ref("poneyland")
    ///    .and_modify(|e| { *e += 1 })
    ///    .or_insert(42);
    /// assert_eq!(map["poneyland"], 42);
    ///
    /// map.entry_ref("poneyland")
    ///    .and_modify(|e| { *e += 1 })
    ///    .or_insert(42);
    /// assert_eq!(map["poneyland"], 43);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            EntryRef::Occupied(mut entry) => {
                f(entry.get_mut());
                EntryRef::Occupied(entry)
            }
            EntryRef::Vacant(entry) => EntryRef::Vacant(entry),
        }
    }

    /// Provides shared access to the key and owned access to the value of
    /// an occupied entry and allows to replace or remove it based on the
    /// value of the returned option.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    ///
    /// let entry = map
    ///     .entry_ref("poneyland")
    ///     .and_replace_entry_with(|_k, _v| panic!());
    ///
    /// match entry {
    ///     EntryRef::Vacant(e) => {
    ///         assert_eq!(e.key(), "poneyland");
    ///     }
    ///     EntryRef::Occupied(_) => panic!(),
    /// }
    ///
    /// map.insert("poneyland".to_string(), 42);
    ///
    /// let entry = map
    ///     .entry_ref("poneyland")
    ///     .and_replace_entry_with(|k, v| {
    ///         assert_eq!(k, "poneyland");
    ///         assert_eq!(v, 42);
    ///         Some(v + 1)
    ///     });
    ///
    /// match entry {
    ///     EntryRef::Occupied(e) => {
    ///         assert_eq!(e.key(), "poneyland");
    ///         assert_eq!(e.get(), &43);
    ///     }
    ///     EntryRef::Vacant(_) => panic!(),
    /// }
    ///
    /// assert_eq!(map["poneyland"], 43);
    ///
    /// let entry = map
    ///     .entry_ref("poneyland")
    ///     .and_replace_entry_with(|_k, _v| None);
    ///
    /// match entry {
    ///     EntryRef::Vacant(e) => assert_eq!(e.key(), "poneyland"),
    ///     EntryRef::Occupied(_) => panic!(),
    /// }
    ///
    /// assert!(!map.contains_key("poneyland"));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn and_replace_entry_with<F>(self, f: F) -> Self
    where
        F: FnOnce(&K, V) -> Option<V>,
    {
        match self {
            EntryRef::Occupied(entry) => entry.replace_entry_with(f),
            EntryRef::Vacant(_) => self,
        }
    }
}

impl<'a, 'b, K, Q: ?Sized, V: Default, S, A: Allocator> EntryRef<'a, 'b, K, Q, V, S, A> {
    /// Ensures a value is in the entry by inserting the default value if empty,
    /// and returns a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, Option<u32>> = HashMap::new();
    ///
    /// // nonexistent key
    /// map.entry_ref("poneyland").or_default();
    /// assert_eq!(map["poneyland"], None);
    ///
    /// map.insert("horseland".to_string(), Some(3));
    ///
    /// // existing key
    /// assert_eq!(map.entry_ref("horseland").or_default(), &mut Some(3));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_default(self) -> &'a mut V
    where
        K: Hash + From<&'b Q>,
        S: BuildHasher,
    {
        match self {
            EntryRef::Occupied(entry) => entry.into_mut(),
            EntryRef::Vacant(entry) => entry.insert(Default::default()),
        }
    }
}

impl<'a, 'b, K, Q: ?Sized, V, S, A: Allocator> OccupiedEntryRef<'a, 'b, K, Q, V, S, A> {
    /// Gets a reference to the key in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// match map.entry_ref("poneyland") {
    ///     EntryRef::Vacant(_) => panic!(),
    ///     EntryRef::Occupied(entry) => assert_eq!(entry.key(), "poneyland"),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &K {
        unsafe { &self.elem.as_ref().0 }
    }

    /// Take the ownership of the key and value from the map.
    /// Keeps the allocated memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// // The map is empty
    /// assert!(map.is_empty() && map.capacity() == 0);
    ///
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// if let EntryRef::Occupied(o) = map.entry_ref("poneyland") {
    ///     // We delete the entry from the map.
    ///     assert_eq!(o.remove_entry(), ("poneyland".to_owned(), 12));
    /// }
    ///
    /// assert_eq!(map.contains_key("poneyland"), false);
    /// // Now map hold none elements but capacity is equal to the old one
    /// assert!(map.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove_entry(self) -> (K, V) {
        unsafe { self.table.table.remove(self.elem).0 }
    }

    /// Gets a reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// match map.entry_ref("poneyland") {
    ///     EntryRef::Vacant(_) => panic!(),
    ///     EntryRef::Occupied(entry) => assert_eq!(entry.get(), &12),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &V {
        unsafe { &self.elem.as_ref().1 }
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// If you need a reference to the `OccupiedEntryRef` which may outlive the
    /// destruction of the `EntryRef` value, see [`into_mut`].
    ///
    /// [`into_mut`]: #method.into_mut
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// assert_eq!(map["poneyland"], 12);
    /// if let EntryRef::Occupied(mut o) = map.entry_ref("poneyland") {
    ///     *o.get_mut() += 10;
    ///     assert_eq!(*o.get(), 22);
    ///
    ///     // We can use the same Entry multiple times.
    ///     *o.get_mut() += 2;
    /// }
    ///
    /// assert_eq!(map["poneyland"], 24);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.elem.as_mut().1 }
    }

    /// Converts the OccupiedEntryRef into a mutable reference to the value in the entry
    /// with a lifetime bound to the map itself.
    ///
    /// If you need multiple references to the `OccupiedEntryRef`, see [`get_mut`].
    ///
    /// [`get_mut`]: #method.get_mut
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// let value: &mut u32;
    /// match map.entry_ref("poneyland") {
    ///     EntryRef::Occupied(entry) => value = entry.into_mut(),
    ///     EntryRef::Vacant(_) => panic!(),
    /// }
    /// *value += 10;
    ///
    /// assert_eq!(map["poneyland"], 22);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut self.elem.as_mut().1 }
    }

    /// Sets the value of the entry, and returns the entry's old value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// if let EntryRef::Occupied(mut o) = map.entry_ref("poneyland") {
    ///     assert_eq!(o.insert(15), 12);
    /// }
    ///
    /// assert_eq!(map["poneyland"], 15);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Takes the value out of the entry, and returns it.
    /// Keeps the allocated memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// // The map is empty
    /// assert!(map.is_empty() && map.capacity() == 0);
    ///
    /// map.entry_ref("poneyland").or_insert(12);
    ///
    /// if let EntryRef::Occupied(o) = map.entry_ref("poneyland") {
    ///     assert_eq!(o.remove(), 12);
    /// }
    ///
    /// assert_eq!(map.contains_key("poneyland"), false);
    /// // Now map hold none elements but capacity is equal to the old one
    /// assert!(map.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Replaces the entry, returning the old key and value. The new key in the hash map will be
    /// the key used to create this entry.
    ///
    /// # Panics
    ///
    /// Will panic if this OccupiedEntryRef was created through [`EntryRef::insert`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    /// use std::rc::Rc;
    ///
    /// let mut map: HashMap<Rc<str>, u32> = HashMap::new();
    /// let key: Rc<str> = Rc::from("Stringthing");
    ///
    /// map.insert(key.clone(), 15);
    /// assert_eq!(Rc::strong_count(&key), 2);
    ///
    /// match map.entry_ref("Stringthing") {
    ///     EntryRef::Occupied(entry) => {
    ///         let (old_key, old_value): (Rc<str>, u32) = entry.replace_entry(16);
    ///         assert!(Rc::ptr_eq(&key, &old_key) && old_value == 15);
    ///     }
    ///     EntryRef::Vacant(_) => panic!(),
    /// }
    ///
    /// assert_eq!(Rc::strong_count(&key), 1);
    /// assert_eq!(map["Stringthing"], 16);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_entry(self, value: V) -> (K, V)
    where
        K: From<&'b Q>,
    {
        let entry = unsafe { self.elem.as_mut() };

        let old_key = mem::replace(&mut entry.0, self.key.unwrap().into_owned());
        let old_value = mem::replace(&mut entry.1, value);

        (old_key, old_value)
    }

    /// Replaces the key in the hash map with the key used to create this entry.
    ///
    /// # Panics
    ///
    /// Will panic if this OccupiedEntryRef was created through [`EntryRef::insert`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    /// use std::rc::Rc;
    ///
    /// let mut map: HashMap<Rc<str>, usize> = HashMap::with_capacity(6);
    /// let mut keys: Vec<Rc<str>> = Vec::with_capacity(6);
    ///
    /// for (value, key) in ["a", "b", "c", "d", "e", "f"].into_iter().enumerate() {
    ///     let rc_key: Rc<str> = Rc::from(key);
    ///     keys.push(rc_key.clone());
    ///     map.insert(rc_key.clone(), value);
    /// }
    ///
    /// assert!(keys.iter().all(|key| Rc::strong_count(key) == 2));
    ///
    /// // It doesn't matter that we kind of use a vector with the same keys,
    /// // because all keys will be newly created from the references
    /// reclaim_memory(&mut map, &keys);
    ///
    /// assert!(keys.iter().all(|key| Rc::strong_count(key) == 1));
    ///
    /// fn reclaim_memory(map: &mut HashMap<Rc<str>, usize>, keys: &[Rc<str>]) {
    ///     for key in keys {
    ///         if let EntryRef::Occupied(entry) = map.entry_ref(key.as_ref()) {
    ///             // Replaces the entry's key with our version of it in `keys`.
    ///             entry.replace_key();
    ///         }
    ///     }
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_key(self) -> K
    where
        K: From<&'b Q>,
    {
        let entry = unsafe { self.elem.as_mut() };
        mem::replace(&mut entry.0, self.key.unwrap().into_owned())
    }

    /// Provides shared access to the key and owned access to the value of
    /// the entry and allows to replace or remove it based on the
    /// value of the returned option.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// map.insert("poneyland".to_string(), 42);
    ///
    /// let entry = match map.entry_ref("poneyland") {
    ///     EntryRef::Occupied(e) => {
    ///         e.replace_entry_with(|k, v| {
    ///             assert_eq!(k, "poneyland");
    ///             assert_eq!(v, 42);
    ///             Some(v + 1)
    ///         })
    ///     }
    ///     EntryRef::Vacant(_) => panic!(),
    /// };
    ///
    /// match entry {
    ///     EntryRef::Occupied(e) => {
    ///         assert_eq!(e.key(), "poneyland");
    ///         assert_eq!(e.get(), &43);
    ///     }
    ///     EntryRef::Vacant(_) => panic!(),
    /// }
    ///
    /// assert_eq!(map["poneyland"], 43);
    ///
    /// let entry = match map.entry_ref("poneyland") {
    ///     EntryRef::Occupied(e) => e.replace_entry_with(|_k, _v| None),
    ///     EntryRef::Vacant(_) => panic!(),
    /// };
    ///
    /// match entry {
    ///     EntryRef::Vacant(e) => {
    ///         assert_eq!(e.key(), "poneyland");
    ///     }
    ///     EntryRef::Occupied(_) => panic!(),
    /// }
    ///
    /// assert!(!map.contains_key("poneyland"));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace_entry_with<F>(self, f: F) -> EntryRef<'a, 'b, K, Q, V, S, A>
    where
        F: FnOnce(&K, V) -> Option<V>,
    {
        unsafe {
            let mut spare_key = None;

            self.table
                .table
                .replace_bucket_with(self.elem.clone(), |(key, value)| {
                    if let Some(new_value) = f(&key, value) {
                        Some((key, new_value))
                    } else {
                        spare_key = Some(KeyOrRef::Owned(key));
                        None
                    }
                });

            if let Some(key) = spare_key {
                EntryRef::Vacant(VacantEntryRef {
                    hash: self.hash,
                    key,
                    table: self.table,
                })
            } else {
                EntryRef::Occupied(self)
            }
        }
    }
}

impl<'a, 'b, K, Q: ?Sized, V, S, A: Allocator> VacantEntryRef<'a, 'b, K, Q, V, S, A> {
    /// Gets a reference to the key that would be used when inserting a value
    /// through the `VacantEntryRef`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// let key: &str = "poneyland";
    /// assert_eq!(map.entry_ref(key).key(), "poneyland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn key(&self) -> &Q
    where
        K: Borrow<Q>,
    {
        self.key.as_ref()
    }

    /// Take ownership of the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::{EntryRef, HashMap};
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// let key: &str = "poneyland";
    ///
    /// match map.entry_ref(key) {
    ///     EntryRef::Occupied(_) => panic!(),
    ///     EntryRef::Vacant(v) => assert_eq!(v.into_key(), "poneyland".to_owned()),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_key(self) -> K
    where
        K: From<&'b Q>,
    {
        self.key.into_owned()
    }

    /// Sets the value of the entry with the VacantEntryRef's key,
    /// and returns a mutable reference to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashMap;
    /// use hashbrown::hash_map::EntryRef;
    ///
    /// let mut map: HashMap<String, u32> = HashMap::new();
    /// let key: &str = "poneyland";
    ///
    /// if let EntryRef::Vacant(o) = map.entry_ref(key) {
    ///     o.insert(37);
    /// }
    /// assert_eq!(map["poneyland"], 37);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Hash + From<&'b Q>,
        S: BuildHasher,
    {
        let table = &mut self.table.table;
        let entry = table.insert_entry(
            self.hash,
            (self.key.into_owned(), value),
            make_hasher::<_, V, S>(&self.table.hash_builder),
        );
        &mut entry.1
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn insert_entry(self, value: V) -> OccupiedEntryRef<'a, 'b, K, Q, V, S, A>
    where
        K: Hash + From<&'b Q>,
        S: BuildHasher,
    {
        let elem = self.table.table.insert(
            self.hash,
            (self.key.into_owned(), value),
            make_hasher::<_, V, S>(&self.table.hash_builder),
        );
        OccupiedEntryRef {
            hash: self.hash,
            key: None,
            elem,
            table: self.table,
        }
    }
}

impl<K, V, S, A> FromIterator<(K, V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
    A: Default + Allocator,
{
    #[cfg_attr(feature = "inline-more", inline)]
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut map =
            Self::with_capacity_and_hasher_in(iter.size_hint().0, S::default(), A::default());
        iter.for_each(|(k, v)| {
            map.insert(k, v);
        });
        map
    }
}

/// Inserts all new key-values from the iterator and replaces values with existing
/// keys with new values returned from the iterator.
impl<K, V, S, A> Extend<(K, V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    /// Inserts all new key-values from the iterator to existing `HashMap<K, V, S, A>`.
    /// Replace values with existing keys with new values returned from the iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, 100);
    ///
    /// let some_iter = [(1, 1), (2, 2)].into_iter();
    /// map.extend(some_iter);
    /// // Replace values with existing keys with new values returned from the iterator.
    /// // So that the map.get(&1) doesn't return Some(&100).
    /// assert_eq!(map.get(&1), Some(&1));
    ///
    /// let some_vec: Vec<_> = vec![(3, 3), (4, 4)];
    /// map.extend(some_vec);
    ///
    /// let some_arr = [(5, 5), (6, 6)];
    /// map.extend(some_arr);
    /// let old_map_len = map.len();
    ///
    /// // You can also extend from another HashMap
    /// let mut new_map = HashMap::new();
    /// new_map.extend(map);
    /// assert_eq!(new_map.len(), old_map_len);
    ///
    /// let mut vec: Vec<_> = new_map.into_iter().collect();
    /// // The `IntoIter` iterator produces items in arbitrary order, so the
    /// // items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)]);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        // Keys may be already present or show multiple times in the iterator.
        // Reserve the entire hint lower bound if the map is empty.
        // Otherwise reserve half the hint (rounded up), so the map
        // will only resize twice in the worst case.
        let iter = iter.into_iter();
        let reserve = if self.is_empty() {
            iter.size_hint().0
        } else {
            (iter.size_hint().0 + 1) / 2
        };
        self.reserve(reserve);
        iter.for_each(move |(k, v)| {
            self.insert(k, v);
        });
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_one(&mut self, (k, v): (K, V)) {
        self.insert(k, v);
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_reserve(&mut self, additional: usize) {
        // Keys may be already present or show multiple times in the iterator.
        // Reserve the entire hint lower bound if the map is empty.
        // Otherwise reserve half the hint (rounded up), so the map
        // will only resize twice in the worst case.
        let reserve = if self.is_empty() {
            additional
        } else {
            (additional + 1) / 2
        };
        self.reserve(reserve);
    }
}

/// Inserts all new key-values from the iterator and replaces values with existing
/// keys with new values returned from the iterator.
impl<'a, K, V, S, A> Extend<(&'a K, &'a V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash + Copy,
    V: Copy,
    S: BuildHasher,
    A: Allocator,
{
    /// Inserts all new key-values from the iterator to existing `HashMap<K, V, S, A>`.
    /// Replace values with existing keys with new values returned from the iterator.
    /// The keys and values must implement [`Copy`] trait.
    ///
    /// [`Copy`]: https://doc.rust-lang.org/core/marker/trait.Copy.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, 100);
    ///
    /// let arr = [(1, 1), (2, 2)];
    /// let some_iter = arr.iter().map(|(k, v)| (k, v));
    /// map.extend(some_iter);
    /// // Replace values with existing keys with new values returned from the iterator.
    /// // So that the map.get(&1) doesn't return Some(&100).
    /// assert_eq!(map.get(&1), Some(&1));
    ///
    /// let some_vec: Vec<_> = vec![(3, 3), (4, 4)];
    /// map.extend(some_vec.iter().map(|(k, v)| (k, v)));
    ///
    /// let some_arr = [(5, 5), (6, 6)];
    /// map.extend(some_arr.iter().map(|(k, v)| (k, v)));
    ///
    /// // You can also extend from another HashMap
    /// let mut new_map = HashMap::new();
    /// new_map.extend(&map);
    /// assert_eq!(new_map, map);
    ///
    /// let mut vec: Vec<_> = new_map.into_iter().collect();
    /// // The `IntoIter` iterator produces items in arbitrary order, so the
    /// // items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)]);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
        self.extend(iter.into_iter().map(|(&key, &value)| (key, value)));
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_one(&mut self, (k, v): (&'a K, &'a V)) {
        self.insert(*k, *v);
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_reserve(&mut self, additional: usize) {
        Extend::<(K, V)>::extend_reserve(self, additional);
    }
}

/// Inserts all new key-values from the iterator and replaces values with existing
/// keys with new values returned from the iterator.
impl<'a, K, V, S, A> Extend<&'a (K, V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash + Copy,
    V: Copy,
    S: BuildHasher,
    A: Allocator,
{
    /// Inserts all new key-values from the iterator to existing `HashMap<K, V, S, A>`.
    /// Replace values with existing keys with new values returned from the iterator.
    /// The keys and values must implement [`Copy`] trait.
    ///
    /// [`Copy`]: https://doc.rust-lang.org/core/marker/trait.Copy.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_map::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, 100);
    ///
    /// let arr = [(1, 1), (2, 2)];
    /// let some_iter = arr.iter();
    /// map.extend(some_iter);
    /// // Replace values with existing keys with new values returned from the iterator.
    /// // So that the map.get(&1) doesn't return Some(&100).
    /// assert_eq!(map.get(&1), Some(&1));
    ///
    /// let some_vec: Vec<_> = vec![(3, 3), (4, 4)];
    /// map.extend(&some_vec);
    ///
    /// let some_arr = [(5, 5), (6, 6)];
    /// map.extend(&some_arr);
    ///
    /// let mut vec: Vec<_> = map.into_iter().collect();
    /// // The `IntoIter` iterator produces items in arbitrary order, so the
    /// // items must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)]);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn extend<T: IntoIterator<Item = &'a (K, V)>>(&mut self, iter: T) {
        self.extend(iter.into_iter().map(|&(key, value)| (key, value)));
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_one(&mut self, &(k, v): &'a (K, V)) {
        self.insert(k, v);
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_reserve(&mut self, additional: usize) {
        Extend::<(K, V)>::extend_reserve(self, additional);
    }
}

#[allow(dead_code)]
fn assert_covariance() {
    fn map_key<'new>(v: HashMap<&'static str, u8>) -> HashMap<&'new str, u8> {
        v
    }
    fn map_val<'new>(v: HashMap<u8, &'static str>) -> HashMap<u8, &'new str> {
        v
    }
    fn iter_key<'a, 'new>(v: Iter<'a, &'static str, u8>) -> Iter<'a, &'new str, u8> {
        v
    }
    fn iter_val<'a, 'new>(v: Iter<'a, u8, &'static str>) -> Iter<'a, u8, &'new str> {
        v
    }
    fn into_iter_key<'new, A: Allocator>(
        v: IntoIter<&'static str, u8, A>,
    ) -> IntoIter<&'new str, u8, A> {
        v
    }
    fn into_iter_val<'new, A: Allocator>(
        v: IntoIter<u8, &'static str, A>,
    ) -> IntoIter<u8, &'new str, A> {
        v
    }
    fn keys_key<'a, 'new>(v: Keys<'a, &'static str, u8>) -> Keys<'a, &'new str, u8> {
        v
    }
    fn keys_val<'a, 'new>(v: Keys<'a, u8, &'static str>) -> Keys<'a, u8, &'new str> {
        v
    }
    fn values_key<'a, 'new>(v: Values<'a, &'static str, u8>) -> Values<'a, &'new str, u8> {
        v
    }
    fn values_val<'a, 'new>(v: Values<'a, u8, &'static str>) -> Values<'a, u8, &'new str> {
        v
    }
    fn drain<'new>(
        d: Drain<'static, &'static str, &'static str>,
    ) -> Drain<'new, &'new str, &'new str> {
        d
    }
}

#[cfg(test)]
mod test_map {
    use super::DefaultHashBuilder;
    use super::Entry::{Occupied, Vacant};
    use super::EntryRef;
    use super::{HashMap, RawEntryMut};
    use alloc::string::{String, ToString};
    use alloc::sync::Arc;
    use allocator_api2::alloc::{AllocError, Allocator, Global};
    use core::alloc::Layout;
    use core::ptr::NonNull;
    use core::sync::atomic::{AtomicI8, Ordering};
    use rand::{rngs::SmallRng, Rng, SeedableRng};
    use std::borrow::ToOwned;
    use std::cell::RefCell;
    use std::usize;
    use std::vec::Vec;

    #[test]
    fn test_zero_capacities() {
        type HM = HashMap<i32, i32>;

        let m = HM::new();
        assert_eq!(m.capacity(), 0);

        let m = HM::default();
        assert_eq!(m.capacity(), 0);

        let m = HM::with_hasher(DefaultHashBuilder::default());
        assert_eq!(m.capacity(), 0);

        let m = HM::with_capacity(0);
        assert_eq!(m.capacity(), 0);

        let m = HM::with_capacity_and_hasher(0, DefaultHashBuilder::default());
        assert_eq!(m.capacity(), 0);

        let mut m = HM::new();
        m.insert(1, 1);
        m.insert(2, 2);
        m.remove(&1);
        m.remove(&2);
        m.shrink_to_fit();
        assert_eq!(m.capacity(), 0);

        let mut m = HM::new();
        m.reserve(0);
        assert_eq!(m.capacity(), 0);
    }

    #[test]
    fn test_create_capacity_zero() {
        let mut m = HashMap::with_capacity(0);

        assert!(m.insert(1, 1).is_none());

        assert!(m.contains_key(&1));
        assert!(!m.contains_key(&0));
    }

    #[test]
    fn test_insert() {
        let mut m = HashMap::new();
        assert_eq!(m.len(), 0);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(m.len(), 1);
        assert!(m.insert(2, 4).is_none());
        assert_eq!(m.len(), 2);
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&2).unwrap(), 4);
    }

    #[test]
    fn test_clone() {
        let mut m = HashMap::new();
        assert_eq!(m.len(), 0);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(m.len(), 1);
        assert!(m.insert(2, 4).is_none());
        assert_eq!(m.len(), 2);
        #[allow(clippy::redundant_clone)]
        let m2 = m.clone();
        assert_eq!(*m2.get(&1).unwrap(), 2);
        assert_eq!(*m2.get(&2).unwrap(), 4);
        assert_eq!(m2.len(), 2);
    }

    #[test]
    fn test_clone_from() {
        let mut m = HashMap::new();
        let mut m2 = HashMap::new();
        assert_eq!(m.len(), 0);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(m.len(), 1);
        assert!(m.insert(2, 4).is_none());
        assert_eq!(m.len(), 2);
        m2.clone_from(&m);
        assert_eq!(*m2.get(&1).unwrap(), 2);
        assert_eq!(*m2.get(&2).unwrap(), 4);
        assert_eq!(m2.len(), 2);
    }

    thread_local! { static DROP_VECTOR: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) } }

    #[derive(Hash, PartialEq, Eq)]
    struct Droppable {
        k: usize,
    }

    impl Droppable {
        fn new(k: usize) -> Droppable {
            DROP_VECTOR.with(|slot| {
                slot.borrow_mut()[k] += 1;
            });

            Droppable { k }
        }
    }

    impl Drop for Droppable {
        fn drop(&mut self) {
            DROP_VECTOR.with(|slot| {
                slot.borrow_mut()[self.k] -= 1;
            });
        }
    }

    impl Clone for Droppable {
        fn clone(&self) -> Self {
            Droppable::new(self.k)
        }
    }

    #[test]
    fn test_drops() {
        DROP_VECTOR.with(|slot| {
            *slot.borrow_mut() = vec![0; 200];
        });

        {
            let mut m = HashMap::new();

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 0);
                }
            });

            for i in 0..100 {
                let d1 = Droppable::new(i);
                let d2 = Droppable::new(i + 100);
                m.insert(d1, d2);
            }

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 1);
                }
            });

            for i in 0..50 {
                let k = Droppable::new(i);
                let v = m.remove(&k);

                assert!(v.is_some());

                DROP_VECTOR.with(|v| {
                    assert_eq!(v.borrow()[i], 1);
                    assert_eq!(v.borrow()[i + 100], 1);
                });
            }

            DROP_VECTOR.with(|v| {
                for i in 0..50 {
                    assert_eq!(v.borrow()[i], 0);
                    assert_eq!(v.borrow()[i + 100], 0);
                }

                for i in 50..100 {
                    assert_eq!(v.borrow()[i], 1);
                    assert_eq!(v.borrow()[i + 100], 1);
                }
            });
        }

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 0);
            }
        });
    }

    #[test]
    fn test_into_iter_drops() {
        DROP_VECTOR.with(|v| {
            *v.borrow_mut() = vec![0; 200];
        });

        let hm = {
            let mut hm = HashMap::new();

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 0);
                }
            });

            for i in 0..100 {
                let d1 = Droppable::new(i);
                let d2 = Droppable::new(i + 100);
                hm.insert(d1, d2);
            }

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 1);
                }
            });

            hm
        };

        // By the way, ensure that cloning doesn't screw up the dropping.
        drop(hm.clone());

        {
            let mut half = hm.into_iter().take(50);

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 1);
                }
            });

            for _ in half.by_ref() {}

            DROP_VECTOR.with(|v| {
                let nk = (0..100).filter(|&i| v.borrow()[i] == 1).count();

                let nv = (0..100).filter(|&i| v.borrow()[i + 100] == 1).count();

                assert_eq!(nk, 50);
                assert_eq!(nv, 50);
            });
        };

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 0);
            }
        });
    }

    #[test]
    fn test_empty_remove() {
        let mut m: HashMap<i32, bool> = HashMap::new();
        assert_eq!(m.remove(&0), None);
    }

    #[test]
    fn test_empty_entry() {
        let mut m: HashMap<i32, bool> = HashMap::new();
        match m.entry(0) {
            Occupied(_) => panic!(),
            Vacant(_) => {}
        }
        assert!(*m.entry(0).or_insert(true));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_empty_entry_ref() {
        let mut m: HashMap<std::string::String, bool> = HashMap::new();
        match m.entry_ref("poneyland") {
            EntryRef::Occupied(_) => panic!(),
            EntryRef::Vacant(_) => {}
        }
        assert!(*m.entry_ref("poneyland").or_insert(true));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_empty_iter() {
        let mut m: HashMap<i32, bool> = HashMap::new();
        assert_eq!(m.drain().next(), None);
        assert_eq!(m.keys().next(), None);
        assert_eq!(m.values().next(), None);
        assert_eq!(m.values_mut().next(), None);
        assert_eq!(m.iter().next(), None);
        assert_eq!(m.iter_mut().next(), None);
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
        assert_eq!(m.into_iter().next(), None);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // FIXME: takes too long
    fn test_lots_of_insertions() {
        let mut m = HashMap::new();

        // Try this a few times to make sure we never screw up the hashmap's
        // internal state.
        for _ in 0..10 {
            assert!(m.is_empty());

            for i in 1..1001 {
                assert!(m.insert(i, i).is_none());

                for j in 1..=i {
                    let r = m.get(&j);
                    assert_eq!(r, Some(&j));
                }

                for j in i + 1..1001 {
                    let r = m.get(&j);
                    assert_eq!(r, None);
                }
            }

            for i in 1001..2001 {
                assert!(!m.contains_key(&i));
            }

            // remove forwards
            for i in 1..1001 {
                assert!(m.remove(&i).is_some());

                for j in 1..=i {
                    assert!(!m.contains_key(&j));
                }

                for j in i + 1..1001 {
                    assert!(m.contains_key(&j));
                }
            }

            for i in 1..1001 {
                assert!(!m.contains_key(&i));
            }

            for i in 1..1001 {
                assert!(m.insert(i, i).is_none());
            }

            // remove backwards
            for i in (1..1001).rev() {
                assert!(m.remove(&i).is_some());

                for j in i..1001 {
                    assert!(!m.contains_key(&j));
                }

                for j in 1..i {
                    assert!(m.contains_key(&j));
                }
            }
        }
    }

    #[test]
    fn test_find_mut() {
        let mut m = HashMap::new();
        assert!(m.insert(1, 12).is_none());
        assert!(m.insert(2, 8).is_none());
        assert!(m.insert(5, 14).is_none());
        let new = 100;
        match m.get_mut(&5) {
            None => panic!(),
            Some(x) => *x = new,
        }
        assert_eq!(m.get(&5), Some(&new));
    }

    #[test]
    fn test_insert_overwrite() {
        let mut m = HashMap::new();
        assert!(m.insert(1, 2).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert!(m.insert(1, 3).is_some());
        assert_eq!(*m.get(&1).unwrap(), 3);
    }

    #[test]
    fn test_insert_conflicts() {
        let mut m = HashMap::with_capacity(4);
        assert!(m.insert(1, 2).is_none());
        assert!(m.insert(5, 3).is_none());
        assert!(m.insert(9, 4).is_none());
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert_eq!(*m.get(&1).unwrap(), 2);
    }

    #[test]
    fn test_conflict_remove() {
        let mut m = HashMap::with_capacity(4);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert!(m.insert(5, 3).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert!(m.insert(9, 4).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert!(m.remove(&1).is_some());
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert_eq!(*m.get(&5).unwrap(), 3);
    }

    #[test]
    fn test_insert_unique_unchecked() {
        let mut map = HashMap::new();
        let (k1, v1) = map.insert_unique_unchecked(10, 11);
        assert_eq!((&10, &mut 11), (k1, v1));
        let (k2, v2) = map.insert_unique_unchecked(20, 21);
        assert_eq!((&20, &mut 21), (k2, v2));
        assert_eq!(Some(&11), map.get(&10));
        assert_eq!(Some(&21), map.get(&20));
        assert_eq!(None, map.get(&30));
    }

    #[test]
    fn test_is_empty() {
        let mut m = HashMap::with_capacity(4);
        assert!(m.insert(1, 2).is_none());
        assert!(!m.is_empty());
        assert!(m.remove(&1).is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn test_remove() {
        let mut m = HashMap::new();
        m.insert(1, 2);
        assert_eq!(m.remove(&1), Some(2));
        assert_eq!(m.remove(&1), None);
    }

    #[test]
    fn test_remove_entry() {
        let mut m = HashMap::new();
        m.insert(1, 2);
        assert_eq!(m.remove_entry(&1), Some((1, 2)));
        assert_eq!(m.remove(&1), None);
    }

    #[test]
    fn test_iterate() {
        let mut m = HashMap::with_capacity(4);
        for i in 0..32 {
            assert!(m.insert(i, i * 2).is_none());
        }
        assert_eq!(m.len(), 32);

        let mut observed: u32 = 0;

        for (k, v) in &m {
            assert_eq!(*v, *k * 2);
            observed |= 1 << *k;
        }
        assert_eq!(observed, 0xFFFF_FFFF);
    }

    #[test]
    fn test_keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: HashMap<_, _> = vec.into_iter().collect();
        let keys: Vec<_> = map.keys().copied().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn test_values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: HashMap<_, _> = vec.into_iter().collect();
        let values: Vec<_> = map.values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn test_values_mut() {
        let vec = vec![(1, 1), (2, 2), (3, 3)];
        let mut map: HashMap<_, _> = vec.into_iter().collect();
        for value in map.values_mut() {
            *value *= 2;
        }
        let values: Vec<_> = map.values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert!(values.contains(&4));
        assert!(values.contains(&6));
    }

    #[test]
    fn test_into_keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: HashMap<_, _> = vec.into_iter().collect();
        let keys: Vec<_> = map.into_keys().collect();

        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn test_into_values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: HashMap<_, _> = vec.into_iter().collect();
        let values: Vec<_> = map.into_values().collect();

        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn test_find() {
        let mut m = HashMap::new();
        assert!(m.get(&1).is_none());
        m.insert(1, 2);
        match m.get(&1) {
            None => panic!(),
            Some(v) => assert_eq!(*v, 2),
        }
    }

    #[test]
    fn test_eq() {
        let mut m1 = HashMap::new();
        m1.insert(1, 2);
        m1.insert(2, 3);
        m1.insert(3, 4);

        let mut m2 = HashMap::new();
        m2.insert(1, 2);
        m2.insert(2, 3);

        assert!(m1 != m2);

        m2.insert(3, 4);

        assert_eq!(m1, m2);
    }

    #[test]
    fn test_show() {
        let mut map = HashMap::new();
        let empty: HashMap<i32, i32> = HashMap::new();

        map.insert(1, 2);
        map.insert(3, 4);

        let map_str = format!("{map:?}");

        assert!(map_str == "{1: 2, 3: 4}" || map_str == "{3: 4, 1: 2}");
        assert_eq!(format!("{empty:?}"), "{}");
    }

    #[test]
    fn test_expand() {
        let mut m = HashMap::new();

        assert_eq!(m.len(), 0);
        assert!(m.is_empty());

        let mut i = 0;
        let old_raw_cap = m.raw_capacity();
        while old_raw_cap == m.raw_capacity() {
            m.insert(i, i);
            i += 1;
        }

        assert_eq!(m.len(), i);
        assert!(!m.is_empty());
    }

    #[test]
    fn test_behavior_resize_policy() {
        let mut m = HashMap::new();

        assert_eq!(m.len(), 0);
        assert_eq!(m.raw_capacity(), 1);
        assert!(m.is_empty());

        m.insert(0, 0);
        m.remove(&0);
        assert!(m.is_empty());
        let initial_raw_cap = m.raw_capacity();
        m.reserve(initial_raw_cap);
        let raw_cap = m.raw_capacity();

        assert_eq!(raw_cap, initial_raw_cap * 2);

        let mut i = 0;
        for _ in 0..raw_cap * 3 / 4 {
            m.insert(i, i);
            i += 1;
        }
        // three quarters full

        assert_eq!(m.len(), i);
        assert_eq!(m.raw_capacity(), raw_cap);

        for _ in 0..raw_cap / 4 {
            m.insert(i, i);
            i += 1;
        }
        // half full

        let new_raw_cap = m.raw_capacity();
        assert_eq!(new_raw_cap, raw_cap * 2);

        for _ in 0..raw_cap / 2 - 1 {
            i -= 1;
            m.remove(&i);
            assert_eq!(m.raw_capacity(), new_raw_cap);
        }
        // A little more than one quarter full.
        m.shrink_to_fit();
        assert_eq!(m.raw_capacity(), raw_cap);
        // again, a little more than half full
        for _ in 0..raw_cap / 2 {
            i -= 1;
            m.remove(&i);
        }
        m.shrink_to_fit();

        assert_eq!(m.len(), i);
        assert!(!m.is_empty());
        assert_eq!(m.raw_capacity(), initial_raw_cap);
    }

    #[test]
    fn test_reserve_shrink_to_fit() {
        let mut m = HashMap::new();
        m.insert(0, 0);
        m.remove(&0);
        assert!(m.capacity() >= m.len());
        for i in 0..128 {
            m.insert(i, i);
        }
        m.reserve(256);

        let usable_cap = m.capacity();
        for i in 128..(128 + 256) {
            m.insert(i, i);
            assert_eq!(m.capacity(), usable_cap);
        }

        for i in 100..(128 + 256) {
            assert_eq!(m.remove(&i), Some(i));
        }
        m.shrink_to_fit();

        assert_eq!(m.len(), 100);
        assert!(!m.is_empty());
        assert!(m.capacity() >= m.len());

        for i in 0..100 {
            assert_eq!(m.remove(&i), Some(i));
        }
        m.shrink_to_fit();
        m.insert(0, 0);

        assert_eq!(m.len(), 1);
        assert!(m.capacity() >= m.len());
        assert_eq!(m.remove(&0), Some(0));
    }

    #[test]
    fn test_from_iter() {
        let xs = [(1, 1), (2, 2), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: HashMap<_, _> = xs.iter().copied().collect();

        for &(k, v) in &xs {
            assert_eq!(map.get(&k), Some(&v));
        }

        assert_eq!(map.iter().len(), xs.len() - 1);
    }

    #[test]
    fn test_size_hint() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: HashMap<_, _> = xs.iter().copied().collect();

        let mut iter = map.iter();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    fn test_iter_len() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: HashMap<_, _> = xs.iter().copied().collect();

        let mut iter = map.iter();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_mut_size_hint() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let mut map: HashMap<_, _> = xs.iter().copied().collect();

        let mut iter = map.iter_mut();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    fn test_iter_mut_len() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let mut map: HashMap<_, _> = xs.iter().copied().collect();

        let mut iter = map.iter_mut();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_index() {
        let mut map = HashMap::new();

        map.insert(1, 2);
        map.insert(2, 1);
        map.insert(3, 4);

        assert_eq!(map[&2], 1);
    }

    #[test]
    #[should_panic]
    fn test_index_nonexistent() {
        let mut map = HashMap::new();

        map.insert(1, 2);
        map.insert(2, 1);
        map.insert(3, 4);

        #[allow(clippy::no_effect)] // false positive lint
        map[&4];
    }

    #[test]
    fn test_entry() {
        let xs = [(1, 10), (2, 20), (3, 30), (4, 40), (5, 50), (6, 60)];

        let mut map: HashMap<_, _> = xs.iter().copied().collect();

        // Existing key (insert)
        match map.entry(1) {
            Vacant(_) => unreachable!(),
            Occupied(mut view) => {
                assert_eq!(view.get(), &10);
                assert_eq!(view.insert(100), 10);
            }
        }
        assert_eq!(map.get(&1).unwrap(), &100);
        assert_eq!(map.len(), 6);

        // Existing key (update)
        match map.entry(2) {
            Vacant(_) => unreachable!(),
            Occupied(mut view) => {
                let v = view.get_mut();
                let new_v = (*v) * 10;
                *v = new_v;
            }
        }
        assert_eq!(map.get(&2).unwrap(), &200);
        assert_eq!(map.len(), 6);

        // Existing key (take)
        match map.entry(3) {
            Vacant(_) => unreachable!(),
            Occupied(view) => {
                assert_eq!(view.remove(), 30);
            }
        }
        assert_eq!(map.get(&3), None);
        assert_eq!(map.len(), 5);

        // Inexistent key (insert)
        match map.entry(10) {
            Occupied(_) => unreachable!(),
            Vacant(view) => {
                assert_eq!(*view.insert(1000), 1000);
            }
        }
        assert_eq!(map.get(&10).unwrap(), &1000);
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn test_entry_ref() {
        let xs = [
            ("One".to_owned(), 10),
            ("Two".to_owned(), 20),
            ("Three".to_owned(), 30),
            ("Four".to_owned(), 40),
            ("Five".to_owned(), 50),
            ("Six".to_owned(), 60),
        ];

        let mut map: HashMap<_, _> = xs.iter().cloned().collect();

        // Existing key (insert)
        match map.entry_ref("One") {
            EntryRef::Vacant(_) => unreachable!(),
            EntryRef::Occupied(mut view) => {
                assert_eq!(view.get(), &10);
                assert_eq!(view.insert(100), 10);
            }
        }
        assert_eq!(map.get("One").unwrap(), &100);
        assert_eq!(map.len(), 6);

        // Existing key (update)
        match map.entry_ref("Two") {
            EntryRef::Vacant(_) => unreachable!(),
            EntryRef::Occupied(mut view) => {
                let v = view.get_mut();
                let new_v = (*v) * 10;
                *v = new_v;
            }
        }
        assert_eq!(map.get("Two").unwrap(), &200);
        assert_eq!(map.len(), 6);

        // Existing key (take)
        match map.entry_ref("Three") {
            EntryRef::Vacant(_) => unreachable!(),
            EntryRef::Occupied(view) => {
                assert_eq!(view.remove(), 30);
            }
        }
        assert_eq!(map.get("Three"), None);
        assert_eq!(map.len(), 5);

        // Inexistent key (insert)
        match map.entry_ref("Ten") {
            EntryRef::Occupied(_) => unreachable!(),
            EntryRef::Vacant(view) => {
                assert_eq!(*view.insert(1000), 1000);
            }
        }
        assert_eq!(map.get("Ten").unwrap(), &1000);
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn test_entry_take_doesnt_corrupt() {
        #![allow(deprecated)] //rand
                              // Test for #19292
        fn check(m: &HashMap<i32, ()>) {
            for k in m.keys() {
                assert!(m.contains_key(k), "{k} is in keys() but not in the map?");
            }
        }

        let mut m = HashMap::new();

        let mut rng = {
            let seed = u64::from_le_bytes(*b"testseed");
            SmallRng::seed_from_u64(seed)
        };

        // Populate the map with some items.
        for _ in 0..50 {
            let x = rng.gen_range(-10..10);
            m.insert(x, ());
        }

        for _ in 0..1000 {
            let x = rng.gen_range(-10..10);
            match m.entry(x) {
                Vacant(_) => {}
                Occupied(e) => {
                    e.remove();
                }
            }

            check(&m);
        }
    }

    #[test]
    fn test_entry_ref_take_doesnt_corrupt() {
        #![allow(deprecated)] //rand
                              // Test for #19292
        fn check(m: &HashMap<std::string::String, ()>) {
            for k in m.keys() {
                assert!(m.contains_key(k), "{k} is in keys() but not in the map?");
            }
        }

        let mut m = HashMap::new();

        let mut rng = {
            let seed = u64::from_le_bytes(*b"testseed");
            SmallRng::seed_from_u64(seed)
        };

        // Populate the map with some items.
        for _ in 0..50 {
            let mut x = std::string::String::with_capacity(1);
            x.push(rng.gen_range('a'..='z'));
            m.insert(x, ());
        }

        for _ in 0..1000 {
            let mut x = std::string::String::with_capacity(1);
            x.push(rng.gen_range('a'..='z'));
            match m.entry_ref(x.as_str()) {
                EntryRef::Vacant(_) => {}
                EntryRef::Occupied(e) => {
                    e.remove();
                }
            }

            check(&m);
        }
    }

    #[test]
    fn test_extend_ref_k_ref_v() {
        let mut a = HashMap::new();
        a.insert(1, "one");
        let mut b = HashMap::new();
        b.insert(2, "two");
        b.insert(3, "three");

        a.extend(&b);

        assert_eq!(a.len(), 3);
        assert_eq!(a[&1], "one");
        assert_eq!(a[&2], "two");
        assert_eq!(a[&3], "three");
    }

    #[test]
    #[allow(clippy::needless_borrow)]
    fn test_extend_ref_kv_tuple() {
        use std::ops::AddAssign;
        let mut a = HashMap::new();
        a.insert(0, 0);

        fn create_arr<T: AddAssign<T> + Copy, const N: usize>(start: T, step: T) -> [(T, T); N] {
            let mut outs: [(T, T); N] = [(start, start); N];
            let mut element = step;
            outs.iter_mut().skip(1).for_each(|(k, v)| {
                *k += element;
                *v += element;
                element += step;
            });
            outs
        }

        let for_iter: Vec<_> = (0..100).map(|i| (i, i)).collect();
        let iter = for_iter.iter();
        let vec: Vec<_> = (100..200).map(|i| (i, i)).collect();
        a.extend(iter);
        a.extend(&vec);
        a.extend(create_arr::<i32, 100>(200, 1));

        assert_eq!(a.len(), 300);

        for item in 0..300 {
            assert_eq!(a[&item], item);
        }
    }

    #[test]
    fn test_capacity_not_less_than_len() {
        let mut a = HashMap::new();
        let mut item = 0;

        for _ in 0..116 {
            a.insert(item, 0);
            item += 1;
        }

        assert!(a.capacity() > a.len());

        let free = a.capacity() - a.len();
        for _ in 0..free {
            a.insert(item, 0);
            item += 1;
        }

        assert_eq!(a.len(), a.capacity());

        // Insert at capacity should cause allocation.
        a.insert(item, 0);
        assert!(a.capacity() > a.len());
    }

    #[test]
    fn test_occupied_entry_key() {
        let mut a = HashMap::new();
        let key = "hello there";
        let value = "value goes here";
        assert!(a.is_empty());
        a.insert(key, value);
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);

        match a.entry(key) {
            Vacant(_) => panic!(),
            Occupied(e) => assert_eq!(key, *e.key()),
        }
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);
    }

    #[test]
    fn test_occupied_entry_ref_key() {
        let mut a = HashMap::new();
        let key = "hello there";
        let value = "value goes here";
        assert!(a.is_empty());
        a.insert(key.to_owned(), value);
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);

        match a.entry_ref(key) {
            EntryRef::Vacant(_) => panic!(),
            EntryRef::Occupied(e) => assert_eq!(key, e.key()),
        }
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);
    }

    #[test]
    fn test_vacant_entry_key() {
        let mut a = HashMap::new();
        let key = "hello there";
        let value = "value goes here";

        assert!(a.is_empty());
        match a.entry(key) {
            Occupied(_) => panic!(),
            Vacant(e) => {
                assert_eq!(key, *e.key());
                e.insert(value);
            }
        }
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);
    }

    #[test]
    fn test_vacant_entry_ref_key() {
        let mut a: HashMap<std::string::String, &str> = HashMap::new();
        let key = "hello there";
        let value = "value goes here";

        assert!(a.is_empty());
        match a.entry_ref(key) {
            EntryRef::Occupied(_) => panic!(),
            EntryRef::Vacant(e) => {
                assert_eq!(key, e.key());
                e.insert(value);
            }
        }
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);
    }

    #[test]
    fn test_occupied_entry_replace_entry_with() {
        let mut a = HashMap::new();

        let key = "a key";
        let value = "an initial value";
        let new_value = "a new value";

        let entry = a.entry(key).insert(value).replace_entry_with(|k, v| {
            assert_eq!(k, &key);
            assert_eq!(v, value);
            Some(new_value)
        });

        match entry {
            Occupied(e) => {
                assert_eq!(e.key(), &key);
                assert_eq!(e.get(), &new_value);
            }
            Vacant(_) => panic!(),
        }

        assert_eq!(a[key], new_value);
        assert_eq!(a.len(), 1);

        let entry = match a.entry(key) {
            Occupied(e) => e.replace_entry_with(|k, v| {
                assert_eq!(k, &key);
                assert_eq!(v, new_value);
                None
            }),
            Vacant(_) => panic!(),
        };

        match entry {
            Vacant(e) => assert_eq!(e.key(), &key),
            Occupied(_) => panic!(),
        }

        assert!(!a.contains_key(key));
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_occupied_entry_ref_replace_entry_with() {
        let mut a: HashMap<std::string::String, &str> = HashMap::new();

        let key = "a key";
        let value = "an initial value";
        let new_value = "a new value";

        let entry = a.entry_ref(key).insert(value).replace_entry_with(|k, v| {
            assert_eq!(k, key);
            assert_eq!(v, value);
            Some(new_value)
        });

        match entry {
            EntryRef::Occupied(e) => {
                assert_eq!(e.key(), key);
                assert_eq!(e.get(), &new_value);
            }
            EntryRef::Vacant(_) => panic!(),
        }

        assert_eq!(a[key], new_value);
        assert_eq!(a.len(), 1);

        let entry = match a.entry_ref(key) {
            EntryRef::Occupied(e) => e.replace_entry_with(|k, v| {
                assert_eq!(k, key);
                assert_eq!(v, new_value);
                None
            }),
            EntryRef::Vacant(_) => panic!(),
        };

        match entry {
            EntryRef::Vacant(e) => assert_eq!(e.key(), key),
            EntryRef::Occupied(_) => panic!(),
        }

        assert!(!a.contains_key(key));
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_entry_and_replace_entry_with() {
        let mut a = HashMap::new();

        let key = "a key";
        let value = "an initial value";
        let new_value = "a new value";

        let entry = a.entry(key).and_replace_entry_with(|_, _| panic!());

        match entry {
            Vacant(e) => assert_eq!(e.key(), &key),
            Occupied(_) => panic!(),
        }

        a.insert(key, value);

        let entry = a.entry(key).and_replace_entry_with(|k, v| {
            assert_eq!(k, &key);
            assert_eq!(v, value);
            Some(new_value)
        });

        match entry {
            Occupied(e) => {
                assert_eq!(e.key(), &key);
                assert_eq!(e.get(), &new_value);
            }
            Vacant(_) => panic!(),
        }

        assert_eq!(a[key], new_value);
        assert_eq!(a.len(), 1);

        let entry = a.entry(key).and_replace_entry_with(|k, v| {
            assert_eq!(k, &key);
            assert_eq!(v, new_value);
            None
        });

        match entry {
            Vacant(e) => assert_eq!(e.key(), &key),
            Occupied(_) => panic!(),
        }

        assert!(!a.contains_key(key));
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_entry_ref_and_replace_entry_with() {
        let mut a = HashMap::new();

        let key = "a key";
        let value = "an initial value";
        let new_value = "a new value";

        let entry = a.entry_ref(key).and_replace_entry_with(|_, _| panic!());

        match entry {
            EntryRef::Vacant(e) => assert_eq!(e.key(), key),
            EntryRef::Occupied(_) => panic!(),
        }

        a.insert(key.to_owned(), value);

        let entry = a.entry_ref(key).and_replace_entry_with(|k, v| {
            assert_eq!(k, key);
            assert_eq!(v, value);
            Some(new_value)
        });

        match entry {
            EntryRef::Occupied(e) => {
                assert_eq!(e.key(), key);
                assert_eq!(e.get(), &new_value);
            }
            EntryRef::Vacant(_) => panic!(),
        }

        assert_eq!(a[key], new_value);
        assert_eq!(a.len(), 1);

        let entry = a.entry_ref(key).and_replace_entry_with(|k, v| {
            assert_eq!(k, key);
            assert_eq!(v, new_value);
            None
        });

        match entry {
            EntryRef::Vacant(e) => assert_eq!(e.key(), key),
            EntryRef::Occupied(_) => panic!(),
        }

        assert!(!a.contains_key(key));
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_raw_occupied_entry_replace_entry_with() {
        let mut a = HashMap::new();

        let key = "a key";
        let value = "an initial value";
        let new_value = "a new value";

        let entry = a
            .raw_entry_mut()
            .from_key(&key)
            .insert(key, value)
            .replace_entry_with(|k, v| {
                assert_eq!(k, &key);
                assert_eq!(v, value);
                Some(new_value)
            });

        match entry {
            RawEntryMut::Occupied(e) => {
                assert_eq!(e.key(), &key);
                assert_eq!(e.get(), &new_value);
            }
            RawEntryMut::Vacant(_) => panic!(),
        }

        assert_eq!(a[key], new_value);
        assert_eq!(a.len(), 1);

        let entry = match a.raw_entry_mut().from_key(&key) {
            RawEntryMut::Occupied(e) => e.replace_entry_with(|k, v| {
                assert_eq!(k, &key);
                assert_eq!(v, new_value);
                None
            }),
            RawEntryMut::Vacant(_) => panic!(),
        };

        match entry {
            RawEntryMut::Vacant(_) => {}
            RawEntryMut::Occupied(_) => panic!(),
        }

        assert!(!a.contains_key(key));
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_raw_entry_and_replace_entry_with() {
        let mut a = HashMap::new();

        let key = "a key";
        let value = "an initial value";
        let new_value = "a new value";

        let entry = a
            .raw_entry_mut()
            .from_key(&key)
            .and_replace_entry_with(|_, _| panic!());

        match entry {
            RawEntryMut::Vacant(_) => {}
            RawEntryMut::Occupied(_) => panic!(),
        }

        a.insert(key, value);

        let entry = a
            .raw_entry_mut()
            .from_key(&key)
            .and_replace_entry_with(|k, v| {
                assert_eq!(k, &key);
                assert_eq!(v, value);
                Some(new_value)
            });

        match entry {
            RawEntryMut::Occupied(e) => {
                assert_eq!(e.key(), &key);
                assert_eq!(e.get(), &new_value);
            }
            RawEntryMut::Vacant(_) => panic!(),
        }

        assert_eq!(a[key], new_value);
        assert_eq!(a.len(), 1);

        let entry = a
            .raw_entry_mut()
            .from_key(&key)
            .and_replace_entry_with(|k, v| {
                assert_eq!(k, &key);
                assert_eq!(v, new_value);
                None
            });

        match entry {
            RawEntryMut::Vacant(_) => {}
            RawEntryMut::Occupied(_) => panic!(),
        }

        assert!(!a.contains_key(key));
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn test_replace_entry_with_doesnt_corrupt() {
        #![allow(deprecated)] //rand
                              // Test for #19292
        fn check(m: &HashMap<i32, ()>) {
            for k in m.keys() {
                assert!(m.contains_key(k), "{k} is in keys() but not in the map?");
            }
        }

        let mut m = HashMap::new();

        let mut rng = {
            let seed = u64::from_le_bytes(*b"testseed");
            SmallRng::seed_from_u64(seed)
        };

        // Populate the map with some items.
        for _ in 0..50 {
            let x = rng.gen_range(-10..10);
            m.insert(x, ());
        }

        for _ in 0..1000 {
            let x = rng.gen_range(-10..10);
            m.entry(x).and_replace_entry_with(|_, _| None);
            check(&m);
        }
    }

    #[test]
    fn test_replace_entry_ref_with_doesnt_corrupt() {
        #![allow(deprecated)] //rand
                              // Test for #19292
        fn check(m: &HashMap<std::string::String, ()>) {
            for k in m.keys() {
                assert!(m.contains_key(k), "{k} is in keys() but not in the map?");
            }
        }

        let mut m = HashMap::new();

        let mut rng = {
            let seed = u64::from_le_bytes(*b"testseed");
            SmallRng::seed_from_u64(seed)
        };

        // Populate the map with some items.
        for _ in 0..50 {
            let mut x = std::string::String::with_capacity(1);
            x.push(rng.gen_range('a'..='z'));
            m.insert(x, ());
        }

        for _ in 0..1000 {
            let mut x = std::string::String::with_capacity(1);
            x.push(rng.gen_range('a'..='z'));
            m.entry_ref(x.as_str()).and_replace_entry_with(|_, _| None);
            check(&m);
        }
    }

    #[test]
    fn test_retain() {
        let mut map: HashMap<i32, i32> = (0..100).map(|x| (x, x * 10)).collect();

        map.retain(|&k, _| k % 2 == 0);
        assert_eq!(map.len(), 50);
        assert_eq!(map[&2], 20);
        assert_eq!(map[&4], 40);
        assert_eq!(map[&6], 60);
    }

    #[test]
    fn test_extract_if() {
        {
            let mut map: HashMap<i32, i32> = (0..8).map(|x| (x, x * 10)).collect();
            let drained = map.extract_if(|&k, _| k % 2 == 0);
            let mut out = drained.collect::<Vec<_>>();
            out.sort_unstable();
            assert_eq!(vec![(0, 0), (2, 20), (4, 40), (6, 60)], out);
            assert_eq!(map.len(), 4);
        }
        {
            let mut map: HashMap<i32, i32> = (0..8).map(|x| (x, x * 10)).collect();
            map.extract_if(|&k, _| k % 2 == 0).for_each(drop);
            assert_eq!(map.len(), 4);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)] // FIXME: no OOM signalling (https://github.com/rust-lang/miri/issues/613)
    fn test_try_reserve() {
        use crate::TryReserveError::{AllocError, CapacityOverflow};

        const MAX_ISIZE: usize = isize::MAX as usize;

        let mut empty_bytes: HashMap<u8, u8> = HashMap::new();

        if let Err(CapacityOverflow) = empty_bytes.try_reserve(usize::MAX) {
        } else {
            panic!("usize::MAX should trigger an overflow!");
        }

        if let Err(CapacityOverflow) = empty_bytes.try_reserve(MAX_ISIZE) {
        } else {
            panic!("isize::MAX should trigger an overflow!");
        }

        if let Err(AllocError { .. }) = empty_bytes.try_reserve(MAX_ISIZE / 5) {
        } else {
            // This may succeed if there is enough free memory. Attempt to
            // allocate a few more hashmaps to ensure the allocation will fail.
            let mut empty_bytes2: HashMap<u8, u8> = HashMap::new();
            let _ = empty_bytes2.try_reserve(MAX_ISIZE / 5);
            let mut empty_bytes3: HashMap<u8, u8> = HashMap::new();
            let _ = empty_bytes3.try_reserve(MAX_ISIZE / 5);
            let mut empty_bytes4: HashMap<u8, u8> = HashMap::new();
            if let Err(AllocError { .. }) = empty_bytes4.try_reserve(MAX_ISIZE / 5) {
            } else {
                panic!("isize::MAX / 5 should trigger an OOM!");
            }
        }
    }

    #[test]
    fn test_raw_entry() {
        use super::RawEntryMut::{Occupied, Vacant};

        let xs = [(1_i32, 10_i32), (2, 20), (3, 30), (4, 40), (5, 50), (6, 60)];

        let mut map: HashMap<_, _> = xs.iter().copied().collect();

        let compute_hash = |map: &HashMap<i32, i32>, k: i32| -> u64 {
            super::make_hash::<i32, _>(map.hasher(), &k)
        };

        // Existing key (insert)
        match map.raw_entry_mut().from_key(&1) {
            Vacant(_) => unreachable!(),
            Occupied(mut view) => {
                assert_eq!(view.get(), &10);
                assert_eq!(view.insert(100), 10);
            }
        }
        let hash1 = compute_hash(&map, 1);
        assert_eq!(map.raw_entry().from_key(&1).unwrap(), (&1, &100));
        assert_eq!(
            map.raw_entry().from_hash(hash1, |k| *k == 1).unwrap(),
            (&1, &100)
        );
        assert_eq!(
            map.raw_entry().from_key_hashed_nocheck(hash1, &1).unwrap(),
            (&1, &100)
        );
        assert_eq!(map.len(), 6);

        // Existing key (update)
        match map.raw_entry_mut().from_key(&2) {
            Vacant(_) => unreachable!(),
            Occupied(mut view) => {
                let v = view.get_mut();
                let new_v = (*v) * 10;
                *v = new_v;
            }
        }
        let hash2 = compute_hash(&map, 2);
        assert_eq!(map.raw_entry().from_key(&2).unwrap(), (&2, &200));
        assert_eq!(
            map.raw_entry().from_hash(hash2, |k| *k == 2).unwrap(),
            (&2, &200)
        );
        assert_eq!(
            map.raw_entry().from_key_hashed_nocheck(hash2, &2).unwrap(),
            (&2, &200)
        );
        assert_eq!(map.len(), 6);

        // Existing key (take)
        let hash3 = compute_hash(&map, 3);
        match map.raw_entry_mut().from_key_hashed_nocheck(hash3, &3) {
            Vacant(_) => unreachable!(),
            Occupied(view) => {
                assert_eq!(view.remove_entry(), (3, 30));
            }
        }
        assert_eq!(map.raw_entry().from_key(&3), None);
        assert_eq!(map.raw_entry().from_hash(hash3, |k| *k == 3), None);
        assert_eq!(map.raw_entry().from_key_hashed_nocheck(hash3, &3), None);
        assert_eq!(map.len(), 5);

        // Nonexistent key (insert)
        match map.raw_entry_mut().from_key(&10) {
            Occupied(_) => unreachable!(),
            Vacant(view) => {
                assert_eq!(view.insert(10, 1000), (&mut 10, &mut 1000));
            }
        }
        assert_eq!(map.raw_entry().from_key(&10).unwrap(), (&10, &1000));
        assert_eq!(map.len(), 6);

        // Ensure all lookup methods produce equivalent results.
        for k in 0..12 {
            let hash = compute_hash(&map, k);
            let v = map.get(&k).copied();
            let kv = v.as_ref().map(|v| (&k, v));

            assert_eq!(map.raw_entry().from_key(&k), kv);
            assert_eq!(map.raw_entry().from_hash(hash, |q| *q == k), kv);
            assert_eq!(map.raw_entry().from_key_hashed_nocheck(hash, &k), kv);

            match map.raw_entry_mut().from_key(&k) {
                Occupied(o) => assert_eq!(Some(o.get_key_value()), kv),
                Vacant(_) => assert_eq!(v, None),
            }
            match map.raw_entry_mut().from_key_hashed_nocheck(hash, &k) {
                Occupied(o) => assert_eq!(Some(o.get_key_value()), kv),
                Vacant(_) => assert_eq!(v, None),
            }
            match map.raw_entry_mut().from_hash(hash, |q| *q == k) {
                Occupied(o) => assert_eq!(Some(o.get_key_value()), kv),
                Vacant(_) => assert_eq!(v, None),
            }
        }
    }

    #[test]
    fn test_key_without_hash_impl() {
        #[derive(Debug)]
        struct IntWrapper(u64);

        let mut m: HashMap<IntWrapper, (), ()> = HashMap::default();
        {
            assert!(m.raw_entry().from_hash(0, |k| k.0 == 0).is_none());
        }
        {
            let vacant_entry = match m.raw_entry_mut().from_hash(0, |k| k.0 == 0) {
                RawEntryMut::Occupied(..) => panic!("Found entry for key 0"),
                RawEntryMut::Vacant(e) => e,
            };
            vacant_entry.insert_with_hasher(0, IntWrapper(0), (), |k| k.0);
        }
        {
            assert!(m.raw_entry().from_hash(0, |k| k.0 == 0).is_some());
            assert!(m.raw_entry().from_hash(1, |k| k.0 == 1).is_none());
            assert!(m.raw_entry().from_hash(2, |k| k.0 == 2).is_none());
        }
        {
            let vacant_entry = match m.raw_entry_mut().from_hash(1, |k| k.0 == 1) {
                RawEntryMut::Occupied(..) => panic!("Found entry for key 1"),
                RawEntryMut::Vacant(e) => e,
            };
            vacant_entry.insert_with_hasher(1, IntWrapper(1), (), |k| k.0);
        }
        {
            assert!(m.raw_entry().from_hash(0, |k| k.0 == 0).is_some());
            assert!(m.raw_entry().from_hash(1, |k| k.0 == 1).is_some());
            assert!(m.raw_entry().from_hash(2, |k| k.0 == 2).is_none());
        }
        {
            let occupied_entry = match m.raw_entry_mut().from_hash(0, |k| k.0 == 0) {
                RawEntryMut::Occupied(e) => e,
                RawEntryMut::Vacant(..) => panic!("Couldn't find entry for key 0"),
            };
            occupied_entry.remove();
        }
        assert!(m.raw_entry().from_hash(0, |k| k.0 == 0).is_none());
        assert!(m.raw_entry().from_hash(1, |k| k.0 == 1).is_some());
        assert!(m.raw_entry().from_hash(2, |k| k.0 == 2).is_none());
    }

    #[test]
    #[cfg(feature = "raw")]
    fn test_into_iter_refresh() {
        #[cfg(miri)]
        const N: usize = 32;
        #[cfg(not(miri))]
        const N: usize = 128;

        let mut rng = rand::thread_rng();
        for n in 0..N {
            let mut map = HashMap::new();
            for i in 0..n {
                assert!(map.insert(i, 2 * i).is_none());
            }
            let hash_builder = map.hasher().clone();

            let mut it = unsafe { map.table.iter() };
            assert_eq!(it.len(), n);

            let mut i = 0;
            let mut left = n;
            let mut removed = Vec::new();
            loop {
                // occasionally remove some elements
                if i < n && rng.gen_bool(0.1) {
                    let hash_value = super::make_hash(&hash_builder, &i);

                    unsafe {
                        let e = map.table.find(hash_value, |q| q.0.eq(&i));
                        if let Some(e) = e {
                            it.reflect_remove(&e);
                            let t = map.table.remove(e).0;
                            removed.push(t);
                            left -= 1;
                        } else {
                            assert!(removed.contains(&(i, 2 * i)), "{i} not in {removed:?}");
                            let e = map.table.insert(
                                hash_value,
                                (i, 2 * i),
                                super::make_hasher::<_, usize, _>(&hash_builder),
                            );
                            it.reflect_insert(&e);
                            if let Some(p) = removed.iter().position(|e| e == &(i, 2 * i)) {
                                removed.swap_remove(p);
                            }
                            left += 1;
                        }
                    }
                }

                let e = it.next();
                if e.is_none() {
                    break;
                }
                assert!(i < n);
                let t = unsafe { e.unwrap().as_ref() };
                assert!(!removed.contains(t));
                let (key, value) = t;
                assert_eq!(*value, 2 * key);
                i += 1;
            }
            assert!(i <= n);

            // just for safety:
            assert_eq!(map.table.len(), left);
        }
    }

    #[test]
    fn test_const_with_hasher() {
        use core::hash::BuildHasher;
        use std::collections::hash_map::DefaultHasher;

        #[derive(Clone)]
        struct MyHasher;
        impl BuildHasher for MyHasher {
            type Hasher = DefaultHasher;

            fn build_hasher(&self) -> DefaultHasher {
                DefaultHasher::new()
            }
        }

        const EMPTY_MAP: HashMap<u32, std::string::String, MyHasher> =
            HashMap::with_hasher(MyHasher);

        let mut map = EMPTY_MAP;
        map.insert(17, "seventeen".to_owned());
        assert_eq!("seventeen", map[&17]);
    }

    #[test]
    fn test_get_each_mut() {
        let mut map = HashMap::new();
        map.insert("foo".to_owned(), 0);
        map.insert("bar".to_owned(), 10);
        map.insert("baz".to_owned(), 20);
        map.insert("qux".to_owned(), 30);

        let xs = map.get_many_mut(["foo", "qux"]);
        assert_eq!(xs, Some([&mut 0, &mut 30]));

        let xs = map.get_many_mut(["foo", "dud"]);
        assert_eq!(xs, None);

        let xs = map.get_many_mut(["foo", "foo"]);
        assert_eq!(xs, None);

        let ys = map.get_many_key_value_mut(["bar", "baz"]);
        assert_eq!(
            ys,
            Some([(&"bar".to_owned(), &mut 10), (&"baz".to_owned(), &mut 20),]),
        );

        let ys = map.get_many_key_value_mut(["bar", "dip"]);
        assert_eq!(ys, None);

        let ys = map.get_many_key_value_mut(["baz", "baz"]);
        assert_eq!(ys, None);
    }

    #[test]
    #[should_panic = "panic in drop"]
    fn test_clone_from_double_drop() {
        #[derive(Clone)]
        struct CheckedDrop {
            panic_in_drop: bool,
            dropped: bool,
        }
        impl Drop for CheckedDrop {
            fn drop(&mut self) {
                if self.panic_in_drop {
                    self.dropped = true;
                    panic!("panic in drop");
                }
                if self.dropped {
                    panic!("double drop");
                }
                self.dropped = true;
            }
        }
        const DISARMED: CheckedDrop = CheckedDrop {
            panic_in_drop: false,
            dropped: false,
        };
        const ARMED: CheckedDrop = CheckedDrop {
            panic_in_drop: true,
            dropped: false,
        };

        let mut map1 = HashMap::new();
        map1.insert(1, DISARMED);
        map1.insert(2, DISARMED);
        map1.insert(3, DISARMED);
        map1.insert(4, DISARMED);

        let mut map2 = HashMap::new();
        map2.insert(1, DISARMED);
        map2.insert(2, ARMED);
        map2.insert(3, DISARMED);
        map2.insert(4, DISARMED);

        map2.clone_from(&map1);
    }

    #[test]
    #[should_panic = "panic in clone"]
    fn test_clone_from_memory_leaks() {
        use alloc::vec::Vec;

        struct CheckedClone {
            panic_in_clone: bool,
            need_drop: Vec<i32>,
        }
        impl Clone for CheckedClone {
            fn clone(&self) -> Self {
                if self.panic_in_clone {
                    panic!("panic in clone")
                }
                Self {
                    panic_in_clone: self.panic_in_clone,
                    need_drop: self.need_drop.clone(),
                }
            }
        }
        let mut map1 = HashMap::new();
        map1.insert(
            1,
            CheckedClone {
                panic_in_clone: false,
                need_drop: vec![0, 1, 2],
            },
        );
        map1.insert(
            2,
            CheckedClone {
                panic_in_clone: false,
                need_drop: vec![3, 4, 5],
            },
        );
        map1.insert(
            3,
            CheckedClone {
                panic_in_clone: true,
                need_drop: vec![6, 7, 8],
            },
        );
        let _map2 = map1.clone();
    }

    struct MyAllocInner {
        drop_count: Arc<AtomicI8>,
    }

    #[derive(Clone)]
    struct MyAlloc {
        _inner: Arc<MyAllocInner>,
    }

    impl MyAlloc {
        fn new(drop_count: Arc<AtomicI8>) -> Self {
            MyAlloc {
                _inner: Arc::new(MyAllocInner { drop_count }),
            }
        }
    }

    impl Drop for MyAllocInner {
        fn drop(&mut self) {
            println!("MyAlloc freed.");
            self.drop_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    unsafe impl Allocator for MyAlloc {
        fn allocate(&self, layout: Layout) -> std::result::Result<NonNull<[u8]>, AllocError> {
            let g = Global;
            g.allocate(layout)
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            let g = Global;
            g.deallocate(ptr, layout)
        }
    }

    #[test]
    fn test_hashmap_into_iter_bug() {
        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(1));

        {
            let mut map = HashMap::with_capacity_in(10, MyAlloc::new(dropped.clone()));
            for i in 0..10 {
                map.entry(i).or_insert_with(|| "i".to_string());
            }

            for (k, v) in map {
                println!("{}, {}", k, v);
            }
        }

        // All allocator clones should already be dropped.
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
    }

    #[derive(Debug)]
    struct CheckedCloneDrop<T> {
        panic_in_clone: bool,
        panic_in_drop: bool,
        dropped: bool,
        data: T,
    }

    impl<T> CheckedCloneDrop<T> {
        fn new(panic_in_clone: bool, panic_in_drop: bool, data: T) -> Self {
            CheckedCloneDrop {
                panic_in_clone,
                panic_in_drop,
                dropped: false,
                data,
            }
        }
    }

    impl<T: Clone> Clone for CheckedCloneDrop<T> {
        fn clone(&self) -> Self {
            if self.panic_in_clone {
                panic!("panic in clone")
            }
            Self {
                panic_in_clone: self.panic_in_clone,
                panic_in_drop: self.panic_in_drop,
                dropped: self.dropped,
                data: self.data.clone(),
            }
        }
    }

    impl<T> Drop for CheckedCloneDrop<T> {
        fn drop(&mut self) {
            if self.panic_in_drop {
                self.dropped = true;
                panic!("panic in drop");
            }
            if self.dropped {
                panic!("double drop");
            }
            self.dropped = true;
        }
    }

    /// Return hashmap with predefined distribution of elements.
    /// All elements will be located in the same order as elements
    /// returned by iterator.
    ///
    /// This function does not panic, but returns an error as a `String`
    /// to distinguish between a test panic and an error in the input data.
    fn get_test_map<I, T, A>(
        iter: I,
        mut fun: impl FnMut(u64) -> T,
        alloc: A,
    ) -> Result<HashMap<u64, CheckedCloneDrop<T>, DefaultHashBuilder, A>, String>
    where
        I: Iterator<Item = (bool, bool)> + Clone + ExactSizeIterator,
        A: Allocator,
        T: PartialEq + core::fmt::Debug,
    {
        use crate::scopeguard::guard;

        let mut map: HashMap<u64, CheckedCloneDrop<T>, _, A> =
            HashMap::with_capacity_in(iter.size_hint().0, alloc);
        {
            let mut guard = guard(&mut map, |map| {
                for (_, value) in map.iter_mut() {
                    value.panic_in_drop = false
                }
            });

            let mut count = 0;
            // Hash and Key must be equal to each other for controlling the elements placement.
            for (panic_in_clone, panic_in_drop) in iter.clone() {
                if core::mem::needs_drop::<T>() && panic_in_drop {
                    return Err(String::from(
                        "panic_in_drop can be set with a type that doesn't need to be dropped",
                    ));
                }
                guard.table.insert(
                    count,
                    (
                        count,
                        CheckedCloneDrop::new(panic_in_clone, panic_in_drop, fun(count)),
                    ),
                    |(k, _)| *k,
                );
                count += 1;
            }

            // Let's check that all elements are located as we wanted
            let mut check_count = 0;
            for ((key, value), (panic_in_clone, panic_in_drop)) in guard.iter().zip(iter) {
                if *key != check_count {
                    return Err(format!(
                        "key != check_count,\nkey: `{}`,\ncheck_count: `{}`",
                        key, check_count
                    ));
                }
                if value.dropped
                    || value.panic_in_clone != panic_in_clone
                    || value.panic_in_drop != panic_in_drop
                    || value.data != fun(check_count)
                {
                    return Err(format!(
                        "Value is not equal to expected,\nvalue: `{:?}`,\nexpected: \
                        `CheckedCloneDrop {{ panic_in_clone: {}, panic_in_drop: {}, dropped: {}, data: {:?} }}`",
                        value, panic_in_clone, panic_in_drop, false, fun(check_count)
                    ));
                }
                check_count += 1;
            }

            if guard.len() != check_count as usize {
                return Err(format!(
                    "map.len() != check_count,\nmap.len(): `{}`,\ncheck_count: `{}`",
                    guard.len(),
                    check_count
                ));
            }

            if count != check_count {
                return Err(format!(
                    "count != check_count,\ncount: `{}`,\ncheck_count: `{}`",
                    count, check_count
                ));
            }
            core::mem::forget(guard);
        }
        Ok(map)
    }

    const DISARMED: bool = false;
    const ARMED: bool = true;

    const ARMED_FLAGS: [bool; 8] = [
        DISARMED, DISARMED, DISARMED, ARMED, DISARMED, DISARMED, DISARMED, DISARMED,
    ];

    const DISARMED_FLAGS: [bool; 8] = [
        DISARMED, DISARMED, DISARMED, DISARMED, DISARMED, DISARMED, DISARMED, DISARMED,
    ];

    #[test]
    #[should_panic = "panic in clone"]
    fn test_clone_memory_leaks_and_double_drop_one() {
        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(2));

        {
            assert_eq!(ARMED_FLAGS.len(), DISARMED_FLAGS.len());

            let map: HashMap<u64, CheckedCloneDrop<Vec<u64>>, DefaultHashBuilder, MyAlloc> =
                match get_test_map(
                    ARMED_FLAGS.into_iter().zip(DISARMED_FLAGS),
                    |n| vec![n],
                    MyAlloc::new(dropped.clone()),
                ) {
                    Ok(map) => map,
                    Err(msg) => panic!("{msg}"),
                };

            // Clone should normally clone a few elements, and then (when the
            // clone function panics), deallocate both its own memory, memory
            // of `dropped: Arc<AtomicI8>` and the memory of already cloned
            // elements (Vec<i32> memory inside CheckedCloneDrop).
            let _map2 = map.clone();
        }
    }

    #[test]
    #[should_panic = "panic in drop"]
    fn test_clone_memory_leaks_and_double_drop_two() {
        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(2));

        {
            assert_eq!(ARMED_FLAGS.len(), DISARMED_FLAGS.len());

            let map: HashMap<u64, CheckedCloneDrop<u64>, DefaultHashBuilder, _> = match get_test_map(
                DISARMED_FLAGS.into_iter().zip(DISARMED_FLAGS),
                |n| n,
                MyAlloc::new(dropped.clone()),
            ) {
                Ok(map) => map,
                Err(msg) => panic!("{msg}"),
            };

            let mut map2 = match get_test_map(
                DISARMED_FLAGS.into_iter().zip(ARMED_FLAGS),
                |n| n,
                MyAlloc::new(dropped.clone()),
            ) {
                Ok(map) => map,
                Err(msg) => panic!("{msg}"),
            };

            // The `clone_from` should try to drop the elements of `map2` without
            // double drop and leaking the allocator. Elements that have not been
            // dropped leak their memory.
            map2.clone_from(&map);
        }
    }

    /// We check that we have a working table if the clone operation from another
    /// thread ended in a panic (when buckets of maps are equal to each other).
    #[test]
    fn test_catch_panic_clone_from_when_len_is_equal() {
        use std::thread;

        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(2));

        {
            assert_eq!(ARMED_FLAGS.len(), DISARMED_FLAGS.len());

            let mut map = match get_test_map(
                DISARMED_FLAGS.into_iter().zip(DISARMED_FLAGS),
                |n| vec![n],
                MyAlloc::new(dropped.clone()),
            ) {
                Ok(map) => map,
                Err(msg) => panic!("{msg}"),
            };

            thread::scope(|s| {
                let result: thread::ScopedJoinHandle<'_, String> = s.spawn(|| {
                    let scope_map =
                        match get_test_map(ARMED_FLAGS.into_iter().zip(DISARMED_FLAGS), |n| vec![n * 2], MyAlloc::new(dropped.clone())) {
                            Ok(map) => map,
                            Err(msg) => return msg,
                        };
                    if map.table.buckets() != scope_map.table.buckets() {
                        return format!(
                            "map.table.buckets() != scope_map.table.buckets(),\nleft: `{}`,\nright: `{}`",
                            map.table.buckets(), scope_map.table.buckets()
                        );
                    }
                    map.clone_from(&scope_map);
                    "We must fail the cloning!!!".to_owned()
                });
                if let Ok(msg) = result.join() {
                    panic!("{msg}")
                }
            });

            // Let's check that all iterators work fine and do not return elements
            // (especially `RawIterRange`, which does not depend on the number of
            // elements in the table, but looks directly at the control bytes)
            //
            // SAFETY: We know for sure that `RawTable` will outlive
            // the returned `RawIter / RawIterRange` iterator.
            assert_eq!(map.len(), 0);
            assert_eq!(map.iter().count(), 0);
            assert_eq!(unsafe { map.table.iter().count() }, 0);
            assert_eq!(unsafe { map.table.iter().iter.count() }, 0);

            for idx in 0..map.table.buckets() {
                let idx = idx as u64;
                assert!(
                    map.table.find(idx, |(k, _)| *k == idx).is_none(),
                    "Index: {idx}"
                );
            }
        }

        // All allocator clones should already be dropped.
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
    }

    /// We check that we have a working table if the clone operation from another
    /// thread ended in a panic (when buckets of maps are not equal to each other).
    #[test]
    fn test_catch_panic_clone_from_when_len_is_not_equal() {
        use std::thread;

        let dropped: Arc<AtomicI8> = Arc::new(AtomicI8::new(2));

        {
            assert_eq!(ARMED_FLAGS.len(), DISARMED_FLAGS.len());

            let mut map = match get_test_map(
                [DISARMED].into_iter().zip([DISARMED]),
                |n| vec![n],
                MyAlloc::new(dropped.clone()),
            ) {
                Ok(map) => map,
                Err(msg) => panic!("{msg}"),
            };

            thread::scope(|s| {
                let result: thread::ScopedJoinHandle<'_, String> = s.spawn(|| {
                    let scope_map = match get_test_map(
                        ARMED_FLAGS.into_iter().zip(DISARMED_FLAGS),
                        |n| vec![n * 2],
                        MyAlloc::new(dropped.clone()),
                    ) {
                        Ok(map) => map,
                        Err(msg) => return msg,
                    };
                    if map.table.buckets() == scope_map.table.buckets() {
                        return format!(
                            "map.table.buckets() == scope_map.table.buckets(): `{}`",
                            map.table.buckets()
                        );
                    }
                    map.clone_from(&scope_map);
                    "We must fail the cloning!!!".to_owned()
                });
                if let Ok(msg) = result.join() {
                    panic!("{msg}")
                }
            });

            // Let's check that all iterators work fine and do not return elements
            // (especially `RawIterRange`, which does not depend on the number of
            // elements in the table, but looks directly at the control bytes)
            //
            // SAFETY: We know for sure that `RawTable` will outlive
            // the returned `RawIter / RawIterRange` iterator.
            assert_eq!(map.len(), 0);
            assert_eq!(map.iter().count(), 0);
            assert_eq!(unsafe { map.table.iter().count() }, 0);
            assert_eq!(unsafe { map.table.iter().iter.count() }, 0);

            for idx in 0..map.table.buckets() {
                let idx = idx as u64;
                assert!(
                    map.table.find(idx, |(k, _)| *k == idx).is_none(),
                    "Index: {idx}"
                );
            }
        }

        // All allocator clones should already be dropped.
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
    }
}
