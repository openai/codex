// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::{ZeroMap2d, ZeroSlice};

use core::cmp::Ordering;
use core::fmt;
use core::ops::Range;

use crate::map::ZeroMapKV;
use crate::map::ZeroVecLike;

use super::ZeroMap2dBorrowed;

/// An intermediate state of queries over [`ZeroMap2d`] and [`ZeroMap2dBorrowed`].
pub struct ZeroMap2dCursor<'l, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    // Invariant: these fields have the same invariants as they do in ZeroMap2d
    keys0: &'l K0::Slice,
    joiner: &'l ZeroSlice<u32>,
    keys1: &'l K1::Slice,
    values: &'l V::Slice,
    // Invariant: key0_index is in range
    key0_index: usize,
}

impl<'a, K0, K1, V> ZeroMap2dCursor<'a, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// `key0_index` must be in range
    pub(crate) fn from_borrowed(
        borrowed: &ZeroMap2dBorrowed<'a, K0, K1, V>,
        key0_index: usize,
    ) -> Self {
        debug_assert!(key0_index < borrowed.joiner.len());
        ZeroMap2dCursor {
            keys0: borrowed.keys0,
            joiner: borrowed.joiner,
            keys1: borrowed.keys1,
            values: borrowed.values,
            key0_index,
        }
    }
}

impl<'l, 'a, K0, K1, V> ZeroMap2dCursor<'l, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// `key0_index` must be in range
    pub(crate) fn from_cow(cow: &'l ZeroMap2d<'a, K0, K1, V>, key0_index: usize) -> Self {
        debug_assert!(key0_index < cow.joiner.len());
        Self {
            keys0: cow.keys0.zvl_as_borrowed(),
            joiner: &cow.joiner,
            keys1: cow.keys1.zvl_as_borrowed(),
            values: cow.values.zvl_as_borrowed(),
            key0_index,
        }
    }

    /// Returns the key0 corresponding to the cursor position.
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert("one", &1u32, "foo");
    /// assert_eq!(map.get0("one").unwrap().key0(), "one");
    /// ```
    pub fn key0(&self) -> &'l K0::GetType {
        #[expect(clippy::unwrap_used)] // safe by invariant on `self.key0_index`
        self.keys0.zvl_get(self.key0_index).unwrap()
    }

    /// Borrow an ordered iterator over keys1 and values for a particular key0.
    ///
    /// To get the values as copy types, see [`Self::iter1_copied`].
    ///
    /// For an example, see [`ZeroMap2d::iter0()`].
    pub fn iter1(
        &self,
    ) -> impl DoubleEndedIterator<
        Item = (
            &'l <K1 as ZeroMapKV<'a>>::GetType,
            &'l <V as ZeroMapKV<'a>>::GetType,
        ),
    > + ExactSizeIterator
           + '_ {
        let range = self.get_range();
        #[expect(clippy::unwrap_used)] // `self.get_range()` returns a valid range
        range.map(move |idx| {
            (
                self.keys1.zvl_get(idx).unwrap(),
                self.values.zvl_get(idx).unwrap(),
            )
        })
    }

    /// Transform this cursor into an ordered iterator over keys1 for a particular key0.
    pub fn into_iter1(
        self,
    ) -> impl DoubleEndedIterator<
        Item = (
            &'l <K1 as ZeroMapKV<'a>>::GetType,
            &'l <V as ZeroMapKV<'a>>::GetType,
        ),
    > + ExactSizeIterator {
        let range = self.get_range();
        #[expect(clippy::unwrap_used)] // `self.get_range()` returns a valid range
        range.map(move |idx| {
            (
                self.keys1.zvl_get(idx).unwrap(),
                self.values.zvl_get(idx).unwrap(),
            )
        })
    }

    /// Given key0_index, returns the corresponding range of keys1, which will be valid
    pub(super) fn get_range(&self) -> Range<usize> {
        debug_assert!(self.key0_index < self.joiner.len());
        let start = if self.key0_index == 0 {
            0
        } else {
            #[expect(clippy::unwrap_used)] // protected by the debug_assert above
            self.joiner.get(self.key0_index - 1).unwrap()
        };
        #[expect(clippy::unwrap_used)] // protected by the debug_assert above
        let limit = self.joiner.get(self.key0_index).unwrap();
        // These two assertions are true based on the invariants of ZeroMap2d
        debug_assert!(start < limit);
        debug_assert!((limit as usize) <= self.values.zvl_len());
        (start as usize)..(limit as usize)
    }
}

