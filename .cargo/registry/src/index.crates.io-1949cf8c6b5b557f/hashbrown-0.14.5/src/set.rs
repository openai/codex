#[cfg(feature = "raw")]
use crate::raw::RawTable;
use crate::{Equivalent, TryReserveError};
use alloc::borrow::ToOwned;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::iter::{Chain, FusedIterator};
use core::ops::{BitAnd, BitOr, BitXor, Sub};

use super::map::{self, DefaultHashBuilder, HashMap, Keys};
use crate::raw::{Allocator, Global, RawExtractIf};

// Future Optimization (FIXME!)
// =============================
//
// Iteration over zero sized values is a noop. There is no need
// for `bucket.val` in the case of HashSet. I suppose we would need HKT
// to get rid of it properly.

/// A hash set implemented as a `HashMap` where the value is `()`.
///
/// As with the [`HashMap`] type, a `HashSet` requires that the elements
/// implement the [`Eq`] and [`Hash`] traits. This can frequently be achieved by
/// using `#[derive(PartialEq, Eq, Hash)]`. If you implement these yourself,
/// it is important that the following property holds:
///
/// ```text
/// k1 == k2 -> hash(k1) == hash(k2)
/// ```
///
/// In other words, if two keys are equal, their hashes must be equal.
///
///
/// It is a logic error for an item to be modified in such a way that the
/// item's hash, as determined by the [`Hash`] trait, or its equality, as
/// determined by the [`Eq`] trait, changes while it is in the set. This is
/// normally only possible through [`Cell`], [`RefCell`], global state, I/O, or
/// unsafe code.
///
/// It is also a logic error for the [`Hash`] implementation of a key to panic.
/// This is generally only possible if the trait is implemented manually. If a
/// panic does occur then the contents of the `HashSet` may become corrupted and
/// some items may be dropped from the table.
///
/// # Examples
///
/// ```
/// use hashbrown::HashSet;
/// // Type inference lets us omit an explicit type signature (which
/// // would be `HashSet<String>` in this example).
/// let mut books = HashSet::new();
///
/// // Add some books.
/// books.insert("A Dance With Dragons".to_string());
/// books.insert("To Kill a Mockingbird".to_string());
/// books.insert("The Odyssey".to_string());
/// books.insert("The Great Gatsby".to_string());
///
/// // Check for a specific one.
/// if !books.contains("The Winds of Winter") {
///     println!("We have {} books, but The Winds of Winter ain't one.",
///              books.len());
/// }
///
/// // Remove a book.
/// books.remove("The Odyssey");
///
/// // Iterate over everything.
/// for book in &books {
///     println!("{}", book);
/// }
/// ```
///
/// The easiest way to use `HashSet` with a custom type is to derive
/// [`Eq`] and [`Hash`]. We must also derive [`PartialEq`]. This will in the
/// future be implied by [`Eq`].
///
/// ```
/// use hashbrown::HashSet;
/// #[derive(Hash, Eq, PartialEq, Debug)]
/// struct Viking {
///     name: String,
///     power: usize,
/// }
///
/// let mut vikings = HashSet::new();
///
/// vikings.insert(Viking { name: "Einar".to_string(), power: 9 });
/// vikings.insert(Viking { name: "Einar".to_string(), power: 9 });
/// vikings.insert(Viking { name: "Olaf".to_string(), power: 4 });
/// vikings.insert(Viking { name: "Harald".to_string(), power: 8 });
///
/// // Use derived implementation to print the vikings.
/// for x in &vikings {
///     println!("{:?}", x);
/// }
/// ```
///
/// A `HashSet` with fixed list of elements can be initialized from an array:
///
/// ```
/// use hashbrown::HashSet;
///
/// let viking_names: HashSet<&'static str> =
///     [ "Einar", "Olaf", "Harald" ].into_iter().collect();
/// // use the values stored in the set
/// ```
///
/// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
/// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
/// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
/// [`HashMap`]: struct.HashMap.html
/// [`PartialEq`]: https://doc.rust-lang.org/std/cmp/trait.PartialEq.html
/// [`RefCell`]: https://doc.rust-lang.org/std/cell/struct.RefCell.html
pub struct HashSet<T, S = DefaultHashBuilder, A: Allocator = Global> {
    pub(crate) map: HashMap<T, (), S, A>,
}

impl<T: Clone, S: Clone, A: Allocator + Clone> Clone for HashSet<T, S, A> {
    fn clone(&self) -> Self {
        HashSet {
            map: self.map.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.map.clone_from(&source.map);
    }
}

#[cfg(feature = "ahash")]
impl<T> HashSet<T, DefaultHashBuilder> {
    /// Creates an empty `HashSet`.
    ///
    /// The hash set is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`], for example with
    /// [`with_hasher`](HashSet::with_hasher) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let set: HashSet<i32> = HashSet::new();
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Creates an empty `HashSet` with the specified capacity.
    ///
    /// The hash set will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash set will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`], for example with
    /// [`with_capacity_and_hasher`](HashSet::with_capacity_and_hasher) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let set: HashSet<i32> = HashSet::with_capacity(10);
    /// assert!(set.capacity() >= 10);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
        }
    }
}

#[cfg(feature = "ahash")]
impl<T: Hash + Eq, A: Allocator> HashSet<T, DefaultHashBuilder, A> {
    /// Creates an empty `HashSet`.
    ///
    /// The hash set is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`], for example with
    /// [`with_hasher_in`](HashSet::with_hasher_in) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let set: HashSet<i32> = HashSet::new();
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn new_in(alloc: A) -> Self {
        Self {
            map: HashMap::new_in(alloc),
        }
    }

    /// Creates an empty `HashSet` with the specified capacity.
    ///
    /// The hash set will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash set will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`], for example with
    /// [`with_capacity_and_hasher_in`](HashSet::with_capacity_and_hasher_in) method.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let set: HashSet<i32> = HashSet::with_capacity(10);
    /// assert!(set.capacity() >= 10);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            map: HashMap::with_capacity_in(capacity, alloc),
        }
    }
}

impl<T, S, A: Allocator> HashSet<T, S, A> {
    /// Returns the number of elements the set can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let set: HashSet<i32> = HashSet::with_capacity(100);
    /// assert!(set.capacity() >= 100);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// An iterator visiting all elements in arbitrary order.
    /// The iterator element type is `&'a T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let mut set = HashSet::new();
    /// set.insert("a");
    /// set.insert("b");
    ///
    /// // Will print in an arbitrary order.
    /// for x in set.iter() {
    ///     println!("{}", x);
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            iter: self.map.keys(),
        }
    }

    /// Returns the number of elements in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut v = HashSet::new();
    /// assert_eq!(v.len(), 0);
    /// v.insert(1);
    /// assert_eq!(v.len(), 1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the set contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut v = HashSet::new();
    /// assert!(v.is_empty());
    /// v.insert(1);
    /// assert!(!v.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clears the set, returning all elements in an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// assert!(!set.is_empty());
    ///
    /// // print 1, 2, 3 in an arbitrary order
    /// for i in set.drain() {
    ///     println!("{}", i);
    /// }
    ///
    /// assert!(set.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn drain(&mut self) -> Drain<'_, T, A> {
        Drain {
            iter: self.map.drain(),
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let xs = [1,2,3,4,5,6];
    /// let mut set: HashSet<i32> = xs.into_iter().collect();
    /// set.retain(|&k| k % 2 == 0);
    /// assert_eq!(set.len(), 3);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.map.retain(|k, _| f(k));
    }

    /// Drains elements which are true under the given predicate,
    /// and returns an iterator over the removed items.
    ///
    /// In other words, move all elements `e` such that `f(&e)` returns `true` out
    /// into another iterator.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`retain()`] with a negated predicate if you do not need the returned iterator.
    ///
    /// [`retain()`]: HashSet::retain
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<i32> = (0..8).collect();
    /// let drained: HashSet<i32> = set.extract_if(|v| v % 2 == 0).collect();
    ///
    /// let mut evens = drained.into_iter().collect::<Vec<_>>();
    /// let mut odds = set.into_iter().collect::<Vec<_>>();
    /// evens.sort();
    /// odds.sort();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn extract_if<F>(&mut self, f: F) -> ExtractIf<'_, T, F, A>
    where
        F: FnMut(&T) -> bool,
    {
        ExtractIf {
            f,
            inner: RawExtractIf {
                iter: unsafe { self.map.table.iter() },
                table: &mut self.map.table,
            },
        }
    }

    /// Clears the set, removing all values.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut v = HashSet::new();
    /// v.insert(1);
    /// v.clear();
    /// assert!(v.is_empty());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

