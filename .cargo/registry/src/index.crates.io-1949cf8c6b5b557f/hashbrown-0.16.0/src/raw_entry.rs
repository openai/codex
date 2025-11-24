use crate::hash_map::{equivalent, make_hash, make_hasher};
use crate::raw::{Allocator, Bucket, Global, RawTable};
use crate::{Equivalent, HashMap};
use core::fmt::{self, Debug};
use core::hash::{BuildHasher, Hash};
use core::mem;

impl<K, V, S, A: Allocator> HashMap<K, V, S, A> {
    /// Creates a raw entry builder for the `HashMap`.
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
    /// to put the `HashMap` into an inconsistent state which, while memory-safe,
    /// will cause the map to produce seemingly random results. Higher-level and
    /// more foolproof APIs like `entry` should be preferred when possible.
    ///
    /// In particular, the hash used to initialized the raw entry must still be
    /// consistent with the hash of the key that is ultimately stored in the entry.
    /// This is because implementations of `HashMap` may need to recompute hashes
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

    /// Creates a raw immutable entry builder for the `HashMap`.
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
    pub fn from_key<Q>(self, k: &Q) -> RawEntryMut<'a, K, V, S, A>
    where
        S: BuildHasher,
        Q: Hash + Equivalent<K> + ?Sized,
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
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, k: &Q) -> RawEntryMut<'a, K, V, S, A>
    where
        Q: Equivalent<K> + ?Sized,
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
    pub fn from_key<Q>(self, k: &Q) -> Option<(&'a K, &'a V)>
    where
        S: BuildHasher,
        Q: Hash + Equivalent<K> + ?Sized,
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
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, k: &Q) -> Option<(&'a K, &'a V)>
    where
        Q: Equivalent<K> + ?Sized,
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
    /// Sets the value of the entry, and returns a `RawOccupiedEntryMut`.
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

    /// Converts the `OccupiedEntry` into a mutable reference to the value in the entry
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

    /// Converts the `OccupiedEntry` into a mutable reference to the key and value in the entry
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
    /// Sets the value of the entry with the `VacantEntry`'s key,
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

    /// Sets the value of the entry with the `VacantEntry`'s key,
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

#[cfg(test)]
mod test_map {
    use super::HashMap;
    use super::RawEntryMut;

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
}
