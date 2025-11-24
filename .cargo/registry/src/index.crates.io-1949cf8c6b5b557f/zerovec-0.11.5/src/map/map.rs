// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use crate::ule::{AsULE, EncodeAsVarULE, UleError, VarULE};
use crate::{VarZeroVec, ZeroSlice, ZeroVec};
use alloc::borrow::Borrow;
use alloc::boxed::Box;
use core::cmp::Ordering;
use core::fmt;
use core::iter::FromIterator;

/// A zero-copy map datastructure, built on sorted binary-searchable [`ZeroVec`]
/// and [`VarZeroVec`].
///
/// This type, like [`ZeroVec`] and [`VarZeroVec`], is able to zero-copy
/// deserialize from appropriately formatted byte buffers. It is internally copy-on-write, so it can be mutated
/// afterwards as necessary.
///
/// Internally, a `ZeroMap` is a zero-copy vector for keys paired with a zero-copy vector for
/// values, sorted by the keys. Therefore, all types used in `ZeroMap` need to work with either
/// [`ZeroVec`] or [`VarZeroVec`].
///
/// This does mean that for fixed-size data, one must use the regular type (`u32`, `u8`, `char`, etc),
/// whereas for variable-size data, `ZeroMap` will use the dynamically sized version (`str` not `String`,
/// `ZeroSlice` not `ZeroVec`, `FooULE` not `Foo` for custom types)
///
/// # Examples
///
/// ```
/// use zerovec::ZeroMap;
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Data<'a> {
///     #[serde(borrow)]
///     map: ZeroMap<'a, u32, str>,
/// }
///
/// let mut map = ZeroMap::new();
/// map.insert(&1, "one");
/// map.insert(&2, "two");
/// map.insert(&4, "four");
///
/// let data = Data { map };
///
/// let bincode_bytes =
///     bincode::serialize(&data).expect("Serialization should be successful");
///
/// // Will deserialize without any allocations
/// let deserialized: Data = bincode::deserialize(&bincode_bytes)
///     .expect("Deserialization should be successful");
///
/// assert_eq!(data.map.get(&1), Some("one"));
/// assert_eq!(data.map.get(&2), Some("two"));
/// ```
///
/// [`VarZeroVec`]: crate::VarZeroVec
// ZeroMap has only one invariant: keys.len() == values.len()
// It is also expected that the keys are sorted, but this is not an invariant. See #1433
pub struct ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    pub(crate) keys: K::Container,
    pub(crate) values: V::Container,
}