impl<T, S> HashSet<T, S, Global> {
    /// Creates a new empty hash set which will use the given hasher to hash
    /// keys.
    ///
    /// The hash set is initially created with a capacity of 0, so it will not
    /// allocate until it is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`].
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashSet to be useful, see its documentation for details.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut set = HashSet::with_hasher(s);
    /// set.insert(2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub const fn with_hasher(hasher: S) -> Self {
        Self {
            map: HashMap::with_hasher(hasher),
        }
    }

    /// Creates an empty `HashSet` with the specified capacity, using
    /// `hasher` to hash the keys.
    ///
    /// The hash set will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash set will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`].
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashSet to be useful, see its documentation for details.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut set = HashSet::with_capacity_and_hasher(10, s);
    /// set.insert(1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            map: HashMap::with_capacity_and_hasher(capacity, hasher),
        }
    }
}

impl<T, S, A> HashSet<T, S, A>
where
    A: Allocator,
{
    /// Returns a reference to the underlying allocator.
    #[inline]
    pub fn allocator(&self) -> &A {
        self.map.allocator()
    }

    /// Creates a new empty hash set which will use the given hasher to hash
    /// keys.
    ///
    /// The hash set is initially created with a capacity of 0, so it will not
    /// allocate until it is first inserted into.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`].
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashSet to be useful, see its documentation for details.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut set = HashSet::with_hasher(s);
    /// set.insert(2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub const fn with_hasher_in(hasher: S, alloc: A) -> Self {
        Self {
            map: HashMap::with_hasher_in(hasher, alloc),
        }
    }

    /// Creates an empty `HashSet` with the specified capacity, using
    /// `hasher` to hash the keys.
    ///
    /// The hash set will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash set will not allocate.
    ///
    /// # HashDoS resistance
    ///
    /// The `hash_builder` normally use a fixed key by default and that does
    /// not allow the `HashSet` to be protected against attacks such as [`HashDoS`].
    /// Users who require HashDoS resistance should explicitly use
    /// [`ahash::RandomState`] or [`std::collections::hash_map::RandomState`]
    /// as the hasher when creating a [`HashSet`].
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashSet to be useful, see its documentation for details.
    ///
    /// [`HashDoS`]: https://en.wikipedia.org/wiki/Collision_attack
    /// [`std::collections::hash_map::RandomState`]: https://doc.rust-lang.org/std/collections/hash_map/struct.RandomState.html
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let s = DefaultHashBuilder::default();
    /// let mut set = HashSet::with_capacity_and_hasher(10, s);
    /// set.insert(1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn with_capacity_and_hasher_in(capacity: usize, hasher: S, alloc: A) -> Self {
        Self {
            map: HashMap::with_capacity_and_hasher_in(capacity, hasher, alloc),
        }
    }

    /// Returns a reference to the set's [`BuildHasher`].
    ///
    /// [`BuildHasher`]: https://doc.rust-lang.org/std/hash/trait.BuildHasher.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_map::DefaultHashBuilder;
    ///
    /// let hasher = DefaultHashBuilder::default();
    /// let set: HashSet<i32> = HashSet::with_hasher(hasher);
    /// let hasher: &DefaultHashBuilder = set.hasher();
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn hasher(&self) -> &S {
        self.map.hasher()
    }
}