impl<'l, 'a, K0, K1, V> ZeroMap2dCursor<'l, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a>,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: Copy,
{
    /// Borrow an ordered iterator over keys1 and values for a particular key0.
    ///
    /// The values are returned as copy types.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroMap2d;
    ///
    /// let zm2d: ZeroMap2d<str, u8, u16> =
    ///     [("a", 0u8, 1u16), ("b", 1u8, 1000u16), ("b", 2u8, 2000u16)]
    ///         .into_iter()
    ///         .collect();
    ///
    /// let mut total_value = 0;
    ///
    /// for cursor in zm2d.iter0() {
    ///     for (_, value) in cursor.iter1_copied() {
    ///         total_value += value;
    ///     }
    /// }
    ///
    /// assert_eq!(total_value, 3001);
    /// ```
    pub fn iter1_copied(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&'l <K1 as ZeroMapKV<'a>>::GetType, V)> + ExactSizeIterator + '_
    {
        let range = self.get_range();
        #[expect(clippy::unwrap_used)] // `self.get_range()` returns a valid range
        range.map(move |idx| {
            (
                self.keys1.zvl_get(idx).unwrap(),
                self.get1_copied_at(idx).unwrap(),
            )
        })
    }
    /// Transform this cursor into an ordered iterator over keys1 for a particular key0.
    ///
    /// The values are returned as copy types.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerovec::ZeroMap2d;
    ///
    /// let zm2d: ZeroMap2d<str, u8, u16> =
    ///     [("a", 0u8, 1u16), ("b", 1u8, 1000u16), ("b", 2u8, 2000u16)]
    ///         .into_iter()
    ///         .collect();
    ///
    /// let mut total_value = 0;
    ///
    /// for cursor in zm2d.iter0() {
    ///     for (_, value) in cursor.into_iter1_copied() {
    ///         total_value += value;
    ///     }
    /// }
    ///
    /// assert_eq!(total_value, 3001);
    /// ```
    pub fn into_iter1_copied(
        self,
    ) -> impl DoubleEndedIterator<Item = (&'l <K1 as ZeroMapKV<'a>>::GetType, V)> + ExactSizeIterator
    {
        let range = self.get_range();
        #[expect(clippy::unwrap_used)] // `self.get_range()` returns a valid range
        range.map(move |idx| {
            (
                self.keys1.zvl_get(idx).unwrap(),
                self.get1_copied_at(idx).unwrap(),
            )
        })
    }

    fn get1_copied_at(&self, index: usize) -> Option<V> {
        let ule = self.values.zvl_get(index)?;
        let mut result = Option::<V>::None;
        V::Container::zvl_get_as_t(ule, |v| result.replace(*v));
        #[expect(clippy::unwrap_used)] // `zvl_get_as_t` guarantees that the callback is invoked
        Some(result.unwrap())
    }
}

impl<'l, 'a, K0, K1, V> ZeroMap2dCursor<'l, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a> + Ord,
    V: ZeroMapKV<'a>,
    K0: ?Sized,
    K1: ?Sized,
    V: ?Sized,
{
    /// Gets the value for a key1 from this cursor, or `None` if key1 is not in the map.
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert("one", &1u32, "foo");
    /// assert_eq!(map.get0("one").unwrap().get1(&1), Some("foo"));
    /// assert_eq!(map.get0("one").unwrap().get1(&2), None);
    /// ```
    pub fn get1(&self, key1: &K1) -> Option<&'l V::GetType> {
        let key1_index = self.get_key1_index(key1)?;
        #[expect(clippy::unwrap_used)] // key1_index is valid
        Some(self.values.zvl_get(key1_index).unwrap())
    }

    /// Gets the value for a predicate from this cursor, or `None` if key1 is not in the map.
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map = ZeroMap2d::new();
    /// map.insert("one", &1u32, "foo");
    /// assert_eq!(map.get0("one").unwrap().get1_by(|v| v.cmp(&1)), Some("foo"));
    /// assert_eq!(map.get0("one").unwrap().get1_by(|v| v.cmp(&2)), None);
    /// ```
    pub fn get1_by(&self, predicate: impl FnMut(&K1) -> Ordering) -> Option<&'l V::GetType> {
        let key1_index = self.get_key1_index_by(predicate)?;
        #[expect(clippy::unwrap_used)] // key1_index is valid
        Some(self.values.zvl_get(key1_index).unwrap())
    }

    /// Given key0_index and predicate, returns the index into the values array
    fn get_key1_index_by(&self, predicate: impl FnMut(&K1) -> Ordering) -> Option<usize> {
        let range = self.get_range();
        debug_assert!(range.start < range.end); // '<' because every key0 should have a key1
        debug_assert!(range.end <= self.keys1.zvl_len());
        let start = range.start;
        #[expect(clippy::expect_used)] // protected by the debug_assert above
        let binary_search_result = self
            .keys1
            .zvl_binary_search_in_range_by(predicate, range)
            .expect("in-bounds range");
        binary_search_result.ok().map(move |s| s + start)
    }

    /// Given key0_index and key1, returns the index into the values array
    fn get_key1_index(&self, key1: &K1) -> Option<usize> {
        let range = self.get_range();
        debug_assert!(range.start < range.end); // '<' because every key0 should have a key1
        debug_assert!(range.end <= self.keys1.zvl_len());
        let start = range.start;
        #[expect(clippy::expect_used)] // protected by the debug_assert above
        let binary_search_result = self
            .keys1
            .zvl_binary_search_in_range(key1, range)
            .expect("in-bounds range");
        binary_search_result.ok().map(move |s| s + start)
    }
}

