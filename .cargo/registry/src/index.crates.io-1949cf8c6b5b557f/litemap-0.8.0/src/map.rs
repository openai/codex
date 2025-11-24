// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::store::*;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt::Debug;
use core::iter::FromIterator;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Index, IndexMut, Range};

macro_rules! litemap_impl(
    ($cfg:meta, $store:ident $(=$defaultty:ty)?) => {
        /// A simple "flat" map based on a sorted vector
        ///
        /// See the [module level documentation][super] for why one should use this.
        ///
        /// The API is roughly similar to that of [`std::collections::BTreeMap`].
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "yoke", derive(yoke::Yokeable))]
        #[cfg($cfg)]
        pub struct LiteMap<K: ?Sized, V: ?Sized, $store $(= $defaultty)?> {
            pub(crate) values: $store,
            pub(crate) _key_type: PhantomData<K>,
            pub(crate) _value_type: PhantomData<V>,
        }
    };

);
// You can't `cfg()` a default generic parameter, and we don't want to write this type twice
// and keep them in sync so we use a small macro
litemap_impl!(feature = "alloc", S = alloc::vec::Vec<(K, V)>);
litemap_impl!(not(feature = "alloc"), S);

#[cfg(feature = "alloc")]
impl<K, V> LiteMap<K, V> {
    /// Construct a new [`LiteMap`] backed by Vec    
    pub const fn new_vec() -> Self {
        Self {
            values: alloc::vec::Vec::new(),
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }
}

impl<K, V, S> LiteMap<K, V, S> {
    /// Construct a new [`LiteMap`] using the given values
    ///
    /// The store must be sorted and have no duplicate keys.
    pub const fn from_sorted_store_unchecked(values: S) -> Self {
        Self {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }
}

#[cfg(feature = "alloc")]
impl<K, V> LiteMap<K, V, Vec<(K, V)>> {
    /// Convert a [`LiteMap`] into a sorted `Vec<(K, V)>`.
    #[inline]
    pub fn into_tuple_vec(self) -> Vec<(K, V)> {
        self.values
    }
}

impl<K: ?Sized, V: ?Sized, S> LiteMap<K, V, S>
where
    S: StoreConstEmpty<K, V>,
{
    /// Create a new empty [`LiteMap`]
    pub const fn new() -> Self {
        Self {
            values: S::EMPTY,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }
}

impl<K: ?Sized, V: ?Sized, S> LiteMap<K, V, S>
where
    S: Store<K, V>,
{
    /// The number of elements in the [`LiteMap`]
    pub fn len(&self) -> usize {
        self.values.lm_len()
    }

    /// Whether the [`LiteMap`] is empty
    pub fn is_empty(&self) -> bool {
        self.values.lm_is_empty()
    }

    /// Get the key-value pair residing at a particular index
    ///
    /// In most cases, prefer [`LiteMap::get()`] over this method.
    #[inline]
    pub fn get_indexed(&self, index: usize) -> Option<(&K, &V)> {
        self.values.lm_get(index)
    }

    /// Get the lowest-rank key/value pair from the `LiteMap`, if it exists.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map =
    ///     LiteMap::<i32, &str, Vec<_>>::from_iter([(1, "uno"), (3, "tres")]);
    ///
    /// assert_eq!(map.first(), Some((&1, &"uno")));
    /// ```
    #[inline]
    pub fn first(&self) -> Option<(&K, &V)> {
        self.values.lm_get(0)
    }

    /// Get the highest-rank key/value pair from the `LiteMap`, if it exists.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map =
    ///     LiteMap::<i32, &str, Vec<_>>::from_iter([(1, "uno"), (3, "tres")]);
    ///
    /// assert_eq!(map.last(), Some((&3, &"tres")));
    /// ```
    #[inline]
    pub fn last(&self) -> Option<(&K, &V)> {
        self.values.lm_last()
    }

    /// Returns a new [`LiteMap`] with owned keys and values.
    ///
    /// The trait bounds allow transforming most slice and string types.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map: LiteMap<&str, &str> = LiteMap::new_vec();
    /// map.insert("one", "uno");
    /// map.insert("two", "dos");
    ///
    /// let boxed_map: LiteMap<Box<str>, Box<str>> = map.to_boxed_keys_values();
    ///
    /// assert_eq!(boxed_map.get("one"), Some(&Box::from("uno")));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn to_boxed_keys_values<KB: ?Sized, VB: ?Sized, SB>(&self) -> LiteMap<Box<KB>, Box<VB>, SB>
    where
        SB: StoreMut<Box<KB>, Box<VB>>,
        K: Borrow<KB>,
        V: Borrow<VB>,
        Box<KB>: for<'a> From<&'a KB>,
        Box<VB>: for<'a> From<&'a VB>,
    {
        let mut values = SB::lm_with_capacity(self.len());
        for i in 0..self.len() {
            #[allow(clippy::unwrap_used)] // iterating over our own length
            let (k, v) = self.values.lm_get(i).unwrap();
            values.lm_push(Box::from(k.borrow()), Box::from(v.borrow()))
        }
        LiteMap {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }

    /// Returns a new [`LiteMap`] with owned keys and cloned values.
    ///
    /// The trait bounds allow transforming most slice and string types.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map: LiteMap<&str, usize> = LiteMap::new_vec();
    /// map.insert("one", 11);
    /// map.insert("two", 22);
    ///
    /// let boxed_map: LiteMap<Box<str>, usize> = map.to_boxed_keys();
    ///
    /// assert_eq!(boxed_map.get("one"), Some(&11));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn to_boxed_keys<KB: ?Sized, SB>(&self) -> LiteMap<Box<KB>, V, SB>
    where
        V: Clone,
        SB: StoreMut<Box<KB>, V>,
        K: Borrow<KB>,
        Box<KB>: for<'a> From<&'a KB>,
    {
        let mut values = SB::lm_with_capacity(self.len());
        for i in 0..self.len() {
            #[allow(clippy::unwrap_used)] // iterating over our own length
            let (k, v) = self.values.lm_get(i).unwrap();
            values.lm_push(Box::from(k.borrow()), v.clone())
        }
        LiteMap {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }

    /// Returns a new [`LiteMap`] with cloned keys and owned values.
    ///
    /// The trait bounds allow transforming most slice and string types.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map: LiteMap<usize, &str> = LiteMap::new_vec();
    /// map.insert(11, "uno");
    /// map.insert(22, "dos");
    ///
    /// let boxed_map: LiteMap<usize, Box<str>> = map.to_boxed_values();
    ///
    /// assert_eq!(boxed_map.get(&11), Some(&Box::from("uno")));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn to_boxed_values<VB: ?Sized, SB>(&self) -> LiteMap<K, Box<VB>, SB>
    where
        K: Clone,
        SB: StoreMut<K, Box<VB>>,
        V: Borrow<VB>,
        Box<VB>: for<'a> From<&'a VB>,
    {
        let mut values = SB::lm_with_capacity(self.len());
        for i in 0..self.len() {
            #[allow(clippy::unwrap_used)] // iterating over our own length
            let (k, v) = self.values.lm_get(i).unwrap();
            values.lm_push(k.clone(), Box::from(v.borrow()))
        }
        LiteMap {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }
}

impl<K: ?Sized, V: ?Sized, S> LiteMap<K, V, S>
where
    K: Ord,
    S: Store<K, V>,
{
    /// Get the value associated with `key`, if it exists.
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// assert_eq!(map.get(&1), Some(&"one"));
    /// assert_eq!(map.get(&3), None);
    /// ```
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match self.find_index(key) {
            #[allow(clippy::unwrap_used)] // find_index returns a valid index
            Ok(found) => Some(self.values.lm_get(found).unwrap().1),
            Err(_) => None,
        }
    }

    /// Binary search the map with `predicate` to find a key, returning the value.
    pub fn get_by(&self, predicate: impl FnMut(&K) -> Ordering) -> Option<&V> {
        let index = self.values.lm_binary_search_by(predicate).ok()?;
        self.values.lm_get(index).map(|(_, v)| v)
    }

    /// Returns whether `key` is contained in this map
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// assert!(map.contains_key(&1));
    /// assert!(!map.contains_key(&3));
    /// ```
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.find_index(key).is_ok()
    }

    /// Obtain the index for a given key, or if the key is not found, the index
    /// at which it would be inserted.
    ///
    /// (The return value works equivalently to [`slice::binary_search_by()`])
    ///
    /// The indices returned can be used with [`Self::get_indexed()`]. Prefer using
    /// [`Self::get()`] directly where possible.
    #[inline]
    pub fn find_index<Q>(&self, key: &Q) -> Result<usize, usize>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.values.lm_binary_search_by(|k| k.borrow().cmp(key))
    }
}

impl<K: ?Sized, V: ?Sized, S> LiteMap<K, V, S>
where
    S: StoreSlice<K, V>,
{
    /// Creates a new [`LiteMap`] from a range of the current [`LiteMap`].
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// map.insert(3, "three");
    ///
    /// let mut sub_map = map.get_indexed_range(1..3).expect("valid range");
    /// assert_eq!(sub_map.get(&1), None);
    /// assert_eq!(sub_map.get(&2), Some(&"two"));
    /// assert_eq!(sub_map.get(&3), Some(&"three"));
    /// ```
    pub fn get_indexed_range(&self, range: Range<usize>) -> Option<LiteMap<K, V, &S::Slice>> {
        let subslice = self.values.lm_get_range(range)?;
        Some(LiteMap {
            values: subslice,
            _key_type: PhantomData,
            _value_type: PhantomData,
        })
    }

    /// Borrows this [`LiteMap`] as one of its slice type.
    ///
    /// This can be useful in situations where you need a `LiteMap` by value but do not want
    /// to clone the owned version.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    ///
    /// let borrowed_map = map.as_sliced();
    /// assert_eq!(borrowed_map.get(&1), Some(&"one"));
    /// assert_eq!(borrowed_map.get(&2), Some(&"two"));
    /// ```
    pub fn as_sliced(&self) -> LiteMap<K, V, &S::Slice> {
        // Won't panic: 0..self.len() is within range
        #[allow(clippy::unwrap_used)]
        let subslice = self.values.lm_get_range(0..self.len()).unwrap();
        LiteMap {
            values: subslice,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }

    /// Borrows the backing buffer of this [`LiteMap`] as its slice type.
    ///
    /// The slice will be sorted.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    ///
    /// let slice = map.as_slice();
    /// assert_eq!(slice, &[(1, "one"), (2, "two")]);
    /// ```
    pub fn as_slice(&self) -> &S::Slice {
        // Won't panic: 0..self.len() is within range
        #[allow(clippy::unwrap_used)]
        self.values.lm_get_range(0..self.len()).unwrap()
    }
}

impl<'a, K: 'a, V: 'a, S> LiteMap<K, V, S>
where
    S: Store<K, V>,
{
    /// Returns a new [`LiteMap`] with keys and values borrowed from this one.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map: LiteMap<Box<usize>, String> = LiteMap::new_vec();
    /// map.insert(Box::new(1), "one".to_string());
    /// map.insert(Box::new(2), "two".to_string());
    ///
    /// let borrowed_map: LiteMap<&usize, &str> = map.to_borrowed_keys_values();
    ///
    /// assert_eq!(borrowed_map.get(&1), Some(&"one"));
    /// ```
    pub fn to_borrowed_keys_values<KB: ?Sized, VB: ?Sized, SB>(
        &'a self,
    ) -> LiteMap<&'a KB, &'a VB, SB>
    where
        K: Borrow<KB>,
        V: Borrow<VB>,
        SB: StoreMut<&'a KB, &'a VB>,
    {
        let mut values = SB::lm_with_capacity(self.len());
        for i in 0..self.len() {
            #[allow(clippy::unwrap_used)] // iterating over our own length
            let (k, v) = self.values.lm_get(i).unwrap();
            values.lm_push(k.borrow(), v.borrow())
        }
        LiteMap {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }

    /// Returns a new [`LiteMap`] with keys borrowed from this one and cloned values.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map: LiteMap<Box<usize>, String> = LiteMap::new_vec();
    /// map.insert(Box::new(1), "one".to_string());
    /// map.insert(Box::new(2), "two".to_string());
    ///
    /// let borrowed_map: LiteMap<&usize, String> = map.to_borrowed_keys();
    ///
    /// assert_eq!(borrowed_map.get(&1), Some(&"one".to_string()));
    /// ```
    pub fn to_borrowed_keys<KB: ?Sized, SB>(&'a self) -> LiteMap<&'a KB, V, SB>
    where
        K: Borrow<KB>,
        V: Clone,
        SB: StoreMut<&'a KB, V>,
    {
        let mut values = SB::lm_with_capacity(self.len());
        for i in 0..self.len() {
            #[allow(clippy::unwrap_used)] // iterating over our own length
            let (k, v) = self.values.lm_get(i).unwrap();
            values.lm_push(k.borrow(), v.clone())
        }
        LiteMap {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }

    /// Returns a new [`LiteMap`] with values borrowed from this one and cloned keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map: LiteMap<Box<usize>, String> = LiteMap::new_vec();
    /// map.insert(Box::new(1), "one".to_string());
    /// map.insert(Box::new(2), "two".to_string());
    ///
    /// let borrowed_map: LiteMap<Box<usize>, &str> = map.to_borrowed_values();
    ///
    /// assert_eq!(borrowed_map.get(&1), Some(&"one"));
    /// ```
    pub fn to_borrowed_values<VB: ?Sized, SB>(&'a self) -> LiteMap<K, &'a VB, SB>
    where
        K: Clone,
        V: Borrow<VB>,
        SB: StoreMut<K, &'a VB>,
    {
        let mut values = SB::lm_with_capacity(self.len());
        for i in 0..self.len() {
            #[allow(clippy::unwrap_used)] // iterating over our own length
            let (k, v) = self.values.lm_get(i).unwrap();
            values.lm_push(k.clone(), v.borrow())
        }
        LiteMap {
            values,
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }
}

impl<K, V, S> LiteMap<K, V, S>
where
    S: StoreMut<K, V>,
{
    /// Construct a new [`LiteMap`] with a given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: S::lm_with_capacity(capacity),
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }

    /// Remove all elements from the [`LiteMap`]
    pub fn clear(&mut self) {
        self.values.lm_clear()
    }

    /// Reserve capacity for `additional` more elements to be inserted into
    /// the [`LiteMap`] to avoid frequent reallocations.
    ///
    /// See [`Vec::reserve()`] for more information.
    ///
    /// [`Vec::reserve()`]: alloc::vec::Vec::reserve
    pub fn reserve(&mut self, additional: usize) {
        self.values.lm_reserve(additional)
    }
}

impl<K, V, S> LiteMap<K, V, S>
where
    K: Ord,
    S: StoreMut<K, V>,
{
    /// Get the value associated with `key`, if it exists, as a mutable reference.
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// if let Some(mut v) = map.get_mut(&1) {
    ///     *v = "uno";
    /// }
    /// assert_eq!(map.get(&1), Some(&"uno"));
    /// ```
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match self.find_index(key) {
            #[allow(clippy::unwrap_used)] // find_index returns a valid index
            Ok(found) => Some(self.values.lm_get_mut(found).unwrap().1),
            Err(_) => None,
        }
    }

    /// Appends `value` with `key` to the end of the underlying vector, returning
    /// `key` and `value` _if it failed_. Useful for extending with an existing
    /// sorted list.
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// assert!(map.try_append(1, "uno").is_none());
    /// assert!(map.try_append(3, "tres").is_none());
    ///
    /// assert!(
    ///     matches!(map.try_append(3, "tres-updated"), Some((3, "tres-updated"))),
    ///     "append duplicate of last key",
    /// );
    ///
    /// assert!(
    ///     matches!(map.try_append(2, "dos"), Some((2, "dos"))),
    ///     "append out of order"
    /// );
    ///
    /// assert_eq!(map.get(&1), Some(&"uno"));
    ///
    /// // contains the original value for the key: 3
    /// assert_eq!(map.get(&3), Some(&"tres"));
    ///
    /// // not appended since it wasn't in order
    /// assert_eq!(map.get(&2), None);
    /// ```
    #[must_use]
    pub fn try_append(&mut self, key: K, value: V) -> Option<(K, V)> {
        if let Some(last) = self.values.lm_last() {
            if last.0 >= &key {
                return Some((key, value));
            }
        }

        self.values.lm_push(key, value);
        None
    }

    /// Insert `value` with `key`, returning the existing value if it exists.
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// assert_eq!(map.get(&1), Some(&"one"));
    /// assert_eq!(map.get(&3), None);
    /// ```
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_save_key(key, value).map(|(_, v)| v)
    }

    /// Version of [`Self::insert()`] that returns both the key and the old value.
    fn insert_save_key(&mut self, key: K, value: V) -> Option<(K, V)> {
        match self.values.lm_binary_search_by(|k| k.cmp(&key)) {
            #[allow(clippy::unwrap_used)] // Index came from binary_search
            Ok(found) => Some((
                key,
                mem::replace(self.values.lm_get_mut(found).unwrap().1, value),
            )),
            Err(ins) => {
                self.values.lm_insert(ins, key, value);
                None
            }
        }
    }

    /// Attempts to insert a unique entry into the map.
    ///
    /// If `key` is not already in the map, inserts it with the corresponding `value`
    /// and returns `None`.
    ///
    /// If `key` is already in the map, no change is made to the map, and the key and value
    /// are returned back to the caller.
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(3, "three");
    ///
    /// // 2 is not yet in the map...
    /// assert_eq!(map.try_insert(2, "two"), None);
    /// assert_eq!(map.len(), 3);
    ///
    /// // ...but now it is.
    /// assert_eq!(map.try_insert(2, "TWO"), Some((2, "TWO")));
    /// assert_eq!(map.len(), 3);
    /// ```
    pub fn try_insert(&mut self, key: K, value: V) -> Option<(K, V)> {
        match self.values.lm_binary_search_by(|k| k.cmp(&key)) {
            Ok(_) => Some((key, value)),
            Err(ins) => {
                self.values.lm_insert(ins, key, value);
                None
            }
        }
    }

    /// Attempts to insert a unique entry into the map.
    ///
    /// If `key` is not already in the map, invokes the closure to compute `value`, inserts
    /// the pair into the map, and returns a reference to the value. The closure is passed
    /// a reference to the `key` argument.
    ///
    /// If `key` is already in the map, a reference to the existing value is returned.
    ///
    /// Additionally, the index of the value in the map is returned. If it is not desirable
    /// to hold on to the mutable reference's lifetime, the index can be used to access the
    /// element via [`LiteMap::get_indexed()`].
    ///
    /// The closure returns a `Result` to allow for a fallible insertion function. If the
    /// creation of `value` is infallible, you can use [`core::convert::Infallible`].
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// /// Helper function to unwrap an `Infallible` result from the insertion function
    /// fn unwrap_infallible<T>(result: Result<T, core::convert::Infallible>) -> T {
    ///     result.unwrap_or_else(|never| match never {})
    /// }
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(3, "three");
    ///
    /// // 2 is not yet in the map...
    /// let result1 = unwrap_infallible(
    ///     map.try_get_or_insert(2, |_| Ok("two"))
    /// );
    /// assert_eq!(result1.1, &"two");
    /// assert_eq!(map.len(), 3);
    ///
    /// // ...but now it is.
    /// let result1 = unwrap_infallible(
    ///     map.try_get_or_insert(2, |_| Ok("TWO"))
    /// );
    /// assert_eq!(result1.1, &"two");
    /// assert_eq!(map.len(), 3);
    /// ```
    pub fn try_get_or_insert<E>(
        &mut self,
        key: K,
        value: impl FnOnce(&K) -> Result<V, E>,
    ) -> Result<(usize, &V), E> {
        let idx = match self.values.lm_binary_search_by(|k| k.cmp(&key)) {
            Ok(idx) => idx,
            Err(idx) => {
                let value = value(&key)?;
                self.values.lm_insert(idx, key, value);
                idx
            }
        };
        #[allow(clippy::unwrap_used)] // item at idx found or inserted above
        Ok((idx, self.values.lm_get(idx).unwrap().1))
    }

    /// Remove the value at `key`, returning it if it exists.
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// assert_eq!(map.remove(&1), Some("one"));
    /// assert_eq!(map.get(&1), None);
    /// ```
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match self.values.lm_binary_search_by(|k| k.borrow().cmp(key)) {
            Ok(found) => Some(self.values.lm_remove(found).1),
            Err(_) => None,
        }
    }
}

impl<K, V, S> LiteMap<K, V, S>
where
    K: Ord,
    S: StoreIntoIterator<K, V> + StoreFromIterator<K, V>,
{
    /// Insert all elements from `other` into this `LiteMap`.
    ///
    /// If `other` contains keys that already exist in `self`, the values in `other` replace the
    /// corresponding ones in `self`, and the rejected items from `self` are returned as a new
    /// `LiteMap`. Otherwise, `None` is returned.
    ///
    /// The implementation of this function is optimized if `self` and `other` have no overlap.
    ///
    /// # Examples
    ///
    /// ```
    /// use litemap::LiteMap;
    ///
    /// let mut map1 = LiteMap::new_vec();
    /// map1.insert(1, "one");
    /// map1.insert(2, "two");
    ///
    /// let mut map2 = LiteMap::new_vec();
    /// map2.insert(2, "TWO");
    /// map2.insert(4, "FOUR");
    ///
    /// let leftovers = map1.extend_from_litemap(map2);
    ///
    /// assert_eq!(map1.len(), 3);
    /// assert_eq!(map1.get(&1), Some("one").as_ref());
    /// assert_eq!(map1.get(&2), Some("TWO").as_ref());
    /// assert_eq!(map1.get(&4), Some("FOUR").as_ref());
    ///
    /// let map3 = leftovers.expect("Duplicate keys");
    /// assert_eq!(map3.len(), 1);
    /// assert_eq!(map3.get(&2), Some("two").as_ref());
    /// ```
    pub fn extend_from_litemap(&mut self, other: Self) -> Option<Self> {
        if self.is_empty() {
            self.values = other.values;
            return None;
        }
        if other.is_empty() {
            return None;
        }
        if self.last().map(|(k, _)| k) < other.first().map(|(k, _)| k) {
            // append other to self
            self.values.lm_extend_end(other.values);
            None
        } else if self.first().map(|(k, _)| k) > other.last().map(|(k, _)| k) {
            // prepend other to self
            self.values.lm_extend_start(other.values);
            None
        } else {
            // insert every element
            let leftover_tuples = other
                .values
                .lm_into_iter()
                .filter_map(|(k, v)| self.insert_save_key(k, v))
                .collect();
            let ret = LiteMap {
                values: leftover_tuples,
                _key_type: PhantomData,
                _value_type: PhantomData,
            };
            if ret.is_empty() {
                None
            } else {
                Some(ret)
            }
        }
    }
}

impl<K, V, S> Default for LiteMap<K, V, S>
where
    S: Store<K, V> + Default,
{
    fn default() -> Self {
        Self {
            values: S::default(),
            _key_type: PhantomData,
            _value_type: PhantomData,
        }
    }
}
impl<K, V, S> Index<&'_ K> for LiteMap<K, V, S>
where
    K: Ord,
    S: Store<K, V>,
{
    type Output = V;
    fn index(&self, key: &K) -> &V {
        #[allow(clippy::panic)] // documented
        match self.get(key) {
            Some(v) => v,
            None => panic!("no entry found for key"),
        }
    }
}
impl<K, V, S> IndexMut<&'_ K> for LiteMap<K, V, S>
where
    K: Ord,
    S: StoreMut<K, V>,
{
    fn index_mut(&mut self, key: &K) -> &mut V {
        #[allow(clippy::panic)] // documented
        match self.get_mut(key) {
            Some(v) => v,
            None => panic!("no entry found for key"),
        }
    }
}
impl<K, V, S> FromIterator<(K, V)> for LiteMap<K, V, S>
where
    K: Ord,
    S: StoreFromIterable<K, V>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let values = S::lm_sort_from_iter(iter);
        Self::from_sorted_store_unchecked(values)
    }
}

impl<'a, K: 'a, V: 'a, S> LiteMap<K, V, S>
where
    S: StoreIterable<'a, K, V>,
{
    /// Produce an ordered iterator over key-value pairs
    pub fn iter(&'a self) -> impl DoubleEndedIterator<Item = (&'a K, &'a V)> {
        self.values.lm_iter()
    }

    /// Produce an ordered iterator over keys
    #[deprecated = "use keys() instead"]
    pub fn iter_keys(&'a self) -> impl DoubleEndedIterator<Item = &'a K> {
        self.values.lm_iter().map(|val| val.0)
    }

    /// Produce an iterator over values, ordered by their keys
    #[deprecated = "use values() instead"]
    pub fn iter_values(&'a self) -> impl DoubleEndedIterator<Item = &'a V> {
        self.values.lm_iter().map(|val| val.1)
    }

    /// Produce an ordered iterator over keys
    pub fn keys(&'a self) -> impl DoubleEndedIterator<Item = &'a K> {
        self.values.lm_iter().map(|val| val.0)
    }

    /// Produce an iterator over values, ordered by their keys
    pub fn values(&'a self) -> impl DoubleEndedIterator<Item = &'a V> {
        self.values.lm_iter().map(|val| val.1)
    }
}

impl<'a, K: 'a, V: 'a, S> LiteMap<K, V, S>
where
    S: StoreIterableMut<'a, K, V>,
{
    /// Produce an ordered mutable iterator over key-value pairs
    pub fn iter_mut(&'a mut self) -> impl DoubleEndedIterator<Item = (&'a K, &'a mut V)> {
        self.values.lm_iter_mut()
    }
}

impl<K, V, S> IntoIterator for LiteMap<K, V, S>
where
    S: StoreIntoIterator<K, V>,
{
    type Item = (K, V);
    type IntoIter = S::KeyValueIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.values.lm_into_iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a LiteMap<K, V, S>
where
    S: StoreIterable<'a, K, V>,
{
    type Item = (&'a K, &'a V);
    type IntoIter = S::KeyValueIter;

    fn into_iter(self) -> Self::IntoIter {
        self.values.lm_iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut LiteMap<K, V, S>
where
    S: StoreIterableMut<'a, K, V>,
{
    type Item = (&'a K, &'a mut V);
    type IntoIter = S::KeyValueIterMut;

    fn into_iter(self) -> Self::IntoIter {
        self.values.lm_iter_mut()
    }
}

impl<K, V, S> LiteMap<K, V, S>
where
    S: StoreBulkMut<K, V>,
{
    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements such that `f((&k, &v))` returns `false`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// let mut map = LiteMap::new_vec();
    /// map.insert(1, "one");
    /// map.insert(2, "two");
    /// map.insert(3, "three");
    ///
    /// // Retain elements with odd keys
    /// map.retain(|k, _| k % 2 == 1);
    ///
    /// assert_eq!(map.get(&1), Some(&"one"));
    /// assert_eq!(map.get(&2), None);
    /// ```
    #[inline]
    pub fn retain<F>(&mut self, predicate: F)
    where
        F: FnMut(&K, &V) -> bool,
    {
        self.values.lm_retain(predicate)
    }
}

impl<'a, K, V> LiteMap<K, V, &'a [(K, V)]> {
    /// Const version of [`LiteMap::len()`] for a slice store.
    ///
    /// Note: This function will no longer be needed if const trait behavior is stabilized.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// static map: LiteMap<&str, usize, &[(&str, usize)]> =
    ///     LiteMap::from_sorted_store_unchecked(&[("a", 11), ("b", 22)]);
    /// static len: usize = map.const_len();
    /// assert_eq!(len, 2);
    /// ```
    #[inline]
    pub const fn const_len(&self) -> usize {
        self.values.len()
    }

    /// Const version of [`LiteMap::is_empty()`] for a slice store.
    ///
    /// Note: This function will no longer be needed if const trait behavior is stabilized.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// static map: LiteMap<&str, usize, &[(&str, usize)]> =
    ///     LiteMap::from_sorted_store_unchecked(&[]);
    /// static is_empty: bool = map.const_is_empty();
    /// assert!(is_empty);
    /// ```
    #[inline]
    pub const fn const_is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Const version of [`LiteMap::get_indexed()`] for a slice store.
    ///
    /// Note: This function will no longer be needed if const trait behavior is stabilized.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// static map: LiteMap<&str, usize, &[(&str, usize)]> =
    ///     LiteMap::from_sorted_store_unchecked(&[("a", 11), ("b", 22)]);
    /// static t: &(&str, usize) = map.const_get_indexed_or_panic(0);
    /// assert_eq!(t.0, "a");
    /// assert_eq!(t.1, 11);
    /// ```
    #[inline]
    #[allow(clippy::indexing_slicing)] // documented
    pub const fn const_get_indexed_or_panic(&self, index: usize) -> &'a (K, V) {
        &self.values[index]
    }
}

const fn const_cmp_bytes(a: &[u8], b: &[u8]) -> Ordering {
    let (max, default) = if a.len() == b.len() {
        (a.len(), Ordering::Equal)
    } else if a.len() < b.len() {
        (a.len(), Ordering::Less)
    } else {
        (b.len(), Ordering::Greater)
    };
    let mut i = 0;
    #[allow(clippy::indexing_slicing)] // indexes in range by above checks
    while i < max {
        if a[i] == b[i] {
            i += 1;
            continue;
        } else if a[i] < b[i] {
            return Ordering::Less;
        } else {
            return Ordering::Greater;
        }
    }
    default
}

impl<'a, V> LiteMap<&'a str, V, &'a [(&'a str, V)]> {
    /// Const function to get the value associated with a `&str` key, if it exists.
    ///
    /// Also returns the index of the value.
    ///
    /// Note: This function will no longer be needed if const trait behavior is stabilized.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// static map: LiteMap<&str, usize, &[(&str, usize)]> =
    ///     LiteMap::from_sorted_store_unchecked(&[
    ///         ("abc", 11),
    ///         ("bcd", 22),
    ///         ("cde", 33),
    ///         ("def", 44),
    ///         ("efg", 55),
    ///     ]);
    ///
    /// static d: Option<(usize, &usize)> = map.const_get_with_index("def");
    /// assert_eq!(d, Some((3, &44)));
    ///
    /// static n: Option<(usize, &usize)> = map.const_get_with_index("dng");
    /// assert_eq!(n, None);
    /// ```
    pub const fn const_get_with_index(&self, key: &str) -> Option<(usize, &'a V)> {
        let mut i = 0;
        let mut j = self.const_len();
        while i < j {
            let mid = (i + j) / 2;
            #[allow(clippy::indexing_slicing)] // in range
            let x = &self.values[mid];
            match const_cmp_bytes(key.as_bytes(), x.0.as_bytes()) {
                Ordering::Equal => return Some((mid, &x.1)),
                Ordering::Greater => i = mid + 1,
                Ordering::Less => j = mid,
            };
        }
        None
    }
}

impl<'a, V> LiteMap<&'a [u8], V, &'a [(&'a [u8], V)]> {
    /// Const function to get the value associated with a `&[u8]` key, if it exists.
    ///
    /// Also returns the index of the value.
    ///
    /// Note: This function will no longer be needed if const trait behavior is stabilized.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use litemap::LiteMap;
    ///
    /// static map: LiteMap<&[u8], usize, &[(&[u8], usize)]> =
    ///     LiteMap::from_sorted_store_unchecked(&[
    ///         (b"abc", 11),
    ///         (b"bcd", 22),
    ///         (b"cde", 33),
    ///         (b"def", 44),
    ///         (b"efg", 55),
    ///     ]);
    ///
    /// static d: Option<(usize, &usize)> = map.const_get_with_index(b"def");
    /// assert_eq!(d, Some((3, &44)));
    ///
    /// static n: Option<(usize, &usize)> = map.const_get_with_index(b"dng");
    /// assert_eq!(n, None);
    /// ```
    pub const fn const_get_with_index(&self, key: &[u8]) -> Option<(usize, &'a V)> {
        let mut i = 0;
        let mut j = self.const_len();
        while i < j {
            let mid = (i + j) / 2;
            #[allow(clippy::indexing_slicing)] // in range
            let x = &self.values[mid];
            match const_cmp_bytes(key, x.0) {
                Ordering::Equal => return Some((mid, &x.1)),
                Ordering::Greater => i = mid + 1,
                Ordering::Less => j = mid,
            };
        }
        None
    }
}

macro_rules! impl_const_get_with_index_for_integer {
    ($integer:ty) => {
        impl<'a, V> LiteMap<$integer, V, &'a [($integer, V)]> {
            /// Const function to get the value associated with an integer key, if it exists.
            ///
            /// Note: This function will no longer be needed if const trait behavior is stabilized.
            ///
            /// Also returns the index of the value.
            pub const fn const_get_with_index(&self, key: $integer) -> Option<(usize, &'a V)> {
                let mut i = 0;
                let mut j = self.const_len();
                while i < j {
                    let mid = (i + j) / 2;
                    #[allow(clippy::indexing_slicing)] // in range
                    let x = &self.values[mid];
                    if key == x.0 {
                        return Some((mid, &x.1));
                    } else if key > x.0 {
                        i = mid + 1;
                    } else {
                        j = mid;
                    }
                }
                return None;
            }
        }
    };
}

impl_const_get_with_index_for_integer!(u8);
impl_const_get_with_index_for_integer!(u16);
impl_const_get_with_index_for_integer!(u32);
impl_const_get_with_index_for_integer!(u64);
impl_const_get_with_index_for_integer!(u128);
impl_const_get_with_index_for_integer!(usize);
impl_const_get_with_index_for_integer!(i8);
impl_const_get_with_index_for_integer!(i16);
impl_const_get_with_index_for_integer!(i32);
impl_const_get_with_index_for_integer!(i64);
impl_const_get_with_index_for_integer!(i128);
impl_const_get_with_index_for_integer!(isize);

/// An entry in a `LiteMap`, which may be either occupied or vacant.
#[allow(clippy::exhaustive_enums)]
pub enum Entry<'a, K, V, S> {
    Occupied(OccupiedEntry<'a, K, V, S>),
    Vacant(VacantEntry<'a, K, V, S>),
}

impl<K, V, S> Debug for Entry<'_, K, V, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Occupied(arg0) => f.debug_tuple("Occupied").field(arg0).finish(),
            Self::Vacant(arg0) => f.debug_tuple("Vacant").field(arg0).finish(),
        }
    }
}

/// A view into an occupied entry in a `LiteMap`.
pub struct OccupiedEntry<'a, K, V, S> {
    map: &'a mut LiteMap<K, V, S>,
    index: usize,
}

impl<K, V, S> Debug for OccupiedEntry<'_, K, V, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("index", &self.index)
            .finish()
    }
}