impl<T, S, A> HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    /// Reserves capacity for at least `additional` more elements to be inserted
    /// in the `HashSet`. The collection may reserve more space to avoid
    /// frequent reallocations.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds [`isize::MAX`] bytes and [`abort`] the program
    /// in case of allocation error. Use [`try_reserve`](HashSet::try_reserve) instead
    /// if you want to handle memory allocation failure.
    ///
    /// [`isize::MAX`]: https://doc.rust-lang.org/std/primitive.isize.html
    /// [`abort`]: https://doc.rust-lang.org/alloc/alloc/fn.handle_alloc_error.html
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let mut set: HashSet<i32> = HashSet::new();
    /// set.reserve(10);
    /// assert!(set.capacity() >= 10);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }

    /// Tries to reserve capacity for at least `additional` more elements to be inserted
    /// in the given `HashSet<K,V>`. The collection may reserve more space to avoid
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
    /// use hashbrown::HashSet;
    /// let mut set: HashSet<i32> = HashSet::new();
    /// set.try_reserve(10).expect("why is the test harness OOMing on 10 bytes?");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.map.try_reserve(additional)
    }

    /// Shrinks the capacity of the set as much as possible. It will drop
    /// down as much as possible while maintaining the internal rules
    /// and possibly leaving some space in accordance with the resize policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set = HashSet::with_capacity(100);
    /// set.insert(1);
    /// set.insert(2);
    /// assert!(set.capacity() >= 100);
    /// set.shrink_to_fit();
    /// assert!(set.capacity() >= 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }

    /// Shrinks the capacity of the set with a lower limit. It will drop
    /// down no lower than the supplied limit while maintaining the internal rules
    /// and possibly leaving some space in accordance with the resize policy.
    ///
    /// Panics if the current capacity is smaller than the supplied
    /// minimum capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set = HashSet::with_capacity(100);
    /// set.insert(1);
    /// set.insert(2);
    /// assert!(set.capacity() >= 100);
    /// set.shrink_to(10);
    /// assert!(set.capacity() >= 10);
    /// set.shrink_to(0);
    /// assert!(set.capacity() >= 2);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.map.shrink_to(min_capacity);
    }

    /// Visits the values representing the difference,
    /// i.e., the values that are in `self` but not in `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let a: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = [4, 2, 3, 4].into_iter().collect();
    ///
    /// // Can be seen as `a - b`.
    /// for x in a.difference(&b) {
    ///     println!("{}", x); // Print 1
    /// }
    ///
    /// let diff: HashSet<_> = a.difference(&b).collect();
    /// assert_eq!(diff, [1].iter().collect());
    ///
    /// // Note that difference is not symmetric,
    /// // and `b - a` means something else:
    /// let diff: HashSet<_> = b.difference(&a).collect();
    /// assert_eq!(diff, [4].iter().collect());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T, S, A> {
        Difference {
            iter: self.iter(),
            other,
        }
    }

    /// Visits the values representing the symmetric difference,
    /// i.e., the values that are in `self` or in `other` but not in both.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let a: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = [4, 2, 3, 4].into_iter().collect();
    ///
    /// // Print 1, 4 in arbitrary order.
    /// for x in a.symmetric_difference(&b) {
    ///     println!("{}", x);
    /// }
    ///
    /// let diff1: HashSet<_> = a.symmetric_difference(&b).collect();
    /// let diff2: HashSet<_> = b.symmetric_difference(&a).collect();
    ///
    /// assert_eq!(diff1, diff2);
    /// assert_eq!(diff1, [1, 4].iter().collect());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn symmetric_difference<'a>(&'a self, other: &'a Self) -> SymmetricDifference<'a, T, S, A> {
        SymmetricDifference {
            iter: self.difference(other).chain(other.difference(self)),
        }
    }

    /// Visits the values representing the intersection,
    /// i.e., the values that are both in `self` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let a: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = [4, 2, 3, 4].into_iter().collect();
    ///
    /// // Print 2, 3 in arbitrary order.
    /// for x in a.intersection(&b) {
    ///     println!("{}", x);
    /// }
    ///
    /// let intersection: HashSet<_> = a.intersection(&b).collect();
    /// assert_eq!(intersection, [2, 3].iter().collect());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn intersection<'a>(&'a self, other: &'a Self) -> Intersection<'a, T, S, A> {
        let (smaller, larger) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        Intersection {
            iter: smaller.iter(),
            other: larger,
        }
    }

    /// Visits the values representing the union,
    /// i.e., all the values in `self` or `other`, without duplicates.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let a: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = [4, 2, 3, 4].into_iter().collect();
    ///
    /// // Print 1, 2, 3, 4 in arbitrary order.
    /// for x in a.union(&b) {
    ///     println!("{}", x);
    /// }
    ///
    /// let union: HashSet<_> = a.union(&b).collect();
    /// assert_eq!(union, [1, 2, 3, 4].iter().collect());
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T, S, A> {
        // We'll iterate one set in full, and only the remaining difference from the other.
        // Use the smaller set for the difference in order to reduce hash lookups.
        let (smaller, larger) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };
        Union {
            iter: larger.iter().chain(smaller.difference(larger)),
        }
    }

    /// Returns `true` if the set contains a value.
    ///
    /// The value may be any borrowed form of the set's value type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the value type.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let set: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// assert_eq!(set.contains(&1), true);
    /// assert_eq!(set.contains(&4), false);
    /// ```
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.contains_key(value)
    }

    /// Returns a reference to the value in the set, if any, that is equal to the given value.
    ///
    /// The value may be any borrowed form of the set's value type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the value type.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let set: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// assert_eq!(set.get(&2), Some(&2));
    /// assert_eq!(set.get(&4), None);
    /// ```
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    where
        Q: Hash + Equivalent<T>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.map.get_key_value(value) {
            Some((k, _)) => Some(k),
            None => None,
        }
    }

    /// Inserts the given `value` into the set if it is not present, then
    /// returns a reference to the value in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// assert_eq!(set.len(), 3);
    /// assert_eq!(set.get_or_insert(2), &2);
    /// assert_eq!(set.get_or_insert(100), &100);
    /// assert_eq!(set.len(), 4); // 100 was inserted
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_or_insert(&mut self, value: T) -> &T {
        // Although the raw entry gives us `&mut T`, we only return `&T` to be consistent with
        // `get`. Key mutation is "raw" because you're not supposed to affect `Eq` or `Hash`.
        self.map
            .raw_entry_mut()
            .from_key(&value)
            .or_insert(value, ())
            .0
    }

    /// Inserts an owned copy of the given `value` into the set if it is not
    /// present, then returns a reference to the value in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<String> = ["cat", "dog", "horse"]
    ///     .iter().map(|&pet| pet.to_owned()).collect();
    ///
    /// assert_eq!(set.len(), 3);
    /// for &pet in &["cat", "dog", "fish"] {
    ///     let value = set.get_or_insert_owned(pet);
    ///     assert_eq!(value, pet);
    /// }
    /// assert_eq!(set.len(), 4); // a new "fish" was inserted
    /// ```
    #[inline]
    pub fn get_or_insert_owned<Q: ?Sized>(&mut self, value: &Q) -> &T
    where
        Q: Hash + Equivalent<T> + ToOwned<Owned = T>,
    {
        // Although the raw entry gives us `&mut T`, we only return `&T` to be consistent with
        // `get`. Key mutation is "raw" because you're not supposed to affect `Eq` or `Hash`.
        self.map
            .raw_entry_mut()
            .from_key(value)
            .or_insert_with(|| (value.to_owned(), ()))
            .0
    }

    /// Inserts a value computed from `f` into the set if the given `value` is
    /// not present, then returns a reference to the value in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<String> = ["cat", "dog", "horse"]
    ///     .iter().map(|&pet| pet.to_owned()).collect();
    ///
    /// assert_eq!(set.len(), 3);
    /// for &pet in &["cat", "dog", "fish"] {
    ///     let value = set.get_or_insert_with(pet, str::to_owned);
    ///     assert_eq!(value, pet);
    /// }
    /// assert_eq!(set.len(), 4); // a new "fish" was inserted
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get_or_insert_with<Q: ?Sized, F>(&mut self, value: &Q, f: F) -> &T
    where
        Q: Hash + Equivalent<T>,
        F: FnOnce(&Q) -> T,
    {
        // Although the raw entry gives us `&mut T`, we only return `&T` to be consistent with
        // `get`. Key mutation is "raw" because you're not supposed to affect `Eq` or `Hash`.
        self.map
            .raw_entry_mut()
            .from_key(value)
            .or_insert_with(|| (f(value), ()))
            .0
    }

    /// Gets the given value's corresponding entry in the set for in-place manipulation.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_set::Entry::*;
    ///
    /// let mut singles = HashSet::new();
    /// let mut dupes = HashSet::new();
    ///
    /// for ch in "a short treatise on fungi".chars() {
    ///     if let Vacant(dupe_entry) = dupes.entry(ch) {
    ///         // We haven't already seen a duplicate, so
    ///         // check if we've at least seen it once.
    ///         match singles.entry(ch) {
    ///             Vacant(single_entry) => {
    ///                 // We found a new character for the first time.
    ///                 single_entry.insert()
    ///             }
    ///             Occupied(single_entry) => {
    ///                 // We've already seen this once, "move" it to dupes.
    ///                 single_entry.remove();
    ///                 dupe_entry.insert();
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// assert!(!singles.contains(&'t') && dupes.contains(&'t'));
    /// assert!(singles.contains(&'u') && !dupes.contains(&'u'));
    /// assert!(!singles.contains(&'v') && !dupes.contains(&'v'));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn entry(&mut self, value: T) -> Entry<'_, T, S, A> {
        match self.map.entry(value) {
            map::Entry::Occupied(entry) => Entry::Occupied(OccupiedEntry { inner: entry }),
            map::Entry::Vacant(entry) => Entry::Vacant(VacantEntry { inner: entry }),
        }
    }

    /// Returns `true` if `self` has no elements in common with `other`.
    /// This is equivalent to checking for an empty intersection.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let a: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// let mut b = HashSet::new();
    ///
    /// assert_eq!(a.is_disjoint(&b), true);
    /// b.insert(4);
    /// assert_eq!(a.is_disjoint(&b), true);
    /// b.insert(1);
    /// assert_eq!(a.is_disjoint(&b), false);
    /// ```
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.iter().all(|v| !other.contains(v))
    }

    /// Returns `true` if the set is a subset of another,
    /// i.e., `other` contains at least all the values in `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let sup: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// let mut set = HashSet::new();
    ///
    /// assert_eq!(set.is_subset(&sup), true);
    /// set.insert(2);
    /// assert_eq!(set.is_subset(&sup), true);
    /// set.insert(4);
    /// assert_eq!(set.is_subset(&sup), false);
    /// ```
    pub fn is_subset(&self, other: &Self) -> bool {
        self.len() <= other.len() && self.iter().all(|v| other.contains(v))
    }

    /// Returns `true` if the set is a superset of another,
    /// i.e., `self` contains at least all the values in `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let sub: HashSet<_> = [1, 2].into_iter().collect();
    /// let mut set = HashSet::new();
    ///
    /// assert_eq!(set.is_superset(&sub), false);
    ///
    /// set.insert(0);
    /// set.insert(1);
    /// assert_eq!(set.is_superset(&sub), false);
    ///
    /// set.insert(2);
    /// assert_eq!(set.is_superset(&sub), true);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Adds a value to the set.
    ///
    /// If the set did not have this value present, `true` is returned.
    ///
    /// If the set did have this value present, `false` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set = HashSet::new();
    ///
    /// assert_eq!(set.insert(2), true);
    /// assert_eq!(set.insert(2), false);
    /// assert_eq!(set.len(), 1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(&mut self, value: T) -> bool {
        self.map.insert(value, ()).is_none()
    }

    /// Insert a value the set without checking if the value already exists in the set.
    ///
    /// Returns a reference to the value just inserted.
    ///
    /// This operation is safe if a value does not exist in the set.
    ///
    /// However, if a value exists in the set already, the behavior is unspecified:
    /// this operation may panic, loop forever, or any following operation with the set
    /// may panic, loop forever or return arbitrary result.
    ///
    /// That said, this operation (and following operations) are guaranteed to
    /// not violate memory safety.
    ///
    /// This operation is faster than regular insert, because it does not perform
    /// lookup before insertion.
    ///
    /// This operation is useful during initial population of the set.
    /// For example, when constructing a set from another set, we know
    /// that values are unique.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert_unique_unchecked(&mut self, value: T) -> &T {
        self.map.insert_unique_unchecked(value, ()).0
    }

    /// Adds a value to the set, replacing the existing value, if any, that is equal to the given
    /// one. Returns the replaced value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set = HashSet::new();
    /// set.insert(Vec::<i32>::new());
    ///
    /// assert_eq!(set.get(&[][..]).unwrap().capacity(), 0);
    /// set.replace(Vec::with_capacity(10));
    /// assert_eq!(set.get(&[][..]).unwrap().capacity(), 10);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace(&mut self, value: T) -> Option<T> {
        match self.map.entry(value) {
            map::Entry::Occupied(occupied) => Some(occupied.replace_key()),
            map::Entry::Vacant(vacant) => {
                vacant.insert(());
                None
            }
        }
    }

    /// Removes a value from the set. Returns whether the value was
    /// present in the set.
    ///
    /// The value may be any borrowed form of the set's value type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the value type.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set = HashSet::new();
    ///
    /// set.insert(2);
    /// assert_eq!(set.remove(&2), true);
    /// assert_eq!(set.remove(&2), false);
    /// ```
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.remove(value).is_some()
    }

    /// Removes and returns the value in the set, if any, that is equal to the given one.
    ///
    /// The value may be any borrowed form of the set's value type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the value type.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<_> = [1, 2, 3].into_iter().collect();
    /// assert_eq!(set.take(&2), Some(2));
    /// assert_eq!(set.take(&2), None);
    /// ```
    ///
    /// [`Eq`]: https://doc.rust-lang.org/std/cmp/trait.Eq.html
    /// [`Hash`]: https://doc.rust-lang.org/std/hash/trait.Hash.html
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T>,
    {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.map.remove_entry(value) {
            Some((k, _)) => Some(k),
            None => None,
        }
    }
}

