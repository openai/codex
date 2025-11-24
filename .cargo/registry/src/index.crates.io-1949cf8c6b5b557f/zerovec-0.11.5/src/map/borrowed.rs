// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ule::AsULE;
use crate::ZeroSlice;

use core::cmp::Ordering;
use core::fmt;

use super::kv::ZeroMapKV;
use super::vecs::ZeroVecLike;

/// A borrowed-only version of [`ZeroMap`](super::ZeroMap)
///
/// This is useful for fully-zero-copy deserialization from non-human-readable
/// serialization formats. It also has the advantage that it can return references that live for
/// the lifetime of the backing buffer as opposed to that of the [`ZeroMapBorrowed`] instance.
///
/// # Examples
///
/// ```
/// use zerovec::maps::ZeroMapBorrowed;
///
/// // Example byte buffer representing the map { 1: "one" }
/// let BINCODE_BYTES: &[u8; 25] = &[
///     4, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 0, 111,
///     110, 101,
/// ];
///
/// // Deserializing to ZeroMap requires no heap allocations.
/// let zero_map: ZeroMapBorrowed<u32, str> =
///     bincode::deserialize(BINCODE_BYTES)
///         .expect("Should deserialize successfully");
/// assert_eq!(zero_map.get(&1), Some("one"));
/// ```
///
/// This can be obtained from a [`ZeroMap`](super::ZeroMap) via [`ZeroMap::as_borrowed`](super::ZeroMap::as_borrowed)
pub struct ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K: ?Sized,
    V: ?Sized,
{
    pub(crate) keys: &'a <K as ZeroMapKV<'a>>::Slice,
    pub(crate) values: &'a <V as ZeroMapKV<'a>>::Slice,
}

impl<'a, K, V> Copy for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K: ?Sized,
    V: ?Sized,
{
}
impl<'a, K, V> Clone for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K: ?Sized,
    V: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, K, V> Default for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K::Slice: 'static,
    V::Slice: 'static,
    K: ?Sized,
    V: ?Sized,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, K, V> ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K::Slice: 'static,
    V::Slice: 'static,
    K: ?Sized,
    V: ?Sized,
{
    /// Creates a new, empty `ZeroMapBorrowed<K, V>`.
    ///
    /// Note: Since [`ZeroMapBorrowed`] is not mutable, the return value will be a stub unless
    /// converted into a [`ZeroMap`](super::ZeroMap).
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::maps::ZeroMapBorrowed;
    ///
    /// let zm: ZeroMapBorrowed<u16, str> = ZeroMapBorrowed::new();
    /// assert!(zm.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            keys: K::Container::zvl_new_borrowed(),
            values: V::Container::zvl_new_borrowed(),
        }
    }
}

impl<'a, K, V> ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K: ?Sized,
    V: ?Sized,
{
    #[doc(hidden)] // databake internal
    pub const unsafe fn from_parts_unchecked(
        keys: &'a <K as ZeroMapKV<'a>>::Slice,
        values: &'a <V as ZeroMapKV<'a>>::Slice,
    ) -> Self {
        Self { keys, values }
    }

    /// The number of elements in the [`ZeroMapBorrowed`]
    pub fn len(self) -> usize {
        self.values.zvl_len()
    }

    /// Whether the [`ZeroMapBorrowed`] is empty
    pub fn is_empty(self) -> bool {
        self.values.zvl_len() == 0
    }
}

impl<'a, K, V> ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + Ord,
    V: ZeroMapKV<'a>,
    K: ?Sized,
    V: ?Sized,
{
    /// Get the value associated with `key`, if it exists.
    ///
    /// This is able to return values that live longer than the map itself
    /// since they borrow directly from the backing buffer. This is the
    /// primary advantage of using [`ZeroMapBorrowed`](super::ZeroMapBorrowed) over [`ZeroMap`](super::ZeroMap).
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// let borrowed = map.as_borrowed();
    /// assert_eq!(borrowed.get(&1), Some("one"));
    /// assert_eq!(borrowed.get(&3), None);
    /// ```
    pub fn get(self, key: &K) -> Option<&'a V::GetType> {
        let index = self.keys.zvl_binary_search(key).ok()?;
        self.values.zvl_get(index)
    }

    /// Binary search the map with `predicate` to find a key, returning the value.
    ///
    /// This is able to return values that live longer than the map itself
    /// since they borrow directly from the backing buffer. This is the
    /// primary advantage of using [`ZeroMapBorrowed`](super::ZeroMapBorrowed) over [`ZeroMap`](super::ZeroMap).
    ///
    /// ```rust
    /// use zerovec::ZeroMap;
    ///
    /// let mut map = ZeroMap::new();
    /// map.insert(&1, "one");
    /// map.insert(&2, "two");
    /// let borrowed = map.as_borrowed();
    /// assert_eq!(borrowed.get_by(|probe| probe.cmp(&1)), Some("one"));
    /// assert_eq!(borrowed.get_by(|probe| probe.cmp(&3)), None);
    /// ```
    pub fn get_by(self, predicate: impl FnMut(&K) -> Ordering) -> Option<&'a V::GetType> {
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
    /// let borrowed = map.as_borrowed();
    /// assert!(borrowed.contains_key(&1));
    /// assert!(!borrowed.contains_key(&3));
    /// ```
    pub fn contains_key(self, key: &K) -> bool {
        self.keys.zvl_binary_search(key).is_ok()
    }
}

