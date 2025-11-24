// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ZeroSlice;

use core::cmp::Ordering;
use core::fmt;

use crate::map::ZeroMapKV;
use crate::map::ZeroVecLike;
use crate::map2d::ZeroMap2dCursor;

/// A borrowed-only version of [`ZeroMap2d`](super::ZeroMap2d)
///
/// This is useful for fully-zero-copy deserialization from non-human-readable
/// serialization formats. It also has the advantage that it can return references that live for
/// the lifetime of the backing buffer as opposed to that of the [`ZeroMap2dBorrowed`] instance.
///
/// # Examples
///
/// ```
/// use zerovec::maps::ZeroMap2dBorrowed;
///
/// // Example byte buffer representing the map { 1: {2: "three" } }
/// let BINCODE_BYTES: &[u8; 47] = &[
///     2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0,
///     0, 0, 0, 0, 0, 0, 2, 0, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 116, 104, 114,
///     101, 101,
/// ];
///
/// // Deserializing to ZeroMap2d requires no heap allocations.
/// let zero_map: ZeroMap2dBorrowed<u16, u16, str> =
///     bincode::deserialize(BINCODE_BYTES)
///         .expect("Should deserialize successfully");
/// assert_eq!(zero_map.get_2d(&1, &2), Some("three"));
/// ```
///
/// This can be obtained from a [`ZeroMap2d`](super::ZeroMap2d) via [`ZeroMap2d::as_borrowed`](super::ZeroMap2d::as_borrowed)
pub struct ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    pub(crate) keys0: &'a K0::Slice,
    pub(crate) joiner: &'a ZeroSlice<u32>,
    pub(crate) keys1: &'a K1::Slice,
    pub(crate) values: &'a V::Slice,
}

impl<'a, K0, K1, V> Copy for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
}

impl<'a, K0, K1, V> Clone for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, K0, K1, V> Default for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0::Slice: 'static,
    K1::Slice: 'static,
    V::Slice: 'static,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, K0, K1, V> ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0::Slice: 'static,
    K1::Slice: 'static,
    V::Slice: 'static,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// Creates a new, empty `ZeroMap2dBorrowed<K0, K1, V>`.
    ///
    /// Note: Since [`ZeroMap2dBorrowed`] is not mutable, the return value will be a stub unless
    /// converted into a [`ZeroMap2d`](super::ZeroMap2d).
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::maps::ZeroMap2dBorrowed;
    ///
    /// let zm: ZeroMap2dBorrowed<u16, u16, str> = ZeroMap2dBorrowed::new();
    /// assert!(zm.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            keys0: K0::Container::zvl_new_borrowed(),
            joiner: Default::default(),
            keys1: K1::Container::zvl_new_borrowed(),
            values: V::Container::zvl_new_borrowed(),
        }
    }
}

impl<'a, K0, K1, V> ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    #[doc(hidden)] // databake internal
    pub const unsafe fn from_parts_unchecked(
        keys0: &'a K0::Slice,
        joiner: &'a ZeroSlice<u32>,
        keys1: &'a K1::Slice,
        values: &'a V::Slice,
    ) -> Self {
        Self {
            keys0,
            joiner,
            keys1,
            values,
        }
    }

    /// The number of elements in the [`ZeroMap2dBorrowed`]
    pub fn len(&self) -> usize {
        self.values.zvl_len()
    }

    /// Whether the [`ZeroMap2dBorrowed`] is empty
    pub fn is_empty(&self) -> bool {
        self.values.zvl_len() == 0
    }
}

impl<'a, K0, K1, V> ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Ord,
    K1: ZeroMapKV<'a> + Ord,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// Get the value associated with `key0` and `key1`, if it exists.
    ///
    /// This is able to return values that live longer than the map itself
    /// since they borrow directly from the backing buffer. This is the
    /// primary advantage of using [`ZeroMap2dBorrowed`](super::ZeroMap2dBorrowed) over [`ZeroMap2d`](super::ZeroMap2d).
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert(&1, "one", "foo");
    /// map.insert(&2, "one", "bar");
    /// map.insert(&2, "two", "baz");
    ///
    /// let borrowed = map.as_borrowed();
    /// assert_eq!(borrowed.get_2d(&1, "one"), Some("foo"));
    /// assert_eq!(borrowed.get_2d(&1, "two"), None);
    /// assert_eq!(borrowed.get_2d(&2, "one"), Some("bar"));
    /// assert_eq!(borrowed.get_2d(&2, "two"), Some("baz"));
    /// assert_eq!(borrowed.get_2d(&3, "three"), None);
    /// ```
    pub fn get_2d(&self, key0: &K0, key1: &K1) -> Option<&'a V::GetType> {
        self.get0(key0)?.get1(key1)
    }
}