impl<T, S, A: Allocator> HashSet<T, S, A> {
    /// Returns a reference to the [`RawTable`] used underneath [`HashSet`].
    /// This function is only available if the `raw` feature of the crate is enabled.
    ///
    /// # Note
    ///
    /// Calling this function is safe, but using the raw hash table API may require
    /// unsafe functions or blocks.
    ///
    /// `RawTable` API gives the lowest level of control under the set that can be useful
    /// for extending the HashSet's API, but may lead to *[undefined behavior]*.
    ///
    /// [`HashSet`]: struct.HashSet.html
    /// [`RawTable`]: crate::raw::RawTable
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cfg(feature = "raw")]
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_table(&self) -> &RawTable<(T, ()), A> {
        self.map.raw_table()
    }

    /// Returns a mutable reference to the [`RawTable`] used underneath [`HashSet`].
    /// This function is only available if the `raw` feature of the crate is enabled.
    ///
    /// # Note
    ///
    /// Calling this function is safe, but using the raw hash table API may require
    /// unsafe functions or blocks.
    ///
    /// `RawTable` API gives the lowest level of control under the set that can be useful
    /// for extending the HashSet's API, but may lead to *[undefined behavior]*.
    ///
    /// [`HashSet`]: struct.HashSet.html
    /// [`RawTable`]: crate::raw::RawTable
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[cfg(feature = "raw")]
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn raw_table_mut(&mut self) -> &mut RawTable<(T, ()), A> {
        self.map.raw_table_mut()
    }
}

impl<T, S, A> PartialEq for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter().all(|key| other.contains(key))
    }
}

impl<T, S, A> Eq for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

impl<T, S, A> fmt::Debug for HashSet<T, S, A>
where
    T: fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T, S, A> From<HashMap<T, (), S, A>> for HashSet<T, S, A>
where
    A: Allocator,
{
    fn from(map: HashMap<T, (), S, A>) -> Self {
        Self { map }
    }
}

impl<T, S, A> FromIterator<T> for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
    A: Default + Allocator,
{
    #[cfg_attr(feature = "inline-more", inline)]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::with_hasher_in(Default::default(), Default::default());
        set.extend(iter);
        set
    }
}

// The default hasher is used to match the std implementation signature
#[cfg(feature = "ahash")]
impl<T, A, const N: usize> From<[T; N]> for HashSet<T, DefaultHashBuilder, A>
where
    T: Eq + Hash,
    A: Default + Allocator,
{
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let set1 = HashSet::from([1, 2, 3, 4]);
    /// let set2: HashSet<_> = [1, 2, 3, 4].into();
    /// assert_eq!(set1, set2);
    /// ```
    fn from(arr: [T; N]) -> Self {
        arr.into_iter().collect()
    }
}

impl<T, S, A> Extend<T> for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    #[cfg_attr(feature = "inline-more", inline)]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.map.extend(iter.into_iter().map(|k| (k, ())));
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_one(&mut self, k: T) {
        self.map.insert(k, ());
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_reserve(&mut self, additional: usize) {
        Extend::<(T, ())>::extend_reserve(&mut self.map, additional);
    }
}

impl<'a, T, S, A> Extend<&'a T> for HashSet<T, S, A>
where
    T: 'a + Eq + Hash + Copy,
    S: BuildHasher,
    A: Allocator,
{
    #[cfg_attr(feature = "inline-more", inline)]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend(iter.into_iter().copied());
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_one(&mut self, k: &'a T) {
        self.map.insert(*k, ());
    }

    #[inline]
    #[cfg(feature = "nightly")]
    fn extend_reserve(&mut self, additional: usize) {
        Extend::<(T, ())>::extend_reserve(&mut self.map, additional);
    }
}

impl<T, S, A> Default for HashSet<T, S, A>
where
    S: Default,
    A: Default + Allocator,
{
    /// Creates an empty `HashSet<T, S>` with the `Default` value for the hasher.
    #[cfg_attr(feature = "inline-more", inline)]
    fn default() -> Self {
        Self {
            map: HashMap::default(),
        }
    }
}

impl<T, S, A> BitOr<&HashSet<T, S, A>> for &HashSet<T, S, A>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
    A: Allocator,
{
    type Output = HashSet<T, S>;

    /// Returns the union of `self` and `rhs` as a new `HashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let a: HashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = vec![3, 4, 5].into_iter().collect();
    ///
    /// let set = &a | &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2, 3, 4, 5];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitor(self, rhs: &HashSet<T, S, A>) -> HashSet<T, S> {
        self.union(rhs).cloned().collect()
    }
}