impl<'l, 'a, K0, K1, V> ZeroMap2dCursor<'l, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a>,
    K1: ZeroMapKV<'a> + Ord,
    V: ZeroMapKV<'a>,
    V: Copy,
    K0: ?Sized,
    K1: ?Sized,
{
    /// For cases when `V` is fixed-size, obtain a direct copy of `V` instead of `V::ULE`
    ///
    /// ```rust
    /// use zerovec::ZeroMap2d;
    ///
    /// let mut map: ZeroMap2d<u16, u16, u16> = ZeroMap2d::new();
    /// map.insert(&1, &2, &3);
    /// map.insert(&1, &4, &5);
    /// map.insert(&6, &7, &8);
    ///
    /// assert_eq!(map.get0(&6).unwrap().get1_copied(&7), Some(8));
    /// ```
    #[inline]
    pub fn get1_copied(&self, key1: &K1) -> Option<V> {
        let key1_index = self.get_key1_index(key1)?;
        self.get1_copied_at(key1_index)
    }

    /// For cases when `V` is fixed-size, obtain a direct copy of `V` instead of `V::ULE`
    #[inline]
    pub fn get1_copied_by(&self, predicate: impl FnMut(&K1) -> Ordering) -> Option<V> {
        let key1_index = self.get_key1_index_by(predicate)?;
        self.get1_copied_at(key1_index)
    }
}

// We can't use the default PartialEq because ZeroMap2d is invariant
// so otherwise rustc will not automatically allow you to compare ZeroMaps
// with different lifetimes
impl<'m, 'n, 'a, 'b, K0, K1, V> PartialEq<ZeroMap2dCursor<'n, 'b, K0, K1, V>>
    for ZeroMap2dCursor<'m, 'a, K0, K1, V>
where
    K0: for<'c> ZeroMapKV<'c> + ?Sized,
    K1: for<'c> ZeroMapKV<'c> + ?Sized,
    V: for<'c> ZeroMapKV<'c> + ?Sized,
    <K0 as ZeroMapKV<'a>>::Slice: PartialEq<<K0 as ZeroMapKV<'b>>::Slice>,
    <K1 as ZeroMapKV<'a>>::Slice: PartialEq<<K1 as ZeroMapKV<'b>>::Slice>,
    <V as ZeroMapKV<'a>>::Slice: PartialEq<<V as ZeroMapKV<'b>>::Slice>,
{
    fn eq(&self, other: &ZeroMap2dCursor<'n, 'b, K0, K1, V>) -> bool {
        self.keys0.eq(other.keys0)
            && self.joiner.eq(other.joiner)
            && self.keys1.eq(other.keys1)
            && self.values.eq(other.values)
            && self.key0_index.eq(&other.key0_index)
    }
}

impl<'l, 'a, K0, K1, V> fmt::Debug for ZeroMap2dCursor<'l, 'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + ?Sized,
    K1: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K0::Slice: fmt::Debug,
    K1::Slice: fmt::Debug,
    V::Slice: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("ZeroMap2d")
            .field("keys0", &self.keys0)
            .field("joiner", &self.joiner)
            .field("keys1", &self.keys1)
            .field("values", &self.values)
            .field("key0_index", &self.key0_index)
            .finish()
    }
}