impl<'a, K0, K1, V> ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Ord,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// Gets a cursor for `key0`. If `None`, then `key0` is not in the map. If `Some`,
    /// then `key0` is in the map, and `key1` can be queried.
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert(&1, "one", "foo");
    /// map.insert(&2, "two", "bar");
    /// let borrowed = map.as_borrowed();
    /// assert!(borrowed.get0(&1).is_some());
    /// assert!(borrowed.get0(&3).is_none());
    /// ```
    #[inline]
    pub fn get0<'l>(&'l self, key0: &K0) -> Option<ZeroMap2dCursor<'a, 'a, K0, K1, V>> {
        let key0_index = self.keys0.zvl_binary_search(key0).ok()?;
        Some(ZeroMap2dCursor::from_borrowed(self, key0_index))
    }

    /// Binary search the map for `key0`, returning a cursor.
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert(&1, "one", "foo");
    /// map.insert(&2, "two", "bar");
    /// let borrowed = map.as_borrowed();
    /// assert!(borrowed.get0_by(|probe| probe.cmp(&1)).is_some());
    /// assert!(borrowed.get0_by(|probe| probe.cmp(&3)).is_none());
    /// ```
    pub fn get0_by<'l>(
        &'l self,
        predicate: impl FnMut(&K0) -> Ordering,
    ) -> Option<ZeroMap2dCursor<'a, 'a, K0, K1, V>> {
        let key0_index = self.keys0.zvl_binary_search_by(predicate).ok()?;
        Some(ZeroMap2dCursor::from_borrowed(self, key0_index))
    }

    /// Returns whether `key0` is contained in this map
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert(&1, "one", "foo");
    /// map.insert(&2, "two", "bar");
    /// let borrowed = map.as_borrowed();
    /// assert!(borrowed.contains_key0(&1));
    /// assert!(!borrowed.contains_key0(&3));
    /// ```
    pub fn contains_key0(&self, key0: &K0) -> bool {
        self.keys0.zvl_binary_search(key0).is_ok()
    }
}

impl<'a, K0, K1, V> ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// Produce an ordered iterator over keys0
    pub fn iter0<'l>(&'l self) -> impl Iterator<Item = ZeroMap2dCursor<'a, 'a, K0, K1, V>> + 'l {
        (0..self.keys0.zvl_len()).map(move |idx| ZeroMap2dCursor::from_borrowed(self, idx))
    }
}

impl<'a, K0, K1, V> ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Ord,
    K1: ZeroMapKV<'a> + Ord,
    V: ZeroMapKV<'a>,
    V: Copy,
    K0: ?Sized,
    K1: ?Sized,
{
    /// For cases when `V` is fixed-size, obtain a direct copy of `V` instead of `V::ULE`
    pub fn get_copied_2d(&self, key0: &K0, key1: &K1) -> Option<V> {
        self.get0(key0)?.get1_copied(key1)
    }
}

// We can't use the default PartialEq because ZeroMap2d is invariant
// so otherwise rustc will not automatically allow you to compare ZeroMaps
// with different lifetimes
impl<'a, 'b, K0, K1, V> PartialEq<ZeroMap2dBorrowed<'b, K0, K1, V>>
    for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: for<'c> ZeroMapKV<'c> + ?Sized,
    K1: for<'c> ZeroMapKV<'c> + ?Sized,
    V: for<'c> ZeroMapKV<'c> + ?Sized,
    <K0 as ZeroMapKV<'a>>::Slice: PartialEq<<K0 as ZeroMapKV<'b>>::Slice>,
    <K1 as ZeroMapKV<'a>>::Slice: PartialEq<<K1 as ZeroMapKV<'b>>::Slice>,
    <V as ZeroMapKV<'a>>::Slice: PartialEq<<V as ZeroMapKV<'b>>::Slice>,
{
    fn eq(&self, other: &ZeroMap2dBorrowed<'b, K0, K1, V>) -> bool {
        self.keys0.eq(other.keys0)
            && self.joiner.eq(other.joiner)
            && self.keys1.eq(other.keys1)
            && self.values.eq(other.values)
    }
}

impl<'a, K0, K1, V> fmt::Debug for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + ?Sized,
    K1: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K0::Slice: fmt::Debug,
    K1::Slice: fmt::Debug,
    V::Slice: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("ZeroMap2dBorrowed")
            .field("keys0", &self.keys0)
            .field("joiner", &self.joiner)
            .field("keys1", &self.keys1)
            .field("values", &self.values)
            .finish()
    }
}