impl<T, S, A> BitAnd<&HashSet<T, S, A>> for &HashSet<T, S, A>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
    A: Allocator,
{
    type Output = HashSet<T, S>;

    /// Returns the intersection of `self` and `rhs` as a new `HashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let a: HashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = vec![2, 3, 4].into_iter().collect();
    ///
    /// let set = &a & &b;
    ///
    /// let mut i = 0;
    /// let expected = [2, 3];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitand(self, rhs: &HashSet<T, S, A>) -> HashSet<T, S> {
        self.intersection(rhs).cloned().collect()
    }
}

impl<T, S> BitXor<&HashSet<T, S>> for &HashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = HashSet<T, S>;

    /// Returns the symmetric difference of `self` and `rhs` as a new `HashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let a: HashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = vec![3, 4, 5].into_iter().collect();
    ///
    /// let set = &a ^ &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2, 4, 5];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitxor(self, rhs: &HashSet<T, S>) -> HashSet<T, S> {
        self.symmetric_difference(rhs).cloned().collect()
    }
}

impl<T, S> Sub<&HashSet<T, S>> for &HashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = HashSet<T, S>;

    /// Returns the difference of `self` and `rhs` as a new `HashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let a: HashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: HashSet<_> = vec![3, 4, 5].into_iter().collect();
    ///
    /// let set = &a - &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn sub(self, rhs: &HashSet<T, S>) -> HashSet<T, S> {
        self.difference(rhs).cloned().collect()
    }
}

/// An iterator over the items of a `HashSet`.
///
/// This `struct` is created by the [`iter`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`iter`]: struct.HashSet.html#method.iter
pub struct Iter<'a, K> {
    iter: Keys<'a, K, ()>,
}

/// An owning iterator over the items of a `HashSet`.
///
/// This `struct` is created by the [`into_iter`] method on [`HashSet`]
/// (provided by the `IntoIterator` trait). See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`into_iter`]: struct.HashSet.html#method.into_iter
pub struct IntoIter<K, A: Allocator = Global> {
    iter: map::IntoIter<K, (), A>,
}

/// A draining iterator over the items of a `HashSet`.
///
/// This `struct` is created by the [`drain`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`drain`]: struct.HashSet.html#method.drain
pub struct Drain<'a, K, A: Allocator = Global> {
    iter: map::Drain<'a, K, (), A>,
}

/// A draining iterator over entries of a `HashSet` which don't satisfy the predicate `f`.
///
/// This `struct` is created by the [`extract_if`] method on [`HashSet`]. See its
/// documentation for more.
///
/// [`extract_if`]: struct.HashSet.html#method.extract_if
/// [`HashSet`]: struct.HashSet.html
#[must_use = "Iterators are lazy unless consumed"]
pub struct ExtractIf<'a, K, F, A: Allocator = Global>
where
    F: FnMut(&K) -> bool,
{
    f: F,
    inner: RawExtractIf<'a, (K, ()), A>,
}

/// A lazy iterator producing elements in the intersection of `HashSet`s.
///
/// This `struct` is created by the [`intersection`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`intersection`]: struct.HashSet.html#method.intersection
pub struct Intersection<'a, T, S, A: Allocator = Global> {
    // iterator of the first set
    iter: Iter<'a, T>,
    // the second set
    other: &'a HashSet<T, S, A>,
}

/// A lazy iterator producing elements in the difference of `HashSet`s.
///
/// This `struct` is created by the [`difference`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`difference`]: struct.HashSet.html#method.difference
pub struct Difference<'a, T, S, A: Allocator = Global> {
    // iterator of the first set
    iter: Iter<'a, T>,
    // the second set
    other: &'a HashSet<T, S, A>,
}

/// A lazy iterator producing elements in the symmetric difference of `HashSet`s.
///
/// This `struct` is created by the [`symmetric_difference`] method on
/// [`HashSet`]. See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`symmetric_difference`]: struct.HashSet.html#method.symmetric_difference
pub struct SymmetricDifference<'a, T, S, A: Allocator = Global> {
    iter: Chain<Difference<'a, T, S, A>, Difference<'a, T, S, A>>,
}

/// A lazy iterator producing elements in the union of `HashSet`s.
///
/// This `struct` is created by the [`union`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`HashSet`]: struct.HashSet.html
/// [`union`]: struct.HashSet.html#method.union
pub struct Union<'a, T, S, A: Allocator = Global> {
    iter: Chain<Iter<'a, T>, Difference<'a, T, S, A>>,
}

impl<'a, T, S, A: Allocator> IntoIterator for &'a HashSet<T, S, A> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T, S, A: Allocator> IntoIterator for HashSet<T, S, A> {
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    /// Creates a consuming iterator, that is, one that moves each value out
    /// of the set in arbitrary order. The set cannot be used after calling
    /// this.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// let mut set = HashSet::new();
    /// set.insert("a".to_string());
    /// set.insert("b".to_string());
    ///
    /// // Not possible to collect to a Vec<String> with a regular `.iter()`.
    /// let v: Vec<String> = set.into_iter().collect();
    ///
    /// // Will print in an arbitrary order.
    /// for x in &v {
    ///     println!("{}", x);
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    fn into_iter(self) -> IntoIter<T, A> {
        IntoIter {
            iter: self.map.into_iter(),
        }
    }
}

impl<K> Clone for Iter<'_, K> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Iter {
            iter: self.iter.clone(),
        }
    }
}
impl<'a, K> Iterator for Iter<'a, K> {
    type Item = &'a K;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a K> {
        self.iter.next()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}
impl<'a, K> ExactSizeIterator for Iter<'a, K> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
impl<K> FusedIterator for Iter<'_, K> {}

impl<K: fmt::Debug> fmt::Debug for Iter<'_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<K, A: Allocator> Iterator for IntoIter<K, A> {
    type Item = K;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<K> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.iter.next() {
            Some((k, _)) => Some(k),
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |acc, (k, ())| f(acc, k))
    }
}
impl<K, A: Allocator> ExactSizeIterator for IntoIter<K, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
impl<K, A: Allocator> FusedIterator for IntoIter<K, A> {}

impl<K: fmt::Debug, A: Allocator> fmt::Debug for IntoIter<K, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries_iter = self.iter.iter().map(|(k, _)| k);
        f.debug_list().entries(entries_iter).finish()
    }
}

impl<K, A: Allocator> Iterator for Drain<'_, K, A> {
    type Item = K;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<K> {
        // Avoid `Option::map` because it bloats LLVM IR.
        match self.iter.next() {
            Some((k, _)) => Some(k),
            None => None,
        }
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |acc, (k, ())| f(acc, k))
    }
}
impl<K, A: Allocator> ExactSizeIterator for Drain<'_, K, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
impl<K, A: Allocator> FusedIterator for Drain<'_, K, A> {}

impl<K: fmt::Debug, A: Allocator> fmt::Debug for Drain<'_, K, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries_iter = self.iter.iter().map(|(k, _)| k);
        f.debug_list().entries(entries_iter).finish()
    }
}

impl<K, F, A: Allocator> Iterator for ExtractIf<'_, K, F, A>
where
    F: FnMut(&K) -> bool,
{
    type Item = K;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next(|&mut (ref k, ())| (self.f)(k))
            .map(|(k, ())| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.inner.iter.size_hint().1)
    }
}

impl<K, F, A: Allocator> FusedIterator for ExtractIf<'_, K, F, A> where F: FnMut(&K) -> bool {}

impl<T, S, A: Allocator> Clone for Intersection<'_, T, S, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Intersection {
            iter: self.iter.clone(),
            ..*self
        }
    }
}

impl<'a, T, S, A> Iterator for Intersection<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a T> {
        loop {
            let elt = self.iter.next()?;
            if self.other.contains(elt) {
                return Some(elt);
            }
        }
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |acc, elt| {
            if self.other.contains(elt) {
                f(acc, elt)
            } else {
                acc
            }
        })
    }
}