/// A view into a vacant entry in a `LiteMap`.
pub struct VacantEntry<'a, K, V, S> {
    map: &'a mut LiteMap<K, V, S>,
    key: K,
    index: usize,
}

impl<K, V, S> Debug for VacantEntry<'_, K, V, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VacantEntry")
            .field("index", &self.index)
            .finish()
    }
}

impl<'a, K, V, S> Entry<'a, K, V, S>
where
    K: Ord,
    S: StoreMut<K, V>,
{
    /// Ensures a value is in the entry by inserting the default value if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        self.or_insert(V::default())
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
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
}

impl<'a, K, V, S> OccupiedEntry<'a, K, V, S>
where
    K: Ord,
    S: StoreMut<K, V>,
{
    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &K {
        #[allow(clippy::unwrap_used)] // index is valid while we have a reference to the map
        self.map.values.lm_get(self.index).unwrap().0
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &V {
        #[allow(clippy::unwrap_used)] // index is valid while we have a reference to the map
        self.map.values.lm_get(self.index).unwrap().1
    }

    /// Gets a mutable reference to the value in the entry.
    pub fn get_mut(&mut self) -> &mut V {
        #[allow(clippy::unwrap_used)] // index is valid while we have a reference to the map
        self.map.values.lm_get_mut(self.index).unwrap().1
    }

    /// Converts the entry into a mutable reference to the value in the entry with a lifetime bound to the map.
    pub fn into_mut(self) -> &'a mut V {
        #[allow(clippy::unwrap_used)] // index is valid while we have a reference to the map
        self.map.values.lm_get_mut(self.index).unwrap().1
    }

    /// Sets the value of the entry, and returns the entry's old value.
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Takes the value out of the entry, and returns it.
    pub fn remove(self) -> V {
        self.map.values.lm_remove(self.index).1
    }
}