impl<'a, K, V> Default for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Creates a new, empty `ZeroMap<K, V>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroMap;
    ///
    /// let zm: ZeroMap<u16, str> = ZeroMap::new();
    /// assert!(zm.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            keys: K::Container::zvl_with_capacity(0),
            values: V::Container::zvl_with_capacity(0),
        }
    }

    #[doc(hidden)] // databake internal
    pub const unsafe fn from_parts_unchecked(keys: K::Container, values: V::Container) -> Self {
        Self { keys, values }
    }

    /// Construct a new [`ZeroMap`] with a given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: K::Container::zvl_with_capacity(capacity),
            values: V::Container::zvl_with_capacity(capacity),
        }
    }

    /// Obtain a borrowed version of this map
    pub fn as_borrowed(&'a self) -> ZeroMapBorrowed<'a, K, V> {
        ZeroMapBorrowed {
            keys: self.keys.zvl_as_borrowed(),
            values: self.values.zvl_as_borrowed(),
        }
    }

    /// The number of elements in the [`ZeroMap`]
    pub fn len(&self) -> usize {
        self.values.zvl_len()
    }

    /// Whether the [`ZeroMap`] is empty
    pub fn is_empty(&self) -> bool {
        self.values.zvl_len() == 0
    }

    /// Remove all elements from the [`ZeroMap`]
    pub fn clear(&mut self) {
        self.keys.zvl_clear();
        self.values.zvl_clear();
    }

    /// Reserve capacity for `additional` more elements to be inserted into
    /// the [`ZeroMap`] to avoid frequent reallocations.
    ///
    /// See [`Vec::reserve()`](alloc::vec::Vec::reserve) for more information.
    pub fn reserve(&mut self, additional: usize) {
        self.keys.zvl_reserve(additional);
        self.values.zvl_reserve(additional);
    }
}
impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Get the value associated with `key`, if it exists.
    ///
    /// For fixed-size ([`AsULE`]) `V` types, this _will_ return
    /// their corresponding [`AsULE::ULE`] type. If you wish to work with the `V`
    /// type directly, [`Self::get_copied()`] exists for convenience.
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// assert_eq!(map.get(&1), Some("one"));
    /// assert_eq!(map.get(&3), None);
    /// ```
    pub fn get(&self, key: &K) -> Option<&V::GetType> {
        let index = self.keys.zvl_binary_search(key).ok()?;
        self.values.zvl_get(index)
    }

    /// Binary search the map with `predicate` to find a key, returning the value.
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// assert_eq!(map.get_by(|probe| probe.cmp(&1)), Some("one"));
    /// assert_eq!(map.get_by(|probe| probe.cmp(&3)), None);
    /// ```
    pub fn get_by(&self, predicate: impl FnMut(&K) -> Ordering) -> Option<&V::GetType> {
        let index = self.keys.zvl_binary_search_by(predicate).ok()?;
        self.values.zvl_get(index)
    }

    /// Returns whether `key` is contained in this map
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// assert!(map.contains_key(&1));
    /// assert!(!map.contains_key(&3));
    /// ```
    pub fn contains_key(&self, key: &K) -> bool {
        self.keys.zvl_binary_search(key).is_ok()
    }

    /// Insert `value` with `key`, returning the existing value if it exists.
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// assert_eq!(map.get(&1), Some("one"));
    /// assert_eq!(map.get(&3), None);
    /// ```
    pub fn insert(&mut self, key: &K, value: &V) -> Option<V::OwnedType> {
        match self.keys.zvl_binary_search(key) {
            Ok(index) => Some(self.values.zvl_replace(index, value)),
            Err(index) => {
                self.keys.zvl_insert(index, key);
                self.values.zvl_insert(index, value);
                None
            }
        }
    }

    /// Remove the value at `key`, returning it if it exists.
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// assert_eq!(map.remove(&1), Some("one".to_owned().into_boxed_str()));
    /// assert_eq!(map.get(&1), None);
    /// ```
    pub fn remove(&mut self, key: &K) -> Option<V::OwnedType> {
        let idx = self.keys.zvl_binary_search(key).ok()?;
        self.keys.zvl_remove(idx);
        Some(self.values.zvl_remove(idx))
    }

    /// Appends `value` with `key` to the end of the underlying vector, returning
    /// `key` and `value` _if it failed_. Useful for extending with an existing
    /// sorted list.
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// assert!(map.try_append(&1, "uno").is_none());
    /// assert!(map.try_append(&3, "tres").is_none());
    ///
    /// let unsuccessful = map.try_append(&3, "tres-updated");
    /// assert!(unsuccessful.is_some(), "append duplicate of last key");
    ///
    /// let unsuccessful = map.try_append(&2, "dos");
    /// assert!(unsuccessful.is_some(), "append out of order");
    ///
    /// assert_eq!(map.get(&1), Some("uno"));
    ///
    /// // contains the original value for the key: 3
    /// assert_eq!(map.get(&3), Some("tres"));
    ///
    /// // not appended since it wasn't in order
    /// assert_eq!(map.get(&2), None);
    /// ```
    #[must_use]
    pub fn try_append<'b>(&mut self, key: &'b K, value: &'b V) -> Option<(&'b K, &'b V)> {
        if self.keys.zvl_len() != 0 {
            if let Some(last) = self.keys.zvl_get(self.keys.zvl_len() - 1) {
                if K::Container::t_cmp_get(key, last) != Ordering::Greater {
                    return Some((key, value));
                }
            }
        }

        self.keys.zvl_push(key);
        self.values.zvl_push(value);
        None
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Produce an ordered iterator over key-value pairs
    pub fn iter<'b>(
        &'b self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'b <K as ZeroMapKV<'a>>::GetType,
            &'b <V as ZeroMapKV<'a>>::GetType,
        ),
    > {
        (0..self.keys.zvl_len()).map(move |idx| {
            (
                #[expect(clippy::unwrap_used)] // idx is in-range
                self.keys.zvl_get(idx).unwrap(),
                #[expect(clippy::unwrap_used)] // idx is in-range
                self.values.zvl_get(idx).unwrap(),
            )
        })
    }

    /// Produce an ordered iterator over keys
    pub fn iter_keys<'b>(
        &'b self,
    ) -> impl ExactSizeIterator<Item = &'b <K as ZeroMapKV<'a>>::GetType> {
        #[expect(clippy::unwrap_used)] // idx is in-range
        (0..self.keys.zvl_len()).map(move |idx| self.keys.zvl_get(idx).unwrap())
    }

    /// Produce an iterator over values, ordered by keys
    pub fn iter_values<'b>(
        &'b self,
    ) -> impl ExactSizeIterator<Item = &'b <V as ZeroMapKV<'a>>::GetType> {
        #[expect(clippy::unwrap_used)] // idx is in-range
        (0..self.values.zvl_len()).map(move |idx| self.values.zvl_get(idx).unwrap())
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, K>>,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Cast a `ZeroMap<K, V>` to `ZeroMap<P, V>` where `K` and `P` are [`AsULE`] types
    /// with the same representation.
    ///
    /// # Unchecked Invariants
    ///
    /// If `K` and `P` have different ordering semantics, unexpected behavior may occur.
    pub fn cast_zv_k_unchecked<P>(self) -> ZeroMap<'a, P, V>
    where
        P: AsULE<ULE = K::ULE> + ZeroMapKV<'a, Container = ZeroVec<'a, P>>,
    {
        ZeroMap {
            keys: self.keys.cast(),
            values: self.values,
        }
    }

    /// Convert a `ZeroMap<K, V>` to `ZeroMap<P, V>` where `K` and `P` are [`AsULE`] types
    /// with the same size.
    ///
    /// # Unchecked Invariants
    ///
    /// If `K` and `P` have different ordering semantics, unexpected behavior may occur.
    ///
    /// # Panics
    ///
    /// Panics if `K::ULE` and `P::ULE` are not the same size.
    pub fn try_convert_zv_k_unchecked<P>(self) -> Result<ZeroMap<'a, P, V>, UleError>
    where
        P: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, P>>,
    {
        Ok(ZeroMap {
            keys: self.keys.try_into_converted()?,
            values: self.values,
        })
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, V>>,
{
    /// Cast a `ZeroMap<K, V>` to `ZeroMap<K, P>` where `V` and `P` are [`AsULE`] types
    /// with the same representation.
    ///
    /// # Unchecked Invariants
    ///
    /// If `V` and `P` have different ordering semantics, unexpected behavior may occur.
    pub fn cast_zv_v_unchecked<P>(self) -> ZeroMap<'a, K, P>
    where
        P: AsULE<ULE = V::ULE> + ZeroMapKV<'a, Container = ZeroVec<'a, P>>,
    {
        ZeroMap {
            keys: self.keys,
            values: self.values.cast(),
        }
    }

    /// Convert a `ZeroMap<K, V>` to `ZeroMap<K, P>` where `V` and `P` are [`AsULE`] types
    /// with the same size.
    ///
    /// # Unchecked Invariants
    ///
    /// If `V` and `P` have different ordering semantics, unexpected behavior may occur.
    ///
    /// # Panics
    ///
    /// Panics if `V::ULE` and `P::ULE` are not the same size.
    pub fn try_convert_zv_v_unchecked<P>(self) -> Result<ZeroMap<'a, K, P>, UleError>
    where
        P: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, P>>,
    {
        Ok(ZeroMap {
            keys: self.keys,
            values: self.values.try_into_converted()?,
        })
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a, Container = VarZeroVec<'a, V>> + ?Sized,
    V: VarULE,
{
    /// Same as `insert()`, but allows using [EncodeAsVarULE](crate::ule::EncodeAsVarULE)
    /// types with the value to avoid an extra allocation when dealing with custom ULE types.
    ///
    /// ```rust
    /// use std::borrow::Cow;
    /// use zerovec::ZeroMap;
    ///
    /// #[zerovec::make_varule(PersonULE)]
    /// #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    /// struct Person<'a> {
    ///     age: u8,
    ///     name: Cow<'a, str>,
    /// }
    ///
    /// let mut map: ZeroMap<u32, PersonULE> = ZeroMap::new();
    /// map.insert_var_v(
    ///     &1,
    ///     &Person {
    ///         age: 20,
    ///         name: "Joseph".into(),
    ///     },
    /// );
    /// map.insert_var_v(
    ///     &1,
    ///     &Person {
    ///         age: 35,
    ///         name: "Carla".into(),
    ///     },
    /// );
    /// assert_eq!(&map.get(&1).unwrap().name, "Carla");
    /// assert!(map.get(&3).is_none());
    /// ```
    pub fn insert_var_v<VE: EncodeAsVarULE<V>>(&mut self, key: &K, value: &VE) -> Option<Box<V>> {
        match self.keys.zvl_binary_search(key) {
            Ok(index) => {
                #[expect(clippy::unwrap_used)] // binary search
                let ret = self.values.get(index).unwrap().to_boxed();
                self.values.make_mut().replace(index, value);
                Some(ret)
            }
            Err(index) => {
                self.keys.zvl_insert(index, key);
                self.values.make_mut().insert(index, value);
                None
            }
        }
    }

    // insert_var_k, insert_var_kv are not possible since one cannot perform the binary search with EncodeAsVarULE
    // though we might be able to do it in the future if we add a trait for cross-Ord requirements
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Ord,
    V: Copy + ZeroMapKV<'a>,
{
    /// For cases when `V` is fixed-size, obtain a direct copy of `V` instead of `V::ULE`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, &'a');
    /// map.insert(&2, &'b');
    /// assert_eq!(map.get_copied(&1), Some('a'));
    /// assert_eq!(map.get_copied(&3), None);
    #[inline]
    pub fn get_copied(&self, key: &K) -> Option<V> {
        let index = self.keys.zvl_binary_search(key).ok()?;
        self.get_copied_at(index)
    }

    /// Binary search the map with `predicate` to find a key, returning the value.
    ///
    /// For cases when `V` is fixed-size, use this method to obtain a direct copy of `V`
    /// instead of `V::ULE`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, &'a');
    /// map.insert(&2, &'b');
    /// assert_eq!(map.get_copied_by(|probe| probe.cmp(&1)), Some('a'));
    /// assert_eq!(map.get_copied_by(|probe| probe.cmp(&3)), None);
    /// ```
    #[inline]
    pub fn get_copied_by(&self, predicate: impl FnMut(&K) -> Ordering) -> Option<V> {
        let index = self.keys.zvl_binary_search_by(predicate).ok()?;
        self.get_copied_at(index)
    }

    fn get_copied_at(&self, index: usize) -> Option<V> {
        let ule = self.values.zvl_get(index)?;
        let mut result = Option::<V>::None;
        V::Container::zvl_get_as_t(ule, |v| result.replace(*v));
        #[expect(clippy::unwrap_used)] // `zvl_get_as_t` guarantees that the callback is invoked
        Some(result.unwrap())
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, V>>,
{
    /// Similar to [`Self::iter()`] except it returns a direct copy of the values instead of references
    /// to `V::ULE`, in cases when `V` is fixed-size
    pub fn iter_copied_values<'b>(
        &'b self,
    ) -> impl Iterator<Item = (&'b <K as ZeroMapKV<'a>>::GetType, V)> {
        (0..self.keys.zvl_len()).map(move |idx| {
            (
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len()
                self.keys.zvl_get(idx).unwrap(),
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len() = values.zvl_len()
                ZeroSlice::get(&*self.values, idx).unwrap(),
            )
        })
    }
}

impl<'a, K, V> ZeroMap<'a, K, V>
where
    K: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, K>>,
    V: AsULE + ZeroMapKV<'a, Container = ZeroVec<'a, V>>,
{
    /// Similar to [`Self::iter()`] except it returns a direct copy of the keys values instead of references
    /// to `K::ULE` and `V::ULE`, in cases when `K` and `V` are fixed-size
    pub fn iter_copied<'b>(&'b self) -> impl Iterator<Item = (K, V)> + 'b {
        let keys = &self.keys;
        let values = &self.values;
        (0..keys.len()).map(move |idx| {
            (
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len()
                ZeroSlice::get(&**keys, idx).unwrap(),
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len() = values.zvl_len()
                ZeroSlice::get(&**values, idx).unwrap(),
            )
        })
    }
}

impl<'a, K, V> From<ZeroMapBorrowed<'a, K, V>> for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K: ?Sized,
    V: ?Sized,
{
    fn from(other: ZeroMapBorrowed<'a, K, V>) -> Self {
        Self {
            keys: K::Container::zvl_from_borrowed(other.keys),
            values: V::Container::zvl_from_borrowed(other.values),
        }
    }
}

// We can't use the default PartialEq because ZeroMap is invariant
// so otherwise rustc will not automatically allow you to compare ZeroMaps
// with different lifetimes
impl<'a, 'b, K, V> PartialEq<ZeroMap<'b, K, V>> for ZeroMap<'a, K, V>
where
    K: for<'c> ZeroMapKV<'c> + ?Sized,
    V: for<'c> ZeroMapKV<'c> + ?Sized,
    <K as ZeroMapKV<'a>>::Container: PartialEq<<K as ZeroMapKV<'b>>::Container>,
    <V as ZeroMapKV<'a>>::Container: PartialEq<<V as ZeroMapKV<'b>>::Container>,
{
    fn eq(&self, other: &ZeroMap<'b, K, V>) -> bool {
        self.keys.eq(&other.keys) && self.values.eq(&other.values)
    }
}

impl<'a, K, V> fmt::Debug for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    <K as ZeroMapKV<'a>>::Container: fmt::Debug,
    <V as ZeroMapKV<'a>>::Container: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("ZeroMap")
            .field("keys", &self.keys)
            .field("values", &self.values)
            .finish()
    }
}

impl<'a, K, V> Clone for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    <K as ZeroMapKV<'a>>::Container: Clone,
    <V as ZeroMapKV<'a>>::Container: Clone,
{
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            values: self.values.clone(),
        }
    }
}

impl<'a, A, B, K, V> FromIterator<(A, B)> for ZeroMap<'a, K, V>
where
    A: Borrow<K>,
    B: Borrow<V>,
    K: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (A, B)>,
    {
        let iter = iter.into_iter();
        let mut map = match iter.size_hint() {
            (_, Some(upper)) => Self::with_capacity(upper),
            (lower, None) => Self::with_capacity(lower),
        };

        for (key, value) in iter {
            if let Some((key, value)) = map.try_append(key.borrow(), value.borrow()) {
                map.insert(key, value);
            }
        }
        map
    }
}