impl<T, S, A> fmt::Debug for Intersection<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<T, S, A> FusedIterator for Intersection<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

impl<T, S, A: Allocator> Clone for Difference<'_, T, S, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Difference {
            iter: self.iter.clone(),
            ..*self
        }
    }
}

impl<'a, T, S, A> Iterator for Difference<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a T> {
        loop {
            let elt = self.iter.next()?;
            if !self.other.contains(elt) {
                return Some(elt);
            }
        }
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |acc, elt| {
            if self.other.contains(elt) {
                acc
            } else {
                f(acc, elt)
            }
        })
    }
}

impl<T, S, A> FusedIterator for Difference<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

impl<T, S, A> fmt::Debug for Difference<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<T, S, A: Allocator> Clone for SymmetricDifference<'_, T, S, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        SymmetricDifference {
            iter: self.iter.clone(),
        }
    }
}

impl<'a, T, S, A> Iterator for SymmetricDifference<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a T> {
        self.iter.next()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

impl<T, S, A> FusedIterator for SymmetricDifference<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

impl<T, S, A> fmt::Debug for SymmetricDifference<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<T, S, A: Allocator> Clone for Union<'_, T, S, A> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Union {
            iter: self.iter.clone(),
        }
    }
}

impl<T, S, A> FusedIterator for Union<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

impl<T, S, A> fmt::Debug for Union<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, T, S, A> Iterator for Union<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn next(&mut self) -> Option<&'a T> {
        self.iter.next()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[cfg_attr(feature = "inline-more", inline)]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

/// A view into a single entry in a set, which may either be vacant or occupied.
///
/// This `enum` is constructed from the [`entry`] method on [`HashSet`].
///
/// [`HashSet`]: struct.HashSet.html
/// [`entry`]: struct.HashSet.html#method.entry
///
/// # Examples
///
/// ```
/// use hashbrown::hash_set::{Entry, HashSet, OccupiedEntry};
///
/// let mut set = HashSet::new();
/// set.extend(["a", "b", "c"]);
/// assert_eq!(set.len(), 3);
///
/// // Existing value (insert)
/// let entry: Entry<_, _> = set.entry("a");
/// let _raw_o: OccupiedEntry<_, _> = entry.insert();
/// assert_eq!(set.len(), 3);
/// // Nonexistent value (insert)
/// set.entry("d").insert();
///
/// // Existing value (or_insert)
/// set.entry("b").or_insert();
/// // Nonexistent value (or_insert)
/// set.entry("e").or_insert();
///
/// println!("Our HashSet: {:?}", set);
///
/// let mut vec: Vec<_> = set.iter().copied().collect();
/// // The `Iter` iterator produces items in arbitrary order, so the
/// // items must be sorted to test them against a sorted array.
/// vec.sort_unstable();
/// assert_eq!(vec, ["a", "b", "c", "d", "e"]);
/// ```
pub enum Entry<'a, T, S, A = Global>
where
    A: Allocator,
{
    /// An occupied entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_set::{Entry, HashSet};
    /// let mut set: HashSet<_> = ["a", "b"].into();
    ///
    /// match set.entry("a") {
    ///     Entry::Vacant(_) => unreachable!(),
    ///     Entry::Occupied(_) => { }
    /// }
    /// ```
    Occupied(OccupiedEntry<'a, T, S, A>),

    /// A vacant entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_set::{Entry, HashSet};
    /// let mut set: HashSet<&str> = HashSet::new();
    ///
    /// match set.entry("a") {
    ///     Entry::Occupied(_) => unreachable!(),
    ///     Entry::Vacant(_) => { }
    /// }
    /// ```
    Vacant(VacantEntry<'a, T, S, A>),
}

impl<T: fmt::Debug, S, A: Allocator> fmt::Debug for Entry<'_, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Entry::Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Entry::Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}

/// A view into an occupied entry in a `HashSet`.
/// It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_set::{Entry, HashSet, OccupiedEntry};
///
/// let mut set = HashSet::new();
/// set.extend(["a", "b", "c"]);
///
/// let _entry_o: OccupiedEntry<_, _> = set.entry("a").insert();
/// assert_eq!(set.len(), 3);
///
/// // Existing key
/// match set.entry("a") {
///     Entry::Vacant(_) => unreachable!(),
///     Entry::Occupied(view) => {
///         assert_eq!(view.get(), &"a");
///     }
/// }
///
/// assert_eq!(set.len(), 3);
///
/// // Existing key (take)
/// match set.entry("c") {
///     Entry::Vacant(_) => unreachable!(),
///     Entry::Occupied(view) => {
///         assert_eq!(view.remove(), "c");
///     }
/// }
/// assert_eq!(set.get(&"c"), None);
/// assert_eq!(set.len(), 2);
/// ```
pub struct OccupiedEntry<'a, T, S, A: Allocator = Global> {
    inner: map::OccupiedEntry<'a, T, (), S, A>,
}

impl<T: fmt::Debug, S, A: Allocator> fmt::Debug for OccupiedEntry<'_, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("value", self.get())
            .finish()
    }
}

/// A view into a vacant entry in a `HashSet`.
/// It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
///
/// # Examples
///
/// ```
/// use hashbrown::hash_set::{Entry, HashSet, VacantEntry};
///
/// let mut set = HashSet::<&str>::new();
///
/// let entry_v: VacantEntry<_, _> = match set.entry("a") {
///     Entry::Vacant(view) => view,
///     Entry::Occupied(_) => unreachable!(),
/// };
/// entry_v.insert();
/// assert!(set.contains("a") && set.len() == 1);
///
/// // Nonexistent key (insert)
/// match set.entry("b") {
///     Entry::Vacant(view) => view.insert(),
///     Entry::Occupied(_) => unreachable!(),
/// }
/// assert!(set.contains("b") && set.len() == 2);
/// ```
pub struct VacantEntry<'a, T, S, A: Allocator = Global> {
    inner: map::VacantEntry<'a, T, (), S, A>,
}

impl<T: fmt::Debug, S, A: Allocator> fmt::Debug for VacantEntry<'_, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.get()).finish()
    }
}

impl<'a, T, S, A: Allocator> Entry<'a, T, S, A> {
    /// Sets the value of the entry, and returns an OccupiedEntry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    /// let entry = set.entry("horseyland").insert();
    ///
    /// assert_eq!(entry.get(), &"horseyland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self) -> OccupiedEntry<'a, T, S, A>
    where
        T: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(entry) => entry.insert_entry(),
        }
    }

    /// Ensures a value is in the entry by inserting if it was vacant.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    ///
    /// // nonexistent key
    /// set.entry("poneyland").or_insert();
    /// assert!(set.contains("poneyland"));
    ///
    /// // existing key
    /// set.entry("poneyland").or_insert();
    /// assert!(set.contains("poneyland"));
    /// assert_eq!(set.len(), 1);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn or_insert(self)
    where
        T: Hash,
        S: BuildHasher,
    {
        if let Entry::Vacant(entry) = self {
            entry.insert();
        }
    }

    /// Returns a reference to this entry's value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    /// set.entry("poneyland").or_insert();
    /// // existing key
    /// assert_eq!(set.entry("poneyland").get(), &"poneyland");
    /// // nonexistent key
    /// assert_eq!(set.entry("horseland").get(), &"horseland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &T {
        match *self {
            Entry::Occupied(ref entry) => entry.get(),
            Entry::Vacant(ref entry) => entry.get(),
        }
    }
}