impl<'a, K, V, S> VacantEntry<'a, K, V, S>
where
    K: Ord,
    S: StoreMut<K, V>,
{
    /// Gets a reference to the key that would be used when inserting a value through the `VacantEntry`.
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Sets the value of the entry with the `VacantEntry`'s key, and returns a mutable reference to it.
    pub fn insert(self, value: V) -> &'a mut V {
        // index is valid insert index that was found via binary search
        // it's valid while we have a reference to the map
        self.map.values.lm_insert(self.index, self.key, value);
        #[allow(clippy::unwrap_used)] // we inserted at self.index above
        self.map.values.lm_get_mut(self.index).unwrap().1
    }
}

impl<K, V, S> LiteMap<K, V, S>
where
    K: Ord,
    S: StoreMut<K, V>,
{
    /// Gets the entry for the given key in the map for in-place manipulation.
    pub fn entry(&mut self, key: K) -> Entry<K, V, S> {
        match self.values.lm_binary_search_by(|k| k.cmp(&key)) {
            Ok(index) => Entry::Occupied(OccupiedEntry { map: self, index }),
            Err(index) => Entry::Vacant(VacantEntry {
                map: self,
                key,
                index,
            }),
        }
    }
}

impl<K, V, S> Extend<(K, V)> for LiteMap<K, V, S>
where
    K: Ord,
    S: StoreBulkMut<K, V>,
{
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        self.values.lm_extend(iter)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_iterator() {
        let mut expected = LiteMap::with_capacity(4);
        expected.insert(1, "updated-one");
        expected.insert(2, "original-two");
        expected.insert(3, "original-three");
        expected.insert(4, "updated-four");

        let actual = [
            (1, "original-one"),
            (2, "original-two"),
            (4, "original-four"),
            (4, "updated-four"),
            (1, "updated-one"),
            (3, "original-three"),
        ]
        .into_iter()
        .collect::<LiteMap<_, _>>();

        assert_eq!(expected, actual);
    }

    #[test]
    fn extend() {
        let mut expected: LiteMap<i32, &str> = LiteMap::with_capacity(4);
        expected.insert(1, "updated-one");
        expected.insert(2, "original-two");
        expected.insert(3, "original-three");
        expected.insert(4, "updated-four");

        let mut actual: LiteMap<i32, &str> = LiteMap::new();
        actual.insert(1, "original-one");
        actual.extend([
            (2, "original-two"),
            (4, "original-four"),
            (4, "updated-four"),
            (1, "updated-one"),
            (3, "original-three"),
        ]);

        assert_eq!(expected, actual);
    }

    #[test]
    fn extend2() {
        let mut map: LiteMap<usize, &str> = LiteMap::new();
        map.extend(make_13());
        map.extend(make_24());
        map.extend(make_24());
        map.extend(make_46());
        map.extend(make_13());
        map.extend(make_46());
        assert_eq!(map.len(), 5);
    }

    fn make_13() -> LiteMap<usize, &'static str> {
        let mut result = LiteMap::new();
        result.insert(1, "one");
        result.insert(3, "three");
        result
    }

    fn make_24() -> LiteMap<usize, &'static str> {
        let mut result = LiteMap::new();
        result.insert(2, "TWO");
        result.insert(4, "FOUR");
        result
    }

    fn make_46() -> LiteMap<usize, &'static str> {
        let mut result = LiteMap::new();
        result.insert(4, "four");
        result.insert(6, "six");
        result
    }

    #[test]
    fn extend_from_litemap_append() {
        let mut map = LiteMap::new();
        map.extend_from_litemap(make_13())
            .ok_or(())
            .expect_err("Append to empty map");
        map.extend_from_litemap(make_46())
            .ok_or(())
            .expect_err("Append to lesser map");
        assert_eq!(map.len(), 4);
    }

    #[test]
    fn extend_from_litemap_prepend() {
        let mut map = LiteMap::new();
        map.extend_from_litemap(make_46())
            .ok_or(())
            .expect_err("Prepend to empty map");
        map.extend_from_litemap(make_13())
            .ok_or(())
            .expect_err("Prepend to lesser map");
        assert_eq!(map.len(), 4);
    }

    #[test]
    fn extend_from_litemap_insert() {
        let mut map = LiteMap::new();
        map.extend_from_litemap(make_13())
            .ok_or(())
            .expect_err("Append to empty map");
        map.extend_from_litemap(make_24())
            .ok_or(())
            .expect_err("Insert with no conflict");
        map.extend_from_litemap(make_46())
            .ok_or(())
            .expect("Insert with conflict");
        assert_eq!(map.len(), 5);
    }

    #[test]
    fn test_const_cmp_bytes() {
        let strs = &["a", "aa", "abc", "abde", "bcd", "bcde"];
        for i in 0..strs.len() {
            for j in 0..strs.len() {
                let a = strs[i].as_bytes();
                let b = strs[j].as_bytes();
                assert_eq!(a.cmp(b), const_cmp_bytes(a, b));
            }
        }
    }

    #[test]
    fn into_iterator() {
        let mut map = LiteMap::<_, _, Vec<(_, _)>>::new();
        map.insert(4, "four");
        map.insert(6, "six");
        let mut reference = vec![(6, "six"), (4, "four")];

        for i in map {
            let r = reference.pop().unwrap();
            assert_eq!(r, i);
        }
        assert!(reference.is_empty());
    }

    #[test]
    fn entry_insert() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        assert!(matches!(map.entry(1), Entry::Vacant(_)));
        map.entry(1).or_insert("one");
        assert!(matches!(map.entry(1), Entry::Occupied(_)));
        assert_eq!(map.get(&1), Some(&"one"));
    }

    #[test]
    fn entry_insert_with() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        assert!(matches!(map.entry(1), Entry::Vacant(_)));
        map.entry(1).or_insert_with(|| "one");
        assert!(matches!(map.entry(1), Entry::Occupied(_)));
        assert_eq!(map.get(&1), Some(&"one"));
    }

    #[test]
    fn entry_vacant_insert() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        if let Entry::Vacant(entry) = map.entry(1) {
            entry.insert("one");
        }
        assert_eq!(map.get(&1), Some(&"one"));
    }

    #[test]
    fn entry_occupied_get_mut() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.insert(1, "one");
        if let Entry::Occupied(mut entry) = map.entry(1) {
            *entry.get_mut() = "uno";
        }
        assert_eq!(map.get(&1), Some(&"uno"));
    }

    #[test]
    fn entry_occupied_remove() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.insert(1, "one");
        if let Entry::Occupied(entry) = map.entry(1) {
            entry.remove();
        }
        assert_eq!(map.get(&1), None);
    }

    #[test]
    fn entry_occupied_key() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.insert(1, "one");
        if let Entry::Occupied(entry) = map.entry(1) {
            assert_eq!(entry.key(), &1);
        }
    }

    #[test]
    fn entry_occupied_get() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.insert(1, "one");
        if let Entry::Occupied(entry) = map.entry(1) {
            assert_eq!(entry.get(), &"one");
        }
    }

    #[test]
    fn entry_occupied_insert() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.insert(1, "one");
        if let Entry::Occupied(mut entry) = map.entry(1) {
            assert_eq!(entry.insert("uno"), "one");
        }
        assert_eq!(map.get(&1), Some(&"uno"));
    }

    #[test]
    fn entry_vacant_key() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        if let Entry::Vacant(entry) = map.entry(1) {
            assert_eq!(entry.key(), &1);
        }
    }

    #[test]
    fn entry_or_insert() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.entry(1).or_insert("one");
        assert_eq!(map.get(&1), Some(&"one"));
        map.entry(1).or_insert("uno");
        assert_eq!(map.get(&1), Some(&"one"));
    }

    #[test]
    fn entry_or_insert_with() {
        let mut map: LiteMap<i32, &str> = LiteMap::new();
        map.entry(1).or_insert_with(|| "one");
        assert_eq!(map.get(&1), Some(&"one"));
        map.entry(1).or_insert_with(|| "uno");
        assert_eq!(map.get(&1), Some(&"one"));
    }

    #[test]
    fn entry_or_default() {
        let mut map: LiteMap<i32, String> = LiteMap::new();
        map.entry(1).or_default();
        assert_eq!(map.get(&1), Some(&String::new()));
    }

    #[test]
    fn entry_and_modify() {
        let mut map: LiteMap<i32, i32> = LiteMap::new();
        map.entry(1).or_insert(10);
        map.entry(1).and_modify(|v| *v += 5);
        assert_eq!(map.get(&1), Some(&15));
        map.entry(2).and_modify(|v| *v += 5).or_insert(20);
        assert_eq!(map.get(&2), Some(&20));
    }
}