impl<'a, K, V> ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
{
    /// Produce an ordered iterator over key-value pairs
    pub fn iter(
        self,
    ) -> impl Iterator<
        Item = (
            &'a <K as ZeroMapKV<'a>>::GetType,
            &'a <V as ZeroMapKV<'a>>::GetType,
        ),
    > {
        self.iter_keys().zip(self.iter_values())
    }

    /// Produce an ordered iterator over keys
    pub fn iter_keys(self) -> impl Iterator<Item = &'a <K as ZeroMapKV<'a>>::GetType> {
        #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len()
        (0..self.keys.zvl_len()).map(move |idx| self.keys.zvl_get(idx).unwrap())
    }

    /// Produce an iterator over values, ordered by keys
    pub fn iter_values(self) -> impl Iterator<Item = &'a <V as ZeroMapKV<'a>>::GetType> {
        #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len() == values.zvl_len()
        (0..self.values.zvl_len()).map(move |idx| self.values.zvl_get(idx).unwrap())
    }
}

impl<'a, K, V> ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + Ord + ?Sized,
    V: ZeroMapKV<'a, Slice = ZeroSlice<V>> + AsULE + Copy + 'static,
{
    /// For cases when `V` is fixed-size, obtain a direct copy of `V` instead of `V::ULE`
    pub fn get_copied(self, key: &K) -> Option<V> {
        let index = self.keys.zvl_binary_search(key).ok()?;
        self.values.get(index)
    }

    /// For cases when `V` is fixed-size, obtain a direct copy of `V` instead of `V::ULE`
    pub fn get_copied_by(self, predicate: impl FnMut(&K) -> Ordering) -> Option<V> {
        let index = self.keys.zvl_binary_search_by(predicate).ok()?;
        self.values.get(index)
    }

    /// Similar to [`Self::iter()`] except it returns a direct copy of the values instead of references
    /// to `V::ULE`, in cases when `V` is fixed-size
    pub fn iter_copied_values(
        self,
    ) -> impl Iterator<Item = (&'a <K as ZeroMapKV<'a>>::GetType, V)> {
        (0..self.keys.zvl_len()).map(move |idx| {
            (
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len()
                self.keys.zvl_get(idx).unwrap(),
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len() = values.zvl_len()
                self.values.get(idx).unwrap(),
            )
        })
    }
}

impl<'a, K, V> ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a, Slice = ZeroSlice<K>> + AsULE + Copy + Ord + 'static,
    V: ZeroMapKV<'a, Slice = ZeroSlice<V>> + AsULE + Copy + 'static,
{
    /// Similar to [`Self::iter()`] except it returns a direct copy of the keys values instead of references
    /// to `K::ULE` and `V::ULE`, in cases when `K` and `V` are fixed-size
    pub fn iter_copied(self) -> impl Iterator<Item = (K, V)> + 'a {
        let len = self.keys.zvl_len();
        (0..len).map(move |idx| {
            (
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len()
                ZeroSlice::get(self.keys, idx).unwrap(),
                #[expect(clippy::unwrap_used)] // idx in 0..keys.zvl_len() = values.zvl_len()
                ZeroSlice::get(self.values, idx).unwrap(),
            )
        })
    }
}

// We can't use the default PartialEq because ZeroMap is invariant
// so otherwise rustc will not automatically allow you to compare ZeroMaps
// with different lifetimes
impl<'a, 'b, K, V> PartialEq<ZeroMapBorrowed<'b, K, V>> for ZeroMapBorrowed<'a, K, V>
where
    K: for<'c> ZeroMapKV<'c> + ?Sized,
    V: for<'c> ZeroMapKV<'c> + ?Sized,
    <K as ZeroMapKV<'a>>::Slice: PartialEq<<K as ZeroMapKV<'b>>::Slice>,
    <V as ZeroMapKV<'a>>::Slice: PartialEq<<V as ZeroMapKV<'b>>::Slice>,
{
    fn eq(&self, other: &ZeroMapBorrowed<'b, K, V>) -> bool {
        self.keys.eq(other.keys) && self.values.eq(other.values)
    }
}

impl<'a, K, V> fmt::Debug for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::Slice: fmt::Debug,
    V::Slice: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("ZeroMapBorrowed")
            .field("keys", &self.keys)
            .field("values", &self.values)
            .finish()
    }
}