impl<T, S, A: Allocator> OccupiedEntry<'_, T, S, A> {
    /// Gets a reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_set::{Entry, HashSet};
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    /// set.entry("poneyland").or_insert();
    ///
    /// match set.entry("poneyland") {
    ///     Entry::Vacant(_) => panic!(),
    ///     Entry::Occupied(entry) => assert_eq!(entry.get(), &"poneyland"),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &T {
        self.inner.key()
    }

    /// Takes the value out of the entry, and returns it.
    /// Keeps the allocated memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_set::Entry;
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    /// // The set is empty
    /// assert!(set.is_empty() && set.capacity() == 0);
    ///
    /// set.entry("poneyland").or_insert();
    /// let capacity_before_remove = set.capacity();
    ///
    /// if let Entry::Occupied(o) = set.entry("poneyland") {
    ///     assert_eq!(o.remove(), "poneyland");
    /// }
    ///
    /// assert_eq!(set.contains("poneyland"), false);
    /// // Now set hold none elements but capacity is equal to the old one
    /// assert!(set.len() == 0 && set.capacity() == capacity_before_remove);
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn remove(self) -> T {
        self.inner.remove_entry().0
    }

    /// Replaces the entry, returning the old value. The new value in the hash map will be
    /// the value used to create this entry.
    ///
    /// # Panics
    ///
    /// Will panic if this OccupiedEntry was created through [`Entry::insert`].
    ///
    /// # Examples
    ///
    /// ```
    ///  use hashbrown::hash_set::{Entry, HashSet};
    ///  use std::rc::Rc;
    ///
    ///  let mut set: HashSet<Rc<String>> = HashSet::new();
    ///  let key_one = Rc::new("Stringthing".to_string());
    ///  let key_two = Rc::new("Stringthing".to_string());
    ///
    ///  set.insert(key_one.clone());
    ///  assert!(Rc::strong_count(&key_one) == 2 && Rc::strong_count(&key_two) == 1);
    ///
    ///  match set.entry(key_two.clone()) {
    ///      Entry::Occupied(entry) => {
    ///          let old_key: Rc<String> = entry.replace();
    ///          assert!(Rc::ptr_eq(&key_one, &old_key));
    ///      }
    ///      Entry::Vacant(_) => panic!(),
    ///  }
    ///
    ///  assert!(Rc::strong_count(&key_one) == 1 && Rc::strong_count(&key_two) == 2);
    ///  assert!(set.contains(&"Stringthing".to_owned()));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn replace(self) -> T {
        self.inner.replace_key()
    }
}

impl<'a, T, S, A: Allocator> VacantEntry<'a, T, S, A> {
    /// Gets a reference to the value that would be used when inserting
    /// through the `VacantEntry`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    /// assert_eq!(set.entry("poneyland").get(), &"poneyland");
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn get(&self) -> &T {
        self.inner.key()
    }

    /// Take ownership of the value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::hash_set::{Entry, HashSet};
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    ///
    /// match set.entry("poneyland") {
    ///     Entry::Occupied(_) => panic!(),
    ///     Entry::Vacant(v) => assert_eq!(v.into_value(), "poneyland"),
    /// }
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn into_value(self) -> T {
        self.inner.into_key()
    }

    /// Sets the value of the entry with the VacantEntry's value.
    ///
    /// # Examples
    ///
    /// ```
    /// use hashbrown::HashSet;
    /// use hashbrown::hash_set::Entry;
    ///
    /// let mut set: HashSet<&str> = HashSet::new();
    ///
    /// if let Entry::Vacant(o) = set.entry("poneyland") {
    ///     o.insert();
    /// }
    /// assert!(set.contains("poneyland"));
    /// ```
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn insert(self)
    where
        T: Hash,
        S: BuildHasher,
    {
        self.inner.insert(());
    }

    #[cfg_attr(feature = "inline-more", inline)]
    fn insert_entry(self) -> OccupiedEntry<'a, T, S, A>
    where
        T: Hash,
        S: BuildHasher,
    {
        OccupiedEntry {
            inner: self.inner.insert_entry(()),
        }
    }
}

#[allow(dead_code)]
fn assert_covariance() {
    fn set<'new>(v: HashSet<&'static str>) -> HashSet<&'new str> {
        v
    }
    fn iter<'a, 'new>(v: Iter<'a, &'static str>) -> Iter<'a, &'new str> {
        v
    }
    fn into_iter<'new, A: Allocator>(v: IntoIter<&'static str, A>) -> IntoIter<&'new str, A> {
        v
    }
    fn difference<'a, 'new, A: Allocator>(
        v: Difference<'a, &'static str, DefaultHashBuilder, A>,
    ) -> Difference<'a, &'new str, DefaultHashBuilder, A> {
        v
    }
    fn symmetric_difference<'a, 'new, A: Allocator>(
        v: SymmetricDifference<'a, &'static str, DefaultHashBuilder, A>,
    ) -> SymmetricDifference<'a, &'new str, DefaultHashBuilder, A> {
        v
    }
    fn intersection<'a, 'new, A: Allocator>(
        v: Intersection<'a, &'static str, DefaultHashBuilder, A>,
    ) -> Intersection<'a, &'new str, DefaultHashBuilder, A> {
        v
    }
    fn union<'a, 'new, A: Allocator>(
        v: Union<'a, &'static str, DefaultHashBuilder, A>,
    ) -> Union<'a, &'new str, DefaultHashBuilder, A> {
        v
    }
    fn drain<'new, A: Allocator>(d: Drain<'static, &'static str, A>) -> Drain<'new, &'new str, A> {
        d
    }
}

#[cfg(test)]
mod test_set {
    use super::super::map::DefaultHashBuilder;
    use super::HashSet;
    use std::vec::Vec;

    #[test]
    fn test_zero_capacities() {
        type HS = HashSet<i32>;

        let s = HS::new();
        assert_eq!(s.capacity(), 0);

        let s = HS::default();
        assert_eq!(s.capacity(), 0);

        let s = HS::with_hasher(DefaultHashBuilder::default());
        assert_eq!(s.capacity(), 0);

        let s = HS::with_capacity(0);
        assert_eq!(s.capacity(), 0);

        let s = HS::with_capacity_and_hasher(0, DefaultHashBuilder::default());
        assert_eq!(s.capacity(), 0);

        let mut s = HS::new();
        s.insert(1);
        s.insert(2);
        s.remove(&1);
        s.remove(&2);
        s.shrink_to_fit();
        assert_eq!(s.capacity(), 0);

        let mut s = HS::new();
        s.reserve(0);
        assert_eq!(s.capacity(), 0);
    }

    #[test]
    fn test_disjoint() {
        let mut xs = HashSet::new();
        let mut ys = HashSet::new();
        assert!(xs.is_disjoint(&ys));
        assert!(ys.is_disjoint(&xs));
        assert!(xs.insert(5));
        assert!(ys.insert(11));
        assert!(xs.is_disjoint(&ys));
        assert!(ys.is_disjoint(&xs));
        assert!(xs.insert(7));
        assert!(xs.insert(19));
        assert!(xs.insert(4));
        assert!(ys.insert(2));
        assert!(ys.insert(-11));
        assert!(xs.is_disjoint(&ys));
        assert!(ys.is_disjoint(&xs));
        assert!(ys.insert(7));
        assert!(!xs.is_disjoint(&ys));
        assert!(!ys.is_disjoint(&xs));
    }

    #[test]
    fn test_subset_and_superset() {
        let mut a = HashSet::new();
        assert!(a.insert(0));
        assert!(a.insert(5));
        assert!(a.insert(11));
        assert!(a.insert(7));

        let mut b = HashSet::new();
        assert!(b.insert(0));
        assert!(b.insert(7));
        assert!(b.insert(19));
        assert!(b.insert(250));
        assert!(b.insert(11));
        assert!(b.insert(200));

        assert!(!a.is_subset(&b));
        assert!(!a.is_superset(&b));
        assert!(!b.is_subset(&a));
        assert!(!b.is_superset(&a));

        assert!(b.insert(5));

        assert!(a.is_subset(&b));
        assert!(!a.is_superset(&b));
        assert!(!b.is_subset(&a));
        assert!(b.is_superset(&a));
    }

    #[test]
    fn test_iterate() {
        let mut a = HashSet::new();
        for i in 0..32 {
            assert!(a.insert(i));
        }
        let mut observed: u32 = 0;
        for k in &a {
            observed |= 1 << *k;
        }
        assert_eq!(observed, 0xFFFF_FFFF);
    }

    #[test]
    fn test_intersection() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(11));
        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(77));
        assert!(a.insert(103));
        assert!(a.insert(5));
        assert!(a.insert(-5));

        assert!(b.insert(2));
        assert!(b.insert(11));
        assert!(b.insert(77));
        assert!(b.insert(-9));
        assert!(b.insert(-42));
        assert!(b.insert(5));
        assert!(b.insert(3));

        let mut i = 0;
        let expected = [3, 5, 11, 77];
        for x in a.intersection(&b) {
            assert!(expected.contains(x));
            i += 1;
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_difference() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(3));
        assert!(b.insert(9));

        let mut i = 0;
        let expected = [1, 5, 11];
        for x in a.difference(&b) {
            assert!(expected.contains(x));
            i += 1;
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_symmetric_difference() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(-2));
        assert!(b.insert(3));
        assert!(b.insert(9));
        assert!(b.insert(14));
        assert!(b.insert(22));

        let mut i = 0;
        let expected = [-2, 1, 5, 11, 14, 22];
        for x in a.symmetric_difference(&b) {
            assert!(expected.contains(x));
            i += 1;
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_union() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));
        assert!(a.insert(16));
        assert!(a.insert(19));
        assert!(a.insert(24));

        assert!(b.insert(-2));
        assert!(b.insert(1));
        assert!(b.insert(5));
        assert!(b.insert(9));
        assert!(b.insert(13));
        assert!(b.insert(19));

        let mut i = 0;
        let expected = [-2, 1, 3, 5, 9, 11, 13, 16, 19, 24];
        for x in a.union(&b) {
            assert!(expected.contains(x));
            i += 1;
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_from_map() {
        let mut a = crate::HashMap::new();
        a.insert(1, ());
        a.insert(2, ());
        a.insert(3, ());
        a.insert(4, ());

        let a: HashSet<_> = a.into();

        assert_eq!(a.len(), 4);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));
    }

    #[test]
    fn test_from_iter() {
        let xs = [1, 2, 2, 3, 4, 5, 6, 7, 8, 9];

        let set: HashSet<_> = xs.iter().copied().collect();

        for x in &xs {
            assert!(set.contains(x));
        }

        assert_eq!(set.iter().len(), xs.len() - 1);
    }

    #[test]
    fn test_move_iter() {
        let hs = {
            let mut hs = HashSet::new();

            hs.insert('a');
            hs.insert('b');

            hs
        };

        let v = hs.into_iter().collect::<Vec<char>>();
        assert!(v == ['a', 'b'] || v == ['b', 'a']);
    }

    #[test]
    fn test_eq() {
        // These constants once happened to expose a bug in insert().
        // I'm keeping them around to prevent a regression.
        let mut s1 = HashSet::new();

        s1.insert(1);
        s1.insert(2);
        s1.insert(3);

        let mut s2 = HashSet::new();

        s2.insert(1);
        s2.insert(2);

        assert!(s1 != s2);

        s2.insert(3);

        assert_eq!(s1, s2);
    }

    #[test]
    fn test_show() {
        let mut set = HashSet::new();
        let empty = HashSet::<i32>::new();

        set.insert(1);
        set.insert(2);

        let set_str = format!("{set:?}");

        assert!(set_str == "{1, 2}" || set_str == "{2, 1}");
        assert_eq!(format!("{empty:?}"), "{}");
    }

    #[test]
    fn test_trivial_drain() {
        let mut s = HashSet::<i32>::new();
        for _ in s.drain() {}
        assert!(s.is_empty());
        drop(s);

        let mut s = HashSet::<i32>::new();
        drop(s.drain());
        assert!(s.is_empty());
    }

    #[test]
    fn test_drain() {
        let mut s: HashSet<_> = (1..100).collect();

        // try this a bunch of times to make sure we don't screw up internal state.
        for _ in 0..20 {
            assert_eq!(s.len(), 99);

            {
                let mut last_i = 0;
                let mut d = s.drain();
                for (i, x) in d.by_ref().take(50).enumerate() {
                    last_i = i;
                    assert!(x != 0);
                }
                assert_eq!(last_i, 49);
            }

            if !s.is_empty() {
                panic!("s should be empty!");
            }

            // reset to try again.
            s.extend(1..100);
        }
    }

    #[test]
    fn test_replace() {
        use core::hash;

        #[derive(Debug)]
        #[allow(dead_code)]
        struct Foo(&'static str, i32);

        impl PartialEq for Foo {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for Foo {}

        impl hash::Hash for Foo {
            fn hash<H: hash::Hasher>(&self, h: &mut H) {
                self.0.hash(h);
            }
        }

        let mut s = HashSet::new();
        assert_eq!(s.replace(Foo("a", 1)), None);
        assert_eq!(s.len(), 1);
        assert_eq!(s.replace(Foo("a", 2)), Some(Foo("a", 1)));
        assert_eq!(s.len(), 1);

        let mut it = s.iter();
        assert_eq!(it.next(), Some(&Foo("a", 2)));
        assert_eq!(it.next(), None);
    }

    #[test]
    #[allow(clippy::needless_borrow)]
    fn test_extend_ref() {
        let mut a = HashSet::new();
        a.insert(1);

        a.extend([2, 3, 4]);

        assert_eq!(a.len(), 4);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));

        let mut b = HashSet::new();
        b.insert(5);
        b.insert(6);

        a.extend(&b);

        assert_eq!(a.len(), 6);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));
        assert!(a.contains(&5));
        assert!(a.contains(&6));
    }

    #[test]
    fn test_retain() {
        let xs = [1, 2, 3, 4, 5, 6];
        let mut set: HashSet<i32> = xs.iter().copied().collect();
        set.retain(|&k| k % 2 == 0);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&2));
        assert!(set.contains(&4));
        assert!(set.contains(&6));
    }

    #[test]
    fn test_extract_if() {
        {
            let mut set: HashSet<i32> = (0..8).collect();
            let drained = set.extract_if(|&k| k % 2 == 0);
            let mut out = drained.collect::<Vec<_>>();
            out.sort_unstable();
            assert_eq!(vec![0, 2, 4, 6], out);
            assert_eq!(set.len(), 4);
        }
        {
            let mut set: HashSet<i32> = (0..8).collect();
            set.extract_if(|&k| k % 2 == 0).for_each(drop);
            assert_eq!(set.len(), 4, "Removes non-matching items on drop");
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

        const EMPTY_SET: HashSet<u32, MyHasher> = HashSet::with_hasher(MyHasher);

        let mut set = EMPTY_SET;
        set.insert(19);
        assert!(set.contains(&19));
    }

    #[test]
    fn rehash_in_place() {
        let mut set = HashSet::new();

        for i in 0..224 {
            set.insert(i);
        }

        assert_eq!(
            set.capacity(),
            224,
            "The set must be at or close to capacity to trigger a re hashing"
        );

        for i in 100..1400 {
            set.remove(&(i - 100));
            set.insert(i);
        }
    }

    #[test]
    fn collect() {
        // At the time of writing, this hits the ZST case in from_base_index
        // (and without the `map`, it does not).
        let mut _set: HashSet<_> = (0..3).map(|_| ()).collect();
    }
}
